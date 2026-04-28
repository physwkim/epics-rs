//! Top-level [`PvaGateway`] handle — wires the upstream
//! [`PvaClient`] + [`ChannelCache`] into a downstream [`PvaServer`].
//!
//! Mirrors `pva2pva/p2pApp/gwmain.cpp`'s `configure_*` /
//! `main` flow: build a client to chase upstream PVs, build a server
//! that downstream clients connect to, and route every server op
//! through the cache.

use std::sync::Arc;
use std::time::Duration;

use epics_pva_rs::client::PvaClient;
use epics_pva_rs::error::PvaResult;
use epics_pva_rs::server_native::{PvaServer, PvaServerConfig, runtime::ServerReport};

use super::channel_cache::{ChannelCache, DEFAULT_CLEANUP_INTERVAL};
use super::error::GwResult;
use super::source::GatewayChannelSource;

/// Configuration for [`PvaGateway::start`]. All fields have sensible
/// defaults that mirror pvxs gateway behaviour; override only what
/// you need.
pub struct PvaGatewayConfig {
    /// Upstream PvaClient to use. When `None`, the gateway builds one
    /// with `PvaClient::builder().build()` so it picks up the
    /// `EPICS_PVA_*` environment defaults.
    pub upstream_client: Option<Arc<PvaClient>>,
    /// Downstream server bind config. Use [`PvaServerConfig::isolated`]
    /// for tests that should not pollute the real network.
    pub server_config: PvaServerConfig,
    /// How often the cache prunes idle entries. Pass
    /// [`DEFAULT_CLEANUP_INTERVAL`] (30 s) to match pvxs.
    pub cleanup_interval: Duration,
    /// Per-PV connect timeout: the maximum time `has_pv` /
    /// `get_value` / `subscribe` wait for the upstream IOC to deliver
    /// a first monitor event. Default 5 s.
    pub connect_timeout: Duration,
}

impl Default for PvaGatewayConfig {
    fn default() -> Self {
        Self {
            upstream_client: None,
            server_config: PvaServerConfig::default(),
            cleanup_interval: DEFAULT_CLEANUP_INTERVAL,
            connect_timeout: Duration::from_secs(5),
        }
    }
}

/// Running PVA gateway. Hold this for the lifetime of the gateway
/// process; consume it via [`Self::run`] for daemons or drop to
/// tear everything down.
pub struct PvaGateway {
    cache: Arc<ChannelCache>,
    server: PvaServer,
    /// Cloned `ChannelSource` retained so callers can attach the same
    /// gateway resolution to a second server (rare, but pvxs supports
    /// it for dual-protocol setups).
    source: GatewayChannelSource,
}

impl PvaGateway {
    /// Start a gateway. The downstream server begins accepting on the
    /// configured port; upstream channels are opened lazily on the
    /// first downstream search for each PV.
    pub fn start(config: PvaGatewayConfig) -> GwResult<Self> {
        let client = config
            .upstream_client
            .unwrap_or_else(|| Arc::new(PvaClient::builder().build()));
        let cache = ChannelCache::new(client, config.cleanup_interval);
        let mut source = GatewayChannelSource::new(cache.clone());
        source.connect_timeout = config.connect_timeout;
        let server = PvaServer::start(Arc::new(source.clone()), config.server_config);
        Ok(Self {
            cache,
            server,
            source,
        })
    }

    /// Convenience: loopback-only gateway with auto-picked free
    /// ports. Mirrors `PvaServer::isolated` semantics — useful for
    /// in-process tests where the gateway should not interact with
    /// the real network.
    pub fn isolated(client: Arc<PvaClient>) -> Self {
        let cache = ChannelCache::new(client, DEFAULT_CLEANUP_INTERVAL);
        let source = GatewayChannelSource::new(cache.clone());
        let server = PvaServer::isolated(Arc::new(source.clone()));
        Self {
            cache,
            server,
            source,
        }
    }

    /// Cache handle for diagnostics / iocsh `gwstats`.
    pub fn cache(&self) -> &Arc<ChannelCache> {
        &self.cache
    }

    /// `ChannelSource` clone — useful when you want to attach the
    /// gateway's PV resolution to a separate server (e.g. a
    /// dual-protocol setup).
    pub fn source(&self) -> GatewayChannelSource {
        self.source.clone()
    }

    /// Snapshot of server health: bound ports, alive flags, etc.
    pub fn report(&self) -> ServerReport {
        self.server.report()
    }

    /// Programmatic interrupt — trips `run()` from another task /
    /// thread. Mirrors pvxs `Server::interrupt`.
    pub fn interrupt(&self) {
        self.server.interrupt();
    }

    /// Build a `PvaClient` pre-pointed at the gateway's downstream
    /// listener. Useful for in-process tests where the gateway should
    /// be tested against a known address without UDP discovery.
    /// Mirrors pvxs `Server::clientConfig`.
    pub fn client_config(&self) -> PvaClient {
        self.server.client_config()
    }

    /// Block until SIGINT / SIGTERM, [`Self::interrupt`], or a
    /// subsystem task fails. Mirrors `PvaServer::run`.
    pub async fn run(self) -> PvaResult<()> {
        self.server.run().await
    }

    /// Stop accepting new connections. Existing in-flight ops finish
    /// on their own. Mirrors `PvaServer::stop`.
    pub fn stop(&self) {
        self.server.stop();
    }

    /// Convenience: pre-warm the cache by opening upstream channels
    /// for the listed PV names. Useful in tests that want
    /// determinism, or in production for a "warm-start" sweep.
    pub async fn prefetch(&self, pv_names: &[&str]) {
        for name in pv_names {
            let _ = self.cache.lookup(name, self.source.connect_timeout).await;
        }
    }
}
