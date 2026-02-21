// src/daemon.rs

//! dark-pattern daemon - the core focus session manager.
//!
//! The daemon is implemented using the Actor model:
//! - `DaemonHandle`: The public API (Handle) that communicates with the actor via MPSC.
//! - `DaemonActor`: The background task (Actor) that handles IO and state transitions.
//!
//! # Cold Turkey Enforcement
//! - Sessions cannot be stopped manually once started
//! - Shutdown signals are blocked during active sessions
//! - Sessions expire naturally based on their duration

use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, broadcast};
use tokio::time::{self, Interval};
use nix::sys::signal;
use nix::unistd::Pid;
use std::path::Path;

use crate::config::config::Config;
use crate::fs::{self, FsError};
use crate::host;
use crate::protocols::{self, ProtocolError, ClientMessage, DaemonMessage};
use crate::session::{Session, IdleSession};
use crate::socket::{SocketServer, SocketError, Connection};

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(String),

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
// Command Pattern (Oneshot Responses)
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Actor Handle
// ─────────────────────────────────────────────────────────────────────────────

/// Clone-able handle for communicating with the daemon actor.
/// 
/// This is the public API for interacting with the daemon.
/// Multiple handles can exist (e.g., one per connection handler).
#[derive(Clone)]
pub struct DaemonHandle {
    tx: mpsc::Sender<Command>,
    session_tx: broadcast::Sender<Session>,
}

impl DaemonHandle {
    pub fn new(tx: mpsc::Sender<Command>, session_tx: broadcast::Sender<Session>) -> Self {
        Self { tx, session_tx }
    }

    /// Subscribe to session state changes.
    /// Useful for UI updates or native messaging.
    pub fn subscribe(&self) -> broadcast::Receiver<Session> {
        self.session_tx.subscribe()
    }

    /// Start a new focus session.
    /// 
    /// # Errors
    /// Returns error if a session is already active.
    pub async fn start_session(&self, duration: Duration) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Start { duration, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    /// Attempt to stop the current session.
    /// 
    /// # Cold Turkey Enforcement
    /// This will ALWAYS fail during an active session.
    /// Sessions must expire naturally.
    pub async fn stop_session(&self) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Stop { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }

    /// Get the current session state.
    pub async fn get_status(&self) -> Result<Session, DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::GetStatus { reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))
    }

    /// Request daemon shutdown.
    /// 
    /// # Cold Turkey Enforcement
    /// - `force: false` - Blocked during active session (default behavior)
    /// - `force: true` - Shutdown anyway (for system shutdown/emergency)
    pub async fn shutdown(&self, force: bool) -> Result<(), DaemonError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(Command::Shutdown { force, reply: reply_tx })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;
        reply_rx.await.map_err(|e| DaemonError::Actor(e.to_string()))?
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Actor (IO and Message Handling)
// ─────────────────────────────────────────────────────────────────────────────

pub struct DaemonActor {
    rx: mpsc::Receiver<Command>,
    cmd_tx: mpsc::Sender<Command>,
    session_tx: broadcast::Sender<Session>,
    config: Config,
    socket_server: SocketServer,
    timer: Interval,
    session: Session,
}

impl DaemonActor {
    pub async fn new(
        rx: mpsc::Receiver<Command>,
        cmd_tx: mpsc::Sender<Command>,
        session_tx: broadcast::Sender<Session>,
        config: Config,
    ) -> Result<Self, DaemonError> {
        let socket_server = SocketServer::bind(&config.socket_path).await?;
        let timer = time::interval(Duration::from_secs(1));

        Ok(Self {
            rx,
            cmd_tx,
            session_tx,
            config,
            socket_server,
            timer,
            session: Session::Idle(IdleSession::new()),
        })
    }

    /// The core loop of the actor.
    /// 
    /// Owns the session state and handles:
    /// - Commands from handles
    /// - Timer ticks for expiration
    /// - Socket connections
    pub async fn run(mut self) -> Result<(), DaemonError> {
        // PID file locking
        let _pid_guard = self.lock_pid_file().await?;

        // Broadcast initial state
        let _ = self.session_tx.send(self.session.clone());

        loop {
            tokio::select! {
                // ─────────────────────────────────────────
                // Process commands from DaemonHandle
                // ─────────────────────────────────────────
                Some(cmd) = self.rx.recv() => {
                    match self.handle_command(cmd).await {
                        Ok(should_exit) => {
                            if should_exit {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("Command handling error: {}", e);
                        }
                    }
                }

                // ─────────────────────────────────────────
                // Tick: check session expiration
                // ─────────────────────────────────────────
                _ = self.timer.tick() => {
                    self.handle_tick().await;
                }

                // ─────────────────────────────────────────
                // Accept socket connections
                // ─────────────────────────────────────────
                result = self.socket_server.accept_no_timeout(Duration::from_secs(5)) => {
                    match result {
                        Ok(conn) => {
                            let tx = self.cmd_tx.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(conn, tx).await {
                                    eprintln!("Error handling client connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Socket accept error: {}", e);
                        }
                    }
                }
            }
        }

        // Cleanup on exit
        self.cleanup().await?;
        Ok(())
    }

    /// Handle a command and return (new_session, should_exit).
    async fn handle_command(
        &mut self,
        cmd: Command,
    ) -> Result<bool, DaemonError> {
        let mut should_exit = false;

        match cmd {
            Command::Start { duration, reply } => {
                let result = match self.session {
                    Session::Idle(idle) => {
                        // IO: Add blocks to hosts file
                        match self.add_blocks().await {
                            Ok(()) => {
                                // Typestate Transition: Idle -> Active
                                let active = idle.start(duration);
                                self.session = Session::Active(active);
                                let _ = self.session_tx.send(self.session.clone());
                                Ok(())
                            }
                            Err(e) => Err(e)
                        }
                    }
                    Session::Active(_) => {
                        Err(DaemonError::Session("Session already active".into()))
                    }
                };
                let _ = reply.send(result);
            }

            Command::Stop { reply } => {
                // COLD TURKEY: Stop is NEVER allowed during active session
                let result = if self.session.is_active() {
                    Err(DaemonError::Session(
                        "Cannot stop active session. Stay focused!".into()
                    ))
                } else {
                    // Already idle, nothing to do
                    Ok(())
                };
                let _ = reply.send(result);
            }

            Command::GetStatus { reply } => {
                let _ = reply.send(self.session.clone());
            }

            Command::Shutdown { force, reply } => {
                if self.session.is_active() && !force {
                    // COLD TURKEY: Block shutdown during active session
                    let remaining = self.session.remaining().as_secs();
                    let _ = reply.send(Err(DaemonError::ShutdownBlocked(remaining)));
                    
                    eprintln!();
                    eprintln!("╔══════════════════════════════════════════════════════════╗");
                    eprintln!("║  ⚠️  SHUTDOWN BLOCKED — Focus session active             ║");
                    eprintln!("║  Time remaining: {:>4} seconds                           ║", remaining);
                    eprintln!("║  Stay focused! Shutdown will proceed when session ends.  ║");
                    eprintln!("╚══════════════════════════════════════════════════════════╝");
                    eprintln!();
                } else {
                    if force && self.session.is_active() {
                        eprintln!("Warning: Forced shutdown during active session");
                    }
                    let _ = reply.send(Ok(()));
                    should_exit = true;
                }
            }
        }

        Ok(should_exit)
    }

    /// Handle timer tick: check for session expiration.
    async fn handle_tick(&mut self) {
        match self.session {
            Session::Active(active) if active.remaining() == Duration::ZERO => {
                println!("Session expired. Removing blocks...");
                
                if let Err(e) = self.remove_blocks().await {
                    eprintln!("Failed to remove blocks on expiration: {}", e);
                }

                let new_session = Session::Idle(active.stop());
                let _ = self.session_tx.send(new_session.clone());
                self.session = new_session;
            }
            _ => {}
        }
    }

    /// Add blocked hosts to /etc/hosts.
    async fn add_blocks(&self) -> Result<(), DaemonError> {
        let content = fs::read_file(&self.config.hosts_file).await?;
        let new_content = host::add_blocked_hosts(&content, self.config.blocked.as_slice());
        fs::write_file_atomic(&self.config.hosts_file, &new_content).await?;
        println!("Blocks added: {:?}", self.config.blocked.as_slice());
        Ok(())
    }

    /// Remove blocked hosts from /etc/hosts.
    async fn remove_blocks(&self) -> Result<(), DaemonError> {
        let content = fs::read_file(&self.config.hosts_file).await?;
        let new_content = host::remove_blocked_hosts(&content);
        fs::write_file_atomic(&self.config.hosts_file, &new_content).await?;
        println!("Blocks removed");
        Ok(())
    }

    /// Cleanup on shutdown.
    async fn cleanup(&self) -> Result<(), DaemonError> {
        println!("Cleaning up...");

        // Remove any remaining blocks
        let content = fs::read_file(&self.config.hosts_file).await?;
        if host::has_blocked_hosts(&content) {
            self.remove_blocks().await?;
        }

        // Remove socket file
        if let Err(e) = tokio::fs::remove_file(&self.config.socket_path).await {
            eprintln!("Failed to remove socket file: {}", e);
        }

        println!("Cleanup complete");
        Ok(())
    }

    async fn lock_pid_file(&self) -> Result<PidGuard, DaemonError> {
        let pid_path = &self.config.pid_path;
        if fs::file_exists(pid_path).await? {
            // Check if process is actually running
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
// Connection Handler
// ─────────────────────────────────────────────────────────────────────────────

/// Handle an individual socket connection from a client.
async fn handle_connection(
    mut conn: Connection,
    tx: mpsc::Sender<Command>,
) -> Result<(), DaemonError> {
    while let Some(bytes) = conn.recv().await? {
        let client_msg: ClientMessage = protocols::decode(&bytes)?;
        let response = process_client_message(client_msg, &tx).await?;
        conn.send(&protocols::encode(&response)?).await?;
    }
    Ok(())
}

/// Process a single client message and return the response.
async fn process_client_message(
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
            })
            .await
            .map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(())) => Ok(DaemonMessage::Started { duration }),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::Stop => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::Stop { reply: reply_tx })
                .await
                .map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Ok(())) => Ok(DaemonMessage::Stopped),
                Ok(Err(e)) => Ok(DaemonMessage::Error(e.to_string())),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }

        ClientMessage::GetStatus => {
            let (reply_tx, reply_rx) = oneshot::channel();
            tx.send(Command::GetStatus { reply: reply_tx })
                .await
                .map_err(|e| DaemonError::Actor(e.to_string()))?;

            match reply_rx.await {
                Ok(Session::Idle(_)) => Ok(DaemonMessage::StatusIdle),
                Ok(Session::Active(a)) => Ok(DaemonMessage::StatusWithTime {
                    time_left: a.remaining().as_secs(),
                }),
                Err(e) => Ok(DaemonMessage::Error(e.to_string())),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize and start the daemon actor.
/// 
/// Returns a `DaemonHandle` for interacting with the daemon.
pub async fn run(config: Config) -> Result<DaemonHandle, DaemonError> {
    let (tx, rx) = mpsc::channel(32);
    let (session_tx, _) = broadcast::channel(16);

    let actor = DaemonActor::new(rx, tx.clone(), session_tx.clone(), config).await?;
    let handle = DaemonHandle::new(tx, session_tx);

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
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn dummy_config(hosts_path: &Path, socket_path: &Path, pid_path: &Path) -> Config {
        let toml = format!(
            r#"
            hosts_file = "{}"
            socket_path = "{}"
            pid_path = "{}"
            blocked = ["example.com", "distraction.net"]
            "#,
            hosts_path.display(),
            socket_path.display(),
            pid_path.display()
        );
        Config::from_toml(&toml).unwrap()
    }

    fn unique_path(suffix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        PathBuf::from(format!("/tmp/dark-pattern_test_{}_{}.{}", std::process::id(), id, suffix))
    }

    #[tokio::test]
    async fn test_initial_state_is_idle() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        let status = handle.get_status().await.unwrap();
        assert!(!status.is_active());

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_start_session_activates_and_adds_blocks() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        // Verify session is active
        let status = handle.get_status().await.unwrap();
        assert!(status.is_active());
        assert!(status.remaining() > Duration::from_secs(50));

        // Verify blocks were added
        let content = std::fs::read_to_string(hosts_temp.path()).unwrap();
        assert!(content.contains("example.com"));
        assert!(content.contains("distraction.net"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_cannot_start_session_twice() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        handle.start_session(Duration::from_secs(60)).await.unwrap();
        
        let result = handle.start_session(Duration::from_secs(30)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already active"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_cold_turkey_stop_rejected_during_active() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        // COLD TURKEY: This should fail!
        let result = handle.stop_session().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot stop"));

        // Session should still be active
        let status = handle.get_status().await.unwrap();
        assert!(status.is_active());

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_cold_turkey_shutdown_blocked_during_active() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        handle.start_session(Duration::from_secs(60)).await.unwrap();

        // Non-forced shutdown should be blocked
        let result = handle.shutdown(false).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DaemonError::ShutdownBlocked(_)));

        // Forced shutdown should work
        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_session_expires_and_removes_blocks() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        // Start a short session
        handle.start_session(Duration::from_secs(2)).await.unwrap();

        // Verify blocks added
        let content = std::fs::read_to_string(hosts_temp.path()).unwrap();
        assert!(content.contains("example.com"));

        // Wait for expiration
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Verify session is idle
        let status = handle.get_status().await.unwrap();
        assert!(!status.is_active());

        // Verify blocks removed
        let content = std::fs::read_to_string(hosts_temp.path()).unwrap();
        assert!(!content.contains("example.com"));

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_broadcast_subscription() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();
        let mut subscriber = handle.subscribe();

        // Start session and receive broadcast
        handle.start_session(Duration::from_secs(60)).await.unwrap();

        // Subscriber might receive the initial Idle state first
        let session = subscriber.recv().await.unwrap();
        let session = if !session.is_active() {
            subscriber.recv().await.unwrap()
        } else {
            session
        };
        assert!(session.is_active());

        handle.shutdown(true).await.unwrap();
    }

    #[tokio::test]
    async fn test_cleanup_removes_socket_file() {
        let hosts_temp = NamedTempFile::new().unwrap();
        let socket_path = unique_path("sock");
        let pid_path = unique_path("pid");
        let config = dummy_config(hosts_temp.path(), &socket_path, &pid_path);

        let handle = run(config).await.unwrap();

        // Socket file should exist
        assert!(socket_path.exists());

        handle.shutdown(true).await.unwrap();

        // Give cleanup a moment
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Socket file should be removed
        assert!(!socket_path.exists());
    }
}
