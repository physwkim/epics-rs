//! Per-client console connection.
//!
//! Mirrors C `clientItem` (`clientFactory.cc`). One instance per
//! TCP/UNIX socket. Owns:
//! - the socket halves (read/write)
//! - a [`super::telnet::TelnetParser`] for IAC handling
//! - a `readonly` flag (set at construction; read-only clients have
//!   their input silently dropped, matching `clientItem::readFromFd`
//!   line 192)
//! - the per-client mpsc the supervisor uses to push outgoing bytes

use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// A freshly accepted socket, handed from the listener to the
/// supervisor.
#[derive(Debug)]
pub struct IncomingClient {
    pub stream: ClientStream,
    pub peer: ClientPeer,
    pub readonly: bool,
}

/// Either a TCP or UNIX socket. Hides the platform difference from
/// the supervisor.
#[derive(Debug)]
pub enum ClientStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}

/// Origin of the client. Used in the welcome banner + audit log.
#[derive(Debug, Clone)]
pub enum ClientPeer {
    Tcp(SocketAddr),
    #[cfg(unix)]
    Unix(Option<std::path::PathBuf>),
}

/// Direction of the per-client mpsc — supervisor-to-client. The
/// supervisor sends [`OutboundFrame`]s; the client task IAC-encodes
/// them and writes to the socket.
#[derive(Debug, Clone)]
pub enum OutboundFrame {
    /// Plain payload (PTY output, peer echo, banner text).
    Bytes(Vec<u8>),
    /// Raw IAC reply emitted by [`super::telnet`] (negotiation
    /// responses); already correctly formatted, do NOT re-escape.
    RawIac(Vec<u8>),
    /// Disconnect this client gracefully.
    Disconnect,
}

/// Direction of the per-client mpsc — client-to-supervisor.
#[derive(Debug)]
pub enum InboundEvent {
    /// User typed bytes (after IAC strip). Supervisor scans for menu
    /// keys then forwards to the party-line.
    Data { bytes: Vec<u8> },
    /// Client disconnected (EOF or IO error).
    Disconnected,
}

/// Spawn the per-client read+write tasks. Returns the supervisor-side
/// handles: an mpsc sender for outbound frames + a join handle for
/// the client task pair.
///
/// # TODO: implementation
/// - split socket into read/write halves
/// - read task: `socket → TelnetParser::feed → Data event → InboundEvent::Data`
/// - write task: drain `outbound_rx`, IAC-escape Bytes, send Reply raw
/// - on EOF/error in either: emit `InboundEvent::Disconnected` and exit
pub fn spawn_client(
    _client: IncomingClient,
    _inbound_tx: mpsc::Sender<(ClientId, InboundEvent)>,
) -> (ClientId, mpsc::Sender<OutboundFrame>) {
    // TODO: real implementation
    let (tx, _rx) = mpsc::channel(64);
    (ClientId::new(), tx)
}

/// Stable identifier for one client. Used by the supervisor to
/// route outbound frames + book-keep the readonly/user/logger
/// counts shown in the welcome banner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u64);

impl ClientId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::new()
    }
}
