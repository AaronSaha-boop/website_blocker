// src/session.rs

use std::time::{Duration, Instant};

/// A session that is not currently blocking
#[derive(Debug)]
pub struct IdleSession;

/// A session that is actively blocking
#[derive(Debug)]
pub struct ActiveSession {
    started_at: Instant,
    duration: Duration,
}

/// Wrapper enum for holding either session state
#[derive(Debug)]
pub enum Session {
    Idle(IdleSession),
    Active(ActiveSession),
}

impl IdleSession {
    pub fn new() -> Self {
        IdleSession
    }

    /// Start blocking — consumes self, returns ActiveSession
    pub fn start(self, duration: Duration) -> ActiveSession {
        ActiveSession { 
            started_at: Instant::now(), 
            duration, 
        }
    }
}

impl ActiveSession {
    /// Stop blocking — consumes self, returns IdleSession
    pub fn stop(self) -> IdleSession {
        IdleSession
    }

    /// Get remaining time
    pub fn remaining(&self) -> Duration {
        let elapsed = self.started_at.elapsed();
        //elasped() is apart of duration crate, Returns the amount of time elapsed since this instant.
        self.duration.saturating_sub(elapsed)
        //computes self - other
    }

    /// Check if expired — returns appropriate Session variant
    pub fn check_expired(self) -> Session {
        if self.remaining() == Duration::ZERO {
            Session::Idle(IdleSession)
        } else {
            Session::Active(self)
        }
    }
}

impl Default for IdleSession {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

// consuming self pattern 
impl Session {
    /// Create a new idle session
    pub fn new() -> Self {
        Self::Idle(IdleSession::new())
    }

    /// Check if currently active
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }

    /// Get remaining time (zero if idle)
    pub fn remaining(&self) -> Duration {
        match self {
            Self::Idle(_) => Duration::ZERO,
            Self::Active(active) => active.remaining(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    // IdleSession tests
    #[test]
    fn idle_session_new() { 
        let idle = IdleSession::new();
        assert!(matches!(idle, IdleSession));
    }

    #[test]
    fn idle_session_start_returns_active() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(10));
        assert!(matches!(active, ActiveSession { .. }));
    }

    // ActiveSession tests
    #[test]
    fn active_session_stop_returns_idle() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(10));
        let idle_again = active.stop();
        assert!(matches!(idle_again, IdleSession));
    }

    #[test]
    fn active_session_remaining_returns_time_left() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(10));
        let remaining = active.remaining();
        // Should be close to 10 seconds (might be slightly less)
        assert!(remaining.as_secs() >= 9);
    }

    #[test]
    fn active_session_remaining_returns_zero_when_expired() {
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_millis(10));  // Very short!
        sleep(Duration::from_millis(20));  // Wait for expiration
        let remaining = active.remaining();
        assert_eq!(remaining, Duration::ZERO);
    }

    #[test]
    fn active_session_check_expired_transitions_to_idle() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_millis(10));  // Very short!
        sleep(Duration::from_millis(20));  // Wait for expiration
        let session = active.check_expired();
        assert!(matches!(session, Session::Idle(_)));
    }

    #[test]
    fn active_session_check_expired_stays_active_when_time_remains() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(60));  // Long duration
        let session = active.check_expired();
        assert!(matches!(session, Session::Active(_)));
    }

    // Session enum tests
    #[test]
    fn session_is_active_returns_false_for_idle() { 
        let session = Session::new();
        assert!(!session.is_active());
    }

    #[test]
    fn session_is_active_returns_true_for_active() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(60));
        let session = Session::Active(active);
        assert!(session.is_active());
    }       

    #[test]
    fn session_remaining_delegates_correctly() { 
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(10));
        let session = Session::Active(active);
        let remaining = session.remaining();
        assert!(remaining.as_secs() >= 9);
    }
}