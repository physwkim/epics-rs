//! Channel-level GET / PUT / MONITOR / GET_FIELD operations.

use std::sync::atomic::{AtomicU32, Ordering};

use tracing::debug;

use crate::codec::PvaCodec;
use crate::error::{PvaError, PvaResult};
use crate::proto::{ByteOrder, BitSet, Command, PvaHeader, QosFlags, Status, WriteExt};
use crate::pv_request::{build_pv_request, build_pv_request_value_only};
use crate::pvdata::encode::{decode_pv_field, encode_pv_field};
use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarValue};

use super::conn::Connection;
use super::decode::{decode_create_channel_response, decode_get_field_response, decode_op_response, OpResponse};

static NEXT_IOID: AtomicU32 = AtomicU32::new(1);
static NEXT_CID: AtomicU32 = AtomicU32::new(1);

fn alloc_ioid() -> u32 {
    NEXT_IOID.fetch_add(1, Ordering::Relaxed)
}
fn alloc_cid() -> u32 {
    NEXT_CID.fetch_add(1, Ordering::Relaxed)
}

/// Channel ID assigned by the server during CREATE_CHANNEL. Required for
/// subsequent GET/PUT/MONITOR ops.
#[derive(Debug, Clone, Copy)]
pub struct ChannelIds {
    pub cid: u32,
    pub sid: u32,
}

/// Open a channel by name and wait for CREATE_CHANNEL response.
pub async fn create_channel(conn: &mut Connection, name: &str) -> PvaResult<ChannelIds> {
    let cid = alloc_cid();
    let codec = PvaCodec {
        big_endian: matches!(conn.byte_order, ByteOrder::Big),
    };
    let req = codec.build_create_channel(cid, name);
    conn.send(&req).await?;
    let frame = conn.read_app_frame().await?;
    let resp = decode_create_channel_response(&frame)?;
    if !resp.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "create_channel({name}) failed: {:?}",
            resp.status
        )));
    }
    Ok(ChannelIds { cid: resp.cid, sid: resp.sid })
}

/// Run a GET. Returns the introspection (so callers can format) plus the
/// retrieved value.
pub async fn op_get(
    conn: &mut Connection,
    channel: ChannelIds,
    fields: &[&str],
) -> PvaResult<(FieldDesc, PvField)> {
    let codec = PvaCodec {
        big_endian: matches!(conn.byte_order, ByteOrder::Big),
    };
    let ioid = alloc_ioid();
    let pv_req = if fields.is_empty() {
        // Empty pvRequest selects all fields. pvxs tolerates either an empty
        // sentinel (`0xFD 0x02 0x00 0x80 0x00 0x00`) or our build_pv_request
        // emitting a structure with empty `field` block — the latter matches
        // pvxs `pvget -r ""` exactly.
        build_pv_request_no_fields(matches!(conn.byte_order, ByteOrder::Big))
    } else {
        // Build a structure-shape pvRequest selecting just the requested fields.
        crate::pv_request::build_pv_request_fields(fields, matches!(conn.byte_order, ByteOrder::Big))
    };

    // INIT
    let init_req = codec.build_get_init(channel.sid, ioid, &pv_req);
    conn.send(&init_req).await?;
    let init_frame = conn.read_app_frame().await?;
    let init = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected GET INIT response, got {other:?}"
            )))
        }
    };
    if !init.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "GET INIT failed: {:?}",
            init.status
        )));
    }
    let intro = init.introspection;

    // Data
    let data_req = codec.build_get(channel.sid, ioid);
    conn.send(&data_req).await?;
    let data_frame = conn.read_app_frame().await?;
    let data = match decode_op_response(&data_frame, Some(&intro))? {
        OpResponse::Data(d) => d,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected GET data response, got {other:?}"
            )))
        }
    };
    if !data.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "GET data failed: {:?}",
            data.status
        )));
    }

    // Best-effort: send DESTROY_REQUEST so the server reclaims `ioid`.
    let destroy = codec.build_destroy_request(channel.sid, ioid);
    let _ = conn.send(&destroy).await;

    Ok((intro, data.value))
}

/// Run a PUT.
pub async fn op_put(
    conn: &mut Connection,
    channel: ChannelIds,
    value_str: &str,
) -> PvaResult<()> {
    let codec = PvaCodec {
        big_endian: matches!(conn.byte_order, ByteOrder::Big),
    };
    let ioid = alloc_ioid();
    let pv_req = build_pv_request_value_only(matches!(conn.byte_order, ByteOrder::Big));

    // INIT to learn the value field's type.
    let init_req = codec.build_put_init(channel.sid, ioid, &pv_req);
    conn.send(&init_req).await?;
    let init_frame = conn.read_app_frame().await?;
    let init = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected PUT INIT response, got {other:?}"
            )))
        }
    };
    if !init.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "PUT INIT failed: {:?}",
            init.status
        )));
    }
    let intro = init.introspection;

    // Build the value payload from the user-supplied string. We support
    // simple cases: NTScalar { value: <scalar> } where <scalar> is parsed
    // according to the introspection's scalar type.
    let value = build_put_value(&intro, value_str)?;

    // Data: bitset (selecting 'value') + filtered value.
    let mut data_payload = Vec::new();
    let order = conn.byte_order;
    data_payload.put_u32(channel.sid, order);
    data_payload.put_u32(ioid, order);
    data_payload.put_u8(0x00); // subcmd
    let mut changed = BitSet::new();
    if let Some(bit) = intro.bit_for_path("value") {
        changed.set(bit);
    } else {
        changed.set(0); // root
    }
    changed.write_into(order, &mut data_payload);
    encode_pv_field(&value, &intro, order, &mut data_payload);

    let header = PvaHeader::application(false, order, Command::Put.code(), data_payload.len() as u32);
    let mut frame_bytes = Vec::with_capacity(PvaHeader::SIZE + data_payload.len());
    header.write_into(&mut frame_bytes);
    frame_bytes.extend_from_slice(&data_payload);
    conn.send(&frame_bytes).await?;

    // Wait for PUT done.
    let done_frame = conn.read_app_frame().await?;
    let done = match decode_op_response(&done_frame, Some(&intro))? {
        OpResponse::Status(s) => s,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected PUT done, got {other:?}"
            )))
        }
    };
    if !done.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "PUT failed: {:?}",
            done.status
        )));
    }

    let destroy = codec.build_destroy_request(channel.sid, ioid);
    let _ = conn.send(&destroy).await;
    Ok(())
}

/// Subscribe to MONITOR updates. Calls `callback(&FieldDesc, &PvField)` for
/// each event. Returns when the connection closes or `callback` panics.
pub async fn op_monitor<F: FnMut(&FieldDesc, &PvField)>(
    conn: &mut Connection,
    channel: ChannelIds,
    fields: &[&str],
    mut callback: F,
) -> PvaResult<()> {
    let codec = PvaCodec {
        big_endian: matches!(conn.byte_order, ByteOrder::Big),
    };
    let ioid = alloc_ioid();
    let pv_req = if fields.is_empty() {
        build_pv_request_no_fields(matches!(conn.byte_order, ByteOrder::Big))
    } else {
        crate::pv_request::build_pv_request_fields(fields, matches!(conn.byte_order, ByteOrder::Big))
    };
    let init_req = codec.build_monitor_init(channel.sid, ioid, &pv_req);
    conn.send(&init_req).await?;
    let init_frame = conn.read_app_frame().await?;
    let init = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected MONITOR INIT, got {other:?}"
            )))
        }
    };
    let intro = init.introspection;

    // Start the subscription.
    let start = codec.build_monitor_start(channel.sid, ioid, 0);
    conn.send(&start).await?;

    // The first MONITOR event carries the full state (all bits set); subsequent
    // events carry only changed fields. Track the running value for delta
    // reconstruction (deferred — we deliver the raw decoded value to the
    // callback for now since pvxs's decode_pv_field already returns a full
    // structure shaped by the introspection).
    loop {
        let frame = match conn.read_app_frame().await {
            Ok(f) => f,
            Err(e) => {
                debug!("monitor read error: {e}");
                return Err(e);
            }
        };
        match decode_op_response(&frame, Some(&intro))? {
            OpResponse::Data(d) => {
                callback(&intro, &d.value);
            }
            OpResponse::Status(s) => {
                if !s.status.is_success() {
                    return Err(PvaError::Protocol(format!("MONITOR error: {:?}", s.status)));
                }
                return Ok(());
            }
            OpResponse::Init(_) => {
                // Spurious; ignore.
            }
        }
    }
}

/// Run GET_FIELD to fetch the type descriptor without a value.
pub async fn op_get_field(
    conn: &mut Connection,
    channel: ChannelIds,
    sub_field: &str,
) -> PvaResult<FieldDesc> {
    let codec = PvaCodec {
        big_endian: matches!(conn.byte_order, ByteOrder::Big),
    };
    let ioid = alloc_ioid();
    let req = codec.build_get_field(channel.sid, ioid, sub_field);
    conn.send(&req).await?;
    let frame = conn.read_app_frame().await?;
    let resp = decode_get_field_response(&frame)?;
    if !resp.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "GET_FIELD failed: {:?}",
            resp.status
        )));
    }
    resp.introspection
        .ok_or_else(|| PvaError::Protocol("GET_FIELD: missing introspection".to_string()))
}

/// Run an RPC operation. The request structure is sent INIT-then-DATA per
/// pvxs `clientreq.cpp::Operation::doRPC`; the response comes back as a
/// single application data frame with `subcmd == 0x00`.
pub async fn op_rpc(
    conn: &mut Connection,
    channel: ChannelIds,
    request_desc: &FieldDesc,
    request_value: &PvField,
) -> PvaResult<(FieldDesc, PvField)> {
    let order = conn.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    // Build a pvRequest matching the request descriptor (RPC always uses the
    // request structure type itself as the pvRequest envelope).
    let mut pv_req = Vec::new();
    crate::pvdata::encode::encode_type_desc(request_desc, order, &mut pv_req);

    // INIT
    let init = codec.build_get_init(channel.sid, ioid, &pv_req);
    // The INIT command for RPC is CMD_RPC (20), not GET — use op_payload directly.
    let mut p = Vec::with_capacity(9 + pv_req.len());
    p.put_u32(channel.sid, order);
    p.put_u32(ioid, order);
    p.put_u8(QosFlags::INIT);
    p.extend_from_slice(&pv_req);
    let init_header =
        PvaHeader::application(false, order, Command::Rpc.code(), p.len() as u32);
    let mut init_bytes = Vec::with_capacity(8 + p.len());
    init_header.write_into(&mut init_bytes);
    init_bytes.extend_from_slice(&p);
    let _ = init; // shadow the GET-shaped init we built first
    conn.send(&init_bytes).await?;

    let init_frame = conn.read_app_frame().await?;
    let init_resp = match decode_op_response(&init_frame, None)? {
        OpResponse::Init(i) => i,
        other => {
            return Err(PvaError::Protocol(format!(
                "expected RPC INIT response, got {other:?}"
            )))
        }
    };
    if !init_resp.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "RPC INIT failed: {:?}",
            init_resp.status
        )));
    }
    // The server's INIT response carries the *response* introspection.
    let response_intro = init_resp.introspection;

    // DATA: send the request value with subcmd=0x00 + structure-tag prefix +
    // value bytes (no bitset, since RPC arguments are always all-fields).
    let mut data_payload = Vec::new();
    data_payload.put_u32(channel.sid, order);
    data_payload.put_u32(ioid, order);
    data_payload.put_u8(0x00);
    encode_pv_field(request_value, request_desc, order, &mut data_payload);
    let data_header = PvaHeader::application(
        false,
        order,
        Command::Rpc.code(),
        data_payload.len() as u32,
    );
    let mut data_bytes = Vec::with_capacity(8 + data_payload.len());
    data_header.write_into(&mut data_bytes);
    data_bytes.extend_from_slice(&data_payload);
    conn.send(&data_bytes).await?;

    // Response data frame
    let resp_frame = conn.read_app_frame().await?;
    match decode_op_response(&resp_frame, Some(&response_intro))? {
        OpResponse::Data(d) => {
            if !d.status.is_success() {
                return Err(PvaError::Protocol(format!("RPC failed: {:?}", d.status)));
            }
            // Best-effort cleanup: send DESTROY_REQUEST.
            let destroy = codec.build_destroy_request(channel.sid, ioid);
            let _ = conn.send(&destroy).await;
            Ok((response_intro, d.value))
        }
        OpResponse::Status(s) => {
            if !s.status.is_success() {
                return Err(PvaError::Protocol(format!("RPC error: {:?}", s.status)));
            }
            Err(PvaError::Protocol("RPC: empty response".to_string()))
        }
        other => Err(PvaError::Protocol(format!(
            "expected RPC data response, got {other:?}"
        ))),
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────

/// pvxs's "all fields" sentinel: variant `0xFD 0x02 0x00 0x80 0x00 0x00`. We
/// emit it instead of an empty pvRequest so older servers don't reject.
fn build_put_request_no_fields(_be: bool) -> Vec<u8> {
    vec![0xFD, 0x02, 0x00, 0x80, 0x00, 0x00]
}

fn build_pv_request_no_fields(be: bool) -> Vec<u8> {
    build_put_request_no_fields(be)
}

/// Try to construct a `PvField` matching `desc` from a single user-supplied
/// string. Supports NTScalar (`{ value, alarm, timeStamp, ... }`) and bare
/// scalar/scalar-array introspections.
fn build_put_value(desc: &FieldDesc, value_str: &str) -> PvaResult<PvField> {
    match desc {
        FieldDesc::Scalar(st) => ScalarValue::parse(*st, value_str)
            .map(PvField::Scalar)
            .map_err(PvaError::InvalidValue),
        FieldDesc::ScalarArray(st) => {
            // CSV
            let mut items = Vec::new();
            for tok in value_str.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let v = ScalarValue::parse(*st, tok).map_err(PvaError::InvalidValue)?;
                items.push(v);
            }
            Ok(PvField::ScalarArray(items))
        }
        FieldDesc::Structure { fields, struct_id } => {
            let mut s = PvStructure::new(struct_id);
            for (name, child) in fields {
                if name == "value" {
                    s.fields.push((name.clone(), build_put_value(child, value_str)?));
                } else {
                    s.fields
                        .push((name.clone(), crate::pvdata::encode::default_value_for(child)));
                }
            }
            Ok(PvField::Structure(s))
        }
        _ => Err(PvaError::InvalidValue(format!(
            "PUT not supported for descriptor {desc}"
        ))),
    }
}
