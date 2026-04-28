//! Restart policy.
//!
//! Mirrors C procServ's `enum RestartMode { restart, norestart,
//! oneshot }` plus a NRESTARTS-style rate-limit window. The
//! `toggleRestart` keybinding cycles through the three modes:
//! `restart → norestart → oneshot → restart`.
//!
//! ## TODO: hoist to `epics-base-rs::runtime::supervise`
//!
//! `RestartPolicy` is now duplicated in three call sites:
//! - `epics-bridge-rs::ca_gateway::master::supervise`
//! - `epics-pva-rs::client_native::channel_cache::spawn_upstream_monitor`
//!   (different shape, same concept)
//! - `epics-ca-rs` name-server reconnect loop
//!
//! and procserv would be the fourth. Pre-merge of the procserv
//! crate, hoist this to a shared location and have all four
//! consumers share. Tracked separately.

use std::time::{Duration, Instant};

/// Three-state restart mode that cycles via `toggleRestart` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RestartMode {
    /// Default: relaunch the child whenever it exits.
    #[default]
    OnExit,
    /// Disabled: child exit shuts the supervisor down too. C name:
    /// `norestart`.
    Disabled,
    /// One-shot: relaunch exactly once, then act like `Disabled`.
    /// C name: `oneshot`.
    OneShot,
}

impl RestartMode {
    /// Cycle to the next mode (called by `toggleRestart` keybinding).
    /// `OnExit → Disabled → OneShot → OnExit`.
    pub fn next(self) -> Self {
        match self {
            Self::OnExit => Self::Disabled,
            Self::Disabled => Self::OneShot,
            Self::OneShot => Self::OnExit,
        }
    }

    /// Rendered string for status banners (matches C
    /// `restartModeString()` exactly).
    pub fn label(self) -> &'static str {
        match self {
            Self::OnExit => "ON",
            Self::Disabled => "OFF",
            Self::OneShot => "ONESHOT",
        }
    }
}

/// NRESTARTS-style rate limiter. The supervisor records each restart
/// timestamp; if more than `max` happen inside `window` it bails
/// with [`crate::procserv::error::ProcServError::RestartLimitExceeded`].
#[derive(Debug, Clone, Copy)]
pub struct RestartPolicy {
    /// Maximum restarts inside the rolling `window`.
    pub max: u32,
    /// Sliding-window duration over which `max` is measured.
    pub window: Duration,
    /// Minimum delay between restarts (matches C `holdoffTime` AND
    /// rate-limit floor).
    pub min_delay: Duration,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max: 10,
            window: Duration::from_secs(600),
            min_delay: Duration::from_secs(15),
        }
    }
}

/// In-memory restart bookkeeping. Held by the supervisor task.
#[derive(Debug, Default)]
pub struct RestartTracker {
    timestamps: Vec<Instant>,
}

impl RestartTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `Ok` if a fresh restart is permitted by `policy`,
    /// otherwise an error describing the limit hit. On `Ok`, also
    /// appends the current timestamp.
    pub fn try_record(&mut self, policy: &RestartPolicy) -> Result<(), (u32, u64)> {
        let now = Instant::now();
        // Drop entries outside the window.
        self.timestamps.retain(|t| now.duration_since(*t) < policy.window);
        if self.timestamps.len() as u32 >= policy.max {
            return Err((policy.max, policy.window.as_secs()));
        }
        self.timestamps.push(now);
        Ok(())
    }

    pub fn last(&self) -> Option<Instant> {
        self.timestamps.last().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_cycles() {
        assert_eq!(RestartMode::OnExit.next(), RestartMode::Disabled);
        assert_eq!(RestartMode::Disabled.next(), RestartMode::OneShot);
        assert_eq!(RestartMode::OneShot.next(), RestartMode::OnExit);
    }

    #[test]
    fn mode_labels_match_c_procserv() {
        assert_eq!(RestartMode::OnExit.label(), "ON");
        assert_eq!(RestartMode::Disabled.label(), "OFF");
        assert_eq!(RestartMode::OneShot.label(), "ONESHOT");
    }

    #[test]
    fn rate_limit_bails_after_max() {
        let policy = RestartPolicy {
            max: 3,
            window: Duration::from_secs(60),
            min_delay: Duration::from_secs(0),
        };
        let mut tracker = RestartTracker::new();
        assert!(tracker.try_record(&policy).is_ok());
        assert!(tracker.try_record(&policy).is_ok());
        assert!(tracker.try_record(&policy).is_ok());
        assert!(tracker.try_record(&policy).is_err());
    }
}
