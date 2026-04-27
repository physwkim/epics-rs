//! Persistent TCP virtual circuit to a single PVA server.
//!
//! Replaces the old "open-fresh-socket-per-op" `Connection`. Spawns three
//! background tasks per connection:
//!
//! - **Reader**: parses incoming frames, routes them to per-IOID waiters
//!   (oneshot for one-shot ops, mpsc for monitor streams). Updates the
//!   `last_rx` timestamp used by the heartbeat.
//! - **Writer**: drains a `mpsc<Vec<u8>>` queue and writes to the socket.
//!   Owning a single writer task lets every channel/op share the connection
//!   safely without holding an `AsyncMutex` across awaits.
//! - **Heartbeat**: sends `ECHO_REQUEST` every 15 s; if no `last_rx` update
//!   has happened in 30 s, declares the connection dead and triggers
//!   shutdown.
//!
//! When any task exits (read EOF, write error, or heartbeat timeout) the
//! cancellation token fires and the connection is torn down. Channels
//! holding an `Arc<ServerConn>` observe the closed state via [`ServerConn::is_alive`]
//! and transition to "Reconnecting".

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, timeout};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::error::{PvaError, PvaResult};
use crate::proto::{
    encode_string_into, ByteOrder, Command, ControlCommand, HeaderFlags, PvaHeader, Status,
    WriteExt,
};

use super::decode::{
    decode_connection_validated, decode_connection_validation_request, try_parse_frame, Frame,
};

/// How often we send heartbeat ECHO_REQUEST.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
/// Maximum time we'll wait between any incoming bytes before declaring the
/// connection dead.
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);

/// One framed message dispatched to a per-IOID waiter.
type FrameTx = mpsc::UnboundedSender<Frame>;

/// Routing table for incoming frames.
///
/// Each in-flight one-shot op (GET/PUT/GET_FIELD INIT/data) registers a
/// `FrameTx` keyed by its ioid. Streaming ops (MONITOR) register the same
/// way; they keep receiving frames until the handle is dropped or the op
/// is destroyed.
#[derive(Default)]
struct Router {
    by_ioid: HashMap<u32, FrameTx>,
    /// Routes for CREATE_CHANNEL responses by cid.
    by_cid: HashMap<u32, oneshot::Sender<Frame>>,
}

/// A persistent server connection.
pub struct ServerConn {
    pub addr: SocketAddr,
    pub byte_order: ByteOrder,
    writer_tx: mpsc::Sender<Vec<u8>>,
    cancel: CancellationToken,
    alive: Arc<AtomicBool>,
    last_rx_nanos: Arc<AtomicU64>,
    router: Arc<Mutex<Router>>,
}

impl ServerConn {
    /// Open a connection, run the handshake, and start background tasks.
    pub async fn connect(
        target: SocketAddr,
        user: &str,
        host: &str,
        op_timeout: Duration,
    ) -> PvaResult<Arc<Self>> {
        let stream = timeout(op_timeout, TcpStream::connect(target))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;
        stream.set_nodelay(true).ok();

        let (mut reader, writer) = stream.into_split();

        // Step 1+2: read handshake frames until we get CONNECTION_VALIDATION.
        let mut rx_buf: Vec<u8> = Vec::with_capacity(8192);
        let (byte_order, _server_buf, _server_reg, auth_methods) =
            read_handshake_init(&mut reader, &mut rx_buf, op_timeout).await?;

        // Choose auth method: prefer "ca" if offered.
        let negotiated_auth = if auth_methods.iter().any(|m| m == "ca") {
            "ca"
        } else {
            "anonymous"
        };

        // Step 3: send our CONNECTION_VALIDATION reply on the (still-not-spawned) writer.
        let mut writer_owned = writer;
        let reply = build_client_connection_validation(
            byte_order,
            DEFAULT_BUFFER_SIZE,
            DEFAULT_REGISTRY_SIZE,
            0,
            negotiated_auth,
            user,
            host,
        );
        timeout(op_timeout, writer_owned.write_all(&reply))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;

        // Step 4: wait for CONNECTION_VALIDATED.
        wait_for_validated(&mut reader, &mut rx_buf, op_timeout).await?;

        // Spawn background tasks.
        let (writer_tx, mut writer_rx) = mpsc::channel::<Vec<u8>>(256);
        let cancel = CancellationToken::new();
        let alive = Arc::new(AtomicBool::new(true));
        let last_rx_nanos = Arc::new(AtomicU64::new(now_nanos()));
        let router: Arc<Mutex<Router>> = Arc::new(Mutex::new(Router::default()));

        // Writer task
        let cancel_writer = cancel.clone();
        let alive_writer = alive.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_writer.cancelled() => break,
                    msg = writer_rx.recv() => match msg {
                        Some(bytes) => {
                            if writer_owned.write_all(&bytes).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
            alive_writer.store(false, Ordering::SeqCst);
            cancel_writer.cancel();
        });

        // Reader task
        let cancel_reader = cancel.clone();
        let alive_reader = alive.clone();
        let last_rx_reader = last_rx_nanos.clone();
        let router_reader = router.clone();
        let writer_tx_reader = writer_tx.clone();
        let order_reader = byte_order;
        tokio::spawn(async move {
            let mut buf = rx_buf;
            let mut chunk = vec![0u8; 4096];
            loop {
                tokio::select! {
                    _ = cancel_reader.cancelled() => break,
                    res = reader.read(&mut chunk) => match res {
                        Ok(0) => {
                            debug!("server closed");
                            break;
                        }
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            last_rx_reader.store(now_nanos(), Ordering::SeqCst);
                            while let Ok(Some((frame, fn_)) ) = try_parse_frame(&buf) {
                                buf.drain(..fn_);
                                if frame.header.flags.is_control() {
                                    handle_control_frame(&frame, &writer_tx_reader, order_reader).await;
                                    continue;
                                }
                                route_frame(frame, &router_reader);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            alive_reader.store(false, Ordering::SeqCst);
            cancel_reader.cancel();
        });

        // Heartbeat task
        let cancel_hb = cancel.clone();
        let alive_hb = alive.clone();
        let last_rx_hb = last_rx_nanos.clone();
        let writer_tx_hb = writer_tx.clone();
        let order_hb = byte_order;
        tokio::spawn(async move {
            let mut tick = interval(HEARTBEAT_INTERVAL);
            tick.tick().await; // skip first immediate tick
            loop {
                tokio::select! {
                    _ = cancel_hb.cancelled() => break,
                    _ = tick.tick() => {
                        // Liveness check: are we receiving anything?
                        let last = last_rx_hb.load(Ordering::SeqCst);
                        let elapsed = now_nanos().saturating_sub(last);
                        if elapsed > HEARTBEAT_TIMEOUT.as_nanos() as u64 {
                            warn!("PVA connection idle > {HEARTBEAT_TIMEOUT:?}, closing");
                            break;
                        }
                        // Send ECHO_REQUEST control message.
                        let h = PvaHeader::control(false, order_hb, ControlCommand::EchoRequest.code(), 0);
                        let mut bytes = Vec::with_capacity(8);
                        h.write_into(&mut bytes);
                        if writer_tx_hb.send(bytes).await.is_err() {
                            break;
                        }
                    }
                }
            }
            alive_hb.store(false, Ordering::SeqCst);
            cancel_hb.cancel();
        });

        Ok(Arc::new(Self {
            addr: target,
            byte_order,
            writer_tx,
            cancel,
            alive,
            last_rx_nanos,
            router,
        }))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn close(&self) {
        self.cancel.cancel();
        self.alive.store(false, Ordering::SeqCst);
    }

    /// Send a fully-built frame.
    pub async fn send(&self, frame: Vec<u8>) -> PvaResult<()> {
        if !self.is_alive() {
            return Err(PvaError::Protocol("server connection closed".into()));
        }
        self.writer_tx
            .send(frame)
            .await
            .map_err(|_| PvaError::Protocol("writer queue closed".into()))
    }

    /// Register a one-shot waiter for a CREATE_CHANNEL response.
    pub fn register_cid_waiter(&self, cid: u32) -> oneshot::Receiver<Frame> {
        let (tx, rx) = oneshot::channel();
        self.router.lock().by_cid.insert(cid, tx);
        rx
    }

    /// Register a stream of frames matching a particular ioid.
    pub fn register_ioid_stream(&self, ioid: u32) -> mpsc::UnboundedReceiver<Frame> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.router.lock().by_ioid.insert(ioid, tx);
        rx
    }

    pub fn unregister_ioid(&self, ioid: u32) {
        self.router.lock().by_ioid.remove(&ioid);
    }

    /// Wait for the connection to terminate (returns when reader/writer/heartbeat
    /// all have stopped).
    pub async fn wait_closed(&self) {
        self.cancel.cancelled().await;
    }

    /// Time elapsed since the last incoming byte.
    pub fn idle_for(&self) -> Duration {
        let last = self.last_rx_nanos.load(Ordering::SeqCst);
        let now = now_nanos();
        Duration::from_nanos(now.saturating_sub(last))
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

const DEFAULT_BUFFER_SIZE: u32 = 87_040;
const DEFAULT_REGISTRY_SIZE: u16 = 32_767;

fn now_nanos() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

async fn read_handshake_init(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    rx_buf: &mut Vec<u8>,
    op_timeout: Duration,
) -> PvaResult<(ByteOrder, u32, u16, Vec<String>)> {
    let mut byte_order = ByteOrder::Little;
    loop {
        let frame = read_one_frame(reader, rx_buf, op_timeout).await?;
        if frame.header.flags.is_control() {
            if frame.header.command == ControlCommand::SetByteOrder.code() {
                byte_order = frame.header.flags.byte_order();
            }
            continue;
        }
        if frame.header.command == Command::ConnectionValidation.code() {
            let req = decode_connection_validation_request(&frame)?;
            return Ok((
                byte_order,
                req.server_buffer_size,
                req.server_registry_size,
                req.auth_methods,
            ));
        }
    }
}

async fn wait_for_validated(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    rx_buf: &mut Vec<u8>,
    op_timeout: Duration,
) -> PvaResult<()> {
    loop {
        let frame = read_one_frame(reader, rx_buf, op_timeout).await?;
        if frame.header.flags.is_control() {
            continue;
        }
        if frame.header.command == Command::ConnectionValidated.code() {
            let st = decode_connection_validated(&frame)?;
            if !st.is_success() {
                return Err(PvaError::Protocol(format!(
                    "connection validation rejected: {:?}",
                    st
                )));
            }
            return Ok(());
        }
    }
}

async fn read_one_frame(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    rx_buf: &mut Vec<u8>,
    op_timeout: Duration,
) -> PvaResult<Frame> {
    loop {
        if let Some((frame, n)) = try_parse_frame(rx_buf)? {
            rx_buf.drain(..n);
            return Ok(frame);
        }
        let mut chunk = [0u8; 4096];
        let n = match timeout(op_timeout, reader.read(&mut chunk)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(PvaError::Io(e)),
            Err(_) => return Err(PvaError::Timeout),
        };
        if n == 0 {
            return Err(PvaError::Protocol("server closed during handshake".into()));
        }
        rx_buf.extend_from_slice(&chunk[..n]);
    }
}

async fn handle_control_frame(
    frame: &Frame,
    writer_tx: &mpsc::Sender<Vec<u8>>,
    order: ByteOrder,
) {
    if frame.header.command == ControlCommand::EchoRequest.code() {
        // Server pinged us — bounce back.
        let resp = PvaHeader::control(
            false,
            order,
            ControlCommand::EchoResponse.code(),
            frame.header.payload_length,
        );
        let mut bytes = Vec::with_capacity(8);
        resp.write_into(&mut bytes);
        let _ = writer_tx.send(bytes).await;
    }
    // Other control messages (SetMarker, AckMarker, EchoResponse) update
    // last_rx implicitly; no further action.
}

fn route_frame(frame: Frame, router: &Arc<Mutex<Router>>) {
    let cmd = frame.header.command;
    let mut router_guard = router.lock();

    // CREATE_CHANNEL responses route by CID.
    if cmd == Command::CreateChannel.code() {
        if let Some(cid) = peek_u32(&frame.payload, 0, frame.header.flags.byte_order()) {
            if let Some(tx) = router_guard.by_cid.remove(&cid) {
                let _ = tx.send(frame);
                return;
            }
        }
    }

    // Application op responses (GET/PUT/MONITOR/RPC/GET_FIELD) route by IOID.
    let ioid = peek_u32(&frame.payload, 0, frame.header.flags.byte_order());
    if let Some(ioid) = ioid {
        if let Some(tx) = router_guard.by_ioid.get(&ioid).cloned() {
            drop(router_guard);
            let _ = tx.send(frame);
            return;
        }
    }
    // Otherwise: drop silently. (Beacons/SearchResponse are handled
    // out-of-band by the search engine, not here.)
}

fn peek_u32(payload: &[u8], offset: usize, order: ByteOrder) -> Option<u32> {
    if payload.len() < offset + 4 {
        return None;
    }
    let bytes: [u8; 4] = payload[offset..offset + 4].try_into().ok()?;
    Some(match order {
        ByteOrder::Big => u32::from_be_bytes(bytes),
        ByteOrder::Little => u32::from_le_bytes(bytes),
    })
}

fn build_client_connection_validation(
    order: ByteOrder,
    buffer_size: u32,
    registry_size: u16,
    qos: u16,
    auth: &str,
    user: &str,
    host: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.put_u32(buffer_size, order);
    payload.put_u16(registry_size, order);
    payload.put_u16(qos, order);
    encode_string_into(auth, order, &mut payload);

    if auth == "ca" {
        // Variant tag (0xFD) + inline AuthZ structure carrying user+host.
        payload.put_u8(0xFD);
        payload.put_u16(1, order);
        payload.put_u8(0x80);
        payload.put_u8(0x00);
        payload.put_u8(0x02);
        payload.put_u8(0x04);
        payload.extend_from_slice(b"user");
        payload.put_u8(0x60);
        payload.put_u8(0x04);
        payload.extend_from_slice(b"host");
        payload.put_u8(0x60);
        encode_string_into(user, order, &mut payload);
        encode_string_into(host, order, &mut payload);
    }

    let h = PvaHeader::application(
        false,
        order,
        Command::ConnectionValidation.code(),
        payload.len() as u32,
    );
    let mut out = Vec::with_capacity(PvaHeader::SIZE + payload.len());
    h.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

#[allow(unused_imports)]
use crate::proto::{decode_size, decode_string};

#[allow(dead_code)]
fn _suppress(_: HeaderFlags, _: Status) {}
