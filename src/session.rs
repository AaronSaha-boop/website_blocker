// src/session.rs

use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────────
// Session (enum over typestates)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum Session {
    Idle(IdleSession),
    Active(ActiveSession),
}

impl Session {
    pub fn is_active(&self) -> bool {
        matches!(self, Session::Active(_))
    }

    pub fn remaining(&self) -> Duration {
        match self {
            Session::Active(a) => a.remaining(),
            Session::Idle(_) => Duration::ZERO,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Idle State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct IdleSession;

impl IdleSession {
    pub fn new() -> Self {
        Self
    }

    /// Transition: Idle -> Active
    pub fn start(self, duration: Duration) -> ActiveSession {
        ActiveSession {
            deadline: Instant::now() + duration,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Active State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct ActiveSession {
    deadline: Instant,
}

impl ActiveSession {
    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    /// Transition: Active -> Idle
    pub fn stop(self) -> IdleSession {
        IdleSession
    }

    #[cfg(test)]
    pub fn new_for_test(duration: Duration) -> Self {
        Self {
            deadline: Instant::now() + duration,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_session_is_not_active() {
        let session = Session::Idle(IdleSession::new());
        assert!(!session.is_active());
        assert_eq!(session.remaining(), Duration::ZERO);
    }

    #[test]
    fn start_transitions_to_active() {
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(60));
        let session = Session::Active(active);
        assert!(session.is_active());
        assert!(session.remaining() > Duration::from_secs(50));
    }

    #[test]
    fn stop_transitions_to_idle() {
        let idle = IdleSession::new();
        let active = idle.start(Duration::from_secs(60));
        let _back_to_idle = active.stop();
    }

    #[test]
    fn remaining_decreases_over_time() {
        let active = ActiveSession::new_for_test(Duration::from_secs(60));
        let r1 = active.remaining();
        std::thread::sleep(Duration::from_millis(50));
        let r2 = active.remaining();
        assert!(r2 < r1);
    }

    #[test]
    fn expired_session_returns_zero() {
        let active = ActiveSession::new_for_test(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(active.remaining(), Duration::ZERO);
    }
}
