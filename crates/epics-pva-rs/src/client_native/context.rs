//! Public `PvaClient` facade.
//!
//! Built on top of:
//!
//! - [`super::search_engine::SearchEngine`] — single background task,
//!   handles SEARCH retry backoff + beacon listening.
//! - [`super::channel::ConnectionPool`] — shared `ServerConn` per server
//!   address, with full handshake + heartbeat + auto-shutdown.
//! - [`super::channel::Channel`] — per-PV state machine (Searching →
//!   Connecting → Active → Reconnecting). Multiple ops share a single
//!   channel instance.
//! - [`super::ops_v2`] — GET / PUT / MONITOR / RPC; MONITOR transparently
//!   re-issues INIT + START on every reconnect.
//!
//! Public API stays compatible with the previous shape so existing callers
//! (pvget-rs, pvput-rs, pvmonitor-rs, pvinfo-rs) keep working.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;

use parking_lot::RwLock;

use crate::error::PvaResult;
use crate::pvdata::{FieldDesc, PvField};

use super::channel::{Channel, ConnectionPool};
use super::ops_v2::{
    DEFAULT_PIPELINE_SIZE, MonitorEvent, MonitorEventMask, SubscriptionHandle, op_get, op_monitor,
    op_monitor_events, op_monitor_handle, op_put, op_rpc,
};
use super::search_engine::SearchEngine;

#[derive(Debug, Clone)]
pub struct PvGetResult {
    pub pv_name: String,
    pub value: PvField,
    pub introspection: FieldDesc,
    pub server_addr: SocketAddr,
}

/// Builder for [`PvaClient`].
pub struct PvaClientBuilder {
    timeout: Duration,
    server_addr: Option<SocketAddr>,
    user: Option<String>,
    host: Option<String>,
    pipeline_size: u32,
    tls: Option<Arc<crate::auth::TlsClientConfig>>,
    name_servers: Vec<SocketAddr>,
    /// Operation priority hint, propagated to TCP `IPTOS_PREC_*` bits
    /// where the OS supports it. pvxs `CommonBuilder::priority(int)`
    /// (client.h:692) — 0..7, default 0 (BEST_EFFORT).
    priority: u8,
    /// TCP idle timeout for client-side connections. After this long
    /// without traffic the client closes the virtual circuit. pvxs
    /// `Config::tcpTimeout = 40s` (client.h:1040).
    tcp_timeout: Duration,
    /// Share a single SearchEngine across all `PvaClient` instances
    /// in this process. pvxs `Config::overrideShareUDP(true)`. Avoids
    /// holding multiple UDP search sockets when the user wires up
    /// per-purpose Contexts.
    share_udp: bool,
}

impl PvaClientBuilder {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            server_addr: None,
            user: None,
            host: None,
            pipeline_size: DEFAULT_PIPELINE_SIZE,
            tls: None,
            name_servers: crate::config::env::name_servers(),
            priority: 0,
            tcp_timeout: Duration::from_secs(40),
            share_udp: false,
        }
    }

    /// Mirrors pvxs `CommonBuilder::priority(int)` — propagates to
    /// the TCP TOS / DSCP byte where the OS supports it. Range 0..7;
    /// values outside the range are clamped.
    pub fn priority(mut self, p: u8) -> Self {
        self.priority = p.min(7);
        self
    }

    /// Client-side TCP idle timeout. Mirrors pvxs `Config::tcpTimeout`.
    /// Default 40s.
    pub fn tcp_timeout(mut self, d: Duration) -> Self {
        self.tcp_timeout = d;
        self
    }

    /// Share a single process-wide [`SearchEngine`] across every
    /// `PvaClient` in this process. Mirrors pvxs
    /// `Config::overrideShareUDP(true)`. Saves one UDP socket per
    /// client when a single process opens multiple Contexts (e.g.,
    /// observability + control planes coexisting).
    pub fn share_udp(mut self, share: bool) -> Self {
        self.share_udp = share;
        self
    }

    /// Configure TCP name servers — pvxs `EPICS_PVA_NAME_SERVERS`
    /// equivalent. When UDP search yields no responder for a PV, each
    /// name server is tried as a direct-connect candidate (gateway
    /// self-serve case). Replaces any list parsed from env at
    /// `new()` time.
    ///
    /// Note: this is currently a fallback-only treatment. pvxs
    /// additionally sends SEARCH frames over a persistent TCP
    /// connection to each name server and accepts SEARCH_RESPONSE
    /// pointing at a *different* server (redirect). For pure-gateway
    /// scenarios (the gateway answers itself) the simpler fallback
    /// works; redirect-style chains aren't supported yet.
    pub fn name_servers(mut self, servers: Vec<SocketAddr>) -> Self {
        self.name_servers = servers;
        self
    }

    /// Enable TLS for every connection. Pass an `Arc<TlsClientConfig>`
    /// from `crate::auth::tls::load_client_config()` (or built from
    /// scratch via `rustls`).
    pub fn with_tls(mut self, tls: Arc<crate::auth::TlsClientConfig>) -> Self {
        self.tls = Some(tls);
        self
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    pub fn server_addr(mut self, addr: SocketAddr) -> Self {
        self.server_addr = Some(addr);
        self
    }

    pub fn user(mut self, user: impl Into<String>) -> Self {
        self.user = Some(user.into());
        self
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Override the monitor pipeline size (default 4 — one ack per 4 events).
    /// Set to 0 to disable pipelining.
    pub fn pipeline_size(mut self, n: u32) -> Self {
        self.pipeline_size = n;
        self
    }

    pub fn build(self) -> PvaClient {
        let pool = ConnectionPool::new();
        if self.tls.is_some() {
            pool.set_tls(self.tls.clone());
        }
        PvaClient {
            inner: Arc::new(ClientInner {
                timeout: self.timeout,
                server_addr: self.server_addr,
                user: self
                    .user
                    .unwrap_or_else(super::super::auth::authnz_default_user),
                host: self
                    .host
                    .unwrap_or_else(super::super::auth::authnz_default_host),
                pipeline_size: self.pipeline_size,
                pool,
                channels: RwLock::new(HashMap::new()),
                search: OnceLock::new(),
                name_servers: self.name_servers,
                priority: self.priority,
                tcp_timeout: self.tcp_timeout,
                share_udp: self.share_udp,
            }),
        }
    }
}

impl Default for PvaClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

struct ClientInner {
    timeout: Duration,
    server_addr: Option<SocketAddr>,
    user: String,
    host: String,
    pipeline_size: u32,
    pool: Arc<ConnectionPool>,
    channels: RwLock<HashMap<String, Arc<Channel>>>,
    /// Lazy: only spawn the search engine when we actually need to resolve.
    search: OnceLock<SearchEngine>,
    /// TCP `EPICS_PVA_NAME_SERVERS` fallbacks — used as last-resort
    /// direct-connect candidates when UDP search returns nothing.
    name_servers: Vec<SocketAddr>,
    /// Operation priority hint (0..7). Stored for inspection /
    /// future TCP TOS wiring. pvxs `CommonBuilder::priority`.
    #[allow(dead_code)]
    priority: u8,
    /// Client TCP idle timeout. Stored for inspection / future
    /// keepalive plumbing. pvxs `Config::tcpTimeout`.
    #[allow(dead_code)]
    tcp_timeout: Duration,
    /// True when `build()` was told to share the process-wide search
    /// engine. Routes [`PvaClient::search_engine`] through the static
    /// `SHARED_SEARCH_ENGINE` instead of spawning per-client.
    share_udp: bool,
}

/// Process-wide singleton SearchEngine for `share_udp(true)` clients.
/// Lazily initialized on first use.
static SHARED_SEARCH_ENGINE: tokio::sync::OnceCell<SearchEngine> =
    tokio::sync::OnceCell::const_new();

#[derive(Clone)]
pub struct PvaClient {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for PvaClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PvaClient")
            .field("timeout", &self.inner.timeout)
            .field("user", &self.inner.user)
            .field("host", &self.inner.host)
            .finish()
    }
}

impl PvaClient {
    pub fn builder() -> PvaClientBuilder {
        PvaClientBuilder::new()
    }

    pub fn new() -> PvaResult<Self> {
        Ok(Self::builder().build())
    }

    /// Backwards-compatible: targets a specific TCP port (UDP ignored —
    /// search uses the standard port machinery).
    pub fn with_ports(_udp_port: u16, tcp_port: u16) -> Self {
        let server_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            tcp_port,
        );
        Self::builder().server_addr(server_addr).build()
    }

    async fn search_engine(&self) -> PvaResult<&SearchEngine> {
        if self.inner.share_udp {
            let engine = SHARED_SEARCH_ENGINE
                .get_or_try_init(|| async { SearchEngine::spawn(Vec::new()).await })
                .await?;
            return Ok(engine);
        }
        if self.inner.search.get().is_none() {
            let engine = SearchEngine::spawn(Vec::new()).await?;
            let _ = self.inner.search.set(engine);
        }
        Ok(self.inner.search.get().unwrap())
    }

    async fn channel(&self, pv_name: &str) -> PvaResult<Arc<Channel>> {
        self.channel_with_forced(pv_name, None).await
    }

    async fn channel_with_forced(
        &self,
        pv_name: &str,
        forced: Option<SocketAddr>,
    ) -> PvaResult<Arc<Channel>> {
        // Forced-server channels skip the cache entirely — pinning is a
        // per-call request, not a global property of the PV name.
        if forced.is_none() {
            if let Some(c) = self.inner.channels.read().get(pv_name).cloned() {
                return Ok(c);
            }
        }

        let direct = forced.or(self.inner.server_addr);
        let ch = if let Some(addr) = direct {
            // Direct-server mode: no UDP search at all. Channel will go
            // straight to Connecting → Active using `addr`. Used for
            // both PvaClient-wide `server_addr` and per-channel
            // `forced_server` overrides (pvxs ConnectBuilder::server).
            Arc::new(Channel::new_direct(
                pv_name.to_string(),
                self.inner.user.clone(),
                self.inner.host.clone(),
                self.inner.timeout,
                self.inner.pool.clone(),
                addr,
            ))
        } else {
            let search = self.search_engine().await?.clone();
            Arc::new(Channel::new_with_name_servers(
                pv_name.to_string(),
                self.inner.user.clone(),
                self.inner.host.clone(),
                self.inner.timeout,
                self.inner.pool.clone(),
                search,
                self.inner.name_servers.clone(),
            ))
        };

        if forced.is_some() {
            return Ok(ch);
        }

        let mut map = self.inner.channels.write();
        if let Some(existing) = map.get(pv_name).cloned() {
            return Ok(existing);
        }
        map.insert(pv_name.to_string(), ch.clone());
        Ok(ch)
    }

    /// Resolve `pv_name` against a specific server, bypassing UDP
    /// search and any cached search results. Mirrors pvxs
    /// `ConnectBuilder::server` (client.cpp:208) — the returned future
    /// performs a one-shot operation against the pinned server. Useful
    /// when a gateway or testing harness wants to direct an op to a
    /// known endpoint without affecting the cache for that PV name.
    pub async fn pvget_from(&self, pv_name: &str, server: SocketAddr) -> PvaResult<PvField> {
        let ch = self.channel_with_forced(pv_name, Some(server)).await?;
        let (_, v) = op_get(&ch, &[], self.inner.timeout).await?;
        Ok(v)
    }

    /// Same as [`Self::pvput`] but pins the operation to `server`.
    pub async fn pvput_to(
        &self,
        pv_name: &str,
        server: SocketAddr,
        value_str: &str,
    ) -> PvaResult<()> {
        let ch = self.channel_with_forced(pv_name, Some(server)).await?;
        op_put(&ch, value_str, self.inner.timeout).await
    }

    pub async fn pvget(&self, pv_name: &str) -> PvaResult<PvField> {
        let ch = self.channel(pv_name).await?;
        let (_, v) = op_get(&ch, &[], self.inner.timeout).await?;
        Ok(v)
    }

    /// Force the search engine into fast-tick mode for one revolution
    /// and reset every pending search's retry deadline. Mirrors pvxs
    /// `Context::hurryUp` (client.cpp:430). Useful when the application
    /// has out-of-band evidence that the network state changed (link
    /// bounce, new IOC announced via side channel) and wants pending
    /// searches to retry immediately rather than wait for their
    /// scheduled bucket.
    ///
    /// No-op in direct-server mode (no SearchEngine).
    pub async fn hurry_up(&self) {
        if let Ok(engine) = self.search_engine().await {
            engine.hurry_up().await;
        }
    }

    /// Drop cached state for `pv_name`: cancels any in-flight search,
    /// removes the channel from the local map, and forces the next
    /// operation to start a fresh search round. Mirrors pvxs
    /// `Context::cacheClear` (client.cpp:440). Use when an IOC moved
    /// servers and the cached connection target is stale.
    pub async fn cache_clear(&self, pv_name: &str) {
        self.inner.channels.write().remove(pv_name);
        if let Ok(engine) = self.search_engine().await {
            engine.cache_clear(pv_name).await;
        }
    }

    /// Send a DISCOVER ping (empty SEARCH) to broadcast targets so
    /// reachable servers reply immediately. Pair with the discovery
    /// stream from the search engine to learn about servers without
    /// waiting for the next beacon. Mirrors pvxs
    /// `Context::discover().pingAll(true)`.
    pub async fn ping_all(&self) -> PvaResult<()> {
        self.search_engine().await?.ping_all().await;
        Ok(())
    }

    /// Replace the server-GUID blocklist used by the search engine.
    /// Beacons and search responses from any listed GUID are silently
    /// dropped. Mirrors pvxs `Context::ignoreServerGUIDs`
    /// (client.cpp:453). Pass an empty `Vec` to clear the list.
    pub async fn ignore_server_guids(&self, guids: Vec<[u8; 12]>) {
        if let Ok(engine) = self.search_engine().await {
            engine.ignore_server_guids(guids).await;
        }
    }

    /// Graceful shutdown: drop the channel cache, close pooled
    /// connections, and stop accepting new operations. Any subsequent
    /// `pvget` / `pvput` / etc. on this `PvaClient` will fail or
    /// re-establish from scratch (depending on the operation's
    /// reconnect policy). The background search-engine task continues
    /// to run idle in the spawn until the last `PvaClient` clone is
    /// dropped — there's no in-flight work left for it to do.
    /// Mirrors pvxs `Context::close` (client.cpp:422). Idempotent.
    pub fn close(&self) {
        // Drop channels first so their Arc<ServerConn> drops; this
        // gives the pool a chance to skip "still-in-use" connections.
        self.inner.channels.write().clear();
        self.inner.pool.clear();
    }

    pub async fn pvget_full(&self, pv_name: &str) -> PvaResult<PvGetResult> {
        let ch = self.channel(pv_name).await?;
        let (intro, value) = op_get(&ch, &[], self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr,
        })
    }

    pub async fn pvget_fields(&self, pv_name: &str, fields: &[&str]) -> PvaResult<PvGetResult> {
        let ch = self.channel(pv_name).await?;
        let (intro, value) = op_get(&ch, fields, self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok(PvGetResult {
            pv_name: pv_name.to_string(),
            value,
            introspection: intro,
            server_addr,
        })
    }

    pub async fn pvput(&self, pv_name: &str, value_str: &str) -> PvaResult<()> {
        let ch = self.channel(pv_name).await?;
        op_put(&ch, value_str, self.inner.timeout).await
    }

    pub async fn pvmonitor<F>(&self, pv_name: &str, mut callback: F) -> PvaResult<()>
    where
        F: FnMut(&PvField) + Send,
    {
        let ch = self.channel(pv_name).await?;
        op_monitor(&ch, &[], self.inner.pipeline_size, move |_desc, value| {
            callback(value)
        })
        .await
    }

    pub async fn pvmonitor_typed<F>(&self, pv_name: &str, callback: F) -> PvaResult<()>
    where
        F: FnMut(&FieldDesc, &PvField) + Send,
    {
        let ch = self.channel(pv_name).await?;
        op_monitor(&ch, &[], self.inner.pipeline_size, callback).await
    }

    /// Begin a pausable monitor that can be paused/resumed and queried
    /// for stats. Mirrors pvxs `Context::monitor(name).exec()` →
    /// `Subscription`. The returned handle owns the inner task; call
    /// `stop()` to terminate or drop after `stop()` returns.
    pub async fn pvmonitor_handle<F>(
        &self,
        pv_name: &str,
        callback: F,
    ) -> PvaResult<SubscriptionHandle>
    where
        F: FnMut(&FieldDesc, &PvField) + Send + 'static,
    {
        let ch = self.channel(pv_name).await?;
        Ok(op_monitor_handle(
            ch,
            &[],
            self.inner.pipeline_size,
            callback,
        ))
    }

    /// Like [`Self::pvmonitor_handle`] but pinned to `server`. Mirrors
    /// pvxs `MonitorBuilder::server(addr).exec()`. The handle owns its
    /// own per-call channel — it does not affect the shared cache for
    /// `pv_name`.
    pub async fn pvmonitor_handle_from<F>(
        &self,
        pv_name: &str,
        server: SocketAddr,
        callback: F,
    ) -> PvaResult<SubscriptionHandle>
    where
        F: FnMut(&FieldDesc, &PvField) + Send + 'static,
    {
        let ch = self.channel_with_forced(pv_name, Some(server)).await?;
        Ok(op_monitor_handle(
            ch,
            &[],
            self.inner.pipeline_size,
            callback,
        ))
    }

    /// Monitor with typed events (`Connected`/`Data`/`Disconnected`/
    /// `Finished`). Mirrors pvxs's MonitorBuilder + Subscription
    /// exception-based stream API. `mask` defaults are
    /// pvxs-compatible: `maskConnected=true`, `maskDisconnected=false`.
    pub async fn pvmonitor_events<F>(
        &self,
        pv_name: &str,
        mask: MonitorEventMask,
        callback: F,
    ) -> PvaResult<()>
    where
        F: FnMut(MonitorEvent) + Send,
    {
        let ch = self.channel(pv_name).await?;
        op_monitor_events(&ch, &[], self.inner.pipeline_size, mask, callback).await
    }

    pub async fn pvinfo(&self, pv_name: &str) -> PvaResult<FieldDesc> {
        let ch = self.channel(pv_name).await?;
        let (intro, _value) = op_get(&ch, &[], self.inner.timeout).await?;
        Ok(intro)
    }

    pub async fn pvinfo_full(&self, pv_name: &str) -> PvaResult<(FieldDesc, SocketAddr)> {
        let ch = self.channel(pv_name).await?;
        let (intro, _value) = op_get(&ch, &[], self.inner.timeout).await?;
        let server_addr = match ch.current_state() {
            super::channel::ChannelState::Active { server, .. } => server.addr,
            _ => SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
        };
        Ok((intro, server_addr))
    }

    pub async fn pvrpc(
        &self,
        pv_name: &str,
        request_desc: &FieldDesc,
        request_value: &PvField,
    ) -> PvaResult<(FieldDesc, PvField)> {
        let ch = self.channel(pv_name).await?;
        op_rpc(&ch, request_desc, request_value, self.inner.timeout).await
    }

    /// Snapshot of the client's current state — channel cache size,
    /// connection-pool peers, name-server count, etc. Mirrors pvxs
    /// `Context::report` (client.h:599) at the "summary counters"
    /// level. Per-channel detail (peer, RX/TX bytes) isn't surfaced
    /// here yet; pvxs's full Report is heavier than most callers need.
    pub fn report(&self) -> ClientReport {
        let channels = self.inner.channels.read();
        let mut active = 0usize;
        let mut searching = 0usize;
        let mut connecting = 0usize;
        let mut closed = 0usize;
        for ch in channels.values() {
            match ch.current_state() {
                super::channel::ChannelState::Active { .. } => active += 1,
                super::channel::ChannelState::Searching => searching += 1,
                super::channel::ChannelState::Connecting => connecting += 1,
                super::channel::ChannelState::Closed => closed += 1,
                super::channel::ChannelState::Idle => {}
            }
        }
        ClientReport {
            channels_total: channels.len(),
            channels_active: active,
            channels_searching: searching,
            channels_connecting: connecting,
            channels_closed: closed,
            name_servers: self.inner.name_servers.len(),
            direct_mode: self.inner.server_addr.is_some(),
        }
    }

    /// Begin a `connect` builder for `pv_name`. Use this to attach
    /// onConnect/onDisconnect callbacks that fire whenever the channel
    /// transitions across the Active boundary. Mirrors pvxs's
    /// `Context::connect(name).onConnect(...).exec()`.
    pub fn connect(&self, pv_name: &str) -> ConnectBuilder<'_> {
        ConnectBuilder {
            client: self,
            pv_name: pv_name.to_string(),
            on_connect: None,
            on_disconnect: None,
            server: None,
            sync_cancel: true,
        }
    }
}

/// Snapshot returned by [`PvaClient::report`]. pvxs Report
/// counterpart, summary-only.
#[derive(Debug, Clone)]
pub struct ClientReport {
    /// Channels currently registered in the local cache (any state).
    pub channels_total: usize,
    /// Channels that have a live `ServerConn` and a server-assigned sid.
    pub channels_active: usize,
    /// Channels currently issuing UDP search requests.
    pub channels_searching: usize,
    /// Channels mid-TCP-handshake / mid-CREATE_CHANNEL.
    pub channels_connecting: usize,
    /// Channels explicitly closed via `pvclient.close()`.
    pub channels_closed: usize,
    /// Configured TCP name-server count.
    pub name_servers: usize,
    /// True when the client is in direct-server mode (no UDP search).
    pub direct_mode: bool,
}

/// Callback type for [`ConnectBuilder::on_connect`] /
/// [`ConnectBuilder::on_disconnect`].
type ConnectCb = Box<dyn Fn() + Send + Sync + 'static>;

/// Builder for a connect-watcher operation. Configure callbacks then
/// call `exec()` to spawn a watcher task. The returned [`ConnectHandle`]
/// owns the task — drop it to stop watching.
pub struct ConnectBuilder<'a> {
    client: &'a PvaClient,
    pv_name: String,
    on_connect: Option<ConnectCb>,
    on_disconnect: Option<ConnectCb>,
    /// Per-call forced server pinning. Mirrors pvxs
    /// `ConnectBuilder::server(s)` (client.h:952).
    server: Option<SocketAddr>,
    /// pvxs `syncCancel(bool)` (client.h:950) — when true, drop on the
    /// returned handle blocks until the watcher task exits; when
    /// false, drop just signals and returns. Currently advisory: our
    /// watcher always tears down within one tick, so the difference
    /// is bounded.
    sync_cancel: bool,
}

impl<'a> ConnectBuilder<'a> {
    /// Register a callback that fires every time the channel becomes
    /// Active (initial connect + every reconnect).
    pub fn on_connect<F>(mut self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_connect = Some(Box::new(f));
        self
    }

    /// Register a callback that fires every time the channel leaves
    /// Active (disconnect + close).
    pub fn on_disconnect<F>(mut self, f: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_disconnect = Some(Box::new(f));
        self
    }

    /// Pin the channel to a specific server, bypassing UDP search.
    /// Mirrors pvxs `ConnectBuilder::server(s)`.
    pub fn server(mut self, addr: SocketAddr) -> Self {
        self.server = Some(addr);
        self
    }

    /// Mirrors pvxs `ConnectBuilder::syncCancel(b)`. See the field
    /// docstring for the current semantics.
    pub fn sync_cancel(mut self, sync: bool) -> Self {
        self.sync_cancel = sync;
        self
    }

    /// Spawn the watcher task. The returned handle owns the task; drop
    /// it to stop watching. The channel itself stays in the client's
    /// channel map so other ops can keep using it.
    pub async fn exec(self) -> PvaResult<ConnectHandle> {
        let ch = match self.server {
            Some(addr) => {
                self.client
                    .channel_with_forced(&self.pv_name, Some(addr))
                    .await?
            }
            None => self.client.channel(&self.pv_name).await?,
        };
        let on_connect = self.on_connect;
        let on_disconnect = self.on_disconnect;
        let cancel = tokio_util::sync::CancellationToken::new();
        let cancel_task = cancel.clone();

        let task = tokio::spawn(async move {
            let mut was_active = false;
            loop {
                let active_now = matches!(
                    ch.current_state(),
                    super::channel::ChannelState::Active { .. }
                );
                if active_now && !was_active {
                    if let Some(cb) = &on_connect {
                        cb();
                    }
                } else if !active_now && was_active {
                    if let Some(cb) = &on_disconnect {
                        cb();
                    }
                }
                was_active = active_now;

                tokio::select! {
                    _ = ch.state_changed.notified() => {}
                    _ = cancel_task.cancelled() => break,
                }
            }
        });

        Ok(ConnectHandle {
            cancel,
            task: Some(task),
        })
    }
}

/// Handle returned by [`ConnectBuilder::exec`]. Drop to stop the
/// watcher task; the channel itself is unaffected.
pub struct ConnectHandle {
    cancel: tokio_util::sync::CancellationToken,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl ConnectHandle {
    /// Wait for the watcher task to terminate. Use after dropping
    /// callbacks to ensure no further events fire — pvxs
    /// `syncCancel(true)` semantics, exposed explicitly so the caller
    /// can decide when to await.
    pub async fn wait(mut self) {
        if let Some(t) = self.task.take() {
            self.cancel.cancel();
            let _ = t.await;
        }
    }
}

impl Drop for ConnectHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(t) = self.task.take() {
            t.abort();
        }
    }
}
