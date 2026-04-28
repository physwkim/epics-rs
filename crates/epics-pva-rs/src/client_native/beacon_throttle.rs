//! Beacon anomaly throttle.
//!
//! pvxs `clientconn.cpp` 5-minute rule: if we see a server's GUID change
//! (i.e. the server restarted) within a 5-minute window, suppress
//! reconnect attempts for the rest of that window. Without this rule, a
//! server that's flapping (stuck in a restart loop) would cause connection
//! storms from every client trying to reconnect on every beacon.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

/// 5-minute window for the anomaly throttle.
const ANOMALY_WINDOW: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
struct ServerEntry {
    guid: [u8; 12],
    first_seen: Instant,
    last_seen: Instant,
    /// `Some(deadline)` means: ignore reconnect attempts for this server
    /// until `Instant::now() >= deadline`.
    suppress_until: Option<Instant>,
}

#[derive(Default)]
pub struct BeaconTracker {
    inner: RwLock<HashMap<SocketAddr, ServerEntry>>,
}

impl BeaconTracker {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Record an observed beacon.
    ///
    /// Returns `true` if this beacon should trigger a reconnect (i.e. the
    /// server is *not* currently throttled), `false` if it should be
    /// suppressed.
    pub fn observe(&self, server: SocketAddr, guid: [u8; 12]) -> bool {
        let mut map = self.inner.write();
        let now = Instant::now();
        match map.get_mut(&server) {
            None => {
                map.insert(
                    server,
                    ServerEntry {
                        guid,
                        first_seen: now,
                        last_seen: now,
                        suppress_until: None,
                    },
                );
                true
            }
            Some(entry) => {
                entry.last_seen = now;
                if entry.guid == guid {
                    // Same server, same incarnation — pass-through.
                    let allow = !matches!(entry.suppress_until, Some(deadline) if now < deadline);
                    if allow {
                        entry.suppress_until = None;
                    }
                    allow
                } else {
                    // GUID changed → server restarted.
                    entry.guid = guid;
                    if now.duration_since(entry.first_seen) < ANOMALY_WINDOW {
                        // Anomaly: GUID flipped within 5 min of first seen.
                        // Throttle reconnects for the remainder of the window.
                        entry.suppress_until = Some(entry.first_seen + ANOMALY_WINDOW);
                        false
                    } else {
                        entry.first_seen = now;
                        entry.suppress_until = None;
                        true
                    }
                }
            }
        }
    }

    /// Most recent GUID observed for `server`, or `None` if we
    /// haven't seen a beacon from it yet. Used by Channel reconnect
    /// to detect server replacement at the same address (P-G12).
    pub fn guid_for(&self, server: SocketAddr) -> Option<[u8; 12]> {
        self.inner.read().get(&server).map(|e| e.guid)
    }

    /// True iff the server is currently in the throttle window.
    pub fn is_throttled(&self, server: SocketAddr) -> bool {
        let map = self.inner.read();
        match map.get(&server) {
            Some(entry) => match entry.suppress_until {
                Some(deadline) => Instant::now() < deadline,
                None => false,
            },
            None => false,
        }
    }

    /// Forget a server (called when we explicitly disconnect & don't intend
    /// to reconnect).
    pub fn forget(&self, server: SocketAddr) {
        self.inner.write().remove(&server);
    }

    /// Drop entries whose last beacon is older than `max_age`. Returns the
    /// list of (server, guid) tuples that were pruned so the caller can
    /// raise a `Discovered::Timeout` for each. Mirrors pvxs
    /// `tickBeaconClean` (client.cpp:1254) which prunes after 2× the
    /// beacon-clean interval (default 360s).
    pub fn prune_stale(&self, max_age: Duration) -> Vec<(SocketAddr, [u8; 12])> {
        let now = Instant::now();
        let mut map = self.inner.write();
        let mut pruned = Vec::new();
        map.retain(|server, entry| {
            if now.duration_since(entry.last_seen) > max_age {
                pruned.push((*server, entry.guid));
                false
            } else {
                true
            }
        });
        pruned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn addr() -> SocketAddr {
        SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 5075)
    }

    #[test]
    fn first_observation_passes_through() {
        let t = BeaconTracker::new();
        assert!(t.observe(addr(), [1u8; 12]));
    }

    #[test]
    fn same_guid_repeats_pass_through() {
        let t = BeaconTracker::new();
        assert!(t.observe(addr(), [1u8; 12]));
        assert!(t.observe(addr(), [1u8; 12]));
        assert!(t.observe(addr(), [1u8; 12]));
    }

    #[test]
    fn guid_change_within_window_throttles() {
        let t = BeaconTracker::new();
        assert!(t.observe(addr(), [1u8; 12]));
        assert!(!t.observe(addr(), [2u8; 12]));
        assert!(t.is_throttled(addr()));
    }

    #[test]
    fn forget_clears_state() {
        let t = BeaconTracker::new();
        t.observe(addr(), [1u8; 12]);
        t.forget(addr());
        assert!(!t.is_throttled(addr()));
    }

    /// Stale entries — last_seen older than `max_age` — are pruned and
    /// returned so the caller can fire `Discovered::Timeout`. Mirrors
    /// pvxs `tickBeaconClean` (client.cpp:1254).
    #[test]
    fn prune_stale_returns_aged_out_entries() {
        let t = BeaconTracker::new();
        t.observe(addr(), [9u8; 12]);
        // Immediate prune with a far-future age cutoff drops nothing.
        let pruned = t.prune_stale(Duration::from_secs(3600));
        assert!(pruned.is_empty());
        // Negative-ish (zero) cutoff drops everything currently tracked.
        let pruned = t.prune_stale(Duration::from_secs(0));
        assert_eq!(pruned.len(), 1);
        assert_eq!(pruned[0].0, addr());
        assert_eq!(pruned[0].1, [9u8; 12]);
        // Idempotent: a second call with no entries left returns empty.
        assert!(t.prune_stale(Duration::from_secs(0)).is_empty());
    }
}
