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
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::client_native::decode::{try_parse_frame, Frame};
use crate::error::{PvaError, PvaResult};
use crate::proto::{
    encode_size_into, encode_string_into, BitSet, ByteOrder, Command, ControlCommand, PvaHeader,
    Status, WriteExt, PVA_VERSION,
};
use crate::pvdata::encode::{decode_pv_field, decode_type_desc, encode_pv_field, encode_type_desc};
use crate::pvdata::{FieldDesc, PvField};

use super::source::DynSource;

static NEXT_SID: AtomicU32 = AtomicU32::new(1);
fn alloc_sid() -> u32 {
    NEXT_SID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
struct ChannelState {
    name: String,
    cid: u32,
    sid: u32,
    introspection: Option<FieldDesc>,
    /// ioid → (introspection negotiated for this op, kind)
    ops: HashMap<u32, OpState>,
}

#[derive(Debug, Clone)]
struct OpState {
    intro: FieldDesc,
    kind: OpKind,
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
    op_timeout: Duration,
) -> PvaResult<()> {
    let listener = TcpListener::bind(bind_addr).await.map_err(PvaError::Io)?;
    debug!(?bind_addr, "TCP listener up");
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let src = source.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(src, stream, peer, op_timeout).await {
                        debug!(?peer, "connection ended: {e}");
                    }
                });
            }
            Err(e) => {
                error!("accept error: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
}

async fn handle_connection(
    source: DynSource,
    stream: TcpStream,
    peer: SocketAddr,
    op_timeout: Duration,
) -> PvaResult<()> {
    stream.set_nodelay(true).ok();
    let (mut reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(writer));

    let order = ByteOrder::Little;

    // Step 1: send SET_BYTE_ORDER (control message). Per pvxs, the byte order
    // we want to use is encoded in the control header's flag bit 7.
    let set_bo = {
        let mut buf = Vec::with_capacity(8);
        // Control message; flags include byte-order bit.
        let h = PvaHeader::control(true, order, ControlCommand::SetByteOrder.code(), 0);
        h.write_into(&mut buf);
        buf
    };
    writer.lock().await.write_all(&set_bo).await.map_err(PvaError::Io)?;

    // Step 2: send CONNECTION_VALIDATION request (server → client).
    let val_req = build_server_connection_validation(order, 87_040, 32_767, &["ca", "anonymous"]);
    writer.lock().await.write_all(&val_req).await.map_err(PvaError::Io)?;

    // Step 3+: drive the read loop.
    let mut rx_buf: Vec<u8> = Vec::with_capacity(8192);
    let mut channels: HashMap<u32, ChannelState> = HashMap::new();
    let mut handshake_complete = false;
    let _peer = peer;

    loop {
        let frame = read_frame(&mut reader, &mut rx_buf, op_timeout).await?;
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
                writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
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
                writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
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
                writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
            }
            _ => {
                // Unhandled — keep going.
            }
        }
    }
}

async fn read_frame(
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
    let h = PvaHeader::application(true, order, Command::ConnectionValidation.code(), payload.len() as u32);
    let mut out = Vec::new();
    h.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

async fn handle_create_channel(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let _count = cur.get_u16(order).map_err(|e| PvaError::Decode(e.to_string()))?;
    let cid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
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
        writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
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
    // Access rights byte (omitted — pvxs accepts none).
    let h =
        PvaHeader::application(true, order, Command::CreateChannel.code(), payload.len() as u32);
    let mut buf = Vec::new();
    h.write_into(&mut buf);
    buf.extend_from_slice(&payload);
    writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
    Ok(())
}

async fn handle_destroy_channel(
    frame: &Frame,
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
    let cid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
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
    writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
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
        ch.ops.remove(&ioid);
    }
}

async fn handle_op(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    channels: &mut HashMap<u32, ChannelState>,
    order: ByteOrder,
    kind: OpKind,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
    let ioid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
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
        let intro = ch
            .introspection
            .clone()
            .or_else(|| {
                // Sometimes intro wasn't fetched yet; resolve lazily.
                None
            })
            .unwrap_or_else(|| FieldDesc::Variant);

        // Drain pvRequest bytes (decode just to advance cursor — ignore result).
        let _ = decode_type_desc(&mut cur, order);

        ch.ops.insert(
            ioid,
            OpState {
                intro: intro.clone(),
                kind,
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
        encode_type_desc(&intro, order, &mut payload);
        let h = PvaHeader::application(true, order, cmd.code(), payload.len() as u32);
        let mut buf = Vec::new();
        h.write_into(&mut buf);
        buf.extend_from_slice(&payload);
        writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
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
            writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
        }
        OpKind::Put => {
            // Read bitset (which fields client is putting) + value.
            let _changed = BitSet::decode(&mut cur, order)
                .map_err(|e| PvaError::Decode(e.to_string()))?;
            let value = decode_pv_field(&intro, &mut cur, order)
                .map_err(|e| PvaError::Decode(e.to_string()))?;
            let pv_name = ch.name.clone();
            let result = source.put_value(&pv_name, value).await;

            let mut payload = Vec::new();
            payload.put_u32(ioid, order);
            payload.put_u8(0x00);
            match result {
                Ok(()) => Status::ok().write_into(order, &mut payload),
                Err(msg) => Status::error(msg).write_into(order, &mut payload),
            }
            let h = PvaHeader::application(true, order, Command::Put.code(), payload.len() as u32);
            let mut buf = Vec::new();
            h.write_into(&mut buf);
            buf.extend_from_slice(&payload);
            writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
        }
        OpKind::Monitor => {
            // subcmd 0x40 means MONITOR_START; 0x80 means STOP/PIPELINE_ACK.
            // For simplicity we handle START by spawning a subscriber task.
            if subcmd & 0x40 != 0 || subcmd == 0x00 {
                let pv_name = ch.name.clone();
                let intro_clone = intro.clone();
                let writer_clone = writer.clone();
                let src = source.clone();
                tokio::spawn(async move {
                    let Some(mut rx) = src.subscribe(&pv_name).await else {
                        return;
                    };
                    // First event: full snapshot if available.
                    if let Some(initial) = src.get_value(&pv_name).await {
                        let mut payload = Vec::new();
                        payload.put_u32(ioid, order);
                        payload.put_u8(0x00);
                        let bits = BitSet::all_set(intro_clone.total_bits());
                        bits.write_into(order, &mut payload);
                        encode_pv_field(&initial, &intro_clone, order, &mut payload);
                        let h = PvaHeader::application(
                            true,
                            order,
                            Command::Monitor.code(),
                            payload.len() as u32,
                        );
                        let mut buf = Vec::new();
                        h.write_into(&mut buf);
                        buf.extend_from_slice(&payload);
                        let _ = writer_clone.lock().await.write_all(&buf).await;
                    }
                    while let Some(value) = rx.recv().await {
                        let mut payload = Vec::new();
                        payload.put_u32(ioid, order);
                        payload.put_u8(0x00);
                        let bits = BitSet::all_set(intro_clone.total_bits());
                        bits.write_into(order, &mut payload);
                        encode_pv_field(&value, &intro_clone, order, &mut payload);
                        let h = PvaHeader::application(
                            true,
                            order,
                            Command::Monitor.code(),
                            payload.len() as u32,
                        );
                        let mut buf = Vec::new();
                        h.write_into(&mut buf);
                        buf.extend_from_slice(&payload);
                        if writer_clone.lock().await.write_all(&buf).await.is_err() {
                            break;
                        }
                    }
                });
            }
        }
        OpKind::Rpc => {
            // Decode request value using the introspection from INIT.
            let value = match decode_pv_field(&intro, &mut cur, order) {
                Ok(v) => v,
                Err(_) => {
                    // Fall back to a null value if the body is empty (some
                    // legacy clients don't send a value bodies for parameter-
                    // less RPCs).
                    PvField::Null
                }
            };
            let pv_name = ch.name.clone();
            let intro_clone = intro.clone();
            let result = source.rpc(&pv_name, intro_clone, value).await;

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
            writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
        }
    }
    Ok(())
}

async fn handle_get_field(
    source: &DynSource,
    frame: &Frame,
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
    channels: &HashMap<u32, ChannelState>,
    order: ByteOrder,
) -> PvaResult<()> {
    let mut cur = frame.cursor();
    let sid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
    let ioid = cur.get_u32(order).map_err(|e| PvaError::Decode(e.to_string()))?;
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
    writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
    Ok(())
}

async fn send_op_error(
    writer: &Arc<Mutex<tokio::net::tcp::OwnedWriteHalf>>,
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
    writer.lock().await.write_all(&buf).await.map_err(PvaError::Io)?;
    Ok(())
}

#[allow(unused_imports)]
use crate::proto::ReadExt;
const _: u8 = PVA_VERSION;
