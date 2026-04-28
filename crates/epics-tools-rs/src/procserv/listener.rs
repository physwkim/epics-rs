//! Connection acceptors.
//!
//! Spawns one task per configured listener (TCP + UNIX) that accepts
//! incoming connections and hands each fresh socket off to the
//! supervisor as a new [`super::client::IncomingClient`]. Mirrors
//! C `acceptFactory.cc` (`acceptItemTCP` / `acceptItemUNIX`).

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::net::{TcpListener, UnixListener};
use tokio::sync::mpsc;

use crate::procserv::client::{ClientPeer, ClientStream, IncomingClient};
use crate::procserv::error::{ProcServError, ProcServResult};

/// Run the TCP listener loop until the supervisor's `out` channel
/// closes. Each accepted socket is wrapped in [`IncomingClient`] and
/// forwarded.
///
/// `readonly`: when true, every accepted client is read-only —
/// matches C procServ's `--readonly` deployment for sites that want
/// only log-style viewers (separate listening port for observers
/// vs operators).
pub async fn run_tcp(
    bind: SocketAddr,
    readonly: bool,
    out: mpsc::Sender<IncomingClient>,
) -> ProcServResult<()> {
    let listener = TcpListener::bind(bind)
        .await
        .map_err(|e| ProcServError::ListenerBind(format!("TCP {bind}: {e}")))?;
    tracing::info!(addr = %bind, readonly, "procserv-rs: TCP listener accepted");

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let inc = IncomingClient {
                    stream: ClientStream::Tcp(stream),
                    peer: ClientPeer::Tcp(peer),
                    readonly,
                };
                if out.send(inc).await.is_err() {
                    // Supervisor went away; we're shutting down.
                    return Ok(());
                }
            }
            Err(e) => {
                // Per-accept errors (e.g. EMFILE) are recoverable;
                // log and keep listening so a transient
                // file-descriptor exhaustion doesn't kill the gateway.
                tracing::warn!(error = %e, "procserv-rs: TCP accept error");
            }
        }
    }
}

/// UNIX-socket listener. Removes any existing socket file at `path`
/// (the C version `unlink`s up front for the same reason — stale
/// socket files block `bind`). On graceful shutdown the runtime
/// drops the listener; we make a best-effort `unlink` here too,
/// but in case of a hard kill the next start cleans it up.
pub async fn run_unix(
    path: PathBuf,
    readonly: bool,
    out: mpsc::Sender<IncomingClient>,
) -> ProcServResult<()> {
    // Best-effort unlink of stale socket — ignore "not found".
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .map_err(|e| ProcServError::ListenerBind(format!("UNIX {}: {e}", path.display())))?;
    tracing::info!(path = %path.display(), readonly, "procserv-rs: UNIX listener accepted");

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let inc = IncomingClient {
                    stream: ClientStream::Unix(stream),
                    peer: ClientPeer::Unix(Some(path.clone())),
                    readonly,
                };
                if out.send(inc).await.is_err() {
                    return Ok(());
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "procserv-rs: UNIX accept error");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    #[tokio::test]
    async fn tcp_listener_forwards_accepted_socket() {
        // Pick an OS-assigned port.
        let bind = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
        // Bind first to learn the port, then re-bind via run_tcp on
        // that exact port. Simpler: just bind once and use the port.
        let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
        let actual = listener.local_addr().unwrap();
        drop(listener);

        let (tx, mut rx) = mpsc::channel(4);
        let server = tokio::spawn(async move { run_tcp(actual, false, tx).await });

        // Give the listener a moment to bind.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut conn = TcpStream::connect(actual).await.unwrap();
        let inc = rx.recv().await.expect("got incoming");
        assert!(matches!(inc.stream, ClientStream::Tcp(_)));
        assert!(matches!(inc.peer, ClientPeer::Tcp(_)));
        assert!(!inc.readonly);

        // Round-trip a byte to confirm the socket is live.
        let mut server_stream = match inc.stream {
            ClientStream::Tcp(s) => s,
            _ => unreachable!(),
        };
        conn.write_all(b"x").await.unwrap();
        let mut buf = [0u8; 1];
        server_stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf[0], b'x');

        server.abort();
    }
}
