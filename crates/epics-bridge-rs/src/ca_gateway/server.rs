//! Top-level gateway server.
//!
//! Ties together [`PvCache`], [`UpstreamManager`], [`DownstreamServer`],
//! [`PvList`], [`AccessConfig`], [`Stats`] into a single async daemon.
//!
//! ## Main event loop
//!
//! ```text
//! loop {
//!     tokio::select! {
//!         _ = downstream.run()    => break,    // CaServer drives downstream
//!         _ = cleanup_tick.tick() => cache.cleanup() + upstream.sweep_orphaned()
//!         _ = stats_tick.tick()   => stats.refresh() + publish to gateway:* PVs
//!         _ = heartbeat_tick.tick() => heartbeat counter ++
//!         _ = signal_handler      => reload pvlist / dump report
//!     }
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::server::database::PvDatabase;
use tokio::sync::RwLock;

use crate::error::BridgeResult;

use super::access::AccessConfig;
use super::beacon::BeaconAnomaly;
use super::cache::{CacheTimeouts, PvCache};
use super::command::CommandHandler;
use super::downstream::DownstreamServer;
use super::putlog::PutLog;
use super::pvlist::PvList;
use super::stats::Stats;
use super::upstream::UpstreamManager;

/// Configuration for [`GatewayServer`].
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    /// Path to `.pvlist` file.
    pub pvlist_path: Option<PathBuf>,
    /// Inline pvlist content (alternative to file).
    pub pvlist_content: Option<String>,
    /// Path to `.access` (ACF) file.
    pub access_path: Option<PathBuf>,
    /// Optional path to put-event log file.
    pub putlog_path: Option<PathBuf>,
    /// Optional path to a command file processed on SIGUSR1 (Unix only).
    /// Each non-comment line is a [`super::command::GatewayCommand`].
    pub command_path: Option<PathBuf>,
    /// Optional path to a file containing literal upstream PV names to
    /// pre-subscribe (one per line). When set, the gateway pre-fetches
    /// each name on startup. Used because lazy resolution is not yet
    /// supported (see `downstream.rs` doc comment).
    pub preload_path: Option<PathBuf>,
    /// CA server port (downstream side). 0 = use EPICS default.
    pub server_port: u16,
    /// Cache timeouts.
    pub timeouts: CacheTimeouts,
    /// Statistics PV prefix (e.g. `"gateway:"`). Empty disables stats PVs.
    pub stats_prefix: String,
    /// Cleanup sweep interval.
    pub cleanup_interval: Duration,
    /// Statistics refresh interval.
    pub stats_interval: Duration,
    /// Heartbeat increment interval. `None` disables the heartbeat PV.
    pub heartbeat_interval: Option<Duration>,
    /// Read-only mode: rejects all puts.
    pub read_only: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            pvlist_path: None,
            pvlist_content: None,
            access_path: None,
            putlog_path: None,
            command_path: None,
            preload_path: None,
            server_port: 0,
            timeouts: CacheTimeouts::default(),
            stats_prefix: "gateway:".to_string(),
            cleanup_interval: Duration::from_secs(10),
            stats_interval: Duration::from_secs(10),
            heartbeat_interval: Some(Duration::from_secs(1)),
            read_only: false,
        }
    }
}

/// The CA gateway server.
///
/// Construct via [`GatewayServer::build`], then call [`GatewayServer::run`]
/// to start the daemon.
pub struct GatewayServer {
    config: GatewayConfig,
    /// Wrapped in RwLock<Arc<...>> so live reload can swap the pvlist
    /// atomically (via SIGUSR1 PVL command).
    pvlist: Arc<RwLock<Arc<PvList>>>,
    access: Arc<AccessConfig>,
    cache: Arc<RwLock<PvCache>>,
    shadow_db: Arc<PvDatabase>,
    upstream: Arc<RwLock<UpstreamManager>>,
    downstream: Arc<DownstreamServer>,
    stats: Arc<Stats>,
    putlog: Option<Arc<PutLog>>,
    beacon_anomaly: Arc<BeaconAnomaly>,
}

impl GatewayServer {
    /// Build the gateway from configuration.
    ///
    /// Loads pvlist + access files, initializes cache + upstream client,
    /// constructs downstream CA server. Does not start any I/O — call
    /// [`GatewayServer::run`] for that.
    pub async fn build(config: GatewayConfig) -> BridgeResult<Self> {
        // Load .pvlist
        let pvlist = if let Some(path) = &config.pvlist_path {
            super::pvlist::parse_pvlist_file(path)?
        } else if let Some(content) = &config.pvlist_content {
            super::pvlist::parse_pvlist(content)?
        } else {
            // Empty pvlist allows nothing
            PvList::new()
        };
        let pvlist = Arc::new(RwLock::new(Arc::new(pvlist)));

        // Load .access (optional)
        let access = if let Some(path) = &config.access_path {
            AccessConfig::from_file(path)?
        } else {
            AccessConfig::allow_all()
        };
        let access = Arc::new(access);

        // Cache + shadow database
        let cache = Arc::new(RwLock::new(PvCache::new()));
        let shadow_db = Arc::new(PvDatabase::new());

        // Upstream manager
        let upstream = UpstreamManager::new(cache.clone(), shadow_db.clone()).await?;
        let upstream = Arc::new(RwLock::new(upstream));

        // Downstream server
        let downstream = Arc::new(DownstreamServer::new(shadow_db.clone(), config.server_port));

        // Stats
        let stats = Arc::new(Stats::new(config.stats_prefix.clone()));

        // Put-event logger (optional)
        let putlog = config
            .putlog_path
            .as_ref()
            .map(|p| Arc::new(PutLog::new(p.clone())));

        // Beacon anomaly throttle
        let beacon_anomaly = Arc::new(BeaconAnomaly::new());

        let server = Self {
            config,
            pvlist,
            access,
            cache,
            shadow_db,
            upstream,
            downstream,
            stats,
            putlog,
            beacon_anomaly,
        };

        // Pre-register stats PVs in shadow database so downstream can read them
        server.stats.publish_initial(&server.shadow_db).await;

        // Install lazy search resolver: when an unknown name is searched
        // for, check the .pvlist and (if allowed) subscribe upstream.
        server.install_search_resolver().await;

        Ok(server)
    }

    /// Install the lazy search resolver into the shadow PvDatabase.
    ///
    /// This implements the equivalent of C++ ca-gateway's
    /// `gateServer::pvExistTest()` (gateServer.cc:1484), but at a
    /// different layer: C++ overrides `caServer::pvExistTest`, while
    /// epics-rs hooks `PvDatabase::set_search_resolver`. The effect is
    /// the same — when a downstream client searches for an unknown
    /// name, the gateway is given a chance to consult the `.pvlist`,
    /// subscribe upstream, and report whether the name became
    /// resolvable.
    ///
    /// Called once during build().
    async fn install_search_resolver(&self) {
        let pvlist = self.pvlist.clone();
        let upstream = self.upstream.clone();
        let stats = self.stats.clone();

        let resolver: epics_base_rs::server::database::SearchResolver = std::sync::Arc::new(
            move |name: String| -> std::pin::Pin<
                Box<dyn std::future::Future<Output = bool> + Send>,
            > {
                let pvlist = pvlist.clone();
                let upstream = upstream.clone();
                let stats = stats.clone();
                Box::pin(async move {
                    // 1. Check pvlist
                    let m = {
                        let pvlist = pvlist.read().await;
                        pvlist.match_name(&name)
                    };
                    let m = match m {
                        Some(m) => m,
                        None => return false,
                    };

                    // 2. Subscribe upstream — this also adds the PV to the
                    //    shadow database via UpstreamManager::ensure_subscribed
                    let mut up = upstream.write().await;
                    if up.ensure_subscribed(&m.resolved_name).await.is_err() {
                        return false;
                    }
                    drop(up);

                    // 3. Stats: count this resolution
                    stats.record_event();
                    true
                })
            },
        );

        self.shadow_db.set_search_resolver(resolver).await;
    }

    /// Pre-subscribe to upstream PVs from the preload file.
    pub async fn preload_pvs(&self) -> BridgeResult<usize> {
        let path = match &self.config.preload_path {
            Some(p) => p,
            None => return Ok(0),
        };
        let content = std::fs::read_to_string(path)?;
        let mut count = 0;

        for line in content.lines() {
            let name = line.trim();
            if name.is_empty() || name.starts_with('#') {
                continue;
            }

            // Resolve through pvlist (alias or allow check)
            let m = {
                let pvlist = self.pvlist.read().await;
                pvlist.match_name(name)
            };
            let m = match m {
                Some(m) => m,
                None => continue, // Denied or not in list
            };

            let mut up = self.upstream.write().await;
            up.ensure_subscribed(&m.resolved_name).await?;
            count += 1;
        }

        Ok(count)
    }

    /// Access the shadow database (for stats publication, testing).
    pub fn shadow_database(&self) -> &Arc<PvDatabase> {
        &self.shadow_db
    }

    /// Access the cache (for stats, introspection).
    pub fn cache(&self) -> &Arc<RwLock<PvCache>> {
        &self.cache
    }

    /// Access the pvlist (wrapped in RwLock for hot reload).
    pub fn pvlist(&self) -> &Arc<RwLock<Arc<PvList>>> {
        &self.pvlist
    }

    /// Access the access security config.
    pub fn access(&self) -> &Arc<AccessConfig> {
        &self.access
    }

    /// Access stats.
    pub fn stats(&self) -> &Arc<Stats> {
        &self.stats
    }

    /// Access the put-event logger (if configured).
    pub fn putlog(&self) -> Option<&Arc<PutLog>> {
        self.putlog.as_ref()
    }

    /// Access the beacon anomaly throttle.
    pub fn beacon_anomaly(&self) -> &Arc<BeaconAnomaly> {
        &self.beacon_anomaly
    }

    /// Run the gateway daemon. Blocks until shutdown.
    pub async fn run(self) -> BridgeResult<()> {
        // Pre-load configured upstream PVs
        let preloaded = self.preload_pvs().await?;
        eprintln!("[ca-gateway-rs] preloaded {preloaded} upstream PVs");

        let downstream = self.downstream.clone();
        let cache = self.cache.clone();
        let upstream = self.upstream.clone();
        let stats = self.stats.clone();
        let shadow_db = self.shadow_db.clone();
        let timeouts = self.config.timeouts;
        let cleanup_interval = self.config.cleanup_interval;
        let stats_interval = self.config.stats_interval;
        let heartbeat_interval = self.config.heartbeat_interval;

        // Cleanup task
        let cache_for_cleanup = cache.clone();
        let upstream_for_cleanup = upstream.clone();
        let cleanup_handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(cleanup_interval);
            tick.tick().await; // first tick is immediate, skip
            loop {
                tick.tick().await;
                let removed = cache_for_cleanup.write().await.cleanup(&timeouts).await;
                if !removed.is_empty() {
                    let mut up = upstream_for_cleanup.write().await;
                    up.sweep_orphaned().await;
                    eprintln!("[ca-gateway-rs] evicted {} expired PVs", removed.len());
                }
            }
        });

        // Stats refresh task
        let cache_for_stats = cache.clone();
        let upstream_for_stats = upstream.clone();
        let stats_for_refresh = stats.clone();
        let db_for_stats = shadow_db.clone();
        let stats_handle = tokio::spawn(async move {
            let mut tick = tokio::time::interval(stats_interval);
            tick.tick().await;
            loop {
                tick.tick().await;
                let cache_size = cache_for_stats.read().await.len();
                let upstream_count = upstream_for_stats.read().await.subscription_count();
                stats_for_refresh
                    .refresh(&cache_for_stats, &db_for_stats, cache_size, upstream_count)
                    .await;
            }
        });

        // Heartbeat task
        let heartbeat_handle = if let Some(period) = heartbeat_interval {
            let stats_hb = stats.clone();
            let db_hb = shadow_db.clone();
            Some(tokio::spawn(async move {
                let mut tick = tokio::time::interval(period);
                tick.tick().await;
                loop {
                    tick.tick().await;
                    stats_hb.heartbeat_tick(&db_hb).await;
                }
            }))
        } else {
            None
        };

        // SIGUSR1 → command file processing (Unix only)
        let signal_handle = self.spawn_signal_handler();

        // Connection event subscriber (per-host tracking)
        let conn_rx = downstream.connection_events().await;
        let conn_handle = if let Some(mut rx) = conn_rx {
            let stats_for_conn = stats.clone();
            Some(tokio::spawn(async move {
                use epics_ca_rs::server::ServerConnectionEvent;
                loop {
                    match rx.recv().await {
                        Ok(ServerConnectionEvent::Connected(addr)) => {
                            stats_for_conn.record_host(&addr.ip().to_string()).await;
                        }
                        Ok(ServerConnectionEvent::Disconnected(addr)) => {
                            stats_for_conn.forget_host(&addr.ip().to_string()).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }))
        } else {
            None
        };

        // Run downstream CaServer (blocks)
        let downstream_result = downstream.run().await;

        // Cleanup
        cleanup_handle.abort();
        stats_handle.abort();
        if let Some(h) = heartbeat_handle {
            h.abort();
        }
        if let Some(h) = signal_handle {
            h.abort();
        }
        if let Some(h) = conn_handle {
            h.abort();
        }

        downstream_result
    }

    /// Spawn a Unix SIGUSR1 watcher that re-reads the command file.
    /// Returns None on non-Unix or when no command file is configured.
    #[cfg(unix)]
    fn spawn_signal_handler(&self) -> Option<tokio::task::JoinHandle<()>> {
        let cmd_path = self.config.command_path.clone()?;
        let pvlist_path = self.config.pvlist_path.clone();
        let access_path = self.config.access_path.clone();
        let cache = self.cache.clone();
        let pvlist = self.pvlist.clone();

        Some(tokio::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sigusr1 = match signal(SignalKind::user_defined1()) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[ca-gateway-rs] failed to install SIGUSR1 handler: {e}");
                    return;
                }
            };
            let handler = CommandHandler::new(cache, pvlist, pvlist_path, access_path);
            eprintln!(
                "[ca-gateway-rs] SIGUSR1 handler armed (command file: {})",
                cmd_path.display()
            );
            loop {
                if sigusr1.recv().await.is_none() {
                    break;
                }
                eprintln!("[ca-gateway-rs] SIGUSR1 received — processing command file");
                match handler.process_file(&cmd_path).await {
                    Ok(out) => {
                        if !out.is_empty() {
                            print!("{out}");
                        }
                    }
                    Err(e) => {
                        eprintln!("[ca-gateway-rs] command file error: {e}");
                    }
                }
            }
        }))
    }

    /// Stub for non-Unix platforms (no SIGUSR1).
    #[cfg(not(unix))]
    fn spawn_signal_handler(&self) -> Option<tokio::task::JoinHandle<()>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_with_minimal_config() {
        let config = GatewayConfig {
            pvlist_content: Some("".to_string()),
            ..Default::default()
        };
        let server = GatewayServer::build(config).await;
        assert!(server.is_ok(), "build failed: {:?}", server.err());
    }

    #[tokio::test]
    async fn build_with_inline_pvlist() {
        let config = GatewayConfig {
            pvlist_content: Some(
                r#"
                EVALUATION ORDER ALLOW, DENY
                Beam:.* ALLOW BeamGroup 1
                test.* DENY
                "#
                .to_string(),
            ),
            ..Default::default()
        };
        let server = GatewayServer::build(config).await.unwrap();
        let pvlist = server.pvlist().read().await.clone();
        assert!(pvlist.match_name("Beam:current").is_some());
        assert!(pvlist.match_name("test:foo").is_none());
    }

    #[tokio::test]
    async fn build_unknown_acf_path_returns_error() {
        let config = GatewayConfig {
            access_path: Some(PathBuf::from("/nonexistent/file.acf")),
            ..Default::default()
        };
        let result = GatewayServer::build(config).await;
        assert!(result.is_err());
    }
}
