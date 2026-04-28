//! Upstream CA client adapter for the gateway.
//!
//! Manages connections to upstream IOCs via [`epics_ca_rs::client::CaClient`].
//! When a downstream client first searches for a PV, the gateway uses
//! `UpstreamManager` to:
//!
//! 1. Create a CA channel to the upstream IOC
//! 2. Subscribe with a monitor
//! 3. Spawn a task that forwards events into the [`PvCache`] and a
//!    shadow [`PvDatabase`] (which the downstream CaServer queries)
//!
//! When the last downstream subscriber leaves, the upstream channel is
//! kept alive (Inactive state) until the cache cleanup timer evicts it.
//!
//! ## Shadow PvDatabase pattern
//!
//! The gateway uses two stores in parallel:
//!
//! - [`PvCache`] — gateway's view (state machine, subscriber list, stats)
//! - [`PvDatabase`] — `epics-ca-rs::server::CaServer`'s view (the actual
//!   PVs that downstream clients see)
//!
//! Both are kept in sync: every upstream event updates `PvCache.cached`
//! AND posts to the shadow `PvDatabase` via `put_pv_and_post()`, which
//! triggers the CaServer to fan out monitor events to all attached
//! downstream clients.

use std::collections::HashMap;
use std::sync::Arc;

use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::types::EpicsValue;
use epics_ca_rs::client::{CaChannel, CaClient};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::error::{BridgeError, BridgeResult};

use super::cache::{PvCache, PvState};

/// One upstream subscription: the long-lived [`CaChannel`] is shared
/// between the monitor-forwarding task and any direct
/// [`UpstreamManager::put`] / [`UpstreamManager::get`] calls so we
/// don't pay a fresh CREATE_CHAN round-trip per write. Mirrors C
/// CA-gateway behaviour (cas/io/casChannel.cc) where the upstream
/// channel is reused across the gateway's lifetime.
struct UpstreamSubscription {
    channel: Arc<CaChannel>,
    task: JoinHandle<()>,
}

/// Manages upstream CA client connections for the gateway.
///
/// Holds a single shared `CaClient` and tracks per-PV channel +
/// monitor task pairs. The channel is reused for PUT / GET so the
/// gateway does not re-do CA handshake on every write.
pub struct UpstreamManager {
    client: Arc<CaClient>,
    cache: Arc<RwLock<PvCache>>,
    shadow_db: Arc<PvDatabase>,
    /// Active upstream subscriptions, keyed by PV name. Holding the
    /// channel here keeps it alive for the gateway's lifetime so
    /// every PUT / GET reuses one circuit.
    subs: HashMap<String, UpstreamSubscription>,
}

impl UpstreamManager {
    /// Create a new upstream manager.
    ///
    /// The `cache` is the gateway's PV cache. The `shadow_db` is the
    /// `PvDatabase` that the downstream `CaServer` queries.
    pub async fn new(
        cache: Arc<RwLock<PvCache>>,
        shadow_db: Arc<PvDatabase>,
    ) -> BridgeResult<Self> {
        let client = CaClient::new()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("CaClient init: {e}")))?;
        Ok(Self {
            client: Arc::new(client),
            cache,
            shadow_db,
            subs: HashMap::new(),
        })
    }

    /// Number of active upstream subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subs.len()
    }

    /// Whether a given upstream name is currently subscribed.
    pub fn is_subscribed(&self, name: &str) -> bool {
        self.subs.contains_key(name)
    }

    /// Ensure an upstream subscription exists for `upstream_name`.
    ///
    /// If already subscribed, this is a no-op. Otherwise:
    /// 1. Create CA channel to upstream
    /// 2. Subscribe (monitor)
    /// 3. Insert/update cache entry to `Connecting`
    /// 4. Spawn forwarding task
    ///
    /// The cache entry transitions to `Inactive` once the first event
    /// arrives, then `Active` when a downstream subscriber attaches.
    pub async fn ensure_subscribed(&mut self, upstream_name: &str) -> BridgeResult<()> {
        if self.subs.contains_key(upstream_name) {
            return Ok(());
        }

        // Add entry to cache (or get existing) and reset to Connecting
        {
            let mut cache = self.cache.write().await;
            let entry = cache.get_or_create(upstream_name);
            entry.write().await.set_state(PvState::Connecting);
        }

        // Add to shadow PvDatabase as a simple PV with placeholder value
        // (will be overwritten on first upstream event).
        self.shadow_db
            .add_pv(upstream_name, EpicsValue::Double(0.0))
            .await;

        // Create one CA channel and reuse it for both monitor and
        // direct PUT/GET. Stored as Arc so the lifecycle guard fires
        // exactly once when the subscription is dropped.
        let channel = Arc::new(self.client.create_channel(upstream_name));

        // Subscribe (monitor receiver is independent of the channel handle).
        let mut monitor = channel
            .subscribe()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("subscribe failed: {e}")))?;

        // Spawn forwarding task — does NOT borrow the channel, so the
        // direct put()/get() path can use the same channel without
        // contention.
        let cache_clone = self.cache.clone();
        let db_clone = self.shadow_db.clone();
        let name = upstream_name.to_string();
        let task = tokio::spawn(async move {
            while let Some(result) = monitor.recv().await {
                let snapshot = match result {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                // Update gateway cache
                if let Some(entry_arc) = cache_clone.read().await.get(&name) {
                    let mut entry = entry_arc.write().await;
                    // First event → transition Connecting → Inactive
                    if entry.state == PvState::Connecting {
                        entry.set_state(PvState::Inactive);
                    }
                    entry.update(snapshot.clone());
                }

                // Push to shadow PvDatabase to fan out to downstream clients
                let _ = db_clone
                    .put_pv_and_post(&name, snapshot.value.clone())
                    .await;
            }
            // Monitor closed → upstream disconnected
            if let Some(entry_arc) = cache_clone.read().await.get(&name) {
                entry_arc.write().await.set_state(PvState::Disconnect);
            }
        });

        self.subs.insert(
            upstream_name.to_string(),
            UpstreamSubscription { channel, task },
        );
        Ok(())
    }

    /// Remove an upstream subscription and abort its task. Drops
    /// the cached channel — the next `ensure_subscribed` will open
    /// a fresh one.
    pub async fn unsubscribe(&mut self, upstream_name: &str) {
        if let Some(sub) = self.subs.remove(upstream_name) {
            sub.task.abort();
        }
    }

    /// Forward a put operation to the upstream IOC. Reuses the
    /// existing subscribed channel when available, avoiding a
    /// fresh CREATE_CHAN round-trip per write. Falls back to a
    /// transient channel only when the PV has no subscription.
    pub async fn put(&self, upstream_name: &str, value: &EpicsValue) -> BridgeResult<()> {
        if let Some(sub) = self.subs.get(upstream_name) {
            return sub
                .channel
                .put(value)
                .await
                .map_err(|e| BridgeError::PutRejected(format!("upstream put: {e}")));
        }
        let channel = self.client.create_channel(upstream_name);
        channel
            .put(value)
            .await
            .map_err(|e| BridgeError::PutRejected(format!("upstream put: {e}")))
    }

    /// Get the current value from upstream. Reuses the subscribed
    /// channel when available; otherwise opens a transient one.
    pub async fn get(&self, upstream_name: &str) -> BridgeResult<EpicsValue> {
        if let Some(sub) = self.subs.get(upstream_name) {
            let (_dbf, value) = sub
                .channel
                .get()
                .await
                .map_err(|e| BridgeError::PutRejected(format!("upstream get: {e}")))?;
            return Ok(value);
        }
        let channel = self.client.create_channel(upstream_name);
        let (_dbf, value) = channel
            .get()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("upstream get: {e}")))?;
        Ok(value)
    }

    /// Sweep cache and remove upstream subscriptions for entries that
    /// no longer exist in the cache (e.g., evicted by cleanup).
    pub async fn sweep_orphaned(&mut self) {
        let cache = self.cache.read().await;
        let live_names: Vec<String> = self
            .subs
            .keys()
            .filter(|name| cache.get(name).is_none())
            .cloned()
            .collect();
        drop(cache);

        for name in live_names {
            self.unsubscribe(&name).await;
        }
    }

    /// Abort all active subscriptions and shut down.
    pub async fn shutdown(&mut self) {
        for (_name, sub) in self.subs.drain() {
            sub.task.abort();
        }
        self.client.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn manager_construct() {
        let cache = Arc::new(RwLock::new(PvCache::new()));
        let db = Arc::new(PvDatabase::new());
        let mgr = UpstreamManager::new(cache, db).await;
        assert!(mgr.is_ok());

        let mgr = mgr.unwrap();
        assert_eq!(mgr.subscription_count(), 0);
        assert!(!mgr.is_subscribed("ANY"));
    }

    #[test]
    fn _entry_imports() {
        // Sanity check that the cache types are in scope
        let _ = super::super::cache::GwPvEntry::new_connecting("X");
    }
}
