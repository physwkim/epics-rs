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
use crate::pvdata::encode::{encode_pv_field, encode_type_desc};
use crate::pvdata::{FieldDesc, PvField, PvStructure, ScalarValue};

use super::channel::Channel;
use super::decode::{OpResponse, decode_op_response, decode_op_response_cached};

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
    op_get_inner(channel, fields, None, op_timeout).await
}

/// `op_get` variant accepting a pre-built pvRequest blob (bytes
/// produced by [`crate::pv_request::PvRequestExpr::encode`] or one of
/// the `build_pv_request_*` helpers). Lets callers feed
/// `record[pipeline=true,queueSize=N]` etc. through the convenience
/// surface — pvxs `Context::get(name).pvRequest(...)` parity. The raw
/// bytes win over the `fields` path when supplied; pass `None` to
/// fall back to the field-list builder.
pub async fn op_get_raw(
    channel: &Arc<Channel>,
    pv_req: &[u8],
    op_timeout: Duration,
) -> PvaResult<(FieldDesc, PvField)> {
    op_get_inner(channel, &[], Some(pv_req), op_timeout).await
}

async fn op_get_inner(
    channel: &Arc<Channel>,
    fields: &[&str],
    raw_pv_req: Option<&[u8]>,
    op_timeout: Duration,
) -> PvaResult<(FieldDesc, PvField)> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = match raw_pv_req {
        Some(b) => b.to_vec(),
        None if fields.is_empty() => sentinel_all_fields(),
        None => build_pv_request_fields(fields, big_endian),
    };

    let mut stream = server.register_ioid_stream(ioid);
    let cache = server.type_cache();

    // INIT
    let init_req = codec.build_get_init(sid, ioid, &pv_req);
    server.send(init_req).await?;
    let init_frame = await_frame(&mut stream, op_timeout).await?;
    let init = match decode_op_response_cached(&init_frame, None, &mut cache.lock())? {
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

// ── GET_FIELD (introspection only) ────────────────────────────────────

/// Fetch the channel's introspection (FieldDesc) without transferring
/// any value. pvxs `Context::info(name)` parity. Much cheaper than a
/// full GET for large PVs (NTNDArray, multi-MiB arrays) since the
/// server replies with descriptor bytes only — no value encoding,
/// no payload bandwidth proportional to the PV size.
///
/// `subfield` (typically the empty string) selects a sub-tree of the
/// channel's structure; pass "" for the root-level introspection.
pub async fn op_get_field(
    channel: &Arc<Channel>,
    subfield: &str,
    op_timeout: Duration,
) -> PvaResult<FieldDesc> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();
    let mut stream = server.register_ioid_stream(ioid);

    let req = codec.build_get_field(sid, ioid, subfield);
    let send_result = server.send(req).await;
    if send_result.is_err() {
        server.unregister_ioid(ioid);
        return Err(PvaError::Protocol("GET_FIELD send failed".into()));
    }
    let frame = await_frame(&mut stream, op_timeout).await;
    server.unregister_ioid(ioid);
    let frame = frame?;
    let resp = super::decode::decode_get_field_response(&frame)?;
    if !resp.status.is_success() {
        return Err(PvaError::Protocol(format!(
            "GET_FIELD failed: {:?}",
            resp.status
        )));
    }
    resp.introspection
        .ok_or_else(|| PvaError::Protocol("GET_FIELD: no introspection in successful response".into()))
}

// ── PUT ────────────────────────────────────────────────────────────────

pub async fn op_put(
    channel: &Arc<Channel>,
    value_str: &str,
    op_timeout: Duration,
) -> PvaResult<()> {
    op_put_inner(channel, value_str, None, op_timeout).await
}

/// `op_put` variant accepting a pre-built pvRequest blob. Lets
/// callers thread `record[process=true]` (RPC-like blocking puts) or
/// custom field-mask selections through. Bytes typically built via
/// [`crate::pv_request::PvRequestBuilder::build`] +
/// [`crate::pv_request::PvRequestExpr::encode`]. pvxs
/// `Context::put(name).pvRequest(...)` parity.
pub async fn op_put_raw(
    channel: &Arc<Channel>,
    pv_req: &[u8],
    value_str: &str,
    op_timeout: Duration,
) -> PvaResult<()> {
    op_put_inner(channel, value_str, Some(pv_req), op_timeout).await
}

/// PUT a single dotted-path field of the channel's structure (e.g.
/// `"alarm.severity"`, `"value"`, `"display.units"`). pvxs
/// `PutBuilder::set("path", val)` parity. Server receives a value
/// where only `field_path` carries the parsed string and every other
/// field is a default; the changed bitset has only the path's bit
/// set so the server applies just that one field.
///
/// pvRequest is forced to `field(<path>)` so the server INIT
/// negotiation matches the field layout we'll send.
pub async fn op_put_field(
    channel: &Arc<Channel>,
    field_path: &str,
    value_str: &str,
    op_timeout: Duration,
) -> PvaResult<()> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    // pvRequest selects exactly the target field so server-side bitset
    // bookkeeping aligns with the descriptor we'll get back.
    let pv_req = if field_path.is_empty() {
        sentinel_all_fields()
    } else {
        build_pv_request_fields(&[field_path], big_endian)
    };
    let mut stream = server.register_ioid_stream(ioid);
    let cache = server.type_cache();

    let init_req = codec.build_put_init(sid, ioid, &pv_req);
    server.send(init_req).await?;
    let init_frame = await_frame(&mut stream, op_timeout).await?;
    let init = match decode_op_response_cached(&init_frame, None, &mut cache.lock())? {
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

    let parts: Vec<&str> = field_path.split('.').filter(|s| !s.is_empty()).collect();
    let value = build_put_value_for_path(&intro, &parts, value_str)?;
    let bit = intro.bit_for_path(field_path).ok_or_else(|| {
        PvaError::InvalidValue(format!(
            "field path '{field_path}' not present in introspection"
        ))
    })?;

    let mut payload = Vec::new();
    payload.put_u32(sid, order);
    payload.put_u32(ioid, order);
    payload.put_u8(0x00);
    let mut changed = BitSet::new();
    changed.set(bit);
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

async fn op_put_inner(
    channel: &Arc<Channel>,
    value_str: &str,
    raw_pv_req: Option<&[u8]>,
    op_timeout: Duration,
) -> PvaResult<()> {
    let (server, sid) = channel.ensure_active().await?;
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = match raw_pv_req {
        Some(b) => b.to_vec(),
        None => build_pv_request_value_only(big_endian),
    };
    let mut stream = server.register_ioid_stream(ioid);
    let cache = server.type_cache();

    // INIT
    let init_req = codec.build_put_init(sid, ioid, &pv_req);
    server.send(init_req).await?;
    let init_frame = await_frame(&mut stream, op_timeout).await?;
    let init = match decode_op_response_cached(&init_frame, None, &mut cache.lock())? {
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

/// Typed monitor event delivered to callers of [`op_monitor_events`].
/// Mirrors pvxs's separation of `Connected` / `Disconnect` / `Finished`
/// / data exceptions thrown from `Subscription::pop()` (client.h:209).
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Channel just transitioned to Active and the server has
    /// confirmed our INIT/START. Fires once per connect cycle.
    Connected,
    /// Server pushed a value update.
    Data { intro: FieldDesc, value: PvField },
    /// Channel left Active (TCP closed, op error, channel closed).
    Disconnected,
    /// Server signalled end-of-stream via subcmd=0x10 (no further
    /// updates will arrive for this monitor).
    Finished,
}

/// Per-call configuration for [`op_monitor_events`] / handle variants.
/// Mirrors pvxs `MonitorBuilder::maskConnected/maskDisconnected`.
#[derive(Debug, Clone, Copy)]
pub struct MonitorEventMask {
    /// When true, suppress [`MonitorEvent::Connected`].
    pub mask_connected: bool,
    /// When true, suppress [`MonitorEvent::Disconnected`] and
    /// [`MonitorEvent::Finished`].
    pub mask_disconnected: bool,
}

impl Default for MonitorEventMask {
    fn default() -> Self {
        // pvxs defaults: maskConnected=true, maskDisconnected=false.
        Self {
            mask_connected: true,
            mask_disconnected: false,
        }
    }
}

/// Per-subscription metrics. Mirrors pvxs `SubscriptionStat`
/// (client.h:166). Values are observable via [`SubscriptionHandle::stats`]
/// — queue counters reflect the local async pipeline; client-squash
/// is the count of `MonitorOp::Data` frames the loop coalesced
/// because the consumer was slower than the network feed.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionStat {
    /// Total updates delivered to the user callback.
    pub n_delivered: u64,
    /// Total events dropped due to consumer back-pressure (when the
    /// callback can't keep up — currently always zero since the user
    /// callback is synchronous and serial; reserved for future async
    /// flow control).
    pub n_cli_squash: u64,
    /// Number of squash-on-server events reported by the wire (CMD
    /// MONITOR overrun bitset). pvxs surfaces the same field.
    pub n_srv_squash: u64,
    /// Number of MONITOR_ACK frames sent (pipelined window cycles).
    pub n_acks: u64,
    /// Highest events-since-ack value the loop saw. With a healthy
    /// `pipeline_size` this stays close to `pipeline_size`.
    pub max_queue: u32,
    /// Configured `pipeline_size` (call it `limitQueue` in pvxs).
    pub limit_queue: u32,
}

/// Internal shared state — the monitor loop publishes to this on every
/// reconnect / event / pause toggle, and [`SubscriptionHandle`] reads
/// from it.
struct SubscriptionState {
    /// Active `(ServerConn, sid, ioid)` triple. Refreshed on every
    /// reconnect cycle. None when in the gap between connections.
    active: parking_lot::Mutex<
        Option<(
            Arc<super::server_conn::ServerConn>,
            u32, /*sid*/
            u32, /*ioid*/
        )>,
    >,
    paused: std::sync::atomic::AtomicBool,
    stop: std::sync::atomic::AtomicBool,
    stats: parking_lot::Mutex<SubscriptionStat>,
}

/// User-facing handle returned by [`op_monitor_handle`]. Drops cleanly
/// without aborting the inner task — call [`Self::stop`] explicitly to
/// signal teardown. Mirrors pvxs `Subscription` at the public-method
/// level.
pub struct SubscriptionHandle {
    state: Arc<SubscriptionState>,
    task: Option<tokio::task::JoinHandle<PvaResult<()>>>,
}

impl SubscriptionHandle {
    /// Pause server emissions on this subscription. Safe to call
    /// multiple times; second call is a no-op when already paused.
    /// Mirrors pvxs `Subscription::pause(true)` (clientmon.cpp:115).
    /// Best-effort — if the underlying connection is gone we set the
    /// flag and the loop applies it on next reconnect.
    pub async fn pause(&self) {
        let was_paused = self
            .state
            .paused
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if was_paused {
            return;
        }
        let snapshot = self.state.active.lock().clone();
        if let Some((server, sid, ioid)) = snapshot {
            let big_endian = matches!(server.byte_order, ByteOrder::Big);
            let codec = PvaCodec { big_endian };
            let _ = server.send(codec.build_monitor_pause(sid, ioid)).await;
        }
    }

    /// Resume a paused subscription. Mirrors pvxs
    /// `Subscription::pause(false)`.
    pub async fn resume(&self) {
        let was_paused = self
            .state
            .paused
            .swap(false, std::sync::atomic::Ordering::Relaxed);
        if !was_paused {
            return;
        }
        let snapshot = self.state.active.lock().clone();
        if let Some((server, sid, ioid)) = snapshot {
            let big_endian = matches!(server.byte_order, ByteOrder::Big);
            let codec = PvaCodec { big_endian };
            let _ = server.send(codec.build_monitor_resume(sid, ioid)).await;
        }
    }

    /// Snapshot the per-subscription metrics. pvxs `Subscription::stats`
    /// equivalent. The optional `reset` flag (pvxs 1.1.0+) zeros
    /// counters after read.
    pub fn stats(&self, reset: bool) -> SubscriptionStat {
        let mut lock = self.state.stats.lock();
        let snap = lock.clone();
        if reset {
            *lock = SubscriptionStat {
                limit_queue: lock.limit_queue,
                ..Default::default()
            };
        }
        snap
    }

    /// Signal the inner task to terminate at its next opportunity
    /// (async — pvxs `syncCancel(false)` analog). Drop alone does not
    /// stop the task — call this explicitly.
    pub fn stop(&self) {
        self.state
            .stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Stop and await termination. pvxs `syncCancel(true)` analog —
    /// once this returns no further callbacks will fire.
    pub async fn stop_sync(mut self) {
        self.state
            .stop
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.task.take() {
            let _ = t.await;
        }
    }

    /// True if the inner task has finished (channel closed, fatal
    /// error, or `stop()` was called and the loop drained). Use to
    /// drive an auto-restart wrapper without consuming the handle.
    pub fn is_done(&self) -> bool {
        self.task.as_ref().map(|t| t.is_finished()).unwrap_or(true)
    }

    /// A cheap clone-able handle that can pause/resume the
    /// subscription from an unrelated task (no ownership of the
    /// underlying JoinHandle). Used by the PVA gateway to forward
    /// downstream watermark events into upstream pipeline-pause
    /// control messages — pvxs `MonitorControlOp::pipeline` parity.
    pub fn pauser(&self) -> Pauser {
        Pauser {
            state: self.state.clone(),
        }
    }
}

/// Detached pause/resume handle — see [`SubscriptionHandle::pauser`].
#[derive(Clone)]
pub struct Pauser {
    state: Arc<SubscriptionState>,
}

impl Pauser {
    /// Same semantics as [`SubscriptionHandle::pause`]. Async because
    /// it sends a control message to the server.
    pub async fn pause(&self) {
        let was_paused = self
            .state
            .paused
            .swap(true, std::sync::atomic::Ordering::Relaxed);
        if was_paused {
            return;
        }
        let snapshot = self.state.active.lock().clone();
        if let Some((server, sid, ioid)) = snapshot {
            let big_endian = matches!(server.byte_order, ByteOrder::Big);
            let codec = PvaCodec { big_endian };
            let _ = server.send(codec.build_monitor_pause(sid, ioid)).await;
        }
    }

    /// Same semantics as [`SubscriptionHandle::resume`].
    pub async fn resume(&self) {
        let was_paused = self
            .state
            .paused
            .swap(false, std::sync::atomic::Ordering::Relaxed);
        if !was_paused {
            return;
        }
        let snapshot = self.state.active.lock().clone();
        if let Some((server, sid, ioid)) = snapshot {
            let big_endian = matches!(server.byte_order, ByteOrder::Big);
            let codec = PvaCodec { big_endian };
            let _ = server.send(codec.build_monitor_resume(sid, ioid)).await;
        }
    }
}

/// F-G12 raw-frame monitor entry: like [`op_monitor`] but the
/// callback receives the **raw MONITOR DATA body bytes** (the
/// `changed | value | overrun` triplet from the wire) instead of a
/// decoded [`PvField`]. Bridge gateways feed these directly into
/// [`crate::server_native::RawMonitorEvent`] for downstream
/// re-emission without an intermediate decode.
///
/// Callback shape: `(intro: &FieldDesc, body: bytes::Bytes,
/// byte_order: ByteOrder)`. Body is refcount-shared (cheap clone).
pub async fn op_monitor_raw_frames<F>(
    channel: &Arc<Channel>,
    fields: &[&str],
    pipeline_size: u32,
    mut callback: F,
) -> PvaResult<()>
where
    F: FnMut(&FieldDesc, bytes::Bytes, ByteOrder) + Send,
{
    let fields_owned: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
    loop {
        let (server, sid) = match channel.ensure_active().await {
            Ok(p) => p,
            Err(e) => {
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                debug!(pv = %channel.pv_name, err = %e,
                    "raw monitor reconnect failed; retrying in 500ms");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        };
        match run_raw_monitor_loop(server.clone(), sid, &fields_owned, pipeline_size, &mut callback)
            .await
        {
            Ok(()) => return Ok(()),
            Err(MonitorEnd::ChannelClosed) => return Ok(()),
            Err(MonitorEnd::ConnectionLost) => {
                debug!(pv = %channel.pv_name, "raw monitor lost connection; will retry");
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(MonitorEnd::Fatal(e)) => return Err(e),
        }
    }
}

async fn run_raw_monitor_loop<F>(
    server: Arc<super::server_conn::ServerConn>,
    sid: u32,
    fields: &[String],
    pipeline_size: u32,
    callback: &mut F,
) -> Result<(), MonitorEnd>
where
    F: FnMut(&FieldDesc, bytes::Bytes, ByteOrder) + Send,
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
    let init_req = codec.build_monitor_init(sid, ioid, &pv_req);
    server
        .send(init_req)
        .await
        .map_err(|_| MonitorEnd::ConnectionLost)?;
    let init_frame = stream.recv().await.ok_or(MonitorEnd::ConnectionLost)?;
    let cache = server.type_cache();
    let init = match decode_op_response_cached(&init_frame, None, &mut cache.lock()) {
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
        // Inspect subcmd (first byte after the u32 ioid) to filter
        // out non-DATA frames (FINISH at 0x10, OK status, etc.).
        if frame.payload.len() < 5 {
            continue;
        }
        let subcmd = frame.payload[4];
        if subcmd != 0x00 {
            // Pipeline ack / FINISH / status — fall through to the
            // regular decode path so we can recognize FINISH and
            // unwind. ACK frames have subcmd 0x80; we ignore them
            // here since we drive ACKs ourselves below.
            if subcmd & 0x10 != 0 {
                server.unregister_ioid(ioid);
                return Ok(());
            }
            continue;
        }
        // Body = payload[5..] = changed | value | overrun (raw).
        // Wrap in `Bytes` so the broadcast fan-out shares this
        // allocation refcount-style.
        let body = bytes::Bytes::copy_from_slice(&frame.payload[5..]);
        callback(&intro, body, order);
        events_since_ack += 1;
        if pipeline_size > 0 && events_since_ack >= pipeline_size {
            let ack = codec.build_monitor_ack(sid, ioid, events_since_ack);
            if server.send(ack).await.is_err() {
                server.unregister_ioid(ioid);
                return Err(MonitorEnd::ConnectionLost);
            }
            events_since_ack = 0;
        }
    }
}

pub async fn op_monitor<F>(
    channel: &Arc<Channel>,
    fields: &[&str],
    pipeline_size: u32,
    callback: F,
) -> PvaResult<()>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    let fields_owned: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
    op_monitor_inner(channel, fields_owned, None, pipeline_size, callback).await
}

/// `op_monitor` variant accepting a pre-built pvRequest blob. Threads
/// `record[queueSize=N,pipeline=true,...]` and custom field-mask
/// selections through to MONITOR INIT. pvxs
/// `Context::monitor(name).pvRequest(...)` parity. The raw bytes win
/// over the field-list path; field reconnect-replay still works
/// because the bytes are reused on every reconnect cycle.
pub async fn op_monitor_raw<F>(
    channel: &Arc<Channel>,
    pv_req: Vec<u8>,
    pipeline_size: u32,
    callback: F,
) -> PvaResult<()>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    op_monitor_inner(channel, Vec::new(), Some(pv_req), pipeline_size, callback).await
}

async fn op_monitor_inner<F>(
    channel: &Arc<Channel>,
    fields_owned: Vec<String>,
    raw_pv_req: Option<Vec<u8>>,
    pipeline_size: u32,
    mut callback: F,
) -> PvaResult<()>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    loop {
        let (server, sid) = match channel.ensure_active().await {
            Ok(p) => p,
            Err(e) => {
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                debug!(pv = %channel.pv_name, err = %e, "monitor reconnect failed; retrying in 500ms");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        };
        match run_monitor_loop(
            server.clone(),
            sid,
            &fields_owned,
            raw_pv_req.as_deref(),
            pipeline_size,
            &mut callback,
            None,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(MonitorEnd::ChannelClosed) => return Ok(()),
            Err(MonitorEnd::ConnectionLost) => {
                debug!(pv = %channel.pv_name, "monitor lost connection; will retry");
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(MonitorEnd::Fatal(e)) => return Err(e),
        }
    }
}

/// Like [`op_monitor`] but returns a [`SubscriptionHandle`] for
/// pause/resume/stats. The inner monitor loop runs in a spawned task
/// and stops when the handle's `stop()` is called or when the channel
/// is closed.
pub fn op_monitor_handle<F>(
    channel: Arc<Channel>,
    fields: &[&str],
    pipeline_size: u32,
    mut callback: F,
) -> SubscriptionHandle
where
    F: FnMut(&FieldDesc, &PvField) + Send + 'static,
{
    let fields_owned: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
    let state = Arc::new(SubscriptionState {
        active: parking_lot::Mutex::new(None),
        paused: std::sync::atomic::AtomicBool::new(false),
        stop: std::sync::atomic::AtomicBool::new(false),
        stats: parking_lot::Mutex::new(SubscriptionStat {
            limit_queue: pipeline_size,
            ..Default::default()
        }),
    });
    let state_for_task = state.clone();

    let task = tokio::spawn(async move {
        loop {
            if state_for_task
                .stop
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                return Ok(());
            }
            let (server, sid) = match channel.ensure_active().await {
                Ok(p) => p,
                Err(e) => {
                    if matches!(
                        channel.current_state(),
                        super::channel::ChannelState::Closed
                    ) {
                        return Ok(());
                    }
                    debug!(pv = %channel.pv_name, err = %e, "monitor reconnect failed; retrying in 500ms");
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };
            match run_monitor_loop(
                server.clone(),
                sid,
                &fields_owned,
                None,
                pipeline_size,
                &mut callback,
                Some(state_for_task.clone()),
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(MonitorEnd::ChannelClosed) => return Ok(()),
                Err(MonitorEnd::ConnectionLost) => {
                    state_for_task.active.lock().take();
                    if matches!(
                        channel.current_state(),
                        super::channel::ChannelState::Closed
                    ) {
                        return Ok(());
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                Err(MonitorEnd::Fatal(e)) => return Err(e),
            }
        }
    });

    SubscriptionHandle {
        state,
        task: Some(task),
    }
}

/// Run a monitor and deliver [`MonitorEvent`] values to `callback`.
/// Bridges the per-update `(FieldDesc, PvField)` shape of the inner
/// loop to pvxs's typed event stream. The mask flags control whether
/// `Connected`/`Disconnected`/`Finished` events surface or stay
/// suppressed (pvxs `maskConnected` / `maskDisconnected`).
pub async fn op_monitor_events<F>(
    channel: &Arc<Channel>,
    fields: &[&str],
    pipeline_size: u32,
    mask: MonitorEventMask,
    mut callback: F,
) -> PvaResult<()>
where
    F: FnMut(MonitorEvent) + Send,
{
    let fields_owned: Vec<String> = fields.iter().map(|s| s.to_string()).collect();
    loop {
        let (server, sid) = match channel.ensure_active().await {
            Ok(p) => p,
            Err(e) => {
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                debug!(pv = %channel.pv_name, err = %e, "monitor reconnect failed; retrying in 500ms");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        };
        if !mask.mask_connected {
            callback(MonitorEvent::Connected);
        }
        let mut data_callback = |intro: &FieldDesc, value: &PvField| {
            callback(MonitorEvent::Data {
                intro: intro.clone(),
                value: value.clone(),
            });
        };
        let result = run_monitor_loop(
            server.clone(),
            sid,
            &fields_owned,
            None,
            pipeline_size,
            &mut data_callback,
            None,
        )
        .await;
        match result {
            Ok(()) => {
                if !mask.mask_disconnected {
                    callback(MonitorEvent::Finished);
                }
                return Ok(());
            }
            Err(MonitorEnd::ChannelClosed) => {
                if !mask.mask_disconnected {
                    callback(MonitorEvent::Disconnected);
                }
                return Ok(());
            }
            Err(MonitorEnd::ConnectionLost) => {
                if !mask.mask_disconnected {
                    callback(MonitorEvent::Disconnected);
                }
                if matches!(
                    channel.current_state(),
                    super::channel::ChannelState::Closed
                ) {
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Err(MonitorEnd::Fatal(e)) => return Err(e),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum MonitorEnd {
    ChannelClosed,
    ConnectionLost,
    Fatal(PvaError),
}

async fn run_monitor_loop<F>(
    server: Arc<super::server_conn::ServerConn>,
    sid: u32,
    fields: &[String],
    raw_pv_req: Option<&[u8]>,
    pipeline_size: u32,
    callback: &mut F,
    state: Option<Arc<SubscriptionState>>,
) -> Result<(), MonitorEnd>
where
    F: FnMut(&FieldDesc, &PvField) + Send,
{
    let order = server.byte_order;
    let big_endian = matches!(order, ByteOrder::Big);
    let codec = PvaCodec { big_endian };
    let ioid = alloc_ioid();

    let pv_req = match raw_pv_req {
        Some(b) => b.to_vec(),
        None if fields.is_empty() => sentinel_all_fields(),
        None => {
            let refs: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
            build_pv_request_fields(&refs, big_endian)
        }
    };

    let mut stream = server.register_ioid_stream(ioid);

    // INIT
    let init_req = codec.build_monitor_init(sid, ioid, &pv_req);
    server
        .send(init_req)
        .await
        .map_err(|_| MonitorEnd::ConnectionLost)?;
    let init_frame = stream.recv().await.ok_or(MonitorEnd::ConnectionLost)?;
    let cache = server.type_cache();
    let init = match decode_op_response_cached(&init_frame, None, &mut cache.lock()) {
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

    // START with pipeline ack window — unless the handle was paused
    // before this reconnect cycle, in which case start in STOP state
    // so the server doesn't begin emitting until resume() is called.
    let initially_paused = state
        .as_ref()
        .map(|s| s.paused.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false);
    let start = if initially_paused {
        codec.build_monitor_pause(sid, ioid)
    } else {
        codec.build_monitor_start(sid, ioid, pipeline_size)
    };
    server
        .send(start)
        .await
        .map_err(|_| MonitorEnd::ConnectionLost)?;

    if let Some(s) = &state {
        *s.active.lock() = Some((server.clone(), sid, ioid));
    }

    let mut events_since_ack: u32 = 0;
    loop {
        if let Some(s) = &state {
            if s.stop.load(std::sync::atomic::Ordering::Relaxed) {
                server.unregister_ioid(ioid);
                return Err(MonitorEnd::ChannelClosed);
            }
        }
        let frame = match stream.recv().await {
            Some(f) => f,
            None => {
                server.unregister_ioid(ioid);
                if let Some(s) = &state {
                    s.active.lock().take();
                }
                return Err(MonitorEnd::ConnectionLost);
            }
        };
        match decode_op_response(&frame, Some(&intro)) {
            Ok(OpResponse::Data(d)) => {
                callback(&intro, &d.value);
                events_since_ack += 1;
                if let Some(s) = &state {
                    let mut st = s.stats.lock();
                    st.n_delivered += 1;
                    if events_since_ack > st.max_queue {
                        st.max_queue = events_since_ack;
                    }
                }
                let _ = &d; // silence unused-on-some-cfg warning
                if pipeline_size > 0 && events_since_ack >= pipeline_size {
                    let ack = codec.build_monitor_ack(sid, ioid, events_since_ack);
                    if server.send(ack).await.is_err() {
                        server.unregister_ioid(ioid);
                        return Err(MonitorEnd::ConnectionLost);
                    }
                    if let Some(s) = &state {
                        s.stats.lock().n_acks += 1;
                    }
                    events_since_ack = 0;
                }
            }
            Ok(OpResponse::Status(s)) => {
                server.unregister_ioid(ioid);
                if let Some(st) = &state {
                    st.active.lock().take();
                }
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

    // DATA — RPC argument: `type(arg) + full_value(arg)`.
    // pvxs clientget.cpp:307-311 — `to_wire(R, type); to_wire_full(R, arg)`.
    let mut data = Vec::new();
    data.put_u32(sid, order);
    data.put_u32(ioid, order);
    data.put_u8(0x00);
    crate::pvdata::encode::encode_type_desc(request_desc, order, &mut data);
    encode_pv_field(request_value, request_desc, order, &mut data);
    let data_h = PvaHeader::application(false, order, Command::Rpc.code(), data.len() as u32);
    let mut data_frame = Vec::with_capacity(8 + data.len());
    data_h.write_into(&mut data_frame);
    data_frame.extend_from_slice(&data);
    server.send(data_frame).await?;

    let resp_frame = await_frame(&mut stream, op_timeout).await?;
    // RPC response carries its own type — `response_intro` from INIT is
    // unused (RPC INIT has no introspection per pvxs).
    let _ = response_intro;
    let result = match decode_op_response(&resp_frame, None)? {
        OpResponse::Data(d) => {
            if d.status.is_success() {
                let desc = d.response_desc.unwrap_or(FieldDesc::Variant);
                Ok((desc, d.value))
            } else {
                Err(PvaError::Protocol(format!("RPC: {:?}", d.status)))
            }
        }
        OpResponse::Status(s) => Err(PvaError::Protocol(format!("RPC: {:?}", s.status))),
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

/// Build a PUT value where only `field_path` (e.g. `"alarm.severity"`)
/// carries the parsed value; every other field gets a default. Mirrors
/// pvxs `PutBuilder::set("alarm.severity", val)` semantics — the
/// matching changed-bitset must be built separately via
/// [`crate::pvdata::FieldDesc::bit_for_path`]. F-G5.
fn build_put_value_for_path(
    desc: &FieldDesc,
    field_path: &[&str],
    value_str: &str,
) -> PvaResult<PvField> {
    if field_path.is_empty() {
        // Targeting the root: parse value directly into the descriptor
        // shape (recurses into the "value" subfield convention used by
        // build_put_value for compatibility).
        return build_put_value(desc, value_str);
    }
    match desc {
        FieldDesc::Structure { fields, struct_id } => {
            let head = field_path[0];
            let tail = &field_path[1..];
            let mut s = PvStructure::new(struct_id);
            for (name, child) in fields {
                if name == head {
                    s.fields
                        .push((name.clone(), build_put_value_for_path(child, tail, value_str)?));
                } else {
                    s.fields.push((
                        name.clone(),
                        crate::pvdata::encode::default_value_for(child),
                    ));
                }
            }
            // Path didn't match any field → clear failure.
            if !fields.iter().any(|(n, _)| n == head) {
                return Err(PvaError::InvalidValue(format!(
                    "field '{head}' not present in target structure"
                )));
            }
            Ok(PvField::Structure(s))
        }
        FieldDesc::Scalar(st) if field_path.is_empty() => ScalarValue::parse(*st, value_str)
            .map(PvField::Scalar)
            .map_err(PvaError::InvalidValue),
        FieldDesc::ScalarArray(st) if field_path.is_empty() => {
            let mut items = Vec::new();
            for tok in value_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                items.push(ScalarValue::parse(*st, tok).map_err(PvaError::InvalidValue)?);
            }
            Ok(PvField::ScalarArray(items))
        }
        _ => Err(PvaError::InvalidValue(format!(
            "cannot navigate path through {desc} (remaining: {field_path:?})"
        ))),
    }
}

fn build_put_value(desc: &FieldDesc, value_str: &str) -> PvaResult<PvField> {
    match desc {
        FieldDesc::Scalar(st) => ScalarValue::parse(*st, value_str)
            .map(PvField::Scalar)
            .map_err(PvaError::InvalidValue),
        FieldDesc::ScalarArray(st) => {
            let mut items = Vec::new();
            for tok in value_str
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
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
