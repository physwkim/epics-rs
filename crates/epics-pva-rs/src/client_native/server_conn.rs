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
use std::time::Duration;

use parking_lot::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{interval, timeout};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::error::{PvaError, PvaResult};
use crate::proto::{
    ByteOrder, Command, ControlCommand, HeaderFlags, PvaHeader, Status, WriteExt,
    encode_string_into,
};

use super::decode::{
    Frame, decode_connection_validated, decode_connection_validation_request, try_parse_frame,
};

/// How often we send heartbeat ECHO_REQUEST.
///
/// Resolved at call time from `EPICS_PVA_CONN_TMO`: pvxs convention is
/// ECHO every `CONN_TMO / 2` so two heartbeats fit inside the timeout
/// window. Default 15 s when the env var is unset (CONN_TMO defaults
/// to 30 s).
pub fn heartbeat_interval() -> Duration {
    let configured = crate::config::env::conn_timeout_secs() as f64;
    Duration::from_secs_f64((configured / 2.0).max(1.0))
}

/// Maximum time we'll wait between any incoming bytes before declaring
/// the connection dead. pvxs effective timeout = configured × 4/3
/// (config.cpp:187 tmoScale) — without the margin a healthy client
/// races with its second ECHO. Floored at 2 s like pvxs `enforceTimeout`.
pub fn heartbeat_timeout() -> Duration {
    let configured = crate::config::env::conn_timeout_secs() as f64;
    Duration::from_secs_f64((configured * 4.0 / 3.0).max(2.0))
}

/// Hard cap on a single inbound message's payload length on the
/// client side. Mirrors `PvaServerConfig::max_message_size` — without
/// it, a malicious or compromised server announcing a 4 GiB header
/// would force the client to OOM-loop growing rx_buf. 64 MiB matches
/// the server-side default. Override compile-time only for now.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

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
    /// Per-connection FieldDesc cache for 0xFD/0xFE wire markers.
    /// Populated as INIT responses arrive; consulted when subsequent
    /// frames reference a slot. Lives for the life of the connection.
    type_cache: Arc<Mutex<crate::pvdata::encode::TypeCache>>,
}

/// Type-erased read half. We accept either a plain TCP read half or a
/// TLS read half through the same code path.
type DynRead = Box<dyn tokio::io::AsyncRead + Unpin + Send>;
/// Type-erased write half.
type DynWrite = Box<dyn tokio::io::AsyncWrite + Unpin + Send>;

impl ServerConn {
    /// Open a plain TCP connection, run the handshake, and start
    /// background tasks.
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
        let (reader, writer) = stream.into_split();
        let reader: DynRead = Box::new(reader);
        let writer: DynWrite = Box::new(writer);
        Self::run_handshake_and_spawn(target, reader, writer, user, host, op_timeout).await
    }

    /// Open a TLS-wrapped connection (`pvas://`).
    pub async fn connect_tls(
        target: SocketAddr,
        server_name: &str,
        tls: Arc<crate::auth::TlsClientConfig>,
        user: &str,
        host: &str,
        op_timeout: Duration,
    ) -> PvaResult<Arc<Self>> {
        let stream = timeout(op_timeout, TcpStream::connect(target))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;
        stream.set_nodelay(true).ok();

        let connector = tokio_rustls::TlsConnector::from(tls.config.clone());
        let dnsname = rustls::pki_types::ServerName::try_from(server_name.to_string())
            .map_err(|e| PvaError::Protocol(format!("invalid TLS server name: {e}")))?;
        let tls_stream = timeout(op_timeout, connector.connect(dnsname, stream))
            .await
            .map_err(|_| PvaError::Timeout)?
            .map_err(PvaError::Io)?;

        let (reader, writer) = tokio::io::split(tls_stream);
        let reader: DynRead = Box::new(reader);
        let writer: DynWrite = Box::new(writer);
        Self::run_handshake_and_spawn(target, reader, writer, user, host, op_timeout).await
    }

    /// Internal: takes already-split read/write halves, runs the handshake,
    /// then spawns the reader/writer/heartbeat tasks. Used by both
    /// [`connect`] and [`connect_tls`].
    async fn run_handshake_and_spawn(
        target: SocketAddr,
        mut reader: DynRead,
        writer: DynWrite,
        user: &str,
        host: &str,
        op_timeout: Duration,
    ) -> PvaResult<Arc<Self>> {
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
            // P-G21: client-side segmented-message reassembly. Mirror
            // of the server-side state machine added in P-G20. pvxs
            // sends large monitor events (NTNDArray frames, multi-MiB
            // arrays, big NTTable INIT descriptors) as
            // SegFirst..SegMiddle*..SegLast sequences; without
            // reassembly the client decodes each segment as if it
            // were a fresh complete frame, the IOID-routed receiver
            // gets garbage, and the application surfaces a Decode
            // error (or worse — wrong shape silently parsed).
            let mut seg_buf: Vec<u8> = Vec::new();
            let mut seg_cmd: u8 = 0;
            let mut seg_flags: crate::proto::HeaderFlags = crate::proto::HeaderFlags(0);
            let mut expect_seg = false;
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
                            // Peek the header once we have 8 bytes — drop
                            // the connection if the announced payload
                            // exceeds MAX_MESSAGE_SIZE. Defends against a
                            // malicious or compromised server announcing a
                            // 4 GiB header to OOM the client.
                            if buf.len() >= crate::proto::PvaHeader::SIZE {
                                if let Ok(hdr) = crate::proto::PvaHeader::decode(
                                    &mut std::io::Cursor::new(&buf[..])
                                ) {
                                    if !hdr.flags.is_control()
                                        && hdr.payload_length as usize > MAX_MESSAGE_SIZE
                                    {
                                        warn!(
                                            payload = hdr.payload_length,
                                            cap = MAX_MESSAGE_SIZE,
                                            "PVA inbound payload exceeds cap, closing"
                                        );
                                        break;
                                    }
                                }
                            }
                            while let Ok(Some((frame, fn_)) ) = try_parse_frame(&buf) {
                                buf.drain(..fn_);
                                if frame.header.flags.is_control() {
                                    handle_control_frame(&frame, &writer_tx_reader, order_reader).await;
                                    continue;
                                }
                                // P-G21: segmentation gate (mirrors
                                // server-side P-G20 / pvxs conn.cpp:
                                // 228-244). Validate continuation
                                // invariants; accumulate until
                                // SegLast (or unsegmented), then
                                // dispatch the synthetic Frame.
                                let raw_seg = frame.header.flags.0
                                    & crate::proto::HeaderFlags::SEGMENT_MASK;
                                let continuation = raw_seg
                                    & crate::proto::HeaderFlags::SEGMENT_LAST
                                    != 0;
                                if continuation ^ expect_seg
                                    || (continuation
                                        && frame.header.command != seg_cmd)
                                {
                                    warn!(
                                        expect_seg,
                                        continuation,
                                        cmd = frame.header.command,
                                        saved = seg_cmd,
                                        "PVA segmentation violation from server, closing"
                                    );
                                    cancel_reader.cancel();
                                    return;
                                }
                                if raw_seg == 0
                                    || raw_seg
                                        == crate::proto::HeaderFlags::SEGMENT_FIRST
                                {
                                    expect_seg = true;
                                    seg_cmd = frame.header.command;
                                    seg_flags = frame.header.flags;
                                    seg_buf.clear();
                                }
                                // Cap reassembly: peer that streams
                                // SegFirst → SegMiddle … forever would
                                // grow seg_buf without bound otherwise.
                                if seg_buf.len().saturating_add(frame.payload.len())
                                    > MAX_MESSAGE_SIZE
                                {
                                    warn!(
                                        accumulated = seg_buf.len(),
                                        next = frame.payload.len(),
                                        cap = MAX_MESSAGE_SIZE,
                                        "PVA reassembled message exceeds cap, closing"
                                    );
                                    cancel_reader.cancel();
                                    return;
                                }
                                seg_buf.extend_from_slice(&frame.payload);
                                if raw_seg != 0
                                    && raw_seg
                                        != crate::proto::HeaderFlags::SEGMENT_LAST
                                {
                                    continue;
                                }
                                expect_seg = false;
                                let dispatch_frame = if raw_seg == 0 {
                                    frame
                                } else {
                                    Frame {
                                        header: crate::proto::PvaHeader {
                                            version: frame.header.version,
                                            // Strip the segment bits — the
                                            // dispatch path expects an
                                            // unsegmented application frame.
                                            flags: crate::proto::HeaderFlags(
                                                seg_flags.0
                                                    & !crate::proto::HeaderFlags::SEGMENT_MASK,
                                            ),
                                            command: seg_cmd,
                                            payload_length: seg_buf.len() as u32,
                                        },
                                        payload: std::mem::take(&mut seg_buf),
                                    }
                                };
                                route_frame(dispatch_frame, &router_reader);
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
            alive_reader.store(false, Ordering::SeqCst);
            cancel_reader.cancel();
            // Drain the router — drops all per-ioid senders so any
            // outstanding `stream.recv().await` (e.g. monitor loops)
            // wakes with `None` and can react to the disconnect.
            {
                let mut guard = router_reader.lock();
                guard.by_ioid.clear();
                guard.by_cid.clear();
            }
        });

        // Heartbeat task
        let cancel_hb = cancel.clone();
        let alive_hb = alive.clone();
        let last_rx_hb = last_rx_nanos.clone();
        let writer_tx_hb = writer_tx.clone();
        let order_hb = byte_order;
        tokio::spawn(async move {
            let hb_interval = heartbeat_interval();
            let hb_timeout = heartbeat_timeout();
            let mut tick = interval(hb_interval);
            tick.tick().await; // skip first immediate tick
            loop {
                tokio::select! {
                    _ = cancel_hb.cancelled() => break,
                    _ = tick.tick() => {
                        // Liveness check: are we receiving anything?
                        let last = last_rx_hb.load(Ordering::SeqCst);
                        let elapsed = now_nanos().saturating_sub(last);
                        if elapsed > hb_timeout.as_nanos() as u64 {
                            warn!("PVA connection idle > {hb_timeout:?}, closing");
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
            type_cache: Arc::new(Mutex::new(crate::pvdata::encode::TypeCache::new())),
        }))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Get a clone of the per-connection FieldDesc cache (Arc shared).
    /// Used by op decoders to resolve 0xFD/0xFE wire markers.
    pub fn type_cache(&self) -> Arc<Mutex<crate::pvdata::encode::TypeCache>> {
        self.type_cache.clone()
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
    ///
    /// **Backpressure model**: returns an unbounded channel because the
    /// PVA monitor protocol bounds inflight frames at the wire level
    /// via the pipeline-ack window (`pipeline_size`, default 4) — a
    /// well-behaved server stops emitting once the unacked window is
    /// full. The unbounded receiver therefore stays bounded in
    /// practice. A malicious server that ignores the ack window can
    /// still grow this queue, but the per-frame `max_message_size`
    /// cap (`PvaServerConfig::max_message_size`, applied in the
    /// reader) bounds each payload, and the parent connection's
    /// `op_timeout` / `idle_timeout` machinery eventually tears down
    /// truly pathological peers.
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

async fn read_handshake_init<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
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

async fn wait_for_validated<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
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

async fn read_one_frame<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    rx_buf: &mut Vec<u8>,
    op_timeout: Duration,
) -> PvaResult<Frame> {
    loop {
        if let Some((frame, n)) = try_parse_frame(rx_buf)? {
            rx_buf.drain(..n);
            return Ok(frame);
        }
        // Same MAX_MESSAGE_SIZE peek as the streaming reader (P-G8).
        if rx_buf.len() >= crate::proto::PvaHeader::SIZE {
            if let Ok(hdr) = crate::proto::PvaHeader::decode(&mut std::io::Cursor::new(&rx_buf[..]))
            {
                if !hdr.flags.is_control() && hdr.payload_length as usize > MAX_MESSAGE_SIZE {
                    return Err(PvaError::Protocol(format!(
                        "inbound payload {} exceeds MAX_MESSAGE_SIZE {}",
                        hdr.payload_length, MAX_MESSAGE_SIZE
                    )));
                }
            }
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

async fn handle_control_frame(frame: &Frame, writer_tx: &mpsc::Sender<Vec<u8>>, order: ByteOrder) {
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

    // pvxs always reads a Variant payload after the auth method string —
    // even for "anonymous". Send the null-variant marker (0xFF) for
    // anonymous, or an inline structure with user/host[/groups] for
    // "ca". The optional `groups` field carries POSIX group names so
    // server-side ACF can match `group:foo` rules — pvxs ca-auth
    // parity (osgroups.cpp).
    if auth == "ca" {
        let groups = crate::auth::posix_groups();
        // Variant tag (0xFD) + inline AuthZ structure carrying
        // user (str) + host (str) [+ groups (str[])].
        payload.put_u8(0xFD);
        payload.put_u16(1, order);
        payload.put_u8(0x80);
        payload.put_u8(0x00);
        let n_fields = if groups.is_empty() { 2u8 } else { 3u8 };
        payload.put_u8(n_fields);
        payload.put_u8(0x04);
        payload.extend_from_slice(b"user");
        payload.put_u8(0x60); // string
        payload.put_u8(0x04);
        payload.extend_from_slice(b"host");
        payload.put_u8(0x60); // string
        if !groups.is_empty() {
            payload.put_u8(0x06);
            payload.extend_from_slice(b"groups");
            payload.put_u8(0x68); // string[]
        }
        encode_string_into(user, order, &mut payload);
        encode_string_into(host, order, &mut payload);
        if !groups.is_empty() {
            // string-array length prefix (size_t encoding) + each
            // string.
            crate::proto::encode_size_into(groups.len() as u32, order, &mut payload);
            for g in &groups {
                encode_string_into(g, order, &mut payload);
            }
        }
    } else {
        // Null variant — pvxs `readVariant` returns `Value()` for 0xFF.
        payload.put_u8(0xFF);
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
