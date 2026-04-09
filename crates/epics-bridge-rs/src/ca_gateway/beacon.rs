//! Beacon anomaly generation.
//!
//! Corresponds to C++ ca-gateway's `gateServer::generateBeaconAnomaly`
//! (gateServer.cc:422-432). When the gateway adds a new PV to its cache
//! (because a downstream client searched for it for the first time), it
//! triggers a beacon anomaly so that other downstream clients re-search
//! and discover the gateway as a server for that name.
//!
//! Real beacon emission is handled by the underlying [`epics_ca_rs`]
//! `CaServer` beacon emitter (lives in `epics-ca-rs::server::beacon`).
//! This module just throttles "beacon anomaly" requests according to
//! the C++ `GATE_RECONNECT_INHIBIT` (5 minutes) so we don't spam.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Default reconnect inhibit window — matches C++ `GATE_RECONNECT_INHIBIT`.
const DEFAULT_INHIBIT: Duration = Duration::from_secs(60 * 5);

/// Beacon anomaly trigger throttle.
pub struct BeaconAnomaly {
    inhibit: Duration,
    last: Mutex<Option<Instant>>,
}

impl BeaconAnomaly {
    /// Create with the default 5-minute inhibit window.
    pub fn new() -> Self {
        Self::with_inhibit(DEFAULT_INHIBIT)
    }

    /// Create with a custom inhibit window.
    pub fn with_inhibit(inhibit: Duration) -> Self {
        Self {
            inhibit,
            last: Mutex::new(None),
        }
    }

    /// Request a beacon anomaly. Returns true if the request was honored
    /// (not within the inhibit window). The actual beacon emission is the
    /// caller's responsibility (via `epics-ca-rs` server's beacon API).
    pub fn request(&self) -> bool {
        let now = Instant::now();
        let mut last = self.last.lock().unwrap();
        match *last {
            Some(t) if now.duration_since(t) < self.inhibit => false,
            _ => {
                *last = Some(now);
                true
            }
        }
    }

    /// Manually reset the inhibit timer.
    pub fn reset(&self) {
        *self.last.lock().unwrap() = None;
    }

    /// Time since the last honored request.
    pub fn elapsed(&self) -> Option<Duration> {
        self.last.lock().unwrap().map(|t| t.elapsed())
    }
}

impl Default for BeaconAnomaly {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_request_honored() {
        let b = BeaconAnomaly::with_inhibit(Duration::from_secs(60));
        assert!(b.request());
    }

    #[test]
    fn second_immediate_request_inhibited() {
        let b = BeaconAnomaly::with_inhibit(Duration::from_secs(60));
        assert!(b.request());
        assert!(!b.request());
    }

    #[test]
    fn request_after_inhibit_expires() {
        let b = BeaconAnomaly::with_inhibit(Duration::from_millis(10));
        assert!(b.request());
        std::thread::sleep(Duration::from_millis(20));
        assert!(b.request());
    }

    #[test]
    fn reset_clears_inhibit() {
        let b = BeaconAnomaly::with_inhibit(Duration::from_secs(60));
        assert!(b.request());
        assert!(!b.request());
        b.reset();
        assert!(b.request());
    }
}
