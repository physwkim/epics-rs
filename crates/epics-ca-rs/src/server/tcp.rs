use epics_base_rs::runtime::sync::{Mutex, RwLock};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;
use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::broadcast;

/// Maximum accumulated TCP read buffer per client (DoS guard).
/// Mirrors the client-side cap in `client/transport.rs`.
const MAX_ACCUMULATED: usize = 1024 * 1024; // 1 MB

/// Maximum idle time before forcibly closing a TCP client.
/// OS-level TCP keepalive (~30s) handles half-open detection; this is
/// a belt-and-suspenders cap for environments where keepalive is unreliable.
/// 600s default; configurable via EPICS_CAS_INACTIVITY_TMO.
fn inactivity_timeout() -> Duration {
    epics_base_rs::runtime::env::get("EPICS_CAS_INACTIVITY_TMO")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| Duration::from_secs_f64(v.max(30.0)))
        .unwrap_or(Duration::from_secs(600))
}

/// Maximum simultaneous channels per CA client (EPICS_CAS_MAX_CHANNELS).
fn max_channels_per_client() -> usize {
    epics_base_rs::runtime::env::get("EPICS_CAS_MAX_CHANNELS")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(4096)
        .max(1)
}

/// Maximum subscriptions per channel (EPICS_CAS_MAX_SUBS_PER_CHAN).
fn max_subs_per_channel() -> usize {
    epics_base_rs::runtime::env::get("EPICS_CAS_MAX_SUBS_PER_CHAN")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(100)
        .max(1)
}

/// Connection lifecycle event broadcast by the TCP listener.
#[derive(Debug, Clone)]
pub enum ServerConnectionEvent {
    /// New client connection accepted.
    Connected(SocketAddr),
    /// Client connection closed.
    Disconnected(SocketAddr),
}

use crate::protocol::*;
use crate::server::monitor::{FlowControlGate, spawn_monitor_sender};
use epics_base_rs::error::CaResult;
use epics_base_rs::server::access_security::{AccessLevel, AccessSecurityConfig};
use epics_base_rs::server::database::{PvDatabase, PvEntry, parse_pv_name};
use epics_base_rs::server::pv::ProcessVariable;
use epics_base_rs::server::record::RecordInstance;
use epics_base_rs::types::{DbFieldType, EpicsValue, encode_dbr, native_type_for_dbr};

#[derive(Clone)]
enum ChannelTarget {
    SimplePv(Arc<ProcessVariable>),
    RecordField {
        record: Arc<RwLock<RecordInstance>>,
        field: String,
    },
}

struct ChannelEntry {
    target: ChannelTarget,
    cid: u32,
}

struct SubscriptionEntry {
    target: ChannelTarget,
    channel_sid: u32,
    sub_id: u32,
    data_type: u16,
    task: tokio::task::JoinHandle<()>,
}

struct ClientState {
    channels: HashMap<u32, ChannelEntry>,
    subscriptions: HashMap<u32, SubscriptionEntry>,
    channel_access: HashMap<u32, AccessLevel>,
    next_sid: AtomicU32,
    hostname: String,
    username: String,
    acf: Arc<Option<AccessSecurityConfig>>,
    tcp_port: u16,
    client_minor_version: u16,
    flow_control: Arc<FlowControlGate>,
}

impl ClientState {
    fn new(acf: Arc<Option<AccessSecurityConfig>>, tcp_port: u16) -> Self {
        Self {
            channels: HashMap::new(),
            subscriptions: HashMap::new(),
            channel_access: HashMap::new(),
            next_sid: AtomicU32::new(1),
            hostname: String::new(),
            username: String::new(),
            acf,
            tcp_port,
            client_minor_version: 0,
            flow_control: Arc::new(FlowControlGate::default()),
        }
    }

    fn alloc_sid(&self) -> u32 {
        self.next_sid.fetch_add(1, Ordering::Relaxed)
    }

    /// Compute access rights bits for a channel target.
    async fn compute_access(&self, target: &ChannelTarget) -> u32 {
        match target {
            ChannelTarget::SimplePv(_) => {
                if let Some(ref acf_cfg) = *self.acf {
                    match acf_cfg.check_access("DEFAULT", &self.hostname, &self.username) {
                        AccessLevel::ReadWrite => 3,
                        AccessLevel::Read => 1,
                        AccessLevel::NoAccess => 0,
                    }
                } else {
                    3
                }
            }
            ChannelTarget::RecordField { record, field: f } => {
                let instance = record.read().await;
                let is_ro = instance
                    .record
                    .field_list()
                    .iter()
                    .find(|fd| fd.name == f.as_str())
                    .map(|fd| fd.read_only)
                    .unwrap_or(false);
                if is_ro {
                    1
                } else if let Some(ref acf_cfg) = *self.acf {
                    let asg = &instance.common.asg;
                    match acf_cfg.check_access(asg, &self.hostname, &self.username) {
                        AccessLevel::ReadWrite => 3,
                        AccessLevel::Read => 1,
                        AccessLevel::NoAccess => 0,
                    }
                } else {
                    3
                }
            }
        }
    }
}

/// Run the TCP listener for CA connections.
/// Tries to bind to the configured port first; falls back to an ephemeral port
/// (port 0) if the configured port is already in use.
///
/// Notifies `beacon_reset` on each client connect/disconnect so the beacon
/// emitter restarts its fast beacon cycle (matching C EPICS behavior).
pub async fn run_tcp_listener(
    db: Arc<PvDatabase>,
    port: u16,
    acf: Arc<Option<AccessSecurityConfig>>,
    tcp_port_tx: tokio::sync::oneshot::Sender<u16>,
    beacon_reset: std::sync::Arc<tokio::sync::Notify>,
    conn_events: Option<broadcast::Sender<ServerConnectionEvent>>,
) -> CaResult<()> {
    let listener = match TcpListener::bind(("0.0.0.0", port)).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            TcpListener::bind(("0.0.0.0", 0)).await?
        }
        Err(e) => return Err(e.into()),
    };
    let actual_port = listener.local_addr()?.port();
    let _ = tcp_port_tx.send(actual_port);

    loop {
        let (stream, peer) = listener.accept().await?;
        let db = db.clone();
        let acf = acf.clone();
        let beacon_reset = beacon_reset.clone();
        beacon_reset.notify_one();
        if let Some(tx) = &conn_events {
            let _ = tx.send(ServerConnectionEvent::Connected(peer));
        }
        let conn_events = conn_events.clone();

        // Enable OS-level TCP keepalive on accepted socket so half-open
        // connections (e.g. NAT timeout, gateway down) are detected within
        // ~30s. Mirrors client-side keepalive in client/transport.rs.
        {
            let sock = socket2::SockRef::from(&stream);
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(Duration::from_secs(15))
                .with_interval(Duration::from_secs(5));
            let _ = sock.set_keepalive(true);
            let _ = sock.set_tcp_keepalive(&keepalive);
        }

        epics_base_rs::runtime::task::spawn(async move {
            let result = handle_client(stream, db, acf, actual_port).await;
            beacon_reset.notify_one();
            if let Some(tx) = &conn_events {
                let _ = tx.send(ServerConnectionEvent::Disconnected(peer));
            }
            if let Err(e) = result {
                // Suppress normal disconnection errors (client closed connection)
                let is_disconnect = matches!(
                    e,
                    epics_base_rs::error::CaError::Io(ref io) if matches!(
                        io.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::UnexpectedEof
                    )
                );
                if !is_disconnect {
                    eprintln!("client {peer} error: {e}");
                }
            }
        });
    }
}

async fn handle_client(
    stream: tokio::net::TcpStream,
    db: Arc<PvDatabase>,
    acf: Arc<Option<AccessSecurityConfig>>,
    tcp_port: u16,
) -> CaResult<()> {
    // Disable Nagle's algorithm to avoid extra latency for small control messages.
    let _ = stream.set_nodelay(true);

    let (mut reader, writer) = stream.into_split();
    let writer = Arc::new(Mutex::new(BufWriter::new(writer)));
    let mut state = ClientState::new(acf, tcp_port);

    let mut buf = vec![0u8; 8192];
    let mut accumulated = Vec::new();
    let inactivity = inactivity_timeout();

    loop {
        // Bound read with inactivity timeout so a fully-silent half-open
        // connection eventually gets cleaned up even if OS keepalive failed.
        let n = match tokio::time::timeout(inactivity, reader.read(&mut buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => {
                // Inactivity timeout — close the connection.
                eprintln!("CA server: client idle for {}s, closing", inactivity.as_secs());
                break;
            }
        };
        if n == 0 {
            break;
        }
        accumulated.extend_from_slice(&buf[..n]);

        // DoS guard: a malformed or hostile client could declare a huge
        // postsize and stream nothing more, growing this Vec unbounded.
        if accumulated.len() > MAX_ACCUMULATED {
            eprintln!(
                "CA server: client accumulated buffer exceeded {} bytes, closing",
                MAX_ACCUMULATED
            );
            break;
        }

        let mut offset = 0;
        while offset + CaHeader::SIZE <= accumulated.len() {
            let (hdr, hdr_size) = CaHeader::from_bytes_extended(&accumulated[offset..])?;
            let actual_post = hdr.actual_postsize();
            let padded_post = align8(actual_post);
            let msg_len = hdr_size + padded_post;

            if offset + msg_len > accumulated.len() {
                break;
            }

            let payload = if actual_post > 0 {
                accumulated[offset + hdr_size..offset + hdr_size + actual_post].to_vec()
            } else {
                Vec::new()
            };

            dispatch_message(&hdr, &payload, &mut state, &db, &writer).await?;
            offset += msg_len;
        }

        if offset > 0 {
            accumulated.drain(..offset);
        }
    }

    // Cleanup: cancel all subscriptions
    for (_, sub) in state.subscriptions.drain() {
        sub.task.abort();
        match &sub.target {
            ChannelTarget::SimplePv(pv) => {
                pv.remove_subscriber(sub.sub_id).await;
            }
            ChannelTarget::RecordField { record, .. } => {
                record.write().await.remove_subscriber(sub.sub_id);
            }
        }
    }

    Ok(())
}

async fn dispatch_message(
    hdr: &CaHeader,
    payload: &[u8],
    state: &mut ClientState,
    db: &Arc<PvDatabase>,
    writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
) -> CaResult<()> {
    match hdr.cmmd {
        CA_PROTO_VERSION => {
            state.client_minor_version = hdr.count;
            let mut resp = CaHeader::new(CA_PROTO_VERSION);
            resp.data_type = 1;
            resp.count = CA_MINOR_VERSION;
            resp.cid = 1;
            let mut w = writer.lock().await;
            w.write_all(&resp.to_bytes()).await?;
            w.flush().await?;
        }

        CA_PROTO_HOST_NAME => {
            let end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            state.hostname = String::from_utf8_lossy(&payload[..end]).to_string();
            // Re-evaluate access rights for all existing channels
            reeval_access_rights(state, writer).await?;
        }

        CA_PROTO_CLIENT_NAME => {
            let end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            state.username = String::from_utf8_lossy(&payload[..end]).to_string();
            // Re-evaluate access rights for all existing channels
            reeval_access_rights(state, writer).await?;
        }

        CA_PROTO_CREATE_CHAN => {
            // Pre-CA-4.4 clients send claims with no PV name (postsize=0).
            // Silently ignore these, matching C server behavior (camessage.c:1204).
            // The client will retry with v4.4+ format after receiving our VERSION.
            if hdr.actual_postsize() <= 1 {
                return Ok(());
            }

            // DoS guard: refuse new channels once the per-client cap is hit.
            if state.channels.len() >= max_channels_per_client() {
                let mut fail = CaHeader::new(CA_PROTO_CREATE_CH_FAIL);
                fail.cid = hdr.cid;
                let mut w = writer.lock().await;
                w.write_all(&fail.to_bytes()).await?;
                w.flush().await?;
                return Ok(());
            }

            let end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            let pv_name = String::from_utf8_lossy(&payload[..end]).to_string();
            let client_cid = hdr.cid;
            let (_base, field_raw) = parse_pv_name(&pv_name);
            let field = field_raw.to_ascii_uppercase();

            if let Some(entry) = db.find_entry(&pv_name).await {
                let sid = state.alloc_sid();

                let (dbr_type, element_count, target) = match entry {
                    PvEntry::Simple(pv) => {
                        let value = pv.get().await;
                        (
                            value.dbr_type(),
                            value.count() as u32,
                            ChannelTarget::SimplePv(pv),
                        )
                    }
                    PvEntry::Record(rec) => {
                        let instance = rec.read().await;
                        // Use resolve_field for 3-level priority
                        let value = instance.resolve_field(&field);
                        match value {
                            Some(v) => {
                                // For waveform records, get_field("VAL") returns
                                // NORD elements (valid data) but the channel's
                                // native count must be NELM (max capacity) so
                                // clients allocate the right buffer.
                                let element_count = if field == "VAL"
                                    && instance.record.record_type() == "waveform"
                                {
                                    instance
                                        .resolve_field("NELM")
                                        .and_then(|n| match n {
                                            EpicsValue::Long(n) => Some(n.max(0) as u32),
                                            _ => None,
                                        })
                                        .unwrap_or(v.count() as u32)
                                } else {
                                    v.count() as u32
                                };
                                (
                                    v.dbr_type(),
                                    element_count,
                                    ChannelTarget::RecordField {
                                        record: rec.clone(),
                                        field: field.clone(),
                                    },
                                )
                            }
                            None => {
                                // Field not found — send CREATE_CH_FAIL
                                let mut fail = CaHeader::new(CA_PROTO_CREATE_CH_FAIL);
                                fail.cid = client_cid;
                                let mut w = writer.lock().await;
                                w.write_all(&fail.to_bytes()).await?;
                                w.flush().await?;
                                return Ok(());
                            }
                        }
                    }
                };

                let access = state.compute_access(&target).await;
                let access_level = match access {
                    3 => AccessLevel::ReadWrite,
                    1 => AccessLevel::Read,
                    _ => AccessLevel::NoAccess,
                };

                state.channels.insert(
                    sid,
                    ChannelEntry {
                        target,
                        cid: client_cid,
                    },
                );
                state.channel_access.insert(sid, access_level);

                let mut ar = CaHeader::new(CA_PROTO_ACCESS_RIGHTS);
                ar.cid = client_cid;
                ar.available = access;

                let mut resp = CaHeader::new(CA_PROTO_CREATE_CHAN);
                resp.data_type = dbr_type as u16;
                resp.cid = client_cid;
                resp.available = sid;
                resp.set_payload_size(0, element_count);

                let mut w = writer.lock().await;
                w.write_all(&ar.to_bytes()).await?;
                w.write_all(&resp.to_bytes_extended()).await?;
                w.flush().await?;
            } else {
                // PV not found — send CREATE_CH_FAIL
                let mut fail = CaHeader::new(CA_PROTO_CREATE_CH_FAIL);
                fail.cid = client_cid;
                let mut w = writer.lock().await;
                w.write_all(&fail.to_bytes()).await?;
                w.flush().await?;
            }
        }

        CA_PROTO_READ | CA_PROTO_READ_NOTIFY => {
            let is_notify = hdr.cmmd == CA_PROTO_READ_NOTIFY;
            let sid = hdr.cid;
            let ioid = hdr.available;
            let requested_type = hdr.data_type;
            let requested_count = hdr.actual_count();

            let entry = match state.channels.get(&sid) {
                Some(e) => e,
                None => {
                    if is_notify {
                        send_cmd_error(
                            writer,
                            CA_PROTO_READ_NOTIFY,
                            requested_type,
                            ECA_BADCHID,
                            ioid,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };

            let snapshot = get_full_snapshot(&entry.target).await;
            let Some(mut snapshot) = snapshot else {
                if is_notify {
                    send_cmd_error(
                        writer,
                        CA_PROTO_READ_NOTIFY,
                        requested_type,
                        ECA_BADCHID,
                        ioid,
                    )
                    .await?;
                }
                return Ok(());
            };
            // Respect client's requested element count (e.g. caget -# 10)
            if requested_count > 0 && requested_count < snapshot.value.count() {
                snapshot.value.truncate(requested_count as usize);
            }
            let data = match encode_dbr(requested_type, &snapshot) {
                Ok(d) => d,
                Err(_) => {
                    if is_notify {
                        send_cmd_error(
                            writer,
                            CA_PROTO_READ_NOTIFY,
                            requested_type,
                            ECA_BADTYPE,
                            ioid,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };
            let element_count = snapshot.value.count() as u32;
            let mut padded = data;
            padded.resize(align8(padded.len()), 0);

            // For deprecated CA_PROTO_READ (cmd=3), the response carries the
            // SID in cid (not ECA status). Notify clients (cmd=15) get the
            // ECA_NORMAL status so they can demultiplex by ioid.
            let mut resp = if is_notify {
                let mut r = CaHeader::new(CA_PROTO_READ_NOTIFY);
                r.cid = ECA_NORMAL;
                r
            } else {
                let mut r = CaHeader::new(CA_PROTO_READ);
                r.cid = sid;
                r
            };
            // C client TCP parser requires 8-byte aligned postsize
            resp.set_payload_size(padded.len(), element_count);
            resp.data_type = requested_type;
            resp.available = ioid;

            let mut w = writer.lock().await;
            w.write_all(&resp.to_bytes_extended()).await?;
            w.write_all(&padded).await?;
            w.flush().await?;
        }

        CA_PROTO_WRITE | CA_PROTO_WRITE_NOTIFY => {
            let sid = hdr.cid;
            let ioid = hdr.available;
            let is_notify = hdr.cmmd == CA_PROTO_WRITE_NOTIFY;

            let write_type = match DbFieldType::from_u16(hdr.data_type) {
                Ok(t) => t,
                Err(_) => {
                    if is_notify {
                        send_cmd_error(
                            writer,
                            CA_PROTO_WRITE_NOTIFY,
                            hdr.data_type,
                            ECA_BADTYPE,
                            ioid,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };

            let entry = match state.channels.get(&sid) {
                Some(e) => e,
                None => {
                    if is_notify {
                        send_cmd_error(
                            writer,
                            CA_PROTO_WRITE_NOTIFY,
                            hdr.data_type,
                            ECA_BADCHID,
                            ioid,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };

            // Check access level
            let access = state
                .channel_access
                .get(&sid)
                .copied()
                .unwrap_or(AccessLevel::ReadWrite);
            if access != AccessLevel::ReadWrite {
                if is_notify {
                    let mut resp = CaHeader::new(CA_PROTO_WRITE_NOTIFY);
                    resp.data_type = write_type as u16;
                    resp.count = hdr.count;
                    resp.cid = ECA_NOWTACCESS;
                    resp.available = ioid;
                    let mut w = writer.lock().await;
                    w.write_all(&resp.to_bytes()).await?;
                    w.flush().await?;
                }
                return Ok(());
            }

            let count = hdr.actual_count() as usize;
            let write_count = hdr.count; // Echo back in response (matches C EPICS)
            let new_value = match EpicsValue::from_bytes_array(write_type, payload, count) {
                Ok(v) => v,
                Err(_) => {
                    if is_notify {
                        send_cmd_error(
                            writer,
                            CA_PROTO_WRITE_NOTIFY,
                            hdr.data_type,
                            ECA_BADTYPE,
                            ioid,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            };

            let write_result = match &entry.target {
                ChannelTarget::SimplePv(pv) => {
                    pv.set(new_value).await;
                    Ok(None)
                }
                ChannelTarget::RecordField { record, field } => {
                    let name = record.read().await.name.clone();
                    db.put_record_field_from_ca(&name, field, new_value).await
                }
            };

            // F1: CA_PROTO_WRITE (cmd=4) is fire-and-forget — no response
            if is_notify {
                let eca_status = match &write_result {
                    Ok(_) => ECA_NORMAL,
                    Err(e) => e.to_eca_status(),
                };

                // If async processing started (e.g. motor move), spawn a
                // background task to await completion and send the response.
                // This avoids blocking the client handler loop, which would
                // freeze all camonitor subscriptions on this connection.
                let completion_rx: Option<tokio::sync::oneshot::Receiver<()>> =
                    write_result.unwrap_or_default();

                if let Some(rx) = completion_rx {
                    let writer_c = writer.clone();
                    tokio::spawn(async move {
                        // Wait indefinitely for record processing to complete,
                        // matching C EPICS rsrv behavior. The task is cleaned up
                        // automatically if the client disconnects (rx sender dropped).
                        let _ = rx.await;

                        let mut resp = CaHeader::new(CA_PROTO_WRITE_NOTIFY);
                        resp.data_type = write_type as u16;
                        resp.count = write_count;
                        resp.cid = eca_status;
                        resp.available = ioid;

                        let mut w = writer_c.lock().await;
                        let _ = w.write_all(&resp.to_bytes()).await;
                        let _ = w.flush().await;
                    });
                } else {
                    // Synchronous completion — respond immediately
                    let mut resp = CaHeader::new(CA_PROTO_WRITE_NOTIFY);
                    resp.data_type = write_type as u16;
                    resp.count = write_count;
                    resp.cid = eca_status;
                    resp.available = ioid;

                    let mut w = writer.lock().await;
                    w.write_all(&resp.to_bytes()).await?;
                    w.flush().await?;
                }
            }
        }

        CA_PROTO_EVENT_ADD => {
            let sid = hdr.cid;
            let sub_id = hdr.available;
            let requested_type = hdr.data_type;

            // DoS guard: cap subscriptions per channel.
            let subs_for_channel = state
                .subscriptions
                .values()
                .filter(|s| s.channel_sid == sid)
                .count();
            if subs_for_channel >= max_subs_per_channel() {
                send_cmd_error(
                    writer,
                    CA_PROTO_EVENT_ADD,
                    requested_type,
                    ECA_ALLOCMEM,
                    sub_id,
                )
                .await?;
                return Ok(());
            }

            let native_type = match native_type_for_dbr(requested_type) {
                Ok(t) => t,
                Err(_) => {
                    send_cmd_error(
                        writer,
                        CA_PROTO_EVENT_ADD,
                        requested_type,
                        ECA_BADTYPE,
                        sub_id,
                    )
                    .await?;
                    return Ok(());
                }
            };

            let mask = if payload.len() >= 14 {
                u16::from_be_bytes([payload[12], payload[13]])
            } else {
                DBE_VALUE | DBE_ALARM
            };

            let entry = match state.channels.get(&sid) {
                Some(e) => e,
                None => {
                    send_cmd_error(
                        writer,
                        CA_PROTO_EVENT_ADD,
                        requested_type,
                        ECA_BADCHID,
                        sub_id,
                    )
                    .await?;
                    return Ok(());
                }
            };

            {
                match &entry.target {
                    ChannelTarget::SimplePv(pv) => {
                        let rx = pv.add_subscriber(sub_id, native_type, mask).await;

                        // Send initial value
                        let snap = pv.snapshot().await;
                        send_monitor_snapshot(writer, sub_id, requested_type, &snap).await?;

                        let task = spawn_monitor_sender(
                            pv.clone(),
                            sub_id,
                            requested_type,
                            writer.clone(),
                            state.flow_control.clone(),
                            rx,
                        );

                        state.subscriptions.insert(
                            sub_id,
                            SubscriptionEntry {
                                target: ChannelTarget::SimplePv(pv.clone()),
                                channel_sid: sid,
                                sub_id,
                                data_type: requested_type,
                                task,
                            },
                        );
                    }
                    ChannelTarget::RecordField { record, field } => {
                        let mut instance = record.write().await;
                        let rx = instance.add_subscriber(field, sub_id, native_type, mask);

                        // Send initial value with full metadata
                        if let Some(snap) = instance.snapshot_for_field(field) {
                            send_monitor_snapshot(writer, sub_id, requested_type, &snap).await?;
                        }

                        let writer_clone = writer.clone();
                        let flow_control = state.flow_control.clone();
                        let task = epics_base_rs::runtime::task::spawn(async move {
                            let mut rx = rx;
                            while let Some(mut event) = rx.recv().await {
                                if flow_control.is_paused() {
                                    let Some(coalesced) =
                                        flow_control.coalesce_while_paused(&mut rx, event).await
                                    else {
                                        break;
                                    };
                                    event = coalesced;
                                }
                                let payload_bytes =
                                    match encode_dbr(requested_type, &event.snapshot) {
                                        Ok(bytes) => bytes,
                                        Err(_) => break,
                                    };
                                let element_count = event.snapshot.value.count() as u32;
                                let mut padded = payload_bytes;
                                padded.resize(align8(padded.len()), 0);

                                let mut hdr = CaHeader::new(CA_PROTO_EVENT_ADD);
                                // C client TCP parser requires 8-byte aligned postsize
                                hdr.set_payload_size(padded.len(), element_count);
                                hdr.data_type = requested_type;
                                hdr.cid = 1; // ECA_NORMAL
                                hdr.available = sub_id;

                                let hdr_bytes = hdr.to_bytes_extended();
                                let mut w = writer_clone.lock().await;
                                if w.write_all(&hdr_bytes).await.is_err() {
                                    break;
                                }
                                if w.write_all(&padded).await.is_err() {
                                    break;
                                }
                                let _ = w.flush().await;
                            }
                        });

                        state.subscriptions.insert(
                            sub_id,
                            SubscriptionEntry {
                                target: ChannelTarget::RecordField {
                                    record: record.clone(),
                                    field: field.clone(),
                                },
                                channel_sid: sid,
                                sub_id,
                                data_type: requested_type,
                                task,
                            },
                        );
                    }
                }
            }
        }

        CA_PROTO_EVENT_CANCEL => {
            let sub_id = hdr.available;
            if let Some(sub) = state.subscriptions.remove(&sub_id) {
                sub.task.abort();
                match &sub.target {
                    ChannelTarget::SimplePv(pv) => {
                        pv.remove_subscriber(sub.sub_id).await;
                    }
                    ChannelTarget::RecordField { record, .. } => {
                        record.write().await.remove_subscriber(sub.sub_id);
                    }
                }

                // Per spec: send final EVENT_ADD response with count=0
                let mut resp = CaHeader::new(CA_PROTO_EVENT_ADD);
                resp.data_type = sub.data_type;
                resp.count = 0;
                resp.cid = ECA_NORMAL;
                resp.available = sub_id;
                let mut w = writer.lock().await;
                w.write_all(&resp.to_bytes()).await?;
                w.flush().await?;
            }
        }

        CA_PROTO_EVENTS_OFF | CA_PROTO_EVENTS_ON => {
            if hdr.cmmd == CA_PROTO_EVENTS_OFF {
                state.flow_control.pause();
            } else {
                state.flow_control.resume();
            }
        }

        CA_PROTO_READ_SYNC => {
            // READ_SYNC is a barrier/flush for previously queued responses.
            let mut w = writer.lock().await;
            w.flush().await?;
        }

        CA_PROTO_ECHO => {
            let resp = CaHeader::new(CA_PROTO_ECHO);
            let mut w = writer.lock().await;
            w.write_all(&resp.to_bytes()).await?;
            w.flush().await?;
        }

        CA_PROTO_SEARCH => {
            // TCP search — only supported for clients with minor version >= 4
            if state.client_minor_version < 4 {
                return Ok(());
            }
            let end = payload
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(payload.len());
            let pv_name = String::from_utf8_lossy(&payload[..end]).to_string();

            if db.has_name(&pv_name).await {
                // Reply: data_type = tcp_port, cid = 0 (INADDR_ANY), available = client's cid
                // 8-byte payload containing CA_MINOR_VERSION as u16
                let mut resp = CaHeader::new(CA_PROTO_SEARCH);
                resp.data_type = state.tcp_port;
                resp.set_payload_size(8, 0);
                resp.cid = 0; // INADDR_ANY — client uses TCP peer addr
                resp.available = hdr.available;

                let mut search_payload = [0u8; 8];
                search_payload[0..2].copy_from_slice(&CA_MINOR_VERSION.to_be_bytes());

                let mut w = writer.lock().await;
                w.write_all(&resp.to_bytes_extended()).await?;
                w.write_all(&search_payload).await?;
                w.flush().await?;
            } else if hdr.data_type == CA_DO_REPLY {
                // Explicit negative reply requested — send NOT_FOUND so the
                // client doesn't have to wait for a search timeout.
                let mut nf = CaHeader::new(CA_PROTO_NOT_FOUND);
                nf.data_type = CA_DO_REPLY;
                nf.count = CA_MINOR_VERSION;
                nf.cid = hdr.available;
                nf.available = hdr.available;
                let mut w = writer.lock().await;
                w.write_all(&nf.to_bytes()).await?;
                w.flush().await?;
            }
            // Otherwise silent — clients without CA_DO_REPLY treat absence
            // as "this server doesn't have it" and move on.
        }

        CA_PROTO_CLEAR_CHANNEL => {
            let sid = hdr.cid;
            let cid = hdr.available;
            if let Some(_entry) = state.channels.remove(&sid) {
                state.channel_access.remove(&sid);

                // Clean up subscriptions that belong to this channel
                let sub_ids: Vec<u32> = state
                    .subscriptions
                    .iter()
                    .filter(|(_, sub)| sub.channel_sid == sid)
                    .map(|(&id, _)| id)
                    .collect();
                for sub_id in sub_ids {
                    if let Some(sub) = state.subscriptions.remove(&sub_id) {
                        sub.task.abort();
                        match &sub.target {
                            ChannelTarget::SimplePv(pv) => {
                                pv.remove_subscriber(sub.sub_id).await;
                            }
                            ChannelTarget::RecordField { record, .. } => {
                                record.write().await.remove_subscriber(sub.sub_id);
                            }
                        }
                    }
                }

                let mut resp = CaHeader::new(CA_PROTO_CLEAR_CHANNEL);
                resp.data_type = hdr.data_type;
                resp.count = hdr.count;
                resp.cid = sid;
                resp.available = cid;
                let mut w = writer.lock().await;
                w.write_all(&resp.to_bytes()).await?;
                w.flush().await?;
            }
        }

        _ => {
            // Unknown command — send CA_PROTO_ERROR with ECA status and original header
            let error_msg = format!("Unsupported command {}", hdr.cmmd);
            send_ca_error(writer, hdr, ECA_INTERNAL, &error_msg).await?;
        }
    }

    Ok(())
}
async fn get_full_snapshot(
    target: &ChannelTarget,
) -> Option<epics_base_rs::server::snapshot::Snapshot> {
    match target {
        ChannelTarget::SimplePv(pv) => Some(pv.snapshot().await),
        ChannelTarget::RecordField { record, field } => {
            record.read().await.snapshot_for_field(field)
        }
    }
}

async fn send_monitor_snapshot(
    writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    sub_id: u32,
    data_type: u16,
    snapshot: &epics_base_rs::server::snapshot::Snapshot,
) -> CaResult<()> {
    let data = encode_dbr(data_type, snapshot)?;
    let element_count = snapshot.value.count() as u32;
    let mut padded = data;
    padded.resize(align8(padded.len()), 0);

    let mut resp = CaHeader::new(CA_PROTO_EVENT_ADD);
    // C client TCP parser requires 8-byte aligned postsize
    resp.set_payload_size(padded.len(), element_count);
    resp.data_type = data_type;
    resp.cid = 1; // ECA_NORMAL
    resp.available = sub_id;

    let mut w = writer.lock().await;
    w.write_all(&resp.to_bytes_extended()).await?;
    w.write_all(&padded).await?;
    w.flush().await?;
    Ok(())
}

/// Re-evaluate and re-send CA_PROTO_ACCESS_RIGHTS for all open channels.
/// Called when hostname or username changes.
async fn reeval_access_rights(
    state: &mut ClientState,
    writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
) -> CaResult<()> {
    if state.channels.is_empty() {
        return Ok(());
    }
    // Collect channel info first to avoid borrow conflict with compute_access
    let chan_info: Vec<(u32, u32, ChannelTarget)> = state
        .channels
        .iter()
        .map(|(&sid, entry)| (sid, entry.cid, entry.target.clone()))
        .collect();

    let mut w = writer.lock().await;
    for (sid, cid, target) in chan_info {
        let new_access = state.compute_access(&target).await;
        let new_level = match new_access {
            3 => AccessLevel::ReadWrite,
            1 => AccessLevel::Read,
            _ => AccessLevel::NoAccess,
        };
        state.channel_access.insert(sid, new_level);
        let mut ar = CaHeader::new(CA_PROTO_ACCESS_RIGHTS);
        ar.cid = cid;
        ar.available = new_access;
        w.write_all(&ar.to_bytes()).await?;
    }
    w.flush().await?;
    Ok(())
}

/// Send a command-specific zero-payload error response.
/// Used for READ_NOTIFY, WRITE_NOTIFY, and EVENT_ADD error replies.
async fn send_cmd_error(
    writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    cmd: u16,
    data_type: u16,
    eca_status: u32,
    ioid_or_subid: u32,
) -> CaResult<()> {
    let mut resp = CaHeader::new(cmd);
    resp.data_type = data_type;
    resp.count = 0;
    resp.cid = eca_status;
    resp.available = ioid_or_subid;
    let mut w = writer.lock().await;
    w.write_all(&resp.to_bytes()).await?;
    w.flush().await?;
    Ok(())
}

/// Send a CA_PROTO_ERROR response with the original header and an error message.
async fn send_ca_error(
    writer: &Arc<Mutex<BufWriter<OwnedWriteHalf>>>,
    original_hdr: &CaHeader,
    eca_status: u32,
    message: &str,
) -> CaResult<()> {
    let error_msg_bytes = pad_string(message);
    let payload_size = CaHeader::SIZE + error_msg_bytes.len();

    let mut resp = CaHeader::new(CA_PROTO_ERROR);
    resp.set_payload_size(payload_size, 0);
    resp.cid = eca_status;

    let mut w = writer.lock().await;
    w.write_all(&resp.to_bytes_extended()).await?;
    w.write_all(&original_hdr.to_bytes()).await?;
    w.write_all(&error_msg_bytes).await?;
    w.flush().await?;
    Ok(())
}
