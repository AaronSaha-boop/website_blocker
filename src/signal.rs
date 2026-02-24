use crate::session::Session;
use tokio::sync::watch;
use tokio::time::Duration;
use crate::session::IdleSession;

#[derive(Clone)]
pub struct AppState {
    session_rx: watch::Receiver<Session>,
}

impl AppState {
    pub fn new() -> (Self, watch::Sender<Session>) {
        let (tx, rx) = watch::channel(Session::Idle(IdleSession::new()));
        (Self { session_rx: rx }, tx)
    }
    
    pub fn is_active(&self) -> bool { self.session_rx.borrow().is_active() }
    pub fn remaining(&self) -> Duration { self.session_rx.borrow().remaining() }
    
    /// Wait for a shutdown signal (SIGINT or SIGTERM)
    /// Only returns when it's safe to shut down (i.e., session is Idle)
    pub async fn wait_for_shutdown(&mut self) {
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    if !self.is_active() {
                        return;
                    }
                    println!("Session is active. Shutdown blocked. Wait for session to end.");
                }
                _ = sigterm.recv() => {
                    if !self.is_active() {
                        return;
                    }
                    println!("Session is active. Shutdown blocked. Wait for session to end.");
                }
                _ = self.session_rx.changed() => {
                    // If session just became idle, we might want to check if a shutdown was pending
                    // For now, we just continue loop
                }
            }
        }
    }
}

pub async fn wait_for_ctrl_c() {
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl_c event");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::ActiveSession;

    #[tokio::test]
    async fn app_state_reflects_session_state() {
        let (state, tx) = AppState::new();
        
        // Initial state should be idle
        assert!(!state.is_active());
        
        // Update to active
        let active = Session::Active(ActiveSession::new_for_test(Duration::from_secs(60)));
        tx.send(active).unwrap();
        
        assert!(state.is_active());
        assert!(state.remaining() > Duration::from_secs(50));
    }
}
