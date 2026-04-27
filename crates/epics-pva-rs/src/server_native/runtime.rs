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
        }
    }
}

impl PvaServerConfig {
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
    let dyn_source: DynSource = source as Arc<dyn ChannelSourceObj>;
    let guid = random_guid();
    let bind_addr = SocketAddr::new(std::net::IpAddr::V4(config.bind_ip), config.tcp_port);

    // Advertise "tls" in SEARCH_RESPONSE when the server requires TLS, so
    // pvxs clients with TLS configured connect via `pvas://`.
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
    ));
    let tcp_handle = tokio::spawn(run_tcp_server(dyn_source, bind_addr, config.clone()));

    // Either subsystem failing is fatal.
    tokio::select! {
        r = udp_handle => match r {
            Ok(res) => res,
            Err(e) => Err(crate::error::PvaError::Protocol(format!("udp task panic: {e}"))),
        },
        r = tcp_handle => match r {
            Ok(res) => res,
            Err(e) => Err(crate::error::PvaError::Protocol(format!("tcp task panic: {e}"))),
        },
    }
}
