//! Top-level [`PvaServer`] runtime: spawns UDP responder + TCP listener.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::error::PvaResult;

use super::source::{ChannelSource, ChannelSourceObj, DynSource};
use super::tcp::run_tcp_server;
use super::udp::{random_guid, run_udp_responder_with_config};

/// Runtime configuration for [`run_pva_server`].
#[derive(Clone)]
pub struct PvaServerConfig {
    pub tcp_port: u16,
    pub udp_port: u16,
    /// Per-frame read timeout. The server *also* applies the heartbeat-
    /// based idle timeout below — `op_timeout` is just the upper bound on
    /// any single read.
    pub op_timeout: Duration,
    /// Bind address for the TCP listener (default `0.0.0.0`).
    pub bind_ip: Ipv4Addr,
    /// Maximum number of concurrent client connections. Excess incoming
    /// connections are accepted then immediately closed.
    pub max_connections: usize,
    /// Maximum number of channels per single client connection.
    pub max_channels_per_connection: usize,
    /// Idle timeout — server closes connections that haven't received
    /// anything in this window. Applied even if `op_timeout` is longer.
    pub idle_timeout: Duration,
    /// Per-monitor outbound queue depth. When exceeded, the back-pressure
    /// policy kicks in (squash to last value).
    pub monitor_queue_depth: usize,
    /// Optional TLS server config. When `Some`, every accepted TCP
    /// connection is upgraded to TLS via `tokio_rustls::TlsAcceptor`
    /// before the PVA handshake begins.
    pub tls: Option<std::sync::Arc<crate::auth::TlsServerConfig>>,
    /// Wire byte order the server sends in its SET_BYTE_ORDER control
    /// message. Clients adopt whatever the server picks. pvxs's
    /// `Config::overrideSendBE` exposes the same knob; defaults to LE.
    pub wire_byte_order: crate::proto::ByteOrder,
    /// Beacon emit period in seconds. pvxs default 15s. Override via
    /// `EPICS_PVAS_BEACON_PERIOD`.
    pub beacon_period: Duration,
    /// Explicit beacon destinations. When empty (and `auto_beacon` is
    /// true), emit per-NIC limited broadcast. From
    /// `EPICS_PVAS_BEACON_ADDR_LIST`.
    pub beacon_destinations: Vec<std::net::SocketAddr>,
    /// Auto-discover per-NIC broadcast addresses for beacons. From
    /// `EPICS_PVAS_AUTO_BEACON_ADDR_LIST` (default true).
    pub auto_beacon: bool,
    /// Interfaces to bind UDP responder on. When empty, bind 0.0.0.0.
    /// From `EPICS_PVAS_INTF_ADDR_LIST`.
    pub interfaces: Vec<std::net::IpAddr>,
    /// Emit `0xFD` / `0xFE` type-cache markers in INIT and RPC responses
    /// so repeated compound descriptors collapse to a 3-byte reference
    /// (saves 100-500 bytes per repeat for NTScalar / NTTable channels).
    /// pvxs and pvAccessJava both understand the markers; pvAccessCPP
    /// (EPICS Base 7.x) does NOT — leave this off when interop with old
    /// `pvmonitor` / `pvget` is required. Default: `false` for maximum
    /// compatibility.
    pub emit_type_cache: bool,
    /// Outbound queue depth (number of pending PVA frames) per
    /// connection. The dedicated writer task drains this; producers
    /// `await` when the queue is full, propagating backpressure to the
    /// monitor subscribers / read loop instead of letting memory grow
    /// unbounded for slow clients. Default: 1024.
    pub write_queue_depth: usize,
    /// Inbound peer ACL. Each entry is `(IpAddr, port_or_zero)` —
    /// matching connections (TCP) and search packets (UDP) are silently
    /// dropped. `port == 0` matches any port from that IP. Mirrors
    /// pvxs `Config::ignoreAddrs`. Empty = allow all (default).
    pub ignore_addrs: Vec<(std::net::IpAddr, u16)>,
    /// Per-monitor "high" watermark — emit a `tracing::warn!` when an
    /// outbound monitor queue grows past this many items. Default:
    /// `monitor_queue_depth * 3 / 4`. Mirrors pvxs
    /// `MonitorControlOp::setWatermarks` `high` argument; high-mark
    /// callbacks (`onHighMark`) aren't surfaced yet — the watermark
    /// drives diagnostics only.
    pub monitor_high_watermark: usize,
    /// Per-monitor "low" watermark — companion to `high`, currently
    /// unused (pvxs notes the `onLowMark` callback isn't fully
    /// implemented either). Reserved for future flow-control logic.
    pub monitor_low_watermark: usize,
}

impl PvaServerConfig {
    /// True when `peer` matches an entry in `ignore_addrs`. Port 0 in
    /// the entry is a wildcard. O(n) over the list; n is expected to
    /// be small (single-digit) in practice.
    pub fn is_ignored_peer(&self, peer: std::net::SocketAddr) -> bool {
        for (ip, port) in &self.ignore_addrs {
            if peer.ip() == *ip && (*port == 0 || peer.port() == *port) {
                return true;
            }
        }
        false
    }
}

impl Default for PvaServerConfig {
    fn default() -> Self {
        Self {
            tcp_port: 5075,
            udp_port: 5076,
            op_timeout: Duration::from_secs(64_000),
            bind_ip: Ipv4Addr::UNSPECIFIED,
            max_connections: 1024,
            max_channels_per_connection: 1024,
            idle_timeout: Duration::from_secs(45),
            monitor_queue_depth: 64,
            tls: None,
            wire_byte_order: crate::proto::ByteOrder::Little,
            beacon_period: Duration::from_secs(15),
            beacon_destinations: Vec::new(),
            auto_beacon: true,
            interfaces: Vec::new(),
            emit_type_cache: false,
            write_queue_depth: 1024,
            ignore_addrs: Vec::new(),
            monitor_high_watermark: 48, // 64 * 3 / 4 default
            monitor_low_watermark: 0,
        }
    }
}

impl PvaServerConfig {
    /// Loopback-only configuration with random ports — pvxs
    /// `Config::isolated()` (config.cpp:445). The OS picks free TCP
    /// and UDP ports; auto-beacon is disabled so the server doesn't
    /// leak datagrams onto the LAN. Matching client side: see
    /// [`crate::client_native::context::PvaClient::isolated_for`].
    pub fn isolated() -> Self {
        Self {
            tcp_port: 0,
            udp_port: 0,
            bind_ip: Ipv4Addr::LOCALHOST,
            auto_beacon: false,
            beacon_destinations: Vec::new(),
            ..Default::default()
        }
    }

    /// Apply standard EPICS_PVAS_* / EPICS_PVA_* env vars on top of an
    /// existing config. Only fields backed by the recognised vars are
    /// touched — others stay at their existing values.
    pub fn with_env(mut self) -> Self {
        use crate::config::env;
        self.tcp_port = env::server_port();
        self.udp_port = env::server_broadcast_port();
        self.beacon_period = Duration::from_secs(env::beacon_period_secs());
        self.beacon_destinations = env::server_beacon_addr_list();
        self.auto_beacon = env::auto_beacon_addr_list_enabled();
        self.interfaces = env::server_intf_addr_list();
        self
    }
}

/// Run a native PVA server forever.
///
/// The server spawns:
///
/// - UDP search responder on `config.udp_port` (also emits beacons every
///   15 s)
/// - TCP listener on `config.tcp_port` (handles connections concurrently)
pub async fn run_pva_server<S>(source: Arc<S>, config: PvaServerConfig) -> PvaResult<()>
where
    S: ChannelSource + 'static,
{
    let server = PvaServer::start(source, config);
    server.wait().await
}

/// Handle to a running PVA server returned by [`PvaServer::start`].
///
/// Holds the JoinHandles for the UDP responder and TCP listener tasks
/// plus a shutdown channel. Use [`PvaServer::stop`] for graceful
/// shutdown — accept loop exits immediately so no new connections, and
/// existing per-client handler tasks unwind on their next read/write
/// (TCP keepalive plus the read-loop's `op_timeout` bound the stragglers).
/// [`PvaServer::wait`] blocks until both tasks have observed the
/// shutdown and returned.
pub struct PvaServer {
    udp_handle: tokio::task::JoinHandle<PvaResult<()>>,
    tcp_handle: tokio::task::JoinHandle<PvaResult<()>>,
    /// Effective config the server is running under. Captured at
    /// `start()` so [`Self::client_config`] can hand back a builder
    /// pre-pointed at the actual bound TCP port without re-reading env
    /// vars (which may have changed since startup).
    effective_config: PvaServerConfig,
    /// Bound TCP socket address — useful when the configured port was
    /// 0 and the OS picked one.  We capture the configured value here;
    /// callers needing the post-bind port should query the listener
    /// directly (future work).
    bound_tcp_port: u16,
    /// Programmatic interrupt for [`Self::run`]. Not used by `wait()`.
    interrupt: Arc<tokio::sync::Notify>,
}

impl PvaServer {
    /// Convenience factory: a loopback-only server with auto-picked
    /// free ports. Mirrors pvxs `Config::isolated().build()`. Useful
    /// for self-contained tests where a UDP-discoverable production
    /// config would interfere with concurrent runs.
    pub fn isolated<S>(source: Arc<S>) -> Self
    where
        S: ChannelSource + 'static,
    {
        // Pre-bind ephemeral ports so the listeners come up on known
        // ports we can hand to clients via `client_config()`. Without
        // this we'd have to plumb the real port out of the listener
        // task after start() returns.
        let pick_port = || {
            let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .expect("isolated tcp port");
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        let pick_udp = || {
            let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
                .expect("isolated udp port");
            let p = l.local_addr().unwrap().port();
            drop(l);
            p
        };
        let cfg = PvaServerConfig {
            tcp_port: pick_port(),
            udp_port: pick_udp(),
            ..PvaServerConfig::isolated()
        };
        Self::start(source, cfg)
    }

    /// Spawn the UDP responder and TCP listener; return a handle.
    pub fn start<S>(source: Arc<S>, config: PvaServerConfig) -> Self
    where
        S: ChannelSource + 'static,
    {
        let dyn_source: DynSource = source as Arc<dyn ChannelSourceObj>;
        let guid = random_guid();
        let bind_addr = SocketAddr::new(std::net::IpAddr::V4(config.bind_ip), config.tcp_port);

        let protocol: &'static str = if config.tls.is_some() { "tls" } else { "tcp" };
        let udp_handle = tokio::spawn(run_udp_responder_with_config(
            dyn_source.clone(),
            config.udp_port,
            config.tcp_port,
            guid,
            protocol,
            config.beacon_period,
            config.beacon_destinations.clone(),
            config.auto_beacon,
            config.ignore_addrs.clone(),
        ));
        let tcp_handle = tokio::spawn(run_tcp_server(dyn_source, bind_addr, config.clone()));
        let bound_tcp_port = config.tcp_port;

        Self {
            udp_handle,
            tcp_handle,
            effective_config: config,
            bound_tcp_port,
            interrupt: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Build a [`crate::client_native::context::PvaClient`] pointed at
    /// this server on loopback. Mirrors pvxs `Server::clientConfig` —
    /// useful for self-contained tests where you want a client that
    /// talks to the in-process server without UDP discovery.
    pub fn client_config(&self) -> crate::client_native::context::PvaClient {
        let addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            self.bound_tcp_port,
        );
        crate::client_native::context::PvaClient::builder()
            .server_addr(addr)
            .build()
    }

    /// Effective config snapshot. pvxs `Server::config` parity.
    pub fn config(&self) -> &PvaServerConfig {
        &self.effective_config
    }

    /// Block on this server until it stops, SIGINT/SIGTERM is received,
    /// or [`Self::interrupt`] is called. pvxs `Server::run` for CLI
    /// daemons. Returns Ok on graceful shutdown, Err if a subsystem
    /// task panicked or exited abnormally.
    pub async fn run(self) -> PvaResult<()> {
        let interrupt = self.interrupt.clone();
        let ctrl_c = async {
            let _ = tokio::signal::ctrl_c().await;
        };
        tokio::select! {
            _ = ctrl_c => Ok(()),
            _ = interrupt.notified() => Ok(()),
            r = self.udp_handle => match r {
                Ok(res) => res,
                Err(e) if e.is_cancelled() => Ok(()),
                Err(e) => Err(crate::error::PvaError::Protocol(format!("udp task panic: {e}"))),
            },
            r = self.tcp_handle => match r {
                Ok(res) => res,
                Err(e) if e.is_cancelled() => Ok(()),
                Err(e) => Err(crate::error::PvaError::Protocol(format!("tcp task panic: {e}"))),
            },
        }
    }

    /// Trip [`Self::run`] from another task. Mirrors pvxs
    /// `Server::interrupt`.
    pub fn interrupt(&self) {
        self.interrupt.notify_waiters();
    }

    /// Snapshot summary-level diagnostics. pvxs `Server::report`
    /// counterpart at the "is the server up, how is it configured"
    /// level. Per-peer / per-channel counters require book-keeping the
    /// TCP loop doesn't yet maintain; surface what we have today.
    pub fn report(&self) -> ServerReport {
        ServerReport {
            tcp_port: self.bound_tcp_port,
            udp_port: self.effective_config.udp_port,
            tls_enabled: self.effective_config.tls.is_some(),
            ignore_addrs: self.effective_config.ignore_addrs.len(),
            beacon_period_secs: self.effective_config.beacon_period.as_secs(),
            udp_alive: !self.udp_handle.is_finished(),
            tcp_alive: !self.tcp_handle.is_finished(),
        }
    }

    /// Stop accepting new connections. Aborts both background tasks;
    /// per-client tasks already spawned continue independently and
    /// unwind on their next failed I/O. Mirrors pvxs `Server::stop`
    /// (server.cpp:616) at the "no new connections" granularity. For
    /// hard-stop semantics drop the entire `PvaServer` instead.
    pub fn stop(&self) {
        self.tcp_handle.abort();
        self.udp_handle.abort();
    }

    /// Block until either task returns. Either subsystem exiting is
    /// treated as fatal — an Err here means the server is no longer
    /// serving even if `stop()` wasn't called.
    pub async fn wait(self) -> PvaResult<()> {
        tokio::select! {
            r = self.udp_handle => match r {
                Ok(res) => res,
                Err(e) if e.is_cancelled() => Ok(()),
                Err(e) => Err(crate::error::PvaError::Protocol(format!("udp task panic: {e}"))),
            },
            r = self.tcp_handle => match r {
                Ok(res) => res,
                Err(e) if e.is_cancelled() => Ok(()),
                Err(e) => Err(crate::error::PvaError::Protocol(format!("tcp task panic: {e}"))),
            },
        }
    }
}

/// Snapshot returned by [`PvaServer::report`].
#[derive(Debug, Clone)]
pub struct ServerReport {
    pub tcp_port: u16,
    pub udp_port: u16,
    pub tls_enabled: bool,
    pub ignore_addrs: usize,
    pub beacon_period_secs: u64,
    pub udp_alive: bool,
    pub tcp_alive: bool,
}
