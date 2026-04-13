//! PV cache for the CA gateway.
//!
//! Corresponds to C++ `gateServer` PV cache (`pv_list`, `pv_con_list`,
//! `vc_list`) plus the per-PV state machine in `gatePvData`.
//!
//! ## State machine
//!
//! ```text
//!   ┌──────┐  upstream search  ┌────────────┐  connect callback  ┌──────────┐
//!   │ Dead ├──────────────────►│ Connecting ├───────────────────►│ Inactive │
//!   └──────┘                   └─────┬──────┘                    └────┬─────┘
//!      ▲                             │                                │
//!      │                             │ timeout                first subscriber
//!      │                             ▼                                │
//!      │                       ┌──────────┐                           ▼
//!      └───────────────────────┤   Dead   │                      ┌────────┐
//!                              └──────────┘                      │ Active │
//!                                                                └────┬───┘
//!      ┌────────────┐                                                 │
//!      │ Disconnect │◄──── upstream disconnect (Inactive)             │
//!      └─────┬──────┘                                                 │
//!            │                                                        │
//!            │ timeout                                                │
//!            ▼                                                        │
//!      ┌──────────┐                                                   │
//!      │   Dead   │                last subscriber leaves             │
//!      └──────────┘◄──────────────────────────────────────────────────┘

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use epics_base_rs::server::snapshot::Snapshot;
use tokio::sync::RwLock;

/// State of a cached PV in the gateway.
///
/// Corresponds to C++ `gatePvData` states:
/// `gatePvDead`, `gatePvConnect`, `gatePvInactive`, `gatePvActive`,
/// `gatePvDisconnect`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PvState {
    /// No upstream connection, no clients.
    Dead,
    /// Upstream connect in progress.
    Connecting,
    /// Upstream connected, no active downstream subscribers.
    Inactive,
    /// Upstream connected, ≥1 downstream subscriber.
    Active,
    /// Upstream connection lost, cleanup pending.
    Disconnect,
}

impl PvState {
    /// Whether the gateway considers this PV "exists" for downstream search.
    pub fn is_existent(self) -> bool {
        matches!(self, Self::Inactive | Self::Active)
    }
}

/// One PV in the gateway cache.
///
/// Tracks the upstream connection state, the most recent value snapshot
/// (for serving cached reads), the list of downstream subscriber IDs
/// (for fan-out), and timing information for cleanup heuristics.
#[derive(Debug)]
pub struct GwPvEntry {
    /// Upstream PV name (after alias resolution).
    pub name: String,
    /// Current state in the lifecycle FSM.
    pub state: PvState,
    /// Most recent value + metadata received from upstream.
    /// `None` until the first event arrives after upstream connection.
    pub cached: Option<Snapshot>,
    /// Subscription IDs of downstream clients monitoring this PV.
    /// Used as a reference count: when empty, the PV transitions
    /// from `Active` to `Inactive`.
    pub subscribers: Vec<u32>,
    /// When the current state was entered. Used by cleanup to evict
    /// PVs that have been Inactive/Dead/Disconnect for too long.
    pub state_since: Instant,
    /// Total events received from upstream (for stats).
    pub event_count: u64,
}

impl GwPvEntry {
    /// Create a new entry in the `Connecting` state.
    pub fn new_connecting(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            state: PvState::Connecting,
            cached: None,
            subscribers: Vec::new(),
            state_since: Instant::now(),
            event_count: 0,
        }
    }

    /// Transition to a new state and reset the state timestamp.
    pub fn set_state(&mut self, new: PvState) {
        if self.state != new {
            self.state = new;
            self.state_since = Instant::now();
        }
    }

    /// Add a downstream subscriber. If this is the first subscriber and
    /// the PV is Inactive, transition to Active.
    pub fn add_subscriber(&mut self, sid: u32) {
        if !self.subscribers.contains(&sid) {
            self.subscribers.push(sid);
        }
        if self.state == PvState::Inactive && !self.subscribers.is_empty() {
            self.set_state(PvState::Active);
        }
    }

    /// Remove a downstream subscriber. If this was the last subscriber
    /// and the PV is Active, transition to Inactive.
    pub fn remove_subscriber(&mut self, sid: u32) {
        self.subscribers.retain(|s| *s != sid);
        if self.state == PvState::Active && self.subscribers.is_empty() {
            self.set_state(PvState::Inactive);
        }
    }

    /// How many downstream subscribers are currently attached.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Update cached snapshot from a new upstream event.
    pub fn update(&mut self, snap: Snapshot) {
        self.cached = Some(snap);
        self.event_count += 1;
    }

    /// Time elapsed in the current state.
    pub fn time_in_state(&self) -> Duration {
        self.state_since.elapsed()
    }
}

/// Timeout configuration for cache cleanup.
///
/// Defaults match C++ ca-gateway:
/// - `connect_timeout`: 1s — drop Connecting PVs that don't reach Inactive
/// - `inactive_timeout`: 2h — drop Inactive PVs with no subscribers
/// - `dead_timeout`: 2min — drop Dead PVs after this delay
/// - `disconnect_timeout`: 2h — drop Disconnect PVs after this delay
#[derive(Debug, Clone, Copy)]
pub struct CacheTimeouts {
    pub connect_timeout: Duration,
    pub inactive_timeout: Duration,
    pub dead_timeout: Duration,
    pub disconnect_timeout: Duration,
}

impl Default for CacheTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(1),
            inactive_timeout: Duration::from_secs(60 * 60 * 2),
            dead_timeout: Duration::from_secs(60 * 2),
            disconnect_timeout: Duration::from_secs(60 * 60 * 2),
        }
    }
}

/// Gateway PV cache.
///
/// Maps upstream PV name → cache entry. Each entry is wrapped in
/// `Arc<RwLock>` so multiple downstream client tasks and the upstream
/// event handler can share access.
///
/// Corresponds to C++ `gateServer::pv_list` (HashMap of `gatePvData`).
#[derive(Debug, Default)]
pub struct PvCache {
    entries: HashMap<String, Arc<RwLock<GwPvEntry>>>,
}

impl PvCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up an entry by upstream name.
    pub fn get(&self, name: &str) -> Option<Arc<RwLock<GwPvEntry>>> {
        self.entries.get(name).cloned()
    }

    /// Insert a new entry, replacing any existing one with the same name.
    /// Returns the inserted Arc.
    pub fn insert(&mut self, entry: GwPvEntry) -> Arc<RwLock<GwPvEntry>> {
        let name = entry.name.clone();
        let arc = Arc::new(RwLock::new(entry));
        self.entries.insert(name, arc.clone());
        arc
    }

    /// Get an existing entry or create a new one in the `Connecting` state.
    pub fn get_or_create(&mut self, name: &str) -> Arc<RwLock<GwPvEntry>> {
        if let Some(arc) = self.entries.get(name) {
            return arc.clone();
        }
        self.insert(GwPvEntry::new_connecting(name.to_string()))
    }

    /// Remove an entry by name.
    pub fn remove(&mut self, name: &str) -> Option<Arc<RwLock<GwPvEntry>>> {
        self.entries.remove(name)
    }

    /// All entry names (for stats / introspection).
    pub fn names(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Count entries by state.
    pub async fn count_by_state(&self, state: PvState) -> usize {
        let mut count = 0;
        for entry in self.entries.values() {
            if entry.read().await.state == state {
                count += 1;
            }
        }
        count
    }

    /// Sweep expired entries based on timeouts.
    /// Returns the names of removed entries.
    ///
    /// Corresponds to C++ `gateServer::connectCleanup` and
    /// `gateServer::inactiveDeadCleanup`.
    pub async fn cleanup(&mut self, timeouts: &CacheTimeouts) -> Vec<String> {
        let mut to_remove = Vec::new();

        for (name, entry) in &self.entries {
            let entry = entry.read().await;
            let elapsed = entry.time_in_state();
            let expired = match entry.state {
                PvState::Connecting => elapsed > timeouts.connect_timeout,
                PvState::Inactive => elapsed > timeouts.inactive_timeout,
                PvState::Dead => elapsed > timeouts.dead_timeout,
                PvState::Disconnect => elapsed > timeouts.disconnect_timeout,
                PvState::Active => false, // Active PVs are never evicted
            };
            if expired {
                to_remove.push(name.clone());
            }
        }

        for name in &to_remove {
            self.entries.remove(name);
        }
        to_remove
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epics_base_rs::types::EpicsValue;
    use std::time::SystemTime;

    fn dummy_snapshot(v: f64) -> Snapshot {
        Snapshot::new(EpicsValue::Double(v), 0, 0, SystemTime::now())
    }

    #[test]
    fn pv_state_is_existent() {
        assert!(PvState::Inactive.is_existent());
        assert!(PvState::Active.is_existent());
        assert!(!PvState::Dead.is_existent());
        assert!(!PvState::Connecting.is_existent());
        assert!(!PvState::Disconnect.is_existent());
    }

    #[test]
    fn entry_subscriber_lifecycle() {
        let mut e = GwPvEntry::new_connecting("TEMP");
        assert_eq!(e.state, PvState::Connecting);
        assert_eq!(e.subscriber_count(), 0);

        // Simulate upstream connect → Inactive
        e.set_state(PvState::Inactive);
        assert_eq!(e.state, PvState::Inactive);

        // First subscriber → Active
        e.add_subscriber(1);
        assert_eq!(e.state, PvState::Active);
        assert_eq!(e.subscriber_count(), 1);

        // Second subscriber stays Active
        e.add_subscriber(2);
        assert_eq!(e.state, PvState::Active);
        assert_eq!(e.subscriber_count(), 2);

        // Duplicate add is a no-op
        e.add_subscriber(2);
        assert_eq!(e.subscriber_count(), 2);

        // Remove first subscriber stays Active
        e.remove_subscriber(1);
        assert_eq!(e.state, PvState::Active);
        assert_eq!(e.subscriber_count(), 1);

        // Remove last subscriber → Inactive
        e.remove_subscriber(2);
        assert_eq!(e.state, PvState::Inactive);
        assert_eq!(e.subscriber_count(), 0);
    }

    #[test]
    fn entry_update_increments_event_count() {
        let mut e = GwPvEntry::new_connecting("TEMP");
        assert_eq!(e.event_count, 0);
        assert!(e.cached.is_none());

        e.update(dummy_snapshot(1.0));
        assert_eq!(e.event_count, 1);
        assert!(e.cached.is_some());

        e.update(dummy_snapshot(2.0));
        assert_eq!(e.event_count, 2);
    }

    #[tokio::test]
    async fn cache_get_or_create() {
        let mut cache = PvCache::new();
        assert!(cache.is_empty());

        let arc1 = cache.get_or_create("TEMP");
        assert_eq!(cache.len(), 1);
        assert_eq!(arc1.read().await.state, PvState::Connecting);

        // Repeated call returns same Arc
        let arc2 = cache.get_or_create("TEMP");
        assert!(Arc::ptr_eq(&arc1, &arc2));
        assert_eq!(cache.len(), 1);

        // Different name → new entry
        cache.get_or_create("PRESSURE");
        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn cache_count_by_state() {
        let mut cache = PvCache::new();
        let a = cache.insert(GwPvEntry::new_connecting("A"));
        let b = cache.insert(GwPvEntry::new_connecting("B"));
        let _c = cache.insert(GwPvEntry::new_connecting("C"));

        a.write().await.set_state(PvState::Active);
        b.write().await.set_state(PvState::Inactive);

        assert_eq!(cache.count_by_state(PvState::Connecting).await, 1);
        assert_eq!(cache.count_by_state(PvState::Inactive).await, 1);
        assert_eq!(cache.count_by_state(PvState::Active).await, 1);
        assert_eq!(cache.count_by_state(PvState::Dead).await, 0);
    }

    #[tokio::test]
    async fn cache_cleanup_removes_expired() {
        let mut cache = PvCache::new();
        let dead = cache.insert(GwPvEntry::new_connecting("DEAD"));
        let active = cache.insert(GwPvEntry::new_connecting("ALIVE"));

        // Backdate the dead one and put it in Dead state
        {
            let mut e = dead.write().await;
            e.state = PvState::Dead;
            e.state_since = Instant::now() - Duration::from_secs(60 * 60);
        }
        {
            let mut e = active.write().await;
            e.state = PvState::Active;
        }

        let timeouts = CacheTimeouts::default();
        let removed = cache.cleanup(&timeouts).await;

        assert_eq!(removed, vec!["DEAD".to_string()]);
        assert!(cache.get("DEAD").is_none());
        assert!(cache.get("ALIVE").is_some());
    }
}
