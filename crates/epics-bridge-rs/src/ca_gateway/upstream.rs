//! Upstream CA client adapter for the gateway.
//!
//! Manages connections to upstream IOCs via [`epics_ca_rs::client::CaClient`].
//! When a downstream client first searches for a PV, the gateway uses
//! `UpstreamManager` to:
//!
//! 1. Create a CA channel to the upstream IOC
//! 2. Subscribe with a monitor + a one-shot GET to learn the native type
//! 3. Spawn a task that forwards events into the [`PvCache`] and a
//!    shadow [`PvDatabase`] (which the downstream CaServer queries)
//! 4. Install a [`WriteHook`] on the shadow PV so client-originated
//!    writes are forwarded upstream rather than landing locally
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
//!
//! ## Auto-restart
//!
//! The monitor-forwarding task wraps `channel.subscribe()` in an
//! exponential-backoff retry loop so a transient upstream disconnect
//! does not strand the cache entry forever (the entry's `cached`
//! snapshot would otherwise be served indefinitely while no further
//! events arrive). On terminal failure the entry transitions to the
//! `Disconnect` state, which the cleanup tick eventually evicts.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::error::CaError;
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::pv::{WriteContext, WriteHook};
use epics_base_rs::types::EpicsValue;
use epics_ca_rs::client::{CaChannel, CaClient};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

use crate::error::{BridgeError, BridgeResult};

use super::access::AccessConfig;
use super::cache::{PvCache, PvState};
use super::putlog::{PutLog, PutOutcome};
use super::pvlist::PvList;
use super::stats::Stats;

/// One upstream subscription: the long-lived [`CaChannel`] is shared
/// between the monitor-forwarding task and any direct
/// [`UpstreamManager::put`] / [`UpstreamManager::get`] calls so we
/// don't pay a fresh CREATE_CHAN round-trip per write. Mirrors C
/// CA-gateway behaviour (cas/io/casChannel.cc) where the upstream
/// channel is reused across the gateway's lifetime.
struct UpstreamSubscription {
    channel: Arc<CaChannel>,
    task: JoinHandle<()>,
    /// Resolved access security group from the matching `.pvlist` rule.
    /// Cached here so the write hook can call `AccessConfig::can_write`
    /// without re-resolving on every put.
    asg: Option<String>,
    /// Resolved access security level (paired with `asg`).
    asl: i32,
}

/// Shared state every upstream subscription's WriteHook needs. Hoisted
/// out of `UpstreamManager` so the per-subscription closure can capture
/// a single small `Arc` instead of a long list of separate handles.
#[derive(Clone)]
struct WriteHookEnv {
    /// Read-only mode rejects all puts.
    read_only: bool,
    /// Live access security config (hot-reloadable).
    access: Arc<RwLock<Arc<AccessConfig>>>,
    /// Live pvlist (hot-reloadable). Used for `DENY FROM host` checks.
    pvlist: Arc<RwLock<Arc<PvList>>>,
    /// Optional put-event log.
    putlog: Option<Arc<PutLog>>,
    /// Stats counters.
    stats: Arc<Stats>,
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
    /// Shared environment captured by every per-PV WriteHook closure.
    write_env: WriteHookEnv,
    /// Active upstream subscriptions, keyed by PV name. Holding the
    /// channel here keeps it alive for the gateway's lifetime so
    /// every PUT / GET reuses one circuit.
    subs: HashMap<String, UpstreamSubscription>,
}

impl UpstreamManager {
    /// Create a new upstream manager.
    ///
    /// The `cache` is the gateway's PV cache. The `shadow_db` is the
    /// `PvDatabase` that the downstream `CaServer` queries. The remaining
    /// handles (`access`, `pvlist`, `putlog`, `stats`, `read_only`) are
    /// captured by every PV's WriteHook so client-originated puts
    /// enforce the gateway's full policy (read-only, ACL, host deny,
    /// putlog) before forwarding upstream.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        cache: Arc<RwLock<PvCache>>,
        shadow_db: Arc<PvDatabase>,
        access: Arc<RwLock<Arc<AccessConfig>>>,
        pvlist: Arc<RwLock<Arc<PvList>>>,
        putlog: Option<Arc<PutLog>>,
        stats: Arc<Stats>,
        read_only: bool,
    ) -> BridgeResult<Self> {
        let client = CaClient::new()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("CaClient init: {e}")))?;
        Ok(Self {
            client: Arc::new(client),
            cache,
            shadow_db,
            write_env: WriteHookEnv {
                read_only,
                access,
                pvlist,
                putlog,
                stats,
            },
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
    /// 2. Try a one-shot `get()` so the shadow PV is registered with
    ///    the upstream's *native* DBR type rather than `Double(0.0)`
    ///    placeholder — improves first-read fidelity. Falls back to
    ///    `Double(0.0)` if the get fails or times out.
    /// 3. Subscribe (monitor)
    /// 4. Insert/update cache entry to `Connecting`
    /// 5. Spawn forwarding task with auto-restart
    /// 6. Install per-PV WriteHook on the shadow PV
    pub async fn ensure_subscribed(
        &mut self,
        upstream_name: &str,
        asg: Option<String>,
        asl: i32,
    ) -> BridgeResult<()> {
        if self.subs.contains_key(upstream_name) {
            return Ok(());
        }

        // Add entry to cache (or get existing) and reset to Connecting
        {
            let mut cache = self.cache.write().await;
            let entry = cache.get_or_create(upstream_name);
            entry.write().await.set_state(PvState::Connecting);
        }

        // Create one CA channel and reuse it for both monitor and
        // direct PUT/GET. Stored as Arc so the lifecycle guard fires
        // exactly once when the subscription is dropped.
        let channel = Arc::new(self.client.create_channel(upstream_name));

        // DBR negotiation: best-effort initial GET so the shadow
        // PV's first registered type matches upstream's native type.
        // Falls back to a Double placeholder if the get fails or
        // times out — the first monitor event will overwrite the
        // value either way.
        let initial_value = match tokio::time::timeout(
            Duration::from_millis(500),
            channel.get(),
        )
        .await
        {
            Ok(Ok((_dbf, v))) => v,
            _ => EpicsValue::Double(0.0),
        };

        self.shadow_db.add_pv(upstream_name, initial_value).await;

        // Install WriteHook so caput goes upstream rather than landing
        // locally. Capture the per-PV ACL fields by value into the
        // closure — `asg`/`asl` are immutable for the subscription's
        // lifetime; the global env (`access`/`pvlist`/`putlog`/`stats`)
        // is hot-reloadable via interior `RwLock`.
        if let Some(pv) = self.shadow_db.find_pv(upstream_name).await {
            let hook = build_write_hook(
                upstream_name.to_string(),
                channel.clone(),
                asg.clone(),
                asl,
                self.write_env.clone(),
            );
            pv.set_write_hook(hook).await;
        }

        // Subscribe (monitor receiver is independent of the channel handle).
        let mut monitor = channel
            .subscribe()
            .await
            .map_err(|e| BridgeError::PutRejected(format!("subscribe failed: {e}")))?;

        // Spawn forwarding task — does NOT borrow the channel, so the
        // direct put()/get() path can use the same channel without
        // contention. Auto-restart is handled by the loop below: if
        // the upstream monitor ends (closed channel, transient I/O
        // error), we re-subscribe with exponential backoff so the
        // shadow PV resumes receiving updates without the search
        // resolver having to re-issue the entire create_channel.
        let cache_clone = self.cache.clone();
        let db_clone = self.shadow_db.clone();
        let channel_for_task = channel.clone();
        let stats_for_task = self.write_env.stats.clone();
        let name = upstream_name.to_string();
        let task = tokio::spawn(async move {
            let mut backoff = Duration::from_millis(250);
            let max_backoff = Duration::from_secs(30);
            loop {
                while let Some(result) = monitor.recv().await {
                    let snapshot = match result {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    stats_for_task.record_event();

                    // Update gateway cache
                    if let Some(entry_arc) = cache_clone.read().await.get(&name) {
                        let mut entry = entry_arc.write().await;
                        // First event → transition Connecting / Disconnect → Inactive
                        if matches!(entry.state, PvState::Connecting | PvState::Disconnect) {
                            entry.set_state(PvState::Inactive);
                        }
                        entry.update(snapshot.clone());
                    }

                    // Push to shadow PvDatabase to fan out to downstream clients
                    let _ = db_clone
                        .put_pv_and_post(&name, snapshot.value.clone())
                        .await;

                    // Re-arm the backoff after a successful event.
                    backoff = Duration::from_millis(250);
                }

                // Monitor closed — upstream disconnected. Mark cache
                // entry so any cached snapshot reads carry the right
                // state (downstream subscriber bridges will keep
                // receiving the *last known* value until cleanup).
                if let Some(entry_arc) = cache_clone.read().await.get(&name) {
                    entry_arc.write().await.set_state(PvState::Disconnect);
                }

                // Try to re-subscribe with exponential backoff. The
                // CaChannel itself drives reconnect under the hood;
                // this loop merely re-arms the monitor stream once
                // the channel is back up. Bail out only if the cache
                // entry has been evicted (i.e. nobody cares anymore).
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(backoff * 2, max_backoff);

                if cache_clone.read().await.get(&name).is_none() {
                    return;
                }

                match channel_for_task.subscribe().await {
                    Ok(new_monitor) => {
                        monitor = new_monitor;
                        // The next successful event will flip state
                        // back to Inactive (see top of inner loop).
                    }
                    Err(_) => {
                        // Stay in Disconnect; another iteration of the
                        // outer loop will retry after the next sleep.
                        continue;
                    }
                }
            }
        });

        self.subs.insert(
            upstream_name.to_string(),
            UpstreamSubscription {
                channel,
                task,
                asg,
                asl,
            },
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

    /// ASG/ASL recorded for `upstream_name` at subscription time, if
    /// any. Used by tests + diagnostics.
    pub fn asg_for(&self, upstream_name: &str) -> Option<(Option<String>, i32)> {
        self.subs
            .get(upstream_name)
            .map(|s| (s.asg.clone(), s.asl))
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

/// Build the [`WriteHook`] closure for one upstream PV. Called once
/// per `ensure_subscribed`; the resulting `Arc<dyn Fn …>` is installed
/// on the shadow `ProcessVariable` so every client `caput` runs this
/// pipeline.
///
/// Pipeline (matches C ca-gateway `gatePvData::putCB` ordering):
/// 1. Read-only mode → reject + record stat + putlog
/// 2. Host-based DENY (pvlist `FROM host`) → reject + putlog
/// 3. ACF `can_write(asg, asl, user, host)` → reject + putlog
/// 4. Forward `caput` to upstream via the shared channel
/// 5. Putlog the outcome (Ok/Failed) and bump put-count stat
fn build_write_hook(
    pv_name: String,
    channel: Arc<CaChannel>,
    asg: Option<String>,
    asl: i32,
    env: WriteHookEnv,
) -> WriteHook {
    Arc::new(move |new_value: EpicsValue, ctx: WriteContext| {
        let pv_name = pv_name.clone();
        let channel = channel.clone();
        let asg = asg.clone();
        let env = env.clone();
        Box::pin(async move {
            let value_str = format!("{new_value}");

            // 1. read-only mode
            if env.read_only {
                env.stats.record_readonly_reject();
                if let Some(pl) = &env.putlog {
                    let _ = pl
                        .log(
                            &ctx.user,
                            &ctx.host,
                            &pv_name,
                            &value_str,
                            PutOutcome::Denied,
                        )
                        .await;
                }
                return Err(CaError::ReadOnlyField(pv_name));
            }

            // 2. pvlist host-based DENY
            let pvlist = env.pvlist.read().await.clone();
            if pvlist.is_host_denied(&pv_name, &ctx.host) {
                if let Some(pl) = &env.putlog {
                    let _ = pl
                        .log(
                            &ctx.user,
                            &ctx.host,
                            &pv_name,
                            &value_str,
                            PutOutcome::Denied,
                        )
                        .await;
                }
                return Err(CaError::ReadOnlyField(pv_name));
            }
            drop(pvlist);

            // 3. AccessConfig
            let access = env.access.read().await.clone();
            let asg_ref = asg.as_deref().unwrap_or("DEFAULT");
            if !access.can_write(asg_ref, asl, &ctx.user, &ctx.host) {
                if let Some(pl) = &env.putlog {
                    let _ = pl
                        .log(
                            &ctx.user,
                            &ctx.host,
                            &pv_name,
                            &value_str,
                            PutOutcome::Denied,
                        )
                        .await;
                }
                return Err(CaError::ReadOnlyField(pv_name));
            }
            drop(access);

            // 4. Forward upstream — propagate CaError directly so the
            // CA TCP write path surfaces the right ECA status to the
            // caller (e.g. ECA_TIMEOUT, ECA_DISCONN).
            let result = channel.put(&new_value).await;

            // 5. Putlog + stats
            if let Some(pl) = &env.putlog {
                let outcome = if result.is_ok() {
                    PutOutcome::Ok
                } else {
                    PutOutcome::Failed
                };
                let _ = pl
                    .log(&ctx.user, &ctx.host, &pv_name, &value_str, outcome)
                    .await;
            }
            if result.is_ok() {
                env.stats.record_put();
            }
            result
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_env() -> WriteHookEnv {
        WriteHookEnv {
            read_only: false,
            access: Arc::new(RwLock::new(Arc::new(AccessConfig::allow_all()))),
            pvlist: Arc::new(RwLock::new(Arc::new(PvList::new()))),
            putlog: None,
            stats: Arc::new(Stats::new("gw:".into())),
        }
    }

    #[tokio::test]
    async fn manager_construct() {
        let cache = Arc::new(RwLock::new(PvCache::new()));
        let db = Arc::new(PvDatabase::new());
        let env = dummy_env();
        let mgr = UpstreamManager::new(
            cache,
            db,
            env.access.clone(),
            env.pvlist.clone(),
            None,
            env.stats.clone(),
            false,
        )
        .await;
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
