//! Background search engine.
//!
//! Owns:
//!
//! - A **search socket** (ephemeral UDP) used to broadcast SEARCH requests
//!   and receive SEARCH_RESPONSE messages.
//! - A **beacon socket** (UDP bound to 5076 with `SO_REUSEPORT`) used to
//!   listen for unsolicited server BEACON messages.
//!
//! The engine drives:
//!
//! - Per-PV search retry with pvxs-style backoff (15s → 30s → 60s → 120s
//!   → 210s capped).
//! - Beacon-driven fast reconnect: when a beacon arrives for a server we
//!   have a disconnected channel against, the engine re-issues SEARCH for
//!   that channel immediately.
//! - Beacon anomaly throttling via [`super::beacon_throttle::BeaconTracker`].

use std::collections::HashMap;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};
use tokio::time::interval;
use tracing::{debug, warn};

use crate::codec::PvaCodec;
use crate::error::{PvaError, PvaResult};
use crate::proto::{
    decode_size, decode_string, ip_from_bytes, ByteOrder, Command, PvaHeader, ReadExt,
};

use super::beacon_throttle::BeaconTracker;
use super::decode::{decode_search_response, try_parse_frame};

/// Search retry backoff sequence (seconds), matching pvxs `clientdiscover.cpp`.
pub const BACKOFF_SECS: &[u64] = &[1, 1, 2, 5, 10, 15, 30, 60, 120, 210];

/// Default UDP broadcast port for SEARCH/BEACON messages (5076).
pub const DEFAULT_BROADCAST_PORT: u16 = 5076;

/// Command sent into the engine.
pub enum SearchCommand {
    /// Resolve `pv_name` → first server address. Reply via `responder`
    /// once a SEARCH_RESPONSE comes in.
    Find {
        pv_name: String,
        responder: oneshot::Sender<SocketAddr>,
    },
    /// Resolve `pv_name` and collect *all* responses received within
    /// the next [`MULTI_SERVER_WINDOW`]. The reply contains every
    /// server that claimed the PV; the caller can fan-out / fail over.
    FindAll {
        pv_name: String,
        responder: oneshot::Sender<Vec<SocketAddr>>,
    },
    /// Cancel an outstanding search (channel was dropped or closed).
    Cancel { pv_name: String },
    /// Notify the engine that we observed a beacon — used by external code
    /// (e.g. when running an embedded server in the same process) to feed
    /// beacons into the throttle without binding the multicast port.
    BeaconObserved {
        server: SocketAddr,
        guid: [u8; 12],
    },
}

/// How long the engine collects extra SEARCH_RESPONSE entries after the
/// first one for a given pv name (used by [`SearchCommand::FindAll`]).
pub const MULTI_SERVER_WINDOW: Duration = Duration::from_millis(200);

/// Public handle to the engine. Cheap to clone (it's just a sender).
#[derive(Clone)]
pub struct SearchEngine {
    cmd_tx: mpsc::Sender<SearchCommand>,
    pub beacons: Arc<BeaconTracker>,
}

impl SearchEngine {
    /// Spawn the engine. Returns a handle that channels use to issue
    /// `find()` requests.
    pub async fn spawn(extra_targets: Vec<SocketAddr>) -> PvaResult<Self> {
        let beacons = BeaconTracker::new();
        let (cmd_tx, cmd_rx) = mpsc::channel::<SearchCommand>(256);

        let search_socket = bind_ephemeral_udp()?;
        let beacon_socket = bind_beacon_udp(); // Optional — may be None.

        let beacons_clone = beacons.clone();
        tokio::spawn(run_engine(
            cmd_rx,
            search_socket,
            beacon_socket,
            extra_targets,
            beacons_clone,
        ));

        Ok(Self { cmd_tx, beacons })
    }

    /// Issue a search for `pv_name`. Future resolves to the server address
    /// once a response arrives.
    pub async fn find(&self, pv_name: &str) -> PvaResult<SocketAddr> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(SearchCommand::Find {
                pv_name: pv_name.to_string(),
                responder: tx,
            })
            .await
            .map_err(|_| PvaError::Protocol("search engine closed".into()))?;
        rx.await
            .map_err(|_| PvaError::Protocol("search request cancelled".into()))
    }

    /// Collect every SEARCH_RESPONSE for `pv_name` within
    /// [`MULTI_SERVER_WINDOW`]. Returns a ranked list — first is the
    /// fastest responder. Empty list means the search timed out.
    pub async fn find_all(&self, pv_name: &str) -> PvaResult<Vec<SocketAddr>> {
        let (tx, rx) = oneshot::channel();
        self.cmd_tx
            .send(SearchCommand::FindAll {
                pv_name: pv_name.to_string(),
                responder: tx,
            })
            .await
            .map_err(|_| PvaError::Protocol("search engine closed".into()))?;
        rx.await
            .map_err(|_| PvaError::Protocol("search request cancelled".into()))
    }

    pub async fn cancel(&self, pv_name: &str) {
        let _ = self
            .cmd_tx
            .send(SearchCommand::Cancel {
                pv_name: pv_name.to_string(),
            })
            .await;
    }

    pub async fn observe_beacon(&self, server: SocketAddr, guid: [u8; 12]) {
        let _ = self
            .cmd_tx
            .send(SearchCommand::BeaconObserved { server, guid })
            .await;
    }
}

// ── UDP socket helpers ──────────────────────────────────────────────────

fn bind_ephemeral_udp() -> PvaResult<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    sock.set_broadcast(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&"0.0.0.0:0".parse::<SocketAddr>().unwrap().into())?;
    let std_sock: StdUdpSocket = sock.into();
    UdpSocket::from_std(std_sock).map_err(PvaError::Io)
}

fn bind_beacon_udp() -> Option<UdpSocket> {
    let port = std::env::var("EPICS_PVA_BROADCAST_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_BROADCAST_PORT);
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    sock.set_reuse_address(true).ok()?;
    #[cfg(unix)]
    {
        let _ = sock.set_reuse_port(true);
    }
    sock.set_broadcast(true).ok()?;
    sock.set_nonblocking(true).ok()?;
    let bind: SocketAddr = format!("0.0.0.0:{port}").parse().ok()?;
    if sock.bind(&bind.into()).is_err() {
        debug!("beacon socket bind to {port} failed (likely in-use); fast-reconnect disabled");
        return None;
    }
    let std_sock: StdUdpSocket = sock.into();
    UdpSocket::from_std(std_sock).ok()
}

// ── Engine main loop ────────────────────────────────────────────────────

enum Responder {
    Single(oneshot::Sender<SocketAddr>),
    Multi {
        responder: oneshot::Sender<Vec<SocketAddr>>,
        accumulated: Vec<SocketAddr>,
        deadline: Instant,
    },
}

struct Pending {
    pv_name: String,
    responder: Responder,
    last_attempt: Instant,
    attempt: usize,
    search_id: u32,
}

async fn run_engine(
    mut cmd_rx: mpsc::Receiver<SearchCommand>,
    search_socket: UdpSocket,
    beacon_socket: Option<UdpSocket>,
    extra_targets: Vec<SocketAddr>,
    beacons: Arc<BeaconTracker>,
) {
    static NEXT_SEARCH_ID: AtomicU32 = AtomicU32::new(1);

    let codec = PvaCodec { big_endian: false };
    let response_port = search_socket.local_addr().map(|a| a.port()).unwrap_or(0);

    let mut pending: HashMap<u32, Pending> = HashMap::new(); // by search_id
    let mut by_name: HashMap<String, u32> = HashMap::new(); // pv_name → search_id

    let mut tick = interval(Duration::from_secs(1));
    let mut search_buf = vec![0u8; 4096];
    let mut beacon_buf = vec![0u8; 4096];

    loop {
        // Build a beacon-recv future regardless of whether we bound it
        // (using `if let` to keep the select! shape simple).
        let beacon_recv = async {
            match &beacon_socket {
                Some(s) => s.recv_from(&mut beacon_buf).await,
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            cmd = cmd_rx.recv() => match cmd {
                Some(SearchCommand::Find { pv_name, responder }) => {
                    let sid = NEXT_SEARCH_ID.fetch_add(1, Ordering::Relaxed);
                    let p = Pending {
                        pv_name: pv_name.clone(),
                        responder: Responder::Single(responder),
                        last_attempt: Instant::now(),
                        attempt: 0,
                        search_id: sid,
                    };
                    by_name.insert(pv_name, sid);
                    pending.insert(sid, p);
                    if let Some(p) = pending.get(&sid) {
                        let pkt = codec.build_search(0, sid, &p.pv_name, [0,0,0,0], response_port, false);
                        broadcast(&search_socket, &pkt, &extra_targets).await;
                    }
                }
                Some(SearchCommand::FindAll { pv_name, responder }) => {
                    let sid = NEXT_SEARCH_ID.fetch_add(1, Ordering::Relaxed);
                    let p = Pending {
                        pv_name: pv_name.clone(),
                        responder: Responder::Multi {
                            responder,
                            accumulated: Vec::new(),
                            deadline: Instant::now() + MULTI_SERVER_WINDOW,
                        },
                        last_attempt: Instant::now(),
                        attempt: 0,
                        search_id: sid,
                    };
                    by_name.insert(pv_name, sid);
                    pending.insert(sid, p);
                    if let Some(p) = pending.get(&sid) {
                        let pkt = codec.build_search(0, sid, &p.pv_name, [0,0,0,0], response_port, false);
                        broadcast(&search_socket, &pkt, &extra_targets).await;
                    }
                }
                Some(SearchCommand::Cancel { pv_name }) => {
                    if let Some(sid) = by_name.remove(&pv_name) {
                        pending.remove(&sid);
                    }
                }
                Some(SearchCommand::BeaconObserved { server, guid }) => {
                    if beacons.observe(server, guid) {
                        // Wake all pending searches — if any of them
                        // resolves to this server they'll get retried
                        // immediately on the next tick.
                        for p in pending.values_mut() {
                            p.last_attempt = Instant::now() - Duration::from_secs(60);
                        }
                    }
                }
                None => break,
            },

            res = search_socket.recv_from(&mut search_buf) => {
                if let Ok((n, _from)) = res {
                    handle_search_response(&search_buf[..n], &mut pending, &mut by_name, &beacons);
                }
            }

            res = beacon_recv => {
                if let Ok((n, _from)) = res {
                    handle_beacon(&beacon_buf[..n], &beacons, &mut pending);
                }
            }

            _ = tick.tick() => {
                let now = Instant::now();

                // 1. Flush any FindAll multi-window responders whose deadline
                //    has passed (delivers accumulated list, removes entry).
                let mut to_flush = Vec::new();
                for (sid, p) in pending.iter() {
                    if let Responder::Multi { deadline, accumulated, .. } = &p.responder {
                        if now >= *deadline && !accumulated.is_empty() {
                            to_flush.push(*sid);
                        }
                    }
                }
                for sid in to_flush {
                    if let Some(p) = pending.remove(&sid) {
                        by_name.remove(&p.pv_name);
                        if let Responder::Multi { responder, accumulated, .. } = p.responder {
                            let _ = responder.send(accumulated);
                        }
                    }
                }

                // 2. Retry pending searches whose backoff has elapsed.
                for p in pending.values_mut() {
                    let backoff = BACKOFF_SECS[p.attempt.min(BACKOFF_SECS.len()-1)];
                    if now.duration_since(p.last_attempt) >= Duration::from_secs(backoff) {
                        p.last_attempt = now;
                        p.attempt = (p.attempt + 1).min(BACKOFF_SECS.len() - 1);
                        let pkt = codec.build_search(0, p.search_id, &p.pv_name, [0,0,0,0], response_port, false);
                        broadcast(&search_socket, &pkt, &extra_targets).await;
                    }
                }
            }
        }
    }
}

async fn broadcast(socket: &UdpSocket, packet: &[u8], extra_targets: &[SocketAddr]) {
    let mut targets: Vec<SocketAddr> = Vec::with_capacity(8);

    // Limited broadcast to default UDP port.
    let bport = std::env::var("EPICS_PVA_BROADCAST_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_BROADCAST_PORT);
    targets.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), bport));

    if let Ok(env) = std::env::var("EPICS_PVA_ADDR_LIST") {
        for tok in env.split(|c: char| c == ',' || c.is_whitespace()) {
            let tok = tok.trim();
            if tok.is_empty() {
                continue;
            }
            if let Ok(sa) = tok.parse::<SocketAddr>() {
                targets.push(sa);
            } else if let Ok(ip) = tok.parse::<IpAddr>() {
                targets.push(SocketAddr::new(ip, bport));
            }
        }
    }
    for &t in extra_targets {
        targets.push(t);
    }

    for t in targets {
        if let Err(e) = socket.send_to(packet, t).await {
            debug!("search broadcast to {t} failed: {e}");
        }
    }
}

fn handle_search_response(
    bytes: &[u8],
    pending: &mut HashMap<u32, Pending>,
    by_name: &mut HashMap<String, u32>,
    _beacons: &Arc<BeaconTracker>,
) {
    let Ok(Some((frame, _))) = try_parse_frame(bytes) else {
        return;
    };
    let Ok(resp) = decode_search_response(&frame) else {
        return;
    };
    if !resp.found {
        return;
    }
    for cid in resp.cids {
        let server = rewrite_loopback(resp.server_addr);
        let Some(entry) = pending.get_mut(&cid) else {
            continue;
        };
        match &mut entry.responder {
            Responder::Single(_) => {
                // Single responder: deliver and remove.
                let p = pending.remove(&cid).unwrap();
                by_name.remove(&p.pv_name);
                if let Responder::Single(tx) = p.responder {
                    let _ = tx.send(server);
                }
            }
            Responder::Multi { accumulated, .. } => {
                if !accumulated.contains(&server) {
                    accumulated.push(server);
                }
                // Don't deliver yet — wait for the deadline tick to flush.
            }
        }
    }
}

fn handle_beacon(
    bytes: &[u8],
    beacons: &Arc<BeaconTracker>,
    pending: &mut HashMap<u32, Pending>,
) {
    let Ok(Some((frame, _))) = try_parse_frame(bytes) else {
        return;
    };
    if frame.header.command != Command::Beacon.code() {
        return;
    }
    let order = frame.header.flags.byte_order();
    let mut cur = Cursor::new(frame.payload.as_slice());
    let Ok(guid) = cur.get_bytes(12) else {
        return;
    };
    let Ok(_flags) = cur.get_u8() else {
        return;
    };
    let Ok(_seq) = cur.get_u16(order) else {
        return;
    };
    let Ok(_change) = cur.get_u16(order) else {
        return;
    };
    let Ok(addr_bytes) = cur.get_bytes(16) else {
        return;
    };
    let Ok(port) = cur.get_u16(order) else {
        return;
    };
    let _proto = decode_string(&mut cur, order).ok();
    let _status_size = decode_size(&mut cur, order).ok();

    let mut guid_arr = [0u8; 12];
    guid_arr.copy_from_slice(&guid);
    let mut addr_arr = [0u8; 16];
    addr_arr.copy_from_slice(&addr_bytes);
    let Some(ip) = ip_from_bytes(&addr_arr) else {
        return;
    };
    let server = SocketAddr::new(ip, port);

    if beacons.observe(server, guid_arr) {
        // Allow next tick to immediately re-send for any pending search.
        for p in pending.values_mut() {
            p.last_attempt = Instant::now() - Duration::from_secs(60);
        }
    }
}

fn rewrite_loopback(addr: SocketAddr) -> SocketAddr {
    if addr.ip().is_unspecified() {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port())
    } else {
        addr
    }
}

#[allow(dead_code)]
fn _suppress(_: PvaHeader) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_caps_at_last_value() {
        let max = *BACKOFF_SECS.last().unwrap();
        for i in 0..50 {
            let v = BACKOFF_SECS[i.min(BACKOFF_SECS.len() - 1)];
            assert!(v <= max);
        }
    }

    #[test]
    fn rewrite_loopback_preserves_real_addr() {
        let a = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 5075);
        assert_eq!(rewrite_loopback(a), a);
    }

    #[test]
    fn rewrite_loopback_replaces_unspecified() {
        let a = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 5075);
        let r = rewrite_loopback(a);
        assert_eq!(r.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
