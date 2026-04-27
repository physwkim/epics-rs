//! Tiny HTTP introspection endpoint for the CA server.
//!
//! Exposes a handful of read-only routes so an external supervisor
//! (Kubernetes liveness/readiness probe, Grafana scraper, oncall
//! dashboard) can answer "is this IOC alive, and what is it doing?"
//! without speaking CA. Intentionally minimal — we do not want a
//! heavyweight HTTP framework in the IOC's runtime dependency graph,
//! so the implementation is plain tokio + manual HTTP/1.1 line
//! parsing. Suitable for the scale this serves: low QPS, friendly
//! callers.
//!
//! Routes:
//! - `GET /healthz`   — 200 OK if the server is up. Smoke for probes.
//! - `GET /info`      — JSON: port, uptime, build version
//! - `GET /clients`   — JSON list of currently-connected peer addrs
//! - `GET /queues`    — JSON: configured per-client caps and limits
//!
//! Anything else returns 404. The endpoint is bind-protected: bind to
//! `127.0.0.1:<port>` for IOC-local probes, or `0.0.0.0:<port>` if
//! you've cleared the network policy explicitly. There is no auth —
//! treat this like `/proc`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

/// Live-updating snapshot the HTTP handlers read from. Concurrent
/// writers (the TCP listener task) just bump the atomic counters; the
/// peer list is mutex-guarded but only touched on connect/disconnect.
pub struct IntrospectionState {
    /// CA TCP listener port.
    pub ca_port: u16,
    /// Server start instant — used for `uptime_secs`.
    pub started: Instant,
    pub clients_active: AtomicU64,
    pub channels_active: AtomicU64,
    pub peers: Mutex<Vec<SocketAddr>>,
    /// Configured limits, surfaced verbatim by /queues.
    pub max_channels_per_client: u64,
    pub max_subs_per_channel: u64,
    pub rate_limit_msgs_per_sec: u64,
    pub rate_limit_burst: u64,
}

impl IntrospectionState {
    pub fn new(ca_port: u16) -> Arc<Self> {
        Arc::new(Self {
            ca_port,
            started: Instant::now(),
            clients_active: AtomicU64::new(0),
            channels_active: AtomicU64::new(0),
            peers: Mutex::new(Vec::new()),
            max_channels_per_client: 0,
            max_subs_per_channel: 0,
            rate_limit_msgs_per_sec: 0,
            rate_limit_burst: 0,
        })
    }

    pub async fn add_peer(&self, peer: SocketAddr) {
        self.clients_active.fetch_add(1, Ordering::AcqRel);
        let mut p = self.peers.lock().await;
        p.push(peer);
    }

    pub async fn remove_peer(&self, peer: SocketAddr) {
        self.clients_active.fetch_sub(1, Ordering::AcqRel);
        let mut p = self.peers.lock().await;
        if let Some(idx) = p.iter().position(|&a| a == peer) {
            p.swap_remove(idx);
        }
    }

    pub fn add_channel(&self) {
        self.channels_active.fetch_add(1, Ordering::AcqRel);
    }

    pub fn remove_channel(&self) {
        self.channels_active.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Bind the HTTP introspection listener and serve until the listener
/// is dropped. Spawn this on a tokio task; cancellation is by
/// dropping the JoinHandle.
pub async fn run_introspection(
    addr: SocketAddr,
    state: Arc<IntrospectionState>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(bind = %addr, "introspection HTTP server listening");
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "introspection accept failed");
                continue;
            }
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_request(stream, state).await {
                tracing::debug!(peer = %peer, error = %e, "introspection request error");
            }
        });
    }
}

async fn handle_request(
    stream: TcpStream,
    state: Arc<IntrospectionState>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await? == 0 {
        return Ok(());
    }
    // Drain remaining headers — we don't need them, but RFC 7230
    // requires reading until the empty line so the client doesn't see
    // a half-read connection.
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await? == 0 {
            break;
        }
        if header == "\r\n" || header == "\n" {
            break;
        }
    }

    let (method, path) = parse_request_line(&request_line);
    if method != "GET" {
        return write_response(reader.into_inner(), 405, "Method Not Allowed", "").await;
    }

    let (status, body) = match path {
        "/healthz" => (200, "{\"status\":\"ok\"}".to_string()),
        "/info" => (200, render_info(&state)),
        "/clients" => (200, render_clients(&state).await),
        "/queues" => (200, render_queues(&state)),
        _ => (404, "{\"error\":\"not_found\"}".to_string()),
    };
    write_response(reader.into_inner(), status, status_text(status), &body).await
}

fn parse_request_line(line: &str) -> (&str, &str) {
    let line = line.trim_end();
    let mut parts = line.split(' ');
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    (method, path)
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    }
}

async fn write_response(
    mut stream: TcpStream,
    code: u16,
    status: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {code} {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}

fn render_info(state: &IntrospectionState) -> String {
    let uptime = state.started.elapsed().as_secs();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!(
        "{{\"ca_port\":{},\"uptime_secs\":{},\"now_unix\":{},\"version\":\"{}\",\"clients\":{},\"channels\":{}}}",
        state.ca_port,
        uptime,
        now,
        env!("CARGO_PKG_VERSION"),
        state.clients_active.load(Ordering::Acquire),
        state.channels_active.load(Ordering::Acquire),
    )
}

async fn render_clients(state: &IntrospectionState) -> String {
    let peers = state.peers.lock().await;
    let mut s = String::from("{\"clients\":[");
    for (i, p) in peers.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push('"');
        s.push_str(&p.to_string());
        s.push('"');
    }
    s.push_str("]}");
    s
}

fn render_queues(state: &IntrospectionState) -> String {
    format!(
        "{{\"max_channels_per_client\":{},\"max_subs_per_channel\":{},\"rate_limit_msgs_per_sec\":{},\"rate_limit_burst\":{}}}",
        state.max_channels_per_client,
        state.max_subs_per_channel,
        state.rate_limit_msgs_per_sec,
        state.rate_limit_burst,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_line_basic() {
        let (m, p) = parse_request_line("GET /healthz HTTP/1.1\r\n");
        assert_eq!(m, "GET");
        assert_eq!(p, "/healthz");
    }

    #[test]
    fn render_info_contains_expected_fields() {
        let s = IntrospectionState::new(5064);
        s.clients_active.store(3, Ordering::Release);
        let body = render_info(&s);
        assert!(body.contains("\"ca_port\":5064"));
        assert!(body.contains("\"clients\":3"));
        assert!(body.contains("\"version\":"));
    }

    #[tokio::test]
    async fn render_clients_handles_empty() {
        let s = IntrospectionState::new(5064);
        assert_eq!(render_clients(&s).await, "{\"clients\":[]}");
    }

    #[tokio::test]
    async fn end_to_end_healthz() {
        // Bind ephemeral port, fire a real GET /healthz, parse the
        // response. Catches issues with the request reader/writer
        // that the unit-level tests don't exercise.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener); // re-bind inside run_introspection
        let state = IntrospectionState::new(5064);
        let st = state.clone();
        let server = tokio::spawn(async move {
            let _ = run_introspection(addr, st).await;
        });
        // give the listener a moment to bind
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream
            .write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let mut buf = vec![0u8; 256];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .unwrap();
        let s = String::from_utf8_lossy(&buf[..n]);
        assert!(s.contains("200 OK"));
        assert!(s.contains("\"status\":\"ok\""));
        server.abort();
    }
}
