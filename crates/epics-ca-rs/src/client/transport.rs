use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use epics_base_rs::runtime::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::channel::AccessRights;
use crate::protocol::*;

use super::types::{TransportCommand, TransportEvent};

/// Timeout for echo response before declaring connection dead (matches C EPICS CA_ECHO_TIMEOUT).
const ECHO_TIMEOUT_SECS: u64 = 5;

/// Maximum accumulated TCP read buffer before disconnecting.
/// Protects against malformed servers declaring huge payloads.
const MAX_ACCUMULATED: usize = 1024 * 1024; // 1 MB

/// Send buffer backpressure threshold (matches C EPICS flushBlockThreshold).
/// If more than this many frames are pending, the connection is stalled.
const SEND_BACKPRESSURE_FRAMES: usize = 4096;

/// Default echo interval in seconds (matches C EPICS CA_CONN_VERIFY_PERIOD).
/// Overridden by EPICS_CA_CONN_TMO environment variable.
fn echo_idle_secs() -> u64 {
    epics_base_rs::runtime::env::get("EPICS_CA_CONN_TMO")
        .and_then(|s| s.parse::<f64>().ok())
        .map(|v| v.max(1.0) as u64)
        .unwrap_or(30)
}

struct ServerConnection {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pending_frames: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Signal read_loop to enter echo_pending mode immediately
    /// (beacon anomaly → fast dead-connection detection).
    echo_probe: std::sync::Arc<tokio::sync::Notify>,
    _read_task: tokio::task::JoinHandle<()>,
    _write_task: tokio::task::JoinHandle<()>,
}

pub(crate) async fn run_transport_manager(
    mut command_rx: mpsc::UnboundedReceiver<TransportCommand>,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
) {
    let mut connections: HashMap<SocketAddr, ServerConnection> = HashMap::new();

    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            TransportCommand::CreateChannel {
                cid,
                pv_name,
                server_addr,
            } => {
                // Ensure we have a connection to this server
                if !connections.contains_key(&server_addr) {
                    match connect_server(server_addr, event_tx.clone()).await {
                        Some(conn) => {
                            connections.insert(server_addr, conn);
                        }
                        None => {
                            let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                            continue;
                        }
                    }
                }

                // Check connection is still alive (both tasks running)
                let alive = connections
                    .get(&server_addr)
                    .map(|c| !c._read_task.is_finished() && !c._write_task.is_finished())
                    .unwrap_or(false);

                if !alive {
                    // Abort lingering tasks before creating a new connection
                    if let Some(old) = connections.remove(&server_addr) {
                        old._read_task.abort();
                        old._write_task.abort();
                    }
                    match connect_server(server_addr, event_tx.clone()).await {
                        Some(conn) => {
                            connections.insert(server_addr, conn);
                        }
                        None => {
                            let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                            continue;
                        }
                    }
                }

                let pv_payload = pad_string(&pv_name);
                let mut create_hdr = CaHeader::new(CA_PROTO_CREATE_CHAN);
                create_hdr.postsize = pv_payload.len() as u16;
                create_hdr.cid = cid;
                create_hdr.available = CA_MINOR_VERSION as u32;

                let mut frame = create_hdr.to_bytes().to_vec();
                frame.extend_from_slice(&pv_payload);
                send_frame(&mut connections, server_addr, frame, &event_tx);
            }
            TransportCommand::ReadNotify {
                sid,
                data_type,
                count,
                ioid,
                server_addr,
            } => {
                let mut hdr = CaHeader::new(CA_PROTO_READ_NOTIFY);
                hdr.data_type = data_type;
                hdr.cid = sid;
                hdr.available = ioid;
                if count > 0xFFFF {
                    hdr.set_payload_size(0, count);
                } else {
                    hdr.count = count as u16;
                }
                send_frame(
                    &mut connections,
                    server_addr,
                    hdr.to_bytes_extended(),
                    &event_tx,
                );
            }
            TransportCommand::Write {
                sid,
                data_type,
                count,
                payload,
                server_addr,
            } => {
                let padded_len = align8(payload.len());
                let mut padded = payload;
                padded.resize(padded_len, 0);

                let mut hdr = CaHeader::new(CA_PROTO_WRITE);
                hdr.data_type = data_type;
                hdr.cid = sid;
                hdr.set_payload_size(padded.len(), count);

                let mut frame = hdr.to_bytes_extended();
                frame.extend_from_slice(&padded);
                send_frame(&mut connections, server_addr, frame, &event_tx);
            }
            TransportCommand::WriteNotify {
                sid,
                data_type,
                count,
                ioid,
                payload,
                server_addr,
            } => {
                let padded_len = align8(payload.len());
                let mut padded = payload;
                padded.resize(padded_len, 0);

                let mut hdr = CaHeader::new(CA_PROTO_WRITE_NOTIFY);
                hdr.data_type = data_type;
                hdr.cid = sid;
                hdr.available = ioid;
                hdr.set_payload_size(padded.len(), count);

                let mut frame = hdr.to_bytes_extended();
                frame.extend_from_slice(&padded);
                send_frame(&mut connections, server_addr, frame, &event_tx);
            }
            TransportCommand::Subscribe {
                sid,
                data_type,
                count,
                subid,
                mask,
                server_addr,
            } => {
                let mut hdr = CaHeader::new(CA_PROTO_EVENT_ADD);
                hdr.postsize = 16;
                hdr.data_type = data_type;
                hdr.cid = sid;
                hdr.available = subid;
                if count > 0xFFFF {
                    hdr.set_payload_size(16, count);
                } else {
                    hdr.count = count as u16;
                }

                let mut mask_payload = [0u8; 16];
                mask_payload[12..14].copy_from_slice(&mask.to_be_bytes());

                let mut frame = hdr.to_bytes_extended();
                frame.extend_from_slice(&mask_payload);
                send_frame(&mut connections, server_addr, frame, &event_tx);
            }
            TransportCommand::Unsubscribe {
                sid,
                subid,
                data_type,
                server_addr,
            } => {
                let mut hdr = CaHeader::new(CA_PROTO_EVENT_CANCEL);
                hdr.data_type = data_type;
                hdr.cid = sid;
                hdr.available = subid;
                send_frame(
                    &mut connections,
                    server_addr,
                    hdr.to_bytes().to_vec(),
                    &event_tx,
                );
            }
            TransportCommand::ClearChannel {
                cid,
                sid,
                server_addr,
            } => {
                let mut hdr = CaHeader::new(CA_PROTO_CLEAR_CHANNEL);
                hdr.cid = sid;
                hdr.available = cid;
                send_frame(
                    &mut connections,
                    server_addr,
                    hdr.to_bytes().to_vec(),
                    &event_tx,
                );
            }
            TransportCommand::EchoProbe { server_addr } => {
                // Beacon anomaly detected — wake the read_loop so it
                // immediately enters echo_pending mode with a 5s timeout
                // instead of waiting for the 30s idle timeout.
                if let Some(conn) = connections.get(&server_addr) {
                    conn.echo_probe.notify_one();
                }
            }
        }
    }
}

fn send_frame(
    connections: &mut HashMap<SocketAddr, ServerConnection>,
    server_addr: SocketAddr,
    frame: Vec<u8>,
    event_tx: &mpsc::UnboundedSender<TransportEvent>,
) {
    let failed = if let Some(conn) = connections.get(&server_addr) {
        let pending = conn
            .pending_frames
            .load(std::sync::atomic::Ordering::Relaxed);
        if pending >= SEND_BACKPRESSURE_FRAMES {
            eprintln!("CA: {server_addr}: send buffer stalled ({pending} frames pending), closing");
            true
        } else {
            conn.pending_frames
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            conn.write_tx.send(frame).is_err()
        }
    } else {
        false
    };
    if failed {
        connections.remove(&server_addr);
        let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
    }
}

async fn connect_server(
    server_addr: SocketAddr,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
) -> Option<ServerConnection> {
    let stream = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        TcpStream::connect(server_addr),
    )
    .await
    .ok()?
    .ok()?;

    let _ = stream.set_nodelay(true);

    // TCP keepalive: detect dead connections on idle circuits.
    // OS sends probes after 15s idle, every 5s, giving up after 3 failures (~30s total).
    {
        let sock = socket2::SockRef::from(&stream);
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(15))
            .with_interval(Duration::from_secs(5));
        let _ = sock.set_keepalive(true);
        let _ = sock.set_tcp_keepalive(&keepalive);
    }

    let (reader, write_half) = stream.into_split();
    let (write_tx, write_rx) = mpsc::unbounded_channel();
    let pending_frames = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let echo_probe = std::sync::Arc::new(tokio::sync::Notify::new());

    // Build initial handshake as a single frame (VERSION + HOST + CLIENT)
    let mut handshake = Vec::new();

    let mut version_hdr = CaHeader::new(CA_PROTO_VERSION);
    version_hdr.count = CA_MINOR_VERSION;
    handshake.extend_from_slice(&version_hdr.to_bytes());

    let hostname = epics_base_rs::runtime::env::hostname();
    let host_payload = pad_string(&hostname);
    let mut host_hdr = CaHeader::new(CA_PROTO_HOST_NAME);
    host_hdr.postsize = host_payload.len() as u16;
    handshake.extend_from_slice(&host_hdr.to_bytes());
    handshake.extend_from_slice(&host_payload);

    let username = epics_base_rs::runtime::env::get("USER")
        .or_else(|| epics_base_rs::runtime::env::get("USERNAME"))
        .unwrap_or_else(|| "unknown".to_string());
    let user_payload = pad_string(&username);
    let mut user_hdr = CaHeader::new(CA_PROTO_CLIENT_NAME);
    user_hdr.postsize = user_payload.len() as u16;
    handshake.extend_from_slice(&user_hdr.to_bytes());
    handshake.extend_from_slice(&user_payload);

    pending_frames.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let _ = write_tx.send(handshake);

    let write_task = epics_base_rs::runtime::task::spawn(write_loop(
        write_half,
        write_rx,
        server_addr,
        event_tx.clone(),
        pending_frames.clone(),
    ));
    let read_task = epics_base_rs::runtime::task::spawn(read_loop(
        reader,
        server_addr,
        event_tx,
        write_tx.clone(),
        echo_probe.clone(),
    ));

    Some(ServerConnection {
        write_tx,
        pending_frames,
        echo_probe,
        _read_task: read_task,
        _write_task: write_task,
    })
}

async fn write_loop(
    mut writer: tokio::net::tcp::OwnedWriteHalf,
    mut rx: mpsc::UnboundedReceiver<Vec<u8>>,
    server_addr: SocketAddr,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
    pending_frames: std::sync::Arc<std::sync::atomic::AtomicUsize>,
) {
    // Send watchdog: if write stalls for 2x echo timeout, declare circuit dead.
    // Matches C EPICS tcpSendWatchdog behavior.
    let send_timeout = Duration::from_secs(ECHO_TIMEOUT_SECS * 2);
    let mut batch = Vec::with_capacity(4096);
    while let Some(frame) = rx.recv().await {
        let mut drained: usize = 1;
        batch.extend_from_slice(&frame);
        // Drain all pending frames into a single write
        while let Ok(frame) = rx.try_recv() {
            batch.extend_from_slice(&frame);
            drained += 1;
        }
        match tokio::time::timeout(send_timeout, writer.write_all(&batch)).await {
            Ok(Ok(())) => {
                batch.clear();
                // Saturating: read_loop also sends frames (echo, flow
                // control) that bypass send_frame's increment.
                let prev = pending_frames.load(std::sync::atomic::Ordering::Relaxed);
                pending_frames.store(
                    prev.saturating_sub(drained),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
            Ok(Err(_)) | Err(_) => {
                let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                return;
            }
        }
    }
}

async fn read_loop(
    mut reader: tokio::net::tcp::OwnedReadHalf,
    server_addr: SocketAddr,
    event_tx: mpsc::UnboundedSender<TransportEvent>,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    echo_probe: std::sync::Arc<tokio::sync::Notify>,
) {
    let mut buf = vec![0u8; 8192];
    let mut accumulated = Vec::new();
    let idle_timeout = Duration::from_secs(echo_idle_secs());
    let echo_timeout = Duration::from_secs(ECHO_TIMEOUT_SECS);
    let mut echo_pending = false;
    let mut unresponsive_notified = false;
    let mut server_minor_version: u16 = 0;

    // Monitor flow control (C EPICS contiguousMsgCountWhichTriggersFlowControl)
    let mut contiguous_read_count: u32 = 0;
    let mut flow_control_active = false;
    const FLOW_CONTROL_THRESHOLD: u32 = 10;

    loop {
        let timeout = if echo_pending {
            echo_timeout
        } else {
            idle_timeout
        };

        let read_result = tokio::select! {
            result = tokio::time::timeout(timeout, reader.read(&mut buf)) => result,
            () = echo_probe.notified(), if !echo_pending => {
                // Beacon anomaly — immediately send echo probe and
                // switch to the short 5s echo timeout.
                let cmd = if server_minor_version >= 3 {
                    CA_PROTO_ECHO
                } else {
                    CA_PROTO_READ_SYNC
                };
                let echo_hdr = CaHeader::new(cmd);
                if write_tx.send(echo_hdr.to_bytes().to_vec()).is_err() {
                    let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                    return;
                }
                echo_pending = true;
                continue;
            }
        };
        let n = match read_result {
            Ok(Ok(0)) | Ok(Err(_)) => {
                let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                return;
            }
            Ok(Ok(n)) => n,
            Err(_) => {
                // Timeout expired — we caught up (no contiguous data)
                contiguous_read_count = 0;
                if flow_control_active {
                    let hdr = CaHeader::new(CA_PROTO_EVENTS_ON);
                    let _ = write_tx.send(hdr.to_bytes().to_vec());
                    flow_control_active = false;
                }

                if echo_pending {
                    if !unresponsive_notified {
                        // First echo timeout: mark unresponsive, try one more echo
                        let _ = event_tx.send(TransportEvent::CircuitUnresponsive { server_addr });
                        unresponsive_notified = true;
                        let cmd = if server_minor_version >= 3 {
                            CA_PROTO_ECHO
                        } else {
                            CA_PROTO_READ_SYNC
                        };
                        let echo_hdr = CaHeader::new(cmd);
                        if write_tx.send(echo_hdr.to_bytes().to_vec()).is_err() {
                            let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                            return;
                        }
                        continue;
                    }
                    // Second echo timeout — truly dead
                    let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                    return;
                }
                // Idle timeout — send echo heartbeat
                // Use READ_SYNC for pre-v4.3 servers that don't understand ECHO
                let cmd = if server_minor_version >= 3 {
                    CA_PROTO_ECHO
                } else {
                    CA_PROTO_READ_SYNC
                };
                let echo_hdr = CaHeader::new(cmd);
                if write_tx.send(echo_hdr.to_bytes().to_vec()).is_err() {
                    let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
                    return;
                }
                echo_pending = true;
                continue;
            }
        };

        // Data received — connection is alive
        echo_pending = false;
        if unresponsive_notified {
            unresponsive_notified = false;
            let _ = event_tx.send(TransportEvent::CircuitResponsive { server_addr });
        }

        // Flow control: count contiguous reads without a gap
        contiguous_read_count += 1;
        if !flow_control_active && contiguous_read_count >= FLOW_CONTROL_THRESHOLD {
            let hdr = CaHeader::new(CA_PROTO_EVENTS_OFF);
            let _ = write_tx.send(hdr.to_bytes().to_vec());
            flow_control_active = true;
        }
        accumulated.extend_from_slice(&buf[..n]);

        // Guard against unbounded buffer growth from malformed servers.
        if accumulated.len() > MAX_ACCUMULATED {
            eprintln!(
                "CA: {server_addr}: accumulated TCP buffer exceeded {} bytes, closing",
                MAX_ACCUMULATED
            );
            let _ = event_tx.send(TransportEvent::TcpClosed { server_addr });
            return;
        }

        let mut offset = 0;
        while offset + CaHeader::SIZE <= accumulated.len() {
            let (hdr, hdr_size) = match CaHeader::from_bytes_extended(&accumulated[offset..]) {
                Ok(v) => v,
                Err(_) => {
                    eprintln!("CA: {server_addr}: malformed TCP header, skipping");
                    break;
                }
            };
            let actual_post = hdr.actual_postsize();
            let msg_len = hdr_size + align8(actual_post);

            if offset + msg_len > accumulated.len() {
                break;
            }

            let data_start = offset + hdr_size;
            let data_end = data_start + actual_post;

            // Defense-in-depth: verify payload is within buffer bounds
            // even though msg_len check above should guarantee this.
            if data_end > accumulated.len() {
                eprintln!("CA: {server_addr}: payload exceeds buffer bounds, skipping");
                break;
            }

            match hdr.cmmd {
                CA_PROTO_VERSION => {
                    server_minor_version = hdr.count;
                }
                CA_PROTO_ACCESS_RIGHTS => {
                    let _ = event_tx.send(TransportEvent::AccessRightsChanged {
                        cid: hdr.cid,
                        access: AccessRights::from_u32(hdr.available),
                    });
                }
                CA_PROTO_CREATE_CHAN => {
                    let _ = event_tx.send(TransportEvent::ChannelCreated {
                        cid: hdr.cid,
                        sid: hdr.available,
                        data_type: hdr.data_type,
                        element_count: hdr.actual_count(),
                        access: AccessRights::from_u32(0x3),
                        server_addr,
                    });
                }
                CA_PROTO_READ_NOTIFY => {
                    if hdr.cid == ECA_NORMAL {
                        let data = accumulated[data_start..data_start + actual_post].to_vec();
                        let _ = event_tx.send(TransportEvent::ReadResponse {
                            ioid: hdr.available,
                            data_type: hdr.data_type,
                            count: hdr.actual_count(),
                            data,
                        });
                    } else {
                        let _ = event_tx.send(TransportEvent::ReadError {
                            ioid: hdr.available,
                            eca_status: hdr.cid,
                        });
                    }
                }
                CA_PROTO_WRITE_NOTIFY => {
                    let _ = event_tx.send(TransportEvent::WriteResponse {
                        ioid: hdr.available,
                        status: hdr.cid,
                    });
                }
                CA_PROTO_EVENT_ADD => {
                    let data = accumulated[data_start..data_start + actual_post].to_vec();
                    let _ = event_tx.send(TransportEvent::MonitorData {
                        subid: hdr.available,
                        data_type: hdr.data_type,
                        count: hdr.actual_count(),
                        data,
                    });
                }
                CA_PROTO_ECHO | CA_PROTO_READ_SYNC => {
                    // Echo response from server — liveness already handled
                    // above (echo_pending=false).  Do NOT echo back; only
                    // the server echoes requests.  Responding here would
                    // create a tight ping-pong loop.
                }
                CA_PROTO_CREATE_CH_FAIL => {
                    let _ = event_tx.send(TransportEvent::ChannelCreateFailed { cid: hdr.cid });
                }
                CA_PROTO_ERROR => {
                    let orig_cmd = if actual_post >= 16 {
                        let orig_hdr_bytes = &accumulated[data_start..data_start + 16];
                        Some(u16::from_be_bytes([orig_hdr_bytes[0], orig_hdr_bytes[1]]))
                    } else {
                        None
                    };
                    let msg = if actual_post > 16 {
                        let msg_bytes = &accumulated[data_start + 16..data_start + actual_post];
                        let end = msg_bytes
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(msg_bytes.len());
                        String::from_utf8_lossy(&msg_bytes[..end]).to_string()
                    } else {
                        String::new()
                    };
                    eprintln!("CA server error: cmd={:?} msg={}", orig_cmd, msg);
                    let _ = event_tx.send(TransportEvent::ServerError {
                        _original_request: orig_cmd,
                        _message: msg,
                    });
                }
                CA_PROTO_SERVER_DISCONN => {
                    let _ = event_tx.send(TransportEvent::ServerDisconnect {
                        cid: hdr.cid,
                        server_addr,
                    });
                }
                _ => {}
            }

            offset += msg_len;
        }

        if offset > 0 {
            accumulated.drain(..offset);
        }
    }
}
