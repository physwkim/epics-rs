//! Connection acceptors.
//!
//! Spawns one task per configured listener (TCP + UNIX) that accepts
//! incoming connections and hands each fresh socket off to the
//! supervisor as a new [`super::client::ClientConnection`]. Mirrors
//! C `acceptFactory.cc` (`acceptItemTCP` / `acceptItemUNIX`).

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::procserv::client::IncomingClient;
use crate::procserv::error::ProcServResult;

/// Run the TCP listener loop until cancelled. Each accepted socket
/// becomes an [`IncomingClient`] forwarded over `out`.
///
/// `readonly`: when true, every accepted client is read-only —
/// matches C procServ's `--readonly` deployment for sites that want
/// only log-style viewers (e.g. a separate listening port for
/// observers vs operators).
///
/// # TODO: implementation
/// - `tokio::net::TcpListener::bind(bind)`
/// - loop `accept().await` → push `IncomingClient` to `out`
/// - graceful shutdown on cancellation
pub async fn run_tcp(
    _bind: SocketAddr,
    _readonly: bool,
    _out: mpsc::Sender<IncomingClient>,
) -> ProcServResult<()> {
    // TODO: real implementation
    std::future::pending().await
}

/// UNIX-socket equivalent. C procServ uses Linux abstract namespace
/// when the path begins with `@`; portable fallback is a path on
/// disk that needs explicit `unlink` on shutdown.
///
/// # TODO: implementation
/// - `tokio::net::UnixListener::bind(path)`
/// - same accept loop as TCP
/// - clean up the socket file on graceful shutdown
pub async fn run_unix(
    _path: PathBuf,
    _readonly: bool,
    _out: mpsc::Sender<IncomingClient>,
) -> ProcServResult<()> {
    // TODO: real implementation
    std::future::pending().await
}
