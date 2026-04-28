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
}

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
