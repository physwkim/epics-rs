//! TCP listener + per-connection handler.
//!
//! For each accepted client we spawn one task that:
//!
//! 1. Sends SET_BYTE_ORDER + CONNECTION_VALIDATION request
//! 2. Reads client's CONNECTION_VALIDATION response (auth)
//! 3. Sends CONNECTION_VALIDATED
//! 4. Loops reading channel ops (CREATE_CHANNEL / GET / PUT / MONITOR /
//!    GET_FIELD / DESTROY_REQUEST / DESTROY_CHANNEL).
//!
//! Channel state is kept per-connection (a `HashMap<sid, ChannelState>`).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio::time::interval;
use tracing::{debug, error, warn};

use crate::client_native::decode::{Frame, try_parse_frame};
use crate::error::{PvaError, PvaResult};
use crate::proto::{
    BitSet, ByteOrder, Command, ControlCommand, PVA_VERSION, PvaHeader, Status, WriteExt,
    encode_size_into, encode_string_into,
};
use crate::pvdata::encode::{decode_pv_field, decode_type_desc, encode_pv_field, encode_type_desc};
use crate::pvdata::{FieldDesc, PvField};

use super::runtime::PvaServerConfig;
use super::source::DynSource;

static NEXT_SID: AtomicU32 = AtomicU32::new(1);
fn alloc_sid() -> u32 {
    NEXT_SID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ChannelState {
    name: String,
    cid: u32,
    sid: u32,
    introspection: Option<FieldDesc>,
    /// ioid → (introspection negotiated for this op, kind)
    ops: HashMap<u32, OpState>,
}

/// Shared abort guard: when the last clone is dropped (HashMap removal,
/// connection end, ...), the spawned task is aborted automatically.
#[derive(Debug)]
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OpState {
    intro: FieldDesc,
    kind: OpKind,
    /// For MONITOR ops: true once the subscriber task has been spawned.
    /// Subsequent START/pipeline-ack messages are no-ops.
    monitor_started: bool,
    /// Abort guard for the spawned MONITOR subscriber. Drop semantics
    /// (via `AbortOnDrop`) ensure the task is cancelled when the op is
    /// removed from the channel map (DestroyRequest), when the channel
    /// itself is removed (DestroyChannel), or when the connection ends.
    monitor_abort: Option<Arc<AbortOnDrop>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OpKind {
    Get,
    Put,
    Monitor,
    Rpc,
}

/// Run the TCP listener forever.
pub async fn run_tcp_server(
    source: DynSource,
    bind_addr: SocketAddr,
    config: PvaServerConfig,
) -> PvaResult<()> {
    let listener = TcpListener::bind(bind_addr).await.map_err(PvaError::Io)?;
    debug!(?bind_addr, "TCP listener up");
    let active = Arc::new(AtomicUsize::new(0));

    let tls_acceptor = config
        .tls
        .as_ref()
        .map(|cfg| tokio_rustls::TlsAcceptor::from(cfg.config.clone()));

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let cur = active.fetch_add(1, Ordering::SeqCst);
                if cur >= config.max_connections {
                    active.fetch_sub(1, Ordering::SeqCst);
                    warn!(
                        ?peer,
                        "rejecting connection: max_connections={}", config.max_connections
                    );
                    drop(stream);
                    continue;
                }
                let src = source.clone();
                let cfg = config.clone();
                let active_dec = active.clone();
                let acceptor = tls_acceptor.clone();
                tokio::spawn(async move {
                    stream.set_nodelay(true).ok();
                    let result = match acceptor {
                        Some(a) => match a.accept(stream).await {
                            Ok(tls_stream) => {
                                let (r, w) = tokio::io::split(tls_stream);
                                handle_connection_io(src, Box::new(r), Box::new(w), peer, cfg).await
                            }
                            Err(e) => {
                                debug!(?peer, "TLS handshake failed: {e}");
                                Err(PvaError::Io(e))
                            }
                        },
                        None => {
                            let (r, w) = stream.into_split();
                            handle_connection_io(src, Box::new(r), Box::new(w), peer, cfg).await
                        }
                    };
                    if let Err(e) = result {
                        debug!(?peer, "connection ended: {e}");
                    }
                    active_dec.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(e) => {
                error!("accept error: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

/// Type-erased read/write halves so the same handler works for plain TCP
/// and TLS-wrapped streams.
type SrvRead = Box<dyn tokio::io::AsyncRead + Unpin + Send>;
type SrvWrite = Box<dyn tokio::io::AsyncWrite + Unpin + Send>;

async fn handle_connection_io(
    source: DynSource,
    mut reader: SrvRead,
    writer_raw: SrvWrite,
    peer: SocketAddr,
    config: PvaServerConfig,
) -> PvaResult<()> {
    let op_timeout = config.op_timeout;
    let idle_timeout = config.idle_timeout;
    let writer = Arc::new(Mutex::new(writer_raw));

    // Track per-connection liveness for the idle-timeout watchdog and the
    // server-side echo heartbeat task.
    let last_rx = Arc::new(AtomicU64::new(now_nanos()));

    // Spawn server-side heartbeat: send ECHO_REQUEST every 15 s; close if
    // we've been idle for `idle_timeout`.
    let last_rx_hb = last_rx.clone();
    let writer_hb = writer.clone();
    let order_hb = config.wire_byte_order;
    let _hb_handle = tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(15));
        tick.tick().await;
        loop {
            tick.tick().await;
            let last = last_rx_hb.load(Ordering::SeqCst);
            let elapsed = now_nanos().saturating_sub(last);
            if Duration::from_nanos(elapsed) > idle_timeout {
                warn!(?peer, "PVA client idle > {idle_timeout:?}; closing");
                break;
            }
            let h = PvaHeader::control(true, order_hb, ControlCommand::EchoRequest.code(), 0);
            let mut buf = Vec::with_capacity(8);
            h.write_into(&mut buf);
            if writer_hb.lock().await.write_all(&buf).await.is_err() {
                break;
            }
        }
    });

    let order = config.wire_byte_order;

    // Step 1: send SET_BYTE_ORDER (control message). Per pvxs, the byte order
    // we want to use is encoded in the control header's flag bit 7.
    let set_bo = {
        let mut buf = Vec::with_capacity(8);
        // Control message; flags include byte-order bit.
        let h = PvaHeader::control(true, order, ControlCommand::SetByteOrder.code(), 0);
        h.write_into(&mut buf);
        buf
    };
    writer
        .lock()
        .await
        .write_all(&set_bo)
        .await
        .map_err(PvaError::Io)?;

    // Step 2: send CONNECTION_VALIDATION request (server → client).
    let val_req = build_server_connection_validation(order, 87_040, 32_767, &["ca", "anonymous"]);
    writer
        .lock()
        .await
        .write_all(&val_req)
        .await
        .map_err(PvaError::Io)?;

    // Step 3+: drive the read loop.
    let mut rx_buf: Vec<u8> = Vec::with_capacity(8192);
    let mut channels: HashMap<u32, ChannelState> = HashMap::new();
    let mut handshake_complete = false;
    let _peer = peer;

    loop {
        let frame = read_frame(&mut reader, &mut rx_buf, op_timeout).await?;
        last_rx.store(now_nanos(), Ordering::SeqCst);
        if frame.header.flags.is_control() {
            // Handle echo etc., otherwise ignore.
            if frame.header.command == ControlCommand::EchoRequest.code() {
                let mut buf = Vec::new();
                let h = PvaHeader::control(
                    true,
                    order,
                    ControlCommand::EchoResponse.code(),
                    frame.header.payload_length,
                );
                h.write_into(&mut buf);
                writer
                    .lock()
                    .await
                    .write_all(&buf)
                    .await
                    .map_err(PvaError::Io)?;
            }
            continue;
        }

        // Pre-handshake: only CONNECTION_VALIDATION (1) is meaningful; client
        // replies with its buffer/registry/qos/auth payload. We accept any
        // and respond CONNECTION_VALIDATED.
        if !handshake_complete {
            if frame.header.command == Command::ConnectionValidation.code() {
                let mut payload = Vec::new();
                Status::ok().write_into(order, &mut payload);
                let h = PvaHeader::application(
                    true,
                    order,
                    Command::ConnectionValidated.code(),
                    payload.len() as u32,
                );
                let mut buf = Vec::new();
                h.write_into(&mut buf);
                buf.extend_from_slice(&payload);
                writer
                    .lock()
                    .await
                    .write_all(&buf)
                    .await
                    .map_err(PvaError::Io)?;
                handshake_complete = true;
                continue;
            } else {
                // Some clients send CREATE_CHANNEL right after SET_BYTE_ORDER
                // skipping a fresh CONNECTION_VALIDATION exchange — accept.
                handshake_complete = true;
            }
        }

        // Application messages
        match Command::from_code(frame.header.command) {
            Some(Command::CreateChannel) => {
                if channels.len() >= config.max_channels_per_connection {
                    warn!(
                        ?peer,
                        "rejecting CREATE_CHANNEL: per-connection limit reached"
                    );
                    // Reject by sending an error CreateChannel response with cid=u32::MAX.
                    let mut payload = Vec::new();
                    payload.put_u32(u32::MAX, order);
                    payload.put_u32(0u32, order);
                    Status::error("max channels per connection reached".to_string())
                        .write_into(order, &mut payload);
                    let h = PvaHeader::application(
                        true,
                        order,
                        Command::CreateChannel.code(),
                        payload.len() as u32,
                    );
                    let mut buf = Vec::new();
                    h.write_into(&mut buf);
                    buf.extend_from_slice(&payload);
                    writer
                        .lock()
                        .await
                        .write_all(&buf)
                        .await
                        .map_err(PvaError::Io)?;
                    continue;
                }
                handle_create_channel(&source, &frame, &writer, &mut channels, order).await?;
            }
            Some(Command::DestroyChannel) => {
                handle_destroy_channel(&frame, &writer, &mut channels, order).await?;
            }
            Some(Command::Get) => {
                handle_op(
                    &source,
                    &frame,
                    &writer,
                    &mut channels,
                    order,
                    OpKind::Get,
                    &config,
                )
                .await?;
            }
            Some(Command::Put) => {
                handle_op(
                    &source,
                    &frame,
                    &writer,
                    &mut channels,
                    order,
                    OpKind::Put,
                    &config,
                )
                .await?;
            }
            Some(Command::Monitor) => {
                handle_op(
                    &source,
                    &frame,
                    &writer,
                    &mut channels,
                    order,
                    OpKind::Monitor,
                    &config,
                )
                .await?;
            }
            Some(Command::Rpc) => {
                handle_op(
                    &source,
                    &frame,
                    &writer,
                    &mut channels,
                    order,
                    OpKind::Rpc,
                    &config,
                )
                .await?;
            }
            Some(Command::GetField) => {
                handle_get_field(&source, &frame, &writer, &channels, order).await?;
            }
            Some(Command::DestroyRequest) => {
                handle_destroy_request(&frame, &mut channels, order);
            }
            Some(Command::Echo) => {
                // Echo back the same frame.
                let mut buf = Vec::new();
                let h = PvaHeader::application(
                    true,
                    order,
                    Command::Echo.code(),
                    frame.payload.len() as u32,
                );
                h.write_into(&mut buf);
                buf.extend_from_slice(&frame.payload);
                writer
                    .lock()
                    .await
                    .write_all(&buf)
                    .await
                    .map_err(PvaError::Io)?;
            }
            _ => {
                // Unhandled — keep going.
            }
        }
    }
}

async fn read_frame<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    rx_buf: &mut Vec<u8>,
    op_timeout: Duration,
) -> PvaResult<Frame> {
    loop {
        if let Some((frame, n)) = try_parse_frame(rx_buf)? {
            rx_buf.drain(..n);
            return Ok(frame);
        }
        let mut chunk = [0u8; 4096];
        let n = match tokio::time::timeout(op_timeout, reader.read(&mut chunk)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(PvaError::Io(e)),
            Err(_) => return Err(PvaError::Timeout),
        };
        if n == 0 {
            return Err(PvaError::Protocol("client closed".into()));
        }
        rx_buf.extend_from_slice(&chunk[..n]);
    }
}

/// Build a server-side CONNECTION_VALIDATION request (cmd=1, server direction).
///
/// Wire layout (8-byte header + this payload):
///
/// ```text
/// u32 buffer_size
/// u16 introspection_registry_size
/// Size n
/// n × String   (auth method names)
/// ```
fn build_server_connection_validation(
    order: ByteOrder,
    buffer_size: u32,
    registry_size: u16,
    auth_methods: &[&str],
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.put_u32(buffer_size, order);
    payload.put_u16(registry_size, order);
    encode_size_into(auth_methods.len() as u32, order, &mut payload);
    for m in auth_methods {
        encode_string_into(m, order, &mut payload);
    }
    let h = PvaHeader::application(
        true,
        order,
        Command::ConnectionValidation.code(),
        payload.len() as u32,
    );
    let mut out = Vec::new();
    h.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

async fn handle_create_channel(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<SrvWrite>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let _count = cur
        .get_u16(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let cid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let name = crate::proto::decode_string(&mut cur, order)
        .map_err(|e| PvaError::Decode(e.to_string()))?
        .unwrap_or_default();

    if !source.has_pv(&name).await {
        let mut payload = Vec::new();
        payload.put_u32(cid, order);
        payload.put_u32(0u32, order); // sid (placeholder)
        Status::error(format!("unknown PV: {name}")).write_into(order, &mut payload);
        let h = PvaHeader::application(
            true,
            order,
            Command::CreateChannel.code(),
            payload.len() as u32,
        );
        let mut buf = Vec::new();
        h.write_into(&mut buf);
        buf.extend_from_slice(&payload);
        writer
            .lock()
            .await
            .write_all(&buf)
            .await
            .map_err(PvaError::Io)?;
        return Ok(());
    }

    let sid = alloc_sid();
    let intro = source.get_introspection(&name).await;
    channels.insert(
        sid,
        ChannelState {
            name: name.clone(),
            cid,
            sid,
            introspection: intro,
            ops: HashMap::new(),
        },
    );

    let mut payload = Vec::new();
    payload.put_u32(cid, order);
    payload.put_u32(sid, order);
    Status::ok().write_into(order, &mut payload);
    // pvxs serverchan.cpp:349-351 emits `cid + sid + status` only —
    // no access_rights field follows.
    let h = PvaHeader::application(
        true,
        order,
        Command::CreateChannel.code(),
        payload.len() as u32,
    );
    let mut buf = Vec::new();
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    writer
        .lock()
        .await
        .write_all(&buf)
        .await
        .map_err(PvaError::Io)?;
    Ok(())
}

async fn handle_destroy_channel(
    frame: &Frame,
    writer: &Arc<Mutex<SrvWrite>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let cid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    // Removing the channel drops every OpState in `ops`, which drops
    // each `monitor_abort: Option<Arc<AbortOnDrop>>` and cancels the
    // associated subscriber task — preventing orphaned spawns from
    // holding the source's broadcast subscription.
    channels.remove(&sid);
    let mut payload = Vec::new();
    payload.put_u32(sid, order);
    payload.put_u32(cid, order);
    let h = PvaHeader::application(
        true,
        order,
        Command::DestroyChannel.code(),
        payload.len() as u32,
    );
    let mut buf = Vec::new();
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    writer
        .lock()
        .await
        .write_all(&buf)
        .await
        .map_err(PvaError::Io)?;
    Ok(())
}

fn handle_destroy_request(
    frame: &Frame,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
) {
    let mut cur = frame.cursor();
    let Ok(sid) = cur.get_u32(order) else { return };
    let Ok(ioid) = cur.get_u32(order) else { return };
    if let Some(ch) = channels.get_mut(&sid) {
        // Removing the op drops `monitor_abort: Option<Arc<AbortOnDrop>>`.
        // Once the last clone is dropped, the subscriber task aborts.
        ch.ops.remove(&ioid);
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_op(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<SrvWrite>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
    kind: OpKind,
    config: &PvaServerConfig,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let ioid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let subcmd = cur.get_u8().map_err(|e| PvaError::Decode(e.to_string()))?;

    let ch = match channels.get_mut(&sid) {
        Some(c) => c,
        None => {
            // Send error.
            send_op_error(writer, kind, ioid, "unknown channel sid", order).await?;
            return Ok(());
        }
    };

    if subcmd & 0x08 != 0 {
        // INIT — read pvRequest (we ignore filtering details; we always send
        // back the full introspection).
        let intro = ch.introspection.clone().unwrap_or(FieldDesc::Variant);

        // Drain pvRequest bytes (decode just to advance cursor — ignore result).
        let _ = decode_type_desc(&mut cur, order);

        ch.ops.insert(
            ioid,
            OpState {
                intro: intro.clone(),
                kind,
                monitor_started: false,
                monitor_abort: None,
            },
        );

        // Build INIT response: ioid + subcmd + status + introspection
        let cmd = match kind {
            OpKind::Get => Command::Get,
            OpKind::Put => Command::Put,
            OpKind::Monitor => Command::Monitor,
            OpKind::Rpc => Command::Rpc,
        };

        let mut payload = Vec::new();
        payload.put_u32(ioid, order);
        payload.put_u8(subcmd);
        Status::ok().write_into(order, &mut payload);
        // RPC INIT carries no type descriptor (pvxs serverget.cpp:97 —
        // `if (cmd != CMD_RPC) to_wire(R, type)`). For GET/PUT/MONITOR,
        // emit the introspection inline (no 0xFD/0xFE cache markers) so
        // pvAccessCPP clients can parse it.
        if !matches!(kind, OpKind::Rpc) {
            encode_type_desc(&intro, order, &mut payload);
        }
        let h = PvaHeader::application(true, order, cmd.code(), payload.len() as u32);
        let mut buf = Vec::new();
        h.write_into(&mut buf);
        buf.extend_from_slice(&payload);
        writer
            .lock()
            .await
            .write_all(&buf)
            .await
            .map_err(PvaError::Io)?;
        return Ok(());
    }

    // Data phase
    let op = ch.ops.get(&ioid).cloned();
    let intro = match op {
        Some(o) => o.intro,
        None => {
            send_op_error(writer, kind, ioid, "operation not initialised", order).await?;
            return Ok(());
        }
    };

    match kind {
        OpKind::Get => {
            let value = match source.get_value(&ch.name).await {
                Some(v) => v,
                None => {
                    send_op_error(writer, OpKind::Get, ioid, "PV not found", order).await?;
                    return Ok(());
                }
            };
            let mut payload = Vec::new();
            payload.put_u32(ioid, order);
            payload.put_u8(0x00);
            Status::ok().write_into(order, &mut payload);
            // Bitset = all bits set (full snapshot).
            let bits = BitSet::all_set(intro.total_bits());
            bits.write_into(order, &mut payload);
            encode_pv_field(&value, &intro, order, &mut payload);
            let h = PvaHeader::application(true, order, Command::Get.code(), payload.len() as u32);
            let mut buf = Vec::new();
            h.write_into(&mut buf);
            buf.extend_from_slice(&payload);
            writer
                .lock()
                .await
                .write_all(&buf)
                .await
                .map_err(PvaError::Io)?;
        }
        OpKind::Put => {
            // Read bitset (which fields client is putting) + value.
            let _changed =
                BitSet::decode(&mut cur, order).map_err(|e| PvaError::Decode(e.to_string()))?;
            let value = decode_pv_field(&intro, &mut cur, order)
                .map_err(|e| PvaError::Decode(e.to_string()))?;
            let pv_name = ch.name.clone();
            let result = source.put_value(&pv_name, value).await;

            let mut payload = Vec::new();
            payload.put_u32(ioid, order);
            payload.put_u8(0x00);
            match result {
                Ok(()) => {
                    Status::ok().write_into(order, &mut payload);
                    // PUT_GET (subcmd bit 0x40 set on the request): client
                    // wants the post-put value back. Per pvxs serverget.cpp:103
                    // the response carries `bitset + partial value` after the
                    // status.
                    if subcmd & 0x40 != 0 {
                        if let Some(v) = source.get_value(&pv_name).await {
                            let bits = BitSet::all_set(intro.total_bits());
                            bits.write_into(order, &mut payload);
                            encode_pv_field(&v, &intro, order, &mut payload);
                        }
                    }
                }
                Err(msg) => Status::error(msg).write_into(order, &mut payload),
            }
            let h = PvaHeader::application(true, order, Command::Put.code(), payload.len() as u32);
            let mut buf = Vec::new();
            h.write_into(&mut buf);
            buf.extend_from_slice(&payload);
            writer
                .lock()
                .await
                .write_all(&buf)
                .await
                .map_err(PvaError::Io)?;
        }
        OpKind::Monitor => {
            // MONITOR_START / pipeline-ack uses subcmd 0x40 in the
            // formal spec, 0x80 in pvxs/spvirit byte-parity (legacy
            // "pipeline" subcmd). Either signals "start emitting / ack
            // window". Also treat plain 0x00 as "start" for compatibility.
            let is_start_or_ack = subcmd & 0x40 != 0 || subcmd & 0x80 != 0 || subcmd == 0x00;
            // Only spawn the subscriber task once per ioid.
            let already_running = ch
                .ops
                .get(&ioid)
                .map(|s| s.monitor_started)
                .unwrap_or(false);
            if is_start_or_ack && !already_running {
                let pv_name = ch.name.clone();
                let intro_clone = intro.clone();
                let writer_clone = writer.clone();
                let src = source.clone();
                let queue_depth = config.monitor_queue_depth;
                let join = tokio::spawn(async move {
                    let Some(mut rx) = src.subscribe(&pv_name).await else {
                        return;
                    };
                    // Emit initial snapshot.
                    if let Some(initial) = src.get_value(&pv_name).await {
                        let payload = build_monitor_payload(ioid, &intro_clone, &initial, order);
                        if writer_clone.lock().await.write_all(&payload).await.is_err() {
                            return;
                        }
                    }
                    // Back-pressure / squashing loop: drain available
                    // events between writes, keeping only the most recent
                    // value if more than `queue_depth` events stack up.
                    let mut squashing = false;
                    while let Some(mut value) = rx.recv().await {
                        // Drain extras; keep the latest.
                        let mut squashed = 0usize;
                        loop {
                            match rx.try_recv() {
                                Ok(next) => {
                                    value = next;
                                    squashed += 1;
                                    if squashed > queue_depth {
                                        squashing = true;
                                    }
                                }
                                Err(mpsc::error::TryRecvError::Empty) => break,
                                Err(mpsc::error::TryRecvError::Disconnected) => break,
                            }
                        }
                        if squashing {
                            debug!(pv = %pv_name, squashed, "monitor squashed events");
                            squashing = false;
                        }
                        let payload = build_monitor_payload(ioid, &intro_clone, &value, order);
                        if writer_clone.lock().await.write_all(&payload).await.is_err() {
                            return;
                        }
                    }
                    // Source closed — emit MONITOR FINISH (subcmd 0x10 + Status).
                    // pvxs servermon.cpp:148-178 sends a final frame with
                    // subcmd=0x10 to signal end-of-stream so the client can
                    // tear down cleanly.
                    let finish = build_monitor_finish(ioid, order);
                    let _ = writer_clone.lock().await.write_all(&finish).await;
                });
                if let Some(s) = ch.ops.get_mut(&ioid) {
                    s.monitor_started = true;
                    s.monitor_abort = Some(Arc::new(AbortOnDrop(join.abort_handle())));
                }
            }
        }
        OpKind::Rpc => {
            // RPC DATA request from client: `type(arg) + full_value(arg)`.
            // pvxs clientget.cpp:307-311 — `to_wire(R, type); to_wire_full(R, arg)`.
            // The introspection on the channel was negotiated for the
            // *pvRequest* in INIT, not the actual call argument, so we must
            // decode the argument's own type descriptor here.
            let (req_desc, req_value) = match decode_type_desc(&mut cur, order) {
                Ok(desc) => match decode_pv_field(&desc, &mut cur, order) {
                    Ok(v) => (desc, v),
                    Err(_) => (desc, PvField::Null),
                },
                Err(_) => {
                    // Empty body — some clients send parameterless RPCs with
                    // no payload after subcmd.
                    (FieldDesc::Variant, PvField::Null)
                }
            };
            let pv_name = ch.name.clone();
            let _ = intro; // INIT pvRequest descriptor — no longer used here.
            let result = source.rpc(&pv_name, req_desc, req_value).await;

            let mut payload = Vec::new();
            payload.put_u32(ioid, order);
            payload.put_u8(0x00);
            match result {
                Ok((resp_desc, resp_value)) => {
                    Status::ok().write_into(order, &mut payload);
                    encode_type_desc(&resp_desc, order, &mut payload);
                    encode_pv_field(&resp_value, &resp_desc, order, &mut payload);
                }
                Err(msg) => Status::error(msg).write_into(order, &mut payload),
            }
            let h = PvaHeader::application(true, order, Command::Rpc.code(), payload.len() as u32);
            let mut buf = Vec::new();
            h.write_into(&mut buf);
            buf.extend_from_slice(&payload);
            writer
                .lock()
                .await
                .write_all(&buf)
                .await
                .map_err(PvaError::Io)?;
        }
    }
    Ok(())
}

async fn handle_get_field(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<SrvWrite>>,
    channels: &HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let ioid = cur
        .get_u32(order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;
    let _sub = crate::proto::decode_string(&mut cur, order)
        .map_err(|e| PvaError::Decode(e.to_string()))?;

    let intro = match channels.get(&sid).and_then(|c| c.introspection.clone()) {
        Some(d) => d,
        None => {
            // Try to resolve by name lookup
            let name_opt = channels.get(&sid).map(|c| c.name.clone());
            if let Some(name) = name_opt {
                source
                    .get_introspection(&name)
                    .await
                    .unwrap_or(FieldDesc::Variant)
            } else {
                FieldDesc::Variant
            }
        }
    };

    let mut payload = Vec::new();
    payload.put_u32(ioid, order);
    Status::ok().write_into(order, &mut payload);
    encode_type_desc(&intro, order, &mut payload);
    let h = PvaHeader::application(true, order, Command::GetField.code(), payload.len() as u32);
    let mut buf = Vec::new();
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    writer
        .lock()
        .await
        .write_all(&buf)
        .await
        .map_err(PvaError::Io)?;
    Ok(())
}

async fn send_op_error(
    writer: &Arc<Mutex<SrvWrite>>,
    kind: OpKind,
    ioid: u32,
    msg: &str,
    order: ByteOrder,
) -> PvaResult<()> {
    let cmd = match kind {
        OpKind::Get => Command::Get,
        OpKind::Put => Command::Put,
        OpKind::Monitor => Command::Monitor,
        OpKind::Rpc => Command::Rpc,
    };
    let mut payload = Vec::new();
    payload.put_u32(ioid, order);
    payload.put_u8(0x08); // INIT phase err
    Status::error(msg.to_string()).write_into(order, &mut payload);
    let h = PvaHeader::application(true, order, cmd.code(), payload.len() as u32);
    let mut buf = Vec::new();
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    writer
        .lock()
        .await
        .write_all(&buf)
        .await
        .map_err(PvaError::Io)?;
    Ok(())
}

#[allow(unused_imports)]
use crate::proto::ReadExt;
const _: u8 = PVA_VERSION;

/// Build a complete MONITOR data frame (header + payload) for a single value
/// emission. Pulled out so the back-pressure squashing loop can call it.
fn build_monitor_payload(
    ioid: u32,
    intro: &FieldDesc,
    value: &PvField,
    order: ByteOrder,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.put_u32(ioid, order);
    payload.put_u8(0x00);
    // PVA monitor data: changed bitset + partial value + overrun bitset.
    let changed = BitSet::all_set(intro.total_bits());
    changed.write_into(order, &mut payload);
    encode_pv_field(value, intro, order, &mut payload);
    let overrun = BitSet::new(); // no overruns
    overrun.write_into(order, &mut payload);
    let h = PvaHeader::application(true, order, Command::Monitor.code(), payload.len() as u32);
    let mut buf = Vec::with_capacity(8 + payload.len());
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    buf
}

/// Build a MONITOR FINISH frame (subcmd `0x10` + Status). Sent when the
/// underlying source closes its broadcast channel, signalling end-of-stream
/// to the subscribing client. Mirrors pvxs `servermon.cpp:148-178`.
fn build_monitor_finish(ioid: u32, order: ByteOrder) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.put_u32(ioid, order);
    payload.put_u8(0x10);
    Status::ok().write_into(order, &mut payload);
    let h = PvaHeader::application(true, order, Command::Monitor.code(), payload.len() as u32);
    let mut buf = Vec::with_capacity(8 + payload.len());
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    buf
}

fn now_nanos() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client_native::decode::{OpResponse, decode_op_response, try_parse_frame};
    use crate::pvdata::{PvStructure, ScalarType, ScalarValue};

    #[test]
    fn monitor_payload_orders_overrun_after_value() {
        let order = ByteOrder::Little;
        let ioid = 0x1234;
        let intro = FieldDesc::Structure {
            struct_id: "epics:nt/NTScalar:1.0".into(),
            fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Double))],
        };
        let mut value = PvStructure::new("epics:nt/NTScalar:1.0");
        value
            .fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Double(42.5))));

        let bytes = build_monitor_payload(ioid, &intro, &PvField::Structure(value), order);
        let (frame, used) = try_parse_frame(&bytes).unwrap().expect("complete frame");
        assert_eq!(used, bytes.len());

        match decode_op_response(&frame, Some(&intro)).unwrap() {
            OpResponse::Data(data) => {
                assert_eq!(data.ioid, ioid);
                match data.value {
                    PvField::Structure(s) => {
                        assert_eq!(
                            s.get_field("value"),
                            Some(&PvField::Scalar(ScalarValue::Double(42.5)))
                        );
                    }
                    other => panic!("expected structure, got {other:?}"),
                }
            }
            other => panic!("expected monitor data, got {other:?}"),
        }
    }
}
