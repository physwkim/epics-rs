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
use epics_ca_rs::client::CaClient;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::error::{BridgeError, BridgeResult};

use super::cache::{PvCache, PvState};

/// Manages upstream CA client connections for the gateway.
///
/// Holds a single shared `CaClient` and tracks per-PV monitor tasks.
/// Each subscribed PV has a background task that forwards upstream
/// events to the cache + shadow database.
pub struct UpstreamManager {
    client: Arc<CaClient>,
    cache: Arc<RwLock<PvCache>>,
    shadow_db: Arc<PvDatabase>,
    /// Active monitor tasks (one per upstream PV).
    tasks: HashMap<String, JoinHandle<()>>,
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
            tasks: HashMap::new(),
        })
    }

    /// Number of active upstream subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.tasks.len()
    }

    /// Whether a given upstream name is currently subscribed.
    pub fn is_subscribed(&self, name: &str) -> bool {
        self.tasks.contains_key(name)
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
        if self.tasks.contains_key(upstream_name) {
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

        // Create CA channel
        let channel = self.client.create_channel(upstream_name);

        // Subscribe
        let mut monitor = channel
            .subscribe()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("subscribe failed: {e}")))?;

        // Spawn forwarding task
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

        self.tasks.insert(upstream_name.to_string(), task);
        Ok(())
    }

    /// Remove an upstream subscription and abort its task.
    pub async fn unsubscribe(&mut self, upstream_name: &str) {
        if let Some(task) = self.tasks.remove(upstream_name) {
            task.abort();
        }
    }

    /// Forward a put operation to the upstream IOC.
    pub async fn put(&self, upstream_name: &str, value: &EpicsValue) -> BridgeResult<()> {
        let channel = self.client.create_channel(upstream_name);
        channel
            .put(value)
            .await
            .map_err(|e| BridgeError::PutRejected(format!("upstream put: {e}")))
    }

    /// Get the current value from upstream (one-shot, bypasses cache).
    pub async fn get(&self, upstream_name: &str) -> BridgeResult<EpicsValue> {
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
            .tasks
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
        for (_name, task) in self.tasks.drain() {
            task.abort();
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
