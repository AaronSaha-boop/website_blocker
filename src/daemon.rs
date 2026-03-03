// src/daemon.rs

//! dark-pattern daemon — the core focus session manager.
//!
//! Implemented using the Actor model:
//! - `DaemonHandle`: Clone-able public API that sends commands via MPSC.
//! - `DaemonActor`: Background task owning all IO, state, and the database handle.
//!
//! # Cold Turkey Enforcement
//! - Sessions cannot be stopped manually once started.
//! - Shutdown signals are blocked during active sessions.
//! - Sessions expire naturally based on their duration.
//!
//! # Scheduling
//! - Profiles define sets of blocked sites, apps, and DOM rules.
//! - Schedules determine when profiles are active (recurring or one-time).
//! - The daemon checks schedules every tick and applies the merged policy.

use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::{self, Interval};
use nix::sys::signal;
use nix::unistd::Pid;

use crate::config::config::Config;
use crate::db::{
    ActivePolicy, BlockedApp, BlockedWebsite, DbError, DbHandle, DomRule,
    Profile, Schedule,
};
use crate::fs::{self, FsError};
use crate::host;
use crate::protocols::{self, ClientMessage, DaemonMessage, ProtocolError};
use crate::session::{IdleSession, Session};
use crate::socket::{Connection, SocketError, SocketServer};

// ─────────────────────────────────────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("Socket error: {0}")]
    Socket(#[from] SocketError),

    #[error("File system error: {0}")]
    Fs(#[from] FsError),

    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Actor error: {0}")]
    Actor(String),

    #[error("Shutdown blocked: session active with {0} seconds remaining")]
    ShutdownBlocked(u64),
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Command {
    // Session commands
    Start {
        duration: Duration,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    Stop {
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    GetStatus {
        reply: oneshot::Sender<Session>,
    },
    Shutdown {
        force: bool,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // Global website commands (backward compatible)
    AddWebsite {
        url: String,
        reply: oneshot::Sender<Result<bool, DaemonError>>,
    },
    RemoveWebsite {
        url: String,
        reply: oneshot::Sender<Result<bool, DaemonError>>,
    },
    ListWebsites {
        reply: oneshot::Sender<Result<Vec<String>, DaemonError>>,
    },

    // Profile commands
    CreateProfile {
        name: String,
        reply: oneshot::Sender<Result<Profile, DaemonError>>,
    },
    GetProfile {
        id: String,
        reply: oneshot::Sender<Result<Option<Profile>, DaemonError>>,
    },
    ListProfiles {
        reply: oneshot::Sender<Result<Vec<Profile>, DaemonError>>,
    },
    UpdateProfile {
        profile: Profile,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },
    DeleteProfile {
        id: String,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // Schedule commands
    CreateSchedule {
        schedule: Schedule,
        reply: oneshot::Sender<Result<Schedule, DaemonError>>,
    },
    ListSchedules {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<Schedule>, DaemonError>>,
    },
    DeleteSchedule {
        id: i64,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // Profile blocked websites
    AddProfileWebsite {
        profile_id: String,
        domain: String,
        reply: oneshot::Sender<Result<BlockedWebsite, DaemonError>>,
    },
    ListProfileWebsites {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<BlockedWebsite>, DaemonError>>,
    },
    DeleteProfileWebsite {
        id: i64,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // Profile blocked apps
    AddProfileApp {
        profile_id: String,
        app_identifier: String,
        reply: oneshot::Sender<Result<BlockedApp, DaemonError>>,
    },
    ListProfileApps {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<BlockedApp>, DaemonError>>,
    },
    DeleteProfileApp {
        id: i64,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // DOM rules
    AddDomRule {
        profile_id: String,
        site: String,
        toggle: String,
        reply: oneshot::Sender<Result<DomRule, DaemonError>>,
    },
    ListDomRules {
        profile_id: String,
        reply: oneshot::Sender<Result<Vec<DomRule>, DaemonError>>,
    },
    DeleteDomRule {
        id: i64,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    },

    // Active policy (what's blocked right now)
    GetActivePolicy {
        reply: oneshot::Sender<Result<ActivePolicy, DaemonError>>,
    },

    // Notify the actor that a mutation happened (from socket dispatch)
    // so it can recompute and broadcast the policy.
    PolicyMaybeChanged,
}

// ─────────────────────────────────────────────────────────────────────────────
// Handle (public API)
// ─────────────────────────────────────────────────────────────────────────────

/// Clone-able handle for communicating with the daemon actor.
#[derive(Clone)]
pub struct DaemonHandle {
    tx: mpsc::Sender<Command>,
    session_tx: broadcast::Sender<Session>,
}

impl DaemonHandle {
    pub fn subscribe(&self) -> broadcast::Receiver<Session> {
        self.session_tx.subscribe()
    }

    // ── Session commands ─────────────────────────────────────────────────────

    pub async fn start_session(&self, duration: Duration) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Start {
                duration,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn stop_session(&self) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Stop { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn get_status(&self) -> Result<Session, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::GetStatus { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))
    }

    pub async fn shutdown(&self, force: bool) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Shutdown {
                force,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Global website commands ──────────────────────────────────────────────

    pub async fn add_website(&self, url: String) -> Result<bool, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::AddWebsite {
                url,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn remove_website(&self, url: String) -> Result<bool, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::RemoveWebsite {
                url,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn list_websites(&self) -> Result<Vec<String>, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::ListWebsites { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Profile commands ─────────────────────────────────────────────────────

    pub async fn create_profile(&self, name: String) -> Result<Profile, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::CreateProfile {
                name,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn get_profile(&self, id: String) -> Result<Option<Profile>, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::GetProfile { id, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::ListProfiles { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn update_profile(&self, profile: Profile) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::UpdateProfile {
                profile,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn delete_profile(&self, id: String) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::DeleteProfile { id, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Schedule commands ────────────────────────────────────────────────────

    pub async fn create_schedule(&self, schedule: Schedule) -> Result<Schedule, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::CreateSchedule {
                schedule,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn list_schedules(&self, profile_id: String) -> Result<Vec<Schedule>, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::ListSchedules {
                profile_id,
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn delete_schedule(&self, id: i64) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::DeleteSchedule { id, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Profile blocked websites ─────────────────────────────────────────────

    pub async fn add_profile_website(&self, profile_id: String, domain: String) -> Result<BlockedWebsite, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::AddProfileWebsite { profile_id, domain, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Profile blocked apps ──────────────────────────────────────────────────

    pub async fn add_profile_app(&self, profile_id: String, app_identifier: String) -> Result<BlockedApp, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::AddProfileApp { profile_id, app_identifier, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── DOM rules ─────────────────────────────────────────────────────────────

    pub async fn add_dom_rule(&self, profile_id: String, site: String, toggle: String) -> Result<DomRule, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::AddDomRule { profile_id, site, toggle, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    // ── Active policy ────────────────────────────────────────────────────────

    pub async fn get_active_policy(&self) -> Result<ActivePolicy, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::GetActivePolicy { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Actor
// ─────────────────────────────────────────────────────────────────────────────

pub struct DaemonActor {
    rx: mpsc::Receiver<Command>,
    cmd_tx: mpsc::Sender<Command>,
    session_tx: broadcast::Sender<Session>,
    policy_tx: broadcast::Sender<ActivePolicy>,
    config: Config,
    db: DbHandle,
    socket_server: SocketServer,
    timer: Interval,
    session: Session,
    cached_policy: ActivePolicy,
    last_policy_check: Option<String>,
}

impl DaemonActor {
    async fn new(
        rx: mpsc::Receiver<Command>,
        cmd_tx: mpsc::Sender<Command>,
        session_tx: broadcast::Sender<Session>,
        policy_tx: broadcast::Sender<ActivePolicy>,
        config: Config,
    ) -> Result<Self, DaemonError> {
        let socket_server = SocketServer::bind(&config.socket_path).await?;
        let timer = time::interval(Duration::from_secs(1));
        let db = DbHandle::open(&config.db_path)?;

        Ok(Self {
            rx,
            cmd_tx,
            session_tx,
            policy_tx,
            config,
            db,
            socket_server,
            timer,
            session: Session::Idle(IdleSession::new()),
            cached_policy: ActivePolicy::default(),
            last_policy_check: None,
        })
    }

    async fn run(mut self) -> Result<(), DaemonError> {
        let _pid_guard = self.lock_pid_file().await?;
        let _ = self.session_tx.send(self.session);

        // Initial policy load
        if let Ok(policy) = self.get_current_policy().await {
            self.cached_policy = policy;
        }

        loop {
            tokio::select! {
                Some(cmd) = self.rx.recv() => {
                    match self.handle_command(cmd).await {
                        Ok(true) => break,
                        Ok(false) => {}
                        Err(e) => eprintln!("Command error: {}", e),
                    }
                }
                _ = self.timer.tick() => {
                    self.handle_tick().await;
                }
                result = self.socket_server.accept_no_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(conn) => {
                            let tx = self.cmd_tx.clone();
                            let db = self.db.clone();
                            let policy_rx = self.policy_tx.subscribe();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(conn, tx, db, policy_rx).await {
                                    eprintln!("Connection error: {}", e);
                                }
                            });
                        }
                        Err(e) => eprintln!("Accept error: {}", e),
                    }
                }
            }
        }

        self.cleanup().await
    }

    /// Returns `true` if the actor should exit.
    async fn handle_command(&mut self, cmd: Command) -> Result<bool, DaemonError> {
        match cmd {
            // ── Session commands ─────────────────────────────────────────────

            Command::Start { duration, reply } => {
                let result = self.handle_start(duration).await;
                let _ = reply.send(result);
            }

            Command::Stop { reply } => {
                let result = if self.session.is_active() {
                    Err(DaemonError::Session(
                        "Cannot stop active session. Stay focused!".into(),
                    ))
                } else {
                    Ok(())
                };
                let _ = reply.send(result);
            }

            Command::GetStatus { reply } => {
                let _ = reply.send(self.session);
            }

            Command::Shutdown { force, reply } => {
                return self.handle_shutdown(force, reply);
            }

            // Global website commands go through actor for backward compat
            Command::AddWebsite { url, reply } => {
                let result = self
                    .db
                    .add_global_website(url)
                    .await
                    .map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            Command::RemoveWebsite { url, reply } => {
                let result = self
                    .db
                    .remove_global_website(url)
                    .await
                    .map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            Command::ListWebsites { reply } => {
                let result = self
                    .db
                    .list_global_websites()
                    .await
                    .map_err(DaemonError::from);
                let _ = reply.send(result);
            }

            // ── Profile commands ──────────────────────────────────────────

            Command::CreateProfile { name, reply } => {
                let result = self.db.create_profile(name).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::GetProfile { id, reply } => {
                let result = self.db.get_profile(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::ListProfiles { reply } => {
                let result = self.db.list_profiles().await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::UpdateProfile { profile, reply } => {
                let result = self.db.update_profile(profile).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::DeleteProfile { id, reply } => {
                let result = self.db.delete_profile(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            // ── Schedule commands ─────────────────────────────────────────

            Command::CreateSchedule { schedule, reply } => {
                let result = self.db.create_schedule(schedule).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::ListSchedules { profile_id, reply } => {
                let result = self.db.list_schedules(profile_id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::DeleteSchedule { id, reply } => {
                let result = self.db.delete_schedule(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            // ── Profile blocked websites ──────────────────────────────────

            Command::AddProfileWebsite { profile_id, domain, reply } => {
                let website = BlockedWebsite { id: 0, profile_id, domain };
                let result = self.db.create_blocked_website(website).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::ListProfileWebsites { profile_id, reply } => {
                let result = self.db.list_blocked_websites(profile_id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::DeleteProfileWebsite { id, reply } => {
                let result = self.db.delete_blocked_website(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            // ── Profile blocked apps ──────────────────────────────────────

            Command::AddProfileApp { profile_id, app_identifier, reply } => {
                let app = BlockedApp { id: 0, profile_id, app_identifier };
                let result = self.db.create_blocked_app(app).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::ListProfileApps { profile_id, reply } => {
                let result = self.db.list_blocked_apps(profile_id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::DeleteProfileApp { id, reply } => {
                let result = self.db.delete_blocked_app(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            // ── DOM rules ─────────────────────────────────────────────────

            Command::AddDomRule { profile_id, site, toggle, reply } => {
                let rule = DomRule { id: 0, profile_id, site, toggle };
                let result = self.db.create_dom_rule(rule).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }
            Command::ListDomRules { profile_id, reply } => {
                let result = self.db.list_dom_rules(profile_id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::DeleteDomRule { id, reply } => {
                let result = self.db.delete_dom_rule(id).await.map_err(DaemonError::from);
                let _ = reply.send(result);
                self.broadcast_policy_if_changed().await;
            }

            // ── Active policy ─────────────────────────────────────────────

            Command::GetActivePolicy { reply } => {
                let result = self.get_current_policy().await;
                let _ = reply.send(result);
            }

            Command::PolicyMaybeChanged => {
                self.broadcast_policy_if_changed().await;
            }
        }
        Ok(false)
    }

    async fn handle_start(&mut self, duration: Duration) -> Result<(), DaemonError> {
        match self.session {
            Session::Active(_) => Err(DaemonError::Session("Session already active".into())),
            Session::Idle(idle) => {
                // Get current merged policy and cache it
                let policy = self.get_current_policy().await?;
                self.cached_policy = policy;

                // Add blocks to hosts file
                self.add_blocks().await?;

                // Start session
                self.session = Session::Active(idle.start(duration));
                let _ = self.session_tx.send(self.session);
                Ok(())
            }
        }
    }

    fn handle_shutdown(
        &self,
        force: bool,
        reply: oneshot::Sender<Result<(), DaemonError>>,
    ) -> Result<bool, DaemonError> {
        if self.session.is_active() && !force {
            let remaining = self.session.remaining().as_secs();
            let _ = reply.send(Err(DaemonError::ShutdownBlocked(remaining)));
            eprintln!("Shutdown blocked: {} seconds remaining", remaining);
            Ok(false)
        } else {
            if force && self.session.is_active() {
                eprintln!("Warning: forced shutdown during active session");
            }
            let _ = reply.send(Ok(()));
            Ok(true)
        }
    }

    async fn handle_tick(&mut self) {
        // Check for session expiration
        if let Session::Active(active) = self.session {
            if active.remaining() == Duration::ZERO {
                eprintln!("Session expired. Removing blocks...");

                if let Err(e) = self.remove_blocks().await {
                    eprintln!("Failed to remove blocks on expiration: {}", e);
                }

                self.session = Session::Idle(active.stop());
                let _ = self.session_tx.send(self.session);
            }
        }

        // Check if scheduled policy has changed (every minute)
        let now = chrono::Local::now();
        let check_key = format!("{}:{}", now.format("%a"), now.format("%H:%M"));

        if self.last_policy_check.as_ref() != Some(&check_key) {
            self.last_policy_check = Some(check_key);

            if let Ok(new_policy) = self.get_current_policy().await {
                if new_policy != self.cached_policy {
                    eprintln!("Scheduled policy changed, updating blocks...");

                    // If session is active, update hosts file
                    if self.session.is_active() {
                        if let Err(e) = self.add_blocks().await {
                            eprintln!("Failed to update blocks: {}", e);
                        }
                    }

                    // Broadcast to all subscribers (native host, etc.)
                    let _ = self.policy_tx.send(new_policy.clone());
                    self.cached_policy = new_policy;
                }
            }
        }
    }

    // ── Policy helpers ───────────────────────────────────────────────────────

    /// Recompute the active policy and broadcast it to all subscribers if changed.
    async fn broadcast_policy_if_changed(&mut self) {
        if let Ok(new_policy) = self.get_current_policy().await {
            if new_policy != self.cached_policy {
                let n = self.policy_tx.receiver_count();
                eprintln!(
                    "Broadcasting policy change: {} sites, {} rules, {} receivers",
                    new_policy.blocked_websites.len(),
                    new_policy.dom_rules.len(),
                    n
                );
                self.cached_policy = new_policy.clone();
                let _ = self.policy_tx.send(new_policy);
            }
        }
    }

    /// Get current policy by merging active scheduled profiles + global blocklist
    async fn get_current_policy(&self) -> Result<ActivePolicy, DaemonError> {
        let now = chrono::Local::now();
        let day = now.format("%a").to_string().to_lowercase();
        let time = now.format("%H:%M").to_string();

        self.db
            .get_active_policy(day, time)
            .await
            .map_err(DaemonError::from)
    }

    // ── Hosts file IO ────────────────────────────────────────────────────────

    async fn add_blocks(&self) -> Result<(), DaemonError> {
        let websites = &self.cached_policy.blocked_websites;

        if websites.is_empty() {
            eprintln!("Warning: no blocked websites in active policy");
            return Ok(());
        }

        let content = fs::read_file(&self.config.hosts_file).await?;
        let new_content = host::add_blocked_hosts(&content, websites);
        fs::write_file_atomic(&self.config.hosts_file, &new_content).await?;
        eprintln!("Blocks added: {:?}", websites);
        Ok(())
    }

    async fn remove_blocks(&self) -> Result<(), DaemonError> {
        let content = fs::read_file(&self.config.hosts_file).await?;
        let new_content = host::remove_blocked_hosts(&content);
        fs::write_file_atomic(&self.config.hosts_file, &new_content).await?;
        eprintln!("Blocks removed");
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DaemonError> {
        eprintln!("Cleaning up...");

        let content = fs::read_file(&self.config.hosts_file).await?;
        if host::has_blocked_hosts(&content) {
            self.remove_blocks().await?;
        }

        if let Err(e) = tokio::fs::remove_file(&self.config.socket_path).await {
            eprintln!("Failed to remove socket file: {}", e);
        }

        eprintln!("Cleanup complete");
        Ok(())
    }

    // ── PID locking ──────────────────────────────────────────────────────────

    async fn lock_pid_file(&self) -> Result<PidGuard, DaemonError> {
        let pid_path = &self.config.pid_path;

        if fs::file_exists(pid_path).await? {
            let content = fs::read_file(pid_path).await?;
            if let Ok(old_pid) = content.trim().parse::<i32>() {
                if is_process_running(old_pid) {
                    return Err(DaemonError::Config(format!(
                        "Daemon already running with PID {}",
                        old_pid
                    )));
                }
            }
        }

        let pid = std::process::id();
        fs::write_file_atomic(pid_path, &pid.to_string()).await?;
        Ok(PidGuard {
            path: pid_path.clone(),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

struct PidGuard {
    path: PathBuf,
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn is_process_running(pid: i32) -> bool {
    signal::kill(Pid::from_raw(pid), None).is_ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection handler
// ─────────────────────────────────────────────────────────────────────────────

async fn handle_connection(
    mut conn: Connection,
    tx: mpsc::Sender<Command>,
    db: DbHandle,
    mut policy_rx: broadcast::Receiver<ActivePolicy>,
) -> Result<(), DaemonError> {
    let mut subscribed = false;

    loop {
        tokio::select! {
            recv_result = conn.recv() => {
                match recv_result? {
                    Some(bytes) => {
                        let msg: ClientMessage = protocols::decode(&bytes)?;

                        if matches!(msg, ClientMessage::SubscribePolicyChanges) {
                            subscribed = true;
                            let ack = DaemonMessage::Subscribed;
                            conn.send(&protocols::encode(&ack)?).await?;
                            continue;
                        }

                        let response = dispatch(msg, &tx, &db).await;
                        conn.send(&protocols::encode(&response)?).await?;

                        // If this was a mutation, tell the actor to recheck/broadcast
                        if is_mutation(&response) {
                            let _ = tx.send(Command::PolicyMaybeChanged).await;
                        }
                    }
                    None => break, // Connection closed
                }
            }
            Ok(policy) = policy_rx.recv(), if subscribed => {
                let push = DaemonMessage::PolicyChanged(policy);
                conn.send(&protocols::encode(&push)?).await?;
            }
        }
    }

    Ok(())
}

/// Returns true if the response indicates a mutation that could affect the active policy.
fn is_mutation(response: &DaemonMessage) -> bool {
    matches!(
        response,
        DaemonMessage::Profile(_)
            | DaemonMessage::ProfileUpdated
            | DaemonMessage::ProfileDeleted
            | DaemonMessage::BlockedWebsite(_)
            | DaemonMessage::BlockedWebsiteDeleted
            | DaemonMessage::BlockedApp(_)
            | DaemonMessage::BlockedAppDeleted
            | DaemonMessage::DomRule(_)
            | DaemonMessage::DomRuleDeleted
            | DaemonMessage::GlobalWebsiteAdded(_)
            | DaemonMessage::GlobalWebsiteRemoved(_)
            | DaemonMessage::Schedule(_)
            | DaemonMessage::ScheduleUpdated
            | DaemonMessage::ScheduleDeleted
    )
}

/// Dispatch a client message to the appropriate handler.
///
/// DB errors are caught and returned as `DaemonMessage::Error` so that a
/// single bad request doesn't kill the connection.
async fn dispatch(
    msg: ClientMessage,
    tx: &mpsc::Sender<Command>,
    db: &DbHandle,
) -> DaemonMessage {
    match dispatch_inner(msg, tx, db).await {
        Ok(response) => response,
        Err(e) => DaemonMessage::Error(e.to_string()),
    }
}

async fn dispatch_inner(
    msg: ClientMessage,
    tx: &mpsc::Sender<Command>,
    db: &DbHandle,
) -> Result<DaemonMessage, DaemonError> {
    match msg {
        ClientMessage::Ping => Ok(DaemonMessage::Pong),

        ClientMessage::Start { duration } => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::Start {
                duration: Duration::from_secs(duration),
                reply: reply_tx,
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
            reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))??;
            Ok(DaemonMessage::Started { duration })
        }

        ClientMessage::Stop => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::Stop { reply: reply_tx })
                .await
                .map_err(|e| DaemonError::Actor(e.to_string()))?;
            reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))??;
            Ok(DaemonMessage::Stopped)
        }

        ClientMessage::GetStatus => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::GetStatus { reply: reply_tx })
                .await
                .map_err(|e| DaemonError::Actor(e.to_string()))?;
            let session = reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?;
            let sites = db.list_global_websites().await?;

            match session {
                Session::Idle(_) => Ok(DaemonMessage::StatusIdle { sites }),
                Session::Active(a) => Ok(DaemonMessage::StatusWithTime {
                    time_left: a.remaining().as_secs(),
                    sites,
                }),
            }
        }

        // ── Profiles ─────────────────────────────────────────────────────────

        ClientMessage::CreateProfile { name } => {
            let profile = db.create_profile(name).await?;
            Ok(DaemonMessage::Profile(profile))
        }
        ClientMessage::GetProfile { id } => {
            match db.get_profile(id).await? {
                Some(profile) => Ok(DaemonMessage::Profile(profile)),
                None => Ok(DaemonMessage::Error("Profile not found".into())),
            }
        }
        ClientMessage::ListProfiles => {
            let profiles = db.list_profiles().await?;
            Ok(DaemonMessage::ProfileList(profiles))
        }
        ClientMessage::UpdateProfile { profile } => {
            db.update_profile(profile).await?;
            Ok(DaemonMessage::ProfileUpdated)
        }
        ClientMessage::DeleteProfile { id } => {
            db.delete_profile(id).await?;
            Ok(DaemonMessage::ProfileDeleted)
        }

        // ── Schedules ────────────────────────────────────────────────────────

        ClientMessage::CreateSchedule { schedule } => {
            let schedule = db.create_schedule(schedule).await?;
            Ok(DaemonMessage::Schedule(schedule))
        }
        ClientMessage::GetSchedule { id } => {
            match db.get_schedule(id).await? {
                Some(schedule) => Ok(DaemonMessage::Schedule(schedule)),
                None => Ok(DaemonMessage::Error("Schedule not found".into())),
            }
        }
        ClientMessage::ListSchedules { profile_id } => {
            let schedules = db.list_schedules(profile_id).await?;
            Ok(DaemonMessage::ScheduleList(schedules))
        }
        ClientMessage::UpdateSchedule { schedule } => {
            db.update_schedule(schedule).await?;
            Ok(DaemonMessage::ScheduleUpdated)
        }
        ClientMessage::DeleteSchedule { id } => {
            db.delete_schedule(id).await?;
            Ok(DaemonMessage::ScheduleDeleted)
        }

        // ── Blocked Websites ─────────────────────────────────────────────────

        ClientMessage::CreateBlockedWebsite { website } => {
            let website = db.create_blocked_website(website).await?;
            Ok(DaemonMessage::BlockedWebsite(website))
        }
        ClientMessage::ListBlockedWebsites { profile_id } => {
            let websites = db.list_blocked_websites(profile_id).await?;
            Ok(DaemonMessage::BlockedWebsiteList(websites))
        }
        ClientMessage::DeleteBlockedWebsite { id } => {
            db.delete_blocked_website(id).await?;
            Ok(DaemonMessage::BlockedWebsiteDeleted)
        }

        // ── Blocked Apps ─────────────────────────────────────────────────────

        ClientMessage::CreateBlockedApp { app } => {
            let app = db.create_blocked_app(app).await?;
            Ok(DaemonMessage::BlockedApp(app))
        }
        ClientMessage::GetBlockedApp { id } => {
            match db.get_blocked_app(id).await? {
                Some(app) => Ok(DaemonMessage::BlockedApp(app)),
                None => Ok(DaemonMessage::Error("Blocked app not found".into())),
            }
        }
        ClientMessage::ListBlockedApps { profile_id } => {
            let apps = db.list_blocked_apps(profile_id).await?;
            Ok(DaemonMessage::BlockedAppList(apps))
        }
        ClientMessage::DeleteBlockedApp { id } => {
            db.delete_blocked_app(id).await?;
            Ok(DaemonMessage::BlockedAppDeleted)
        }

        // ── DOM Rules ────────────────────────────────────────────────────────

        ClientMessage::CreateDomRule { rule } => {
            let rule = db.create_dom_rule(rule).await?;
            Ok(DaemonMessage::DomRule(rule))
        }
        ClientMessage::GetDomRule { id } => {
            match db.get_dom_rule(id).await? {
                Some(rule) => Ok(DaemonMessage::DomRule(rule)),
                None => Ok(DaemonMessage::Error("DOM rule not found".into())),
            }
        }
        ClientMessage::ListDomRules { profile_id } => {
            let rules = db.list_dom_rules(profile_id).await?;
            Ok(DaemonMessage::DomRuleList(rules))
        }
        ClientMessage::DeleteDomRule { id } => {
            db.delete_dom_rule(id).await?;
            Ok(DaemonMessage::DomRuleDeleted)
        }

        // ── Manual Sessions ──────────────────────────────────────────────────

        ClientMessage::CreateManualSession { session } => {
            let session = db.create_manual_session(session).await?;
            Ok(DaemonMessage::ManualSession(Some(session)))
        }
        ClientMessage::GetManualSession { id } => {
            let session = db.get_manual_session(id).await?;
            Ok(DaemonMessage::ManualSession(session))
        }
        ClientMessage::GetActiveManualSession => {
            let session = db.get_active_manual_session().await?;
            Ok(DaemonMessage::ManualSession(session))
        }
        ClientMessage::ListManualSessions => {
            let sessions = db.list_manual_sessions().await?;
            Ok(DaemonMessage::ManualSessionList(sessions))
        }
        ClientMessage::UpdateManualSession { session } => {
            db.update_manual_session(session).await?;
            Ok(DaemonMessage::ManualSessionUpdated)
        }

        // ── Global Blocked Websites ──────────────────────────────────────────

        ClientMessage::AddGlobalWebsite { domain } => {
            let success = db.add_global_website(domain).await?;
            Ok(DaemonMessage::GlobalWebsiteAdded(success))
        }
        ClientMessage::RemoveGlobalWebsite { domain } => {
            let success = db.remove_global_website(domain).await?;
            Ok(DaemonMessage::GlobalWebsiteRemoved(success))
        }
        ClientMessage::ListGlobalWebsites => {
            let websites = db.list_global_websites().await?;
            Ok(DaemonMessage::GlobalWebsiteList(websites))
        }

        // ── Active Policy ────────────────────────────────────────────────────

        ClientMessage::GetActivePolicy { current_day, current_time } => {
            let policy = db.get_active_policy(current_day, current_time).await?;
            Ok(DaemonMessage::ActivePolicy(policy))
        }

        // Subscription is handled at the connection level, not here
        ClientMessage::SubscribePolicyChanges => {
            Ok(DaemonMessage::Subscribed)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

pub async fn run(config: Config) -> Result<DaemonHandle, DaemonError> {
    let (tx, rx) = mpsc::channel(32);
    let (session_tx, _) = broadcast::channel(16);
    let (policy_tx, _) = broadcast::channel(16);

    let actor = DaemonActor::new(rx, tx.clone(), session_tx.clone(), policy_tx, config).await?;
    let handle = DaemonHandle { tx, session_tx };

    tokio::spawn(async move {
        if let Err(e) = actor.run().await {
            eprintln!("Daemon actor failed: {}", e);
        }
    });

    Ok(handle)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn unique_path(suffix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        PathBuf::from(format!(
            "/tmp/dark-pattern_test_{}_{}.{}",
            std::process::id(),
            id,
            suffix
        ))
    }

    fn test_config(hosts: &Path, socket: &Path, pid: &Path, db: &Path) -> Config {
        Config::from_toml(&format!(
            r#"
            hosts_file = "{}"
            socket_path = "{}"
            pid_path = "{}"
            db_path = "{}"
            "#,
            hosts.display(),
            socket.display(),
            pid.display(),
            db.display(),
        ))
        .unwrap()
    }

    async fn start_test_daemon(websites: &[&str]) -> (DaemonHandle, NamedTempFile) {
        let hosts = NamedTempFile::new().unwrap();
        let db_path = unique_path("db");
        let config = test_config(
            hosts.path(),
            &unique_path("sock"),
            &unique_path("pid"),
            &db_path,
        );

        let handle = run(config).await.unwrap();

        for site in websites {
            handle.add_website(site.to_string()).await.unwrap();
        }

        (handle, hosts)
    }

    #[tokio::test]
    async fn initial_state_is_idle() {
        let (handle, _hosts) = start_test_daemon(&[]).await;
        let status = handle.get_status().await.unwrap();
        assert!(!status.is_active());
        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn start_session_activates_and_adds_blocks() {
        let (handle, hosts) = start_test_daemon(&["example.com", "distraction.net"]).await;

        handle
            .start_session(Duration::from_secs(60))
            .await
            .unwrap();

        let status = handle.get_status().await.unwrap();
        assert!(status.is_active());
        assert!(status.remaining() > Duration::from_secs(50));

        let content = std::fs::read_to_string(hosts.path()).unwrap();
        assert!(content.contains("example.com"));
        assert!(content.contains("distraction.net"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cannot_start_session_twice() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;

        handle
            .start_session(Duration::from_secs(60))
            .await
            .unwrap();
        let result = handle.start_session(Duration::from_secs(30)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already active"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cold_turkey_stop_rejected() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;

        handle
            .start_session(Duration::from_secs(60))
            .await
            .unwrap();

        let result = handle.stop_session().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot stop"));

        assert!(handle.get_status().await.unwrap().is_active());
        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cold_turkey_shutdown_blocked() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;

        handle
            .start_session(Duration::from_secs(60))
            .await
            .unwrap();

        let result = handle.shutdown(false).await;
        assert!(matches!(
            result.unwrap_err(),
            DaemonError::ShutdownBlocked(_)
        ));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn session_expires_and_removes_blocks() {
        let (handle, hosts) = start_test_daemon(&["example.com"]).await;

        handle.start_session(Duration::from_secs(2)).await.unwrap();

        let content = std::fs::read_to_string(hosts.path()).unwrap();
        assert!(content.contains("example.com"));

        tokio::time::sleep(Duration::from_secs(3)).await;

        assert!(!handle.get_status().await.unwrap().is_active());

        let content = std::fs::read_to_string(hosts.path()).unwrap();
        assert!(!content.contains("example.com"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn website_crud_through_handle() {
        let (handle, _hosts) = start_test_daemon(&[]).await;

        assert!(handle.add_website("reddit.com".into()).await.unwrap());
        assert!(handle.add_website("youtube.com".into()).await.unwrap());
        assert!(!handle.add_website("reddit.com".into()).await.unwrap());

        let sites = handle.list_websites().await.unwrap();
        assert_eq!(sites, vec!["reddit.com", "youtube.com"]);

        assert!(handle.remove_website("reddit.com".into()).await.unwrap());
        assert!(!handle.remove_website("reddit.com".into()).await.unwrap());

        let sites = handle.list_websites().await.unwrap();
        assert_eq!(sites, vec!["youtube.com"]);

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn profile_crud() {
        let (handle, _hosts) = start_test_daemon(&[]).await;

        let profile = handle.create_profile("Work Focus".into()).await.unwrap();
        assert_eq!(profile.name, "Work Focus");
        assert!(profile.enabled);

        let profiles = handle.list_profiles().await.unwrap();
        assert_eq!(profiles.len(), 1);

        let fetched = handle.get_profile(profile.id.clone()).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Work Focus");

        handle.delete_profile(profile.id).await.unwrap();
        let profiles = handle.list_profiles().await.unwrap();
        assert!(profiles.is_empty());

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn active_policy_includes_global_websites() {
        let (handle, _hosts) = start_test_daemon(&[]).await;

        handle.add_website("reddit.com".into()).await.unwrap();

        let policy = handle.get_active_policy().await.unwrap();
        assert!(policy.blocked_websites.contains(&"reddit.com".to_string()));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn broadcast_subscription() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;
        let mut sub = handle.subscribe();

        handle
            .start_session(Duration::from_secs(60))
            .await
            .unwrap();

        let session = sub.recv().await.unwrap();
        let session = if !session.is_active() {
            sub.recv().await.unwrap()
        } else {
            session
        };
        assert!(session.is_active());

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cleanup_removes_socket() {
        let hosts = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let config = test_config(
            hosts.path(),
            &socket_path,
            &unique_path("pid"),
            &unique_path("db"),
        );

        let handle = run(config).await.unwrap();
        assert!(socket_path.exists());

        handle.shutdown(true).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn policy_push_over_socket() {
        use crate::client;
        use crate::protocols;

        let hosts = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let config = test_config(
            hosts.path(),
            &socket_path,
            &unique_path("pid"),
            &unique_path("db"),
        );

        let handle = run(config).await.unwrap();

        // Connect subscriber
        let mut sub_conn = client::connect(&socket_path, Duration::from_secs(5))
            .await
            .unwrap();

        // Subscribe to policy changes
        let resp = client::send_and_receive(&mut sub_conn, ClientMessage::SubscribePolicyChanges)
            .await
            .unwrap();
        assert!(matches!(resp, DaemonMessage::Subscribed));

        // Make a mutation via the DaemonHandle (triggers broadcast)
        handle.add_website("reddit.com".into()).await.unwrap();

        // The subscriber should receive a PolicyChanged push
        let push_bytes = tokio::time::timeout(
            Duration::from_secs(5),
            sub_conn.recv(),
        )
        .await
        .expect("Timed out waiting for policy push")
        .unwrap()
        .expect("Connection closed unexpectedly");

        let push_msg: DaemonMessage = protocols::decode(&push_bytes).unwrap();
        match push_msg {
            DaemonMessage::PolicyChanged(policy) => {
                assert!(
                    policy.blocked_websites.contains(&"reddit.com".to_string()),
                    "Expected reddit.com in pushed policy: {:?}",
                    policy.blocked_websites
                );
            }
            other => panic!("Expected PolicyChanged, got {:?}", other),
        }

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn active_policy_includes_profile_dom_rules_and_websites() {
        let (handle, _hosts) = start_test_daemon(&[]).await;

        // Create a profile (enabled by default, no schedule = always active)
        let profile = handle.create_profile("Test".into()).await.unwrap();
        assert!(profile.enabled);

        // Add a blocked website to the profile
        let website = handle
            .add_profile_website(profile.id.clone(), "reddit.com".into())
            .await
            .unwrap();
        assert_eq!(website.domain, "reddit.com");

        // Add a DOM rule to the profile
        let rule = handle
            .add_dom_rule(profile.id.clone(), "youtube".into(), "hideShorts".into())
            .await
            .unwrap();
        assert_eq!(rule.site, "youtube");
        assert_eq!(rule.toggle, "hideShorts");

        // Query active policy — profile has no schedule so LEFT JOIN
        // treats it as always active
        let policy = handle.get_active_policy().await.unwrap();

        assert!(
            policy.blocked_websites.contains(&"reddit.com".to_string()),
            "Expected reddit.com in blocked_websites: {:?}",
            policy.blocked_websites
        );
        assert!(
            policy.dom_rules.iter().any(|r| r.site == "youtube" && r.toggle == "hideShorts"),
            "Expected youtube/hideShorts DOM rule: {:?}",
            policy.dom_rules
        );

        handle.shutdown(true).await.unwrap();
    }
}
