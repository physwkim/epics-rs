//! Channel-aware ops with automatic reconnect.
//!
//! These replace the older `ops::*` functions which operated on a one-shot
//! `Connection` with no reconnect logic. The v2 versions take a
//! [`Channel`] and:
//!
//! - GET / PUT / RPC: a single attempt; if the connection dies mid-op the
//!   error bubbles up and the caller decides whether to retry. (Idempotent
//!   ops like GET could in principle be auto-retried, but pvxs prefers to
//!   surface the error so the user can decide.)
//! - MONITOR: re-issues INIT + START on every reconnect transparently. The
//!   `callback` continues firing as long as the channel isn't closed.
//!
//! Pipeline flow control: if `pipeline_size > 0`, the client periodically
//! sends MONITOR_ACK (subcmd `0x80`) to keep the server's send window
//! open. Default is 4 — every 4 events, ack 4. This matches pvxs's
//! behaviour for pipelined monitors.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;

use crate::codec::PvaCodec;
use crate::error::{PvaError, PvaResult};
use crate::proto::{BitSet, ByteOrder, Command, PvaHeader, QosFlags, WriteExt};
use crate::pv_request::{build_pv_request_fields, build_pv_request_value_only};
use crate::pvdata::encode::{decode_pv_field, encode_pv_field, encode_type_desc};
use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarValue};

use super::channel::Channel;
use super::decode::{decode_op_response, OpResponse};

static NEXT_IOID: AtomicU32 = AtomicU32::new(1);
fn alloc_ioid() -> u32 {
    NEXT_IOID.fetch_add(1, Ordering::Relaxed)
}

/// Default pipeline window for monitors. Tuned to match pvxs.
pub const DEFAULT_PIPELINE_SIZE: u32 = 4;

// ── GET ────────────────────────────────────────────────────────────────

pub async fn op_get(
    channel: &Arc<Channel>,
    fields: &[&str],
    op_timeout: Duration,
) -> PvaResult<(FieldDesc, PvField)> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = if fields.is_empty() {
        sentinel_all_fields()
    } else {
        build_pv_request_fields(fields, big_endian)
    };

    let mut stream = server.register_ioid_stream(ioid);

    // INIT
    let init_req = codec.build_get_init(sid, ioid, &pv_req);
    server.send(init_req).await?;
    let init_frame = await_frame(&mut stream, op_timeout).await?;
    let init = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            server.unregister_ioid(ioid);
            return Err(PvaError::Protocol(format!(
                "expected GET INIT, got {other:?}"
            )));
        }
    };
    if !init.status.is_success() {
        server.unregister_ioid(ioid);
        return Err(PvaError::Protocol(format!(
            "GET INIT failed: {:?}",
            init.status
        )));
    }
    let intro = init.introspection;

    // DATA
    let data_req = codec.build_get(sid, ioid);
    server.send(data_req).await?;
    let data_frame = await_frame(&mut stream, op_timeout).await?;
    let result = match decode_op_response(&data_frame, Some(&intro))? {
        OpResponse::Data(d) => {
            if d.status.is_success() {
                Ok((intro, d.value))
            } else {
                Err(PvaError::Protocol(format!("GET data: {:?}", d.status)))
            }
        }
        other => Err(PvaError::Protocol(format!(
            "expected GET data, got {other:?}"
        ))),
    };

    // Best-effort cleanup
    let destroy = codec.build_destroy_request(sid, ioid);
    let _ = server.send(destroy).await;
    server.unregister_ioid(ioid);
    result
}

// ── PUT ────────────────────────────────────────────────────────────────

pub async fn op_put(
    channel: &Arc<Channel>,
    value_str: &str,
    op_timeout: Duration,
) -> PvaResult<()> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = build_pv_request_value_only(big_endian);
    let mut stream = server.register_ioid_stream(ioid);

    // INIT
    let init_req = codec.build_put_init(sid, ioid, &pv_req);
    server.send(init_req).await?;
    let init_frame = await_frame(&mut stream, op_timeout).await?;
    let init = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            server.unregister_ioid(ioid);
            return Err(PvaError::Protocol(format!(
                "expected PUT INIT, got {other:?}"
            )));
        }
    };
    if !init.status.is_success() {
        server.unregister_ioid(ioid);
        return Err(PvaError::Protocol(format!(
            "PUT INIT failed: {:?}",
            init.status
        )));
    }
    let intro = init.introspection;

    // Build value matching introspection.
    let value = build_put_value(&intro, value_str)?;

    // DATA
    let mut payload = Vec::new();
    payload.put_u32(sid, order);
    payload.put_u32(ioid, order);
    payload.put_u8(0x00);
    let mut changed = BitSet::new();
    if let Some(bit) = intro.bit_for_path("value") {
        changed.set(bit);
    } else {
        changed.set(0);
    }
    changed.write_into(order, &mut payload);
    encode_pv_field(&value, &intro, order, &mut payload);
    let header = PvaHeader::application(false, order, Command::Put.code(), payload.len() as u32);
    let mut frame = Vec::new();
    header.write_into(&mut frame);
    frame.extend_from_slice(&payload);
    server.send(frame).await?;

    let done_frame = await_frame(&mut stream, op_timeout).await?;
    let result = match decode_op_response(&done_frame, Some(&intro))? {
        OpResponse::Status(s) => {
            if s.status.is_success() {
                Ok(())
            } else {
                Err(PvaError::Protocol(format!("PUT failed: {:?}", s.status)))
            }
        }
        other => Err(PvaError::Protocol(format!(
            "expected PUT done, got {other:?}"
        ))),
    };

    let destroy = codec.build_destroy_request(sid, ioid);
    let _ = server.send(destroy).await;
    server.unregister_ioid(ioid);
    result
}

// ── MONITOR (with reconnect) ───────────────────────────────────────────

pub async fn op_monitor<F>(
    channel: &Arc<Channel>,
    fields: &[&str],
    pipeline_size: u32,
    mut callback: F,
) -> PvaResult<()>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    let fields_owned: Vec<String> = fields.iter().map(|s| s.to_string()).collect();

    loop {
        let (server, sid) = channel.ensure_active().await?;
        match run_monitor_loop(
            server.clone(),
            sid,
            &fields_owned,
            pipeline_size,
            &mut callback,
        )
        .await
        {
            Ok(()) => return Ok(()), // monitor finished cleanly (server sent FINISH)
            Err(MonitorEnd::ChannelClosed) => return Ok(()),
            Err(MonitorEnd::ConnectionLost) => {
                // Wait for channel to either die-permanently or reconnect.
                debug!(pv_name = %channel.pv_name, "monitor lost connection; waiting for reconnect");
                channel.wait_until_inactive().await;
                if matches!(channel.current_state(), super::channel::ChannelState::Closed) {
                    return Ok(());
                }
                // Loop will call ensure_active() again.
            }
            Err(MonitorEnd::Fatal(e)) => return Err(e),
        }
    }
}

#[derive(Debug)]
enum MonitorEnd {
    ChannelClosed,
    ConnectionLost,
    Fatal(PvaError),
}

async fn run_monitor_loop<F>(
    server: Arc<super::server_conn::ServerConn>,
    sid: u32,
    fields: &[String],
    pipeline_size: u32,
    callback: &mut F,
) -> Result<(), MonitorEnd>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = if fields.is_empty() {
        sentinel_all_fields()
    } else {
        let refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        build_pv_request_fields(&refs, big_endian)
    };

    let mut stream = server.register_ioid_stream(ioid);

    // INIT
    let init_req = codec.build_monitor_init(sid, ioid, &pv_req);
    server
        .send(init_req)
        .await
        .map_err(|_| MonitorEnd::ConnectionLost)?;
    let init_frame = stream
        .recv()
        .await
        .ok_or(MonitorEnd::ConnectionLost)?;
    let init = match decode_op_response(&init_frame, None) {
        Ok(OpResponse::Init(i)) => i,
        Ok(other) => {
            server.unregister_ioid(ioid);
            return Err(MonitorEnd::Fatal(PvaError::Protocol(format!(
                "expected MONITOR INIT, got {other:?}"
            ))));
        }
        Err(e) => {
            server.unregister_ioid(ioid);
            return Err(MonitorEnd::Fatal(e));
        }
    };
    if !init.status.is_success() {
        server.unregister_ioid(ioid);
        return Err(MonitorEnd::Fatal(PvaError::Protocol(format!(
            "MONITOR INIT failed: {:?}",
            init.status
        ))));
    }
    let intro = init.introspection;

    // START with pipeline ack window.
    let start = codec.build_monitor_start(sid, ioid, pipeline_size);
    server
        .send(start)
        .await
        .map_err(|_| MonitorEnd::ConnectionLost)?;

    let mut events_since_ack: u32 = 0;
    loop {
        let frame = match stream.recv().await {
            Some(f) => f,
            None => {
                server.unregister_ioid(ioid);
                return Err(MonitorEnd::ConnectionLost);
            }
        };
        match decode_op_response(&frame, Some(&intro)) {
            Ok(OpResponse::Data(d)) => {
                callback(&intro, &d.value);
                events_since_ack += 1;
                if pipeline_size > 0 && events_since_ack >= pipeline_size {
                    let ack = codec.build_monitor_start(sid, ioid, events_since_ack);
                    if server.send(ack).await.is_err() {
                        server.unregister_ioid(ioid);
                        return Err(MonitorEnd::ConnectionLost);
                    }
                    events_since_ack = 0;
                }
            }
            Ok(OpResponse::Status(s)) => {
                server.unregister_ioid(ioid);
                if s.status.is_success() {
                    return Ok(());
                } else {
                    return Err(MonitorEnd::Fatal(PvaError::Protocol(format!(
                        "MONITOR error: {:?}",
                        s.status
                    ))));
                }
            }
            Ok(OpResponse::Init(_)) => {
                // Spurious INIT — ignore.
            }
            Err(e) => {
                debug!("MONITOR decode error: {e}");
            }
        }
    }
}

// ── RPC ────────────────────────────────────────────────────────────────

pub async fn op_rpc(
    channel: &Arc<Channel>,
    request_desc: &FieldDesc,
    request_value: &PvField,
    op_timeout: Duration,
) -> PvaResult<(FieldDesc, PvField)> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let mut pv_req = Vec::new();
    encode_type_desc(request_desc, order, &mut pv_req);

    let mut stream = server.register_ioid_stream(ioid);

    // INIT
    let mut init = Vec::with_capacity(9 + pv_req.len());
    init.put_u32(sid, order);
    init.put_u32(ioid, order);
    init.put_u8(QosFlags::INIT);
    init.extend_from_slice(&pv_req);
    let init_h = PvaHeader::application(false, order, Command::Rpc.code(), init.len() as u32);
    let mut init_frame = Vec::with_capacity(8 + init.len());
    init_h.write_into(&mut init_frame);
    init_frame.extend_from_slice(&init);
    server.send(init_frame).await?;

    let init_resp_frame = await_frame(&mut stream, op_timeout).await?;
    let init_resp = match decode_op_response(&init_resp_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            server.unregister_ioid(ioid);
            return Err(PvaError::Protocol(format!(
                "expected RPC INIT, got {other:?}"
            )));
        }
    };
    if !init_resp.status.is_success() {
        server.unregister_ioid(ioid);
        return Err(PvaError::Protocol(format!(
            "RPC INIT: {:?}",
            init_resp.status
        )));
    }
    let response_intro = init_resp.introspection;

    // DATA
    let mut data = Vec::new();
    data.put_u32(sid, order);
    data.put_u32(ioid, order);
    data.put_u8(0x00);
    encode_pv_field(request_value, request_desc, order, &mut data);
    let data_h = PvaHeader::application(false, order, Command::Rpc.code(), data.len() as u32);
    let mut data_frame = Vec::with_capacity(8 + data.len());
    data_h.write_into(&mut data_frame);
    data_frame.extend_from_slice(&data);
    server.send(data_frame).await?;

    let resp_frame = await_frame(&mut stream, op_timeout).await?;
    let result = match decode_op_response(&resp_frame, Some(&response_intro))? {
        OpResponse::Data(d) => {
            if d.status.is_success() {
                Ok((response_intro, d.value))
            } else {
                Err(PvaError::Protocol(format!("RPC: {:?}", d.status)))
            }
        }
        other => Err(PvaError::Protocol(format!(
            "expected RPC data, got {other:?}"
        ))),
    };

    let destroy = codec.build_destroy_request(sid, ioid);
    let _ = server.send(destroy).await;
    server.unregister_ioid(ioid);
    result
}

// ── Helpers ────────────────────────────────────────────────────────────

async fn await_frame(
    stream: &mut mpsc::UnboundedReceiver<super::decode::Frame>,
    op_timeout: Duration,
) -> PvaResult<super::decode::Frame> {
    let frame = timeout(op_timeout, stream.recv())
        .await
        .map_err(|_| PvaError::Timeout)?
        .ok_or_else(|| PvaError::Protocol("connection closed".into()))?;
    Ok(frame)
}

fn sentinel_all_fields() -> Vec<u8> {
    vec![0xFD, 0x02, 0x00, 0x80, 0x00, 0x00]
}

fn build_put_value(desc: &FieldDesc, value_str: &str) -> PvaResult<PvField> {
    match desc {
        FieldDesc::Scalar(st) => ScalarValue::parse(*st, value_str)
            .map(PvField::Scalar)
            .map_err(PvaError::InvalidValue),
        FieldDesc::ScalarArray(st) => {
            let mut items = Vec::new();
            for tok in value_str.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                items.push(ScalarValue::parse(*st, tok).map_err(PvaError::InvalidValue)?);
            }
            Ok(PvField::ScalarArray(items))
        }
        FieldDesc::Structure { fields, struct_id } => {
            let mut s = PvStructure::new(struct_id);
            for (name, child) in fields {
                if name == "value" {
                    s.fields
                        .push((name.clone(), build_put_value(child, value_str)?));
                } else {
                    s.fields.push((
                        name.clone(),
                        crate::pvdata::encode::default_value_for(child),
                    ));
                }
            }
            Ok(PvField::Structure(s))
        }
        _ => Err(PvaError::InvalidValue(format!(
            "PUT not supported for descriptor {desc}"
        ))),
    }
}
