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

use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, broadcast};
use tokio::time::{self, Interval};
use nix::sys::signal;
use nix::unistd::Pid;
use std::path::Path;

use crate::config::config::Config;
use crate::db::{DbHandle, DbError};
use crate::fs::{self, FsError};
use crate::host;
use crate::protocols::{self, ProtocolError, ClientMessage, DaemonMessage};
use crate::session::{Session, IdleSession};
use crate::socket::{SocketServer, SocketError, Connection};

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

    pub async fn start_session(&self, duration: Duration) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::Start { duration, reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn stop_session(&self) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::Stop { reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn get_status(&self) -> Result<Session, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::GetStatus { reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))
    }

    pub async fn shutdown(&self, force: bool) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::Shutdown { force, reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn add_website(&self, url: String) -> Result<bool, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::AddWebsite { url, reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn remove_website(&self, url: String) -> Result<bool, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::RemoveWebsite { url, reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    pub async fn list_websites(&self) -> Result<Vec<String>, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx.send(Command::ListWebsites { reply: reply_tx })
            .await.map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Actor
// ─────────────────────────────────────────────────────────────────────────────

pub struct DaemonActor {
    rx: mpsc::Receiver<Command>,
    cmd_tx: mpsc::Sender<Command>,
    session_tx: broadcast::Sender<Session>,
    config: Config,
    db: DbHandle,
    socket_server: SocketServer,
    timer: Interval,
    session: Session,
}

impl DaemonActor {
    async fn new(
        rx: mpsc::Receiver<Command>,
        cmd_tx: mpsc::Sender<Command>,
        session_tx: broadcast::Sender<Session>,
        config: Config,
    ) -> Result<Self, DaemonError> {
        let socket_server = SocketServer::bind(&config.socket_path).await?;
        let timer = time::interval(Duration::from_secs(1));
        let db = DbHandle::open(&config.db_path)?;

        Ok(Self {
            rx,
            cmd_tx,
            session_tx,
            config,
            db,
            socket_server,
            timer,
            session: Session::Idle(IdleSession::new()),
        })
    }

    async fn run(mut self) -> Result<(), DaemonError> {
        let _pid_guard = self.lock_pid_file().await?;
        let _ = self.session_tx.send(self.session);

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
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(conn, tx).await {
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
            Command::Start { duration, reply } => {
                let result = self.handle_start(duration).await;
                let _ = reply.send(result);
            }
            Command::Stop { reply } => {
                let result = if self.session.is_active() {
                    Err(DaemonError::Session("Cannot stop active session. Stay focused!".into()))
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
            Command::AddWebsite { url, reply } => {
                let result = self.db.add_website(url).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::RemoveWebsite { url, reply } => {
                let result = self.db.remove_website(url).await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
            Command::ListWebsites { reply } => {
                let result = self.db.list_websites().await.map_err(DaemonError::from);
                let _ = reply.send(result);
            }
        }
        Ok(false)
    }

    async fn handle_start(&mut self, duration: Duration) -> Result<(), DaemonError> {
        match self.session {
            Session::Active(_) => {
                Err(DaemonError::Session("Session already active".into()))
            }
            Session::Idle(idle) => {
                self.add_blocks().await?;
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
    }

    // ── Hosts file IO ───────────────────────────────────────────────────────

    async fn add_blocks(&self) -> Result<(), DaemonError> {
        let websites = self.db.list_websites().await?;

        if websites.is_empty() {
            eprintln!("Warning: no blocked websites configured");
            return Ok(());
        }

        let content = fs::read_file(&self.config.hosts_file).await?;
        let new_content = host::add_blocked_hosts(&content, &websites);
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

    // ── PID locking ─────────────────────────────────────────────────────────

    async fn lock_pid_file(&self) -> Result<PidGuard, DaemonError> {
        let pid_path = &self.config.pid_path;

        if fs::file_exists(pid_path).await? {
            let content = fs::read_file(pid_path).await?;
            if let Ok(old_pid) = content.trim().parse::<i32>() {
                if is_process_running(old_pid) {
                    return Err(DaemonError::Config(
                        format!("Daemon already running with PID {}", old_pid),
                    ));
                }
            }
        }

        let pid = std::process::id();
        fs::write_file_atomic(pid_path, &pid.to_string()).await?;
        Ok(PidGuard { path: pid_path.clone() })
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
) -> Result<(), DaemonError> {
    while let Some(bytes) = conn.recv().await? {
        let msg: ClientMessage = protocols::decode(&bytes)?;
        let response = dispatch(msg, &tx).await?;
        conn.send(&protocols::encode(&response)?).await?;
    }
    Ok(())
}

async fn dispatch(
    msg: ClientMessage,
    tx: &mpsc::Sender<Command>,
) -> Result<DaemonMessage, DaemonError> {
    match msg {
        ClientMessage::Ping => Ok(DaemonMessage::Pong),

        ClientMessage::Start { duration } => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::Start {
                duration: Duration::from_secs(duration),
                reply: reply_tx,
            }).await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(())) => Ok(DaemonMessage::Started { duration }),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::Stop => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::Stop { reply: reply_tx })
                .await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(())) => Ok(DaemonMessage::Stopped),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::GetStatus => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::GetStatus { reply: reply_tx })
                .await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Session::Idle(_)) => Ok(DaemonMessage::StatusIdle),
                Ok(Session::Active(a)) => Ok(DaemonMessage::StatusWithTime {
                    time_left: a.remaining().as_secs(),
                }),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::AddWebsite { url } => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::AddWebsite { url: url.clone(), reply: reply_tx })
                .await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(_)) => Ok(DaemonMessage::WebsiteAdded { url }),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::RemoveWebsite { url } => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::RemoveWebsite { url: url.clone(), reply: reply_tx })
                .await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(_)) => Ok(DaemonMessage::WebsiteRemoved { url }),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::ListWebsites => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::ListWebsites { reply: reply_tx })
                .await.map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(websites)) => Ok(DaemonMessage::WebsiteList { websites }),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

pub async fn run(config: Config) -> Result<DaemonHandle, DaemonError> {
    let (tx, rx) = mpsc::channel(32);
    let (session_tx, _) = broadcast::channel(16);

    let actor = DaemonActor::new(rx, tx.clone(), session_tx.clone(), config).await?;
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
            std::process::id(), id, suffix
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
            hosts.display(), socket.display(),
            pid.display(), db.display(),
        )).unwrap()
    }

    /// Spin up a daemon with temp files, seed blocked websites, return the handle.
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

        handle.start_session(Duration::from_secs(60)).await.unwrap();

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

        handle.start_session(Duration::from_secs(60)).await.unwrap();
        let result = handle.start_session(Duration::from_secs(30)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already active"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cold_turkey_stop_rejected() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        let result = handle.stop_session().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot stop"));

        assert!(handle.get_status().await.unwrap().is_active());
        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn cold_turkey_shutdown_blocked() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        let result = handle.shutdown(false).await;
        assert!(matches!(result.unwrap_err(), DaemonError::ShutdownBlocked(_)));

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

        // Add
        assert!(handle.add_website("reddit.com".into()).await.unwrap());
        assert!(handle.add_website("youtube.com".into()).await.unwrap());

        // Duplicate
        assert!(!handle.add_website("reddit.com".into()).await.unwrap());

        // List
        let sites = handle.list_websites().await.unwrap();
        assert_eq!(sites, vec!["reddit.com", "youtube.com"]);

        // Remove
        assert!(handle.remove_website("reddit.com".into()).await.unwrap());
        assert!(!handle.remove_website("reddit.com".into()).await.unwrap());

        let sites = handle.list_websites().await.unwrap();
        assert_eq!(sites, vec!["youtube.com"]);

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn broadcast_subscription() {
        let (handle, _hosts) = start_test_daemon(&["example.com"]).await;
        let mut sub = handle.subscribe();

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        // May receive initial Idle first
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
            hosts.path(), &socket_path,
            &unique_path("pid"), &unique_path("db"),
        );

        let handle = run(config).await.unwrap();
        assert!(socket_path.exists());

        handle.shutdown(true).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!socket_path.exists());
    }
}
