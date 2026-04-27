//! Top-level [`PvaServer`] runtime: spawns UDP responder + TCP listener.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use crate::error::PvaResult;

use super::source::{ChannelSource, ChannelSourceObj, DynSource};
use super::tcp::run_tcp_server;
use super::udp::{random_guid, run_udp_responder};

/// Runtime configuration for [`run_pva_server`].
#[derive(Debug, Clone)]
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
        }
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

    let udp_handle = tokio::spawn(run_udp_responder(
        dyn_source.clone(),
        config.udp_port,
        config.tcp_port,
        guid,
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
