use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use epics_base_rs::net::AsyncUdpV4;
use epics_base_rs::runtime::sync::mpsc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::interval;

use crate::protocol::*;

use super::circuit_breaker::CircuitBreakerRegistry;
use super::types::{SearchReason, SearchRequest, SearchResponse};

/// Snippet of a UDP/TCP search-response datagram, plus the address it
/// arrived from. Used to feed nameserver TCP responses through the same
/// `handle_udp_response` parser as plain UDP search replies.
type ParsedDatagram = (Vec<u8>, SocketAddr);

/// Send `buf` toward `addr`, expanding to a per-NIC fanout when the
/// destination is the limited broadcast `255.255.255.255` or an IPv4
/// multicast group (`224.0.0.0/4`). Per-subnet broadcasts and
/// unicast destinations route via the NIC chosen by [`AsyncUdpV4`].
async fn send_with_fanout(
    socket: &AsyncUdpV4,
    buf: &[u8],
    addr: SocketAddr,
    site: &'static str,
    send_errors: &mut HashMap<SocketAddr, std::io::ErrorKind>,
) {
    let needs_fanout = match addr {
        SocketAddr::V4(v4) => v4.ip().is_broadcast() || v4.ip().is_multicast(),
        SocketAddr::V6(_) => false,
    };
    let result = if needs_fanout {
        socket.fanout_to(buf, addr).await.map(|_| ())
    } else {
        socket.send_to(buf, addr).await.map(|_| ())
    };
    match result {
        Ok(()) => {
            // libca cae597d: log once-on-recovery so operators know
            // when a broken destination came back.
            if let Some(prev) = send_errors.remove(&addr) {
                tracing::info!(
                    target: "epics_ca_rs::search",
                    %addr, site, prev_error = ?prev,
                    "search send_to: recovered"
                );
            }
        }
        Err(e) => {
            // P-7 + libca cae597d (`udpiiu::SearchDestUDP::_lastError`):
            // log on first occurrence and on error-kind change; suppress
            // repeated identical errors so a persistent EHOSTUNREACH
            // doesn't flood the log at search rate.
            let kind = e.kind();
            let prev = send_errors.insert(addr, kind);
            if prev != Some(kind) {
                tracing::warn!(
                    target: "epics_ca_rs::search",
                    %addr,
                    site,
                    error = %e,
                    "search send_to failed"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration constants
// ---------------------------------------------------------------------------

/// pvxs `client.cpp::nBuckets`. 30 buckets at 1 s normal interval gives
/// each pending search a 30-second slot rotation — cooperative tick
/// caps UDP search traffic at roughly `pending.len() / 30` packets per
/// second instead of letting every channel fire on its own backoff.
const N_SEARCH_BUCKETS: usize = 30;

/// Bucket distance to push a re-search after the first retry. Mirrors
/// pvxs `Channel::disconnect` holdoff = 10 buckets to keep failed
/// connect attempts from looping faster than the 30s schedule.
const RETRY_HOLDOFF_CYCLES: u32 = 10;

/// Normal tick cadence (1 search bucket per second).
const NORMAL_TICK: Duration = Duration::from_secs(1);

/// Fast-mode tick cadence after a beacon poke. One full bucket
/// revolution fits in `N_SEARCH_BUCKETS * FAST_TICK = 6 s`.
const FAST_TICK: Duration = Duration::from_millis(200);

/// Maximum bytes per outbound UDP datagram.
const MAX_UDP_SEND: usize = 1024;

/// Penalty hold-off after a failed connect to a server.
const PENALTY_DURATION: Duration = Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Per-channel search state
// ---------------------------------------------------------------------------

struct PendingSearch {
    #[allow(dead_code)]
    cid: u32,
    #[allow(dead_code)]
    pv_name: String,
    /// Pre-built payload: SEARCH header + padded PV name (no VERSION prefix).
    search_payload: Vec<u8>,
    /// Which bucket this search currently lives in.
    bucket: usize,
    /// Number of attempts so far (for retry holdoff).
    attempt: u32,
    /// Tick-decremented hold-off counter: when > 0 at the bucket's
    /// firing tick, the search is NOT transmitted (re-placed into the
    /// next bucket so the counter decrements each cycle). Set to
    /// `RETRY_HOLDOFF_CYCLES` on first retry to space out re-attempts
    /// against a slow IOC. pvxs `Channel::disconnect` holdoff parity.
    holdoff_cycles: u32,
    #[allow(dead_code)]
    last_attempt: Option<Instant>,
}

// ---------------------------------------------------------------------------
// Penalty box
// ---------------------------------------------------------------------------

struct PenaltyEntry {
    until: Instant,
}

// ---------------------------------------------------------------------------
// Top-level engine state
// ---------------------------------------------------------------------------

struct SearchEngineState {
    pending: HashMap<u32, PendingSearch>,
    buckets: Vec<Vec<u32>>,
    current_bucket: usize,
    /// After a beacon poke we run one full revolution at FAST_TICK
    /// cadence so all pending searches retry within ~6 s.
    fast_ticks_remaining: u32,
    penalty: HashMap<SocketAddr, PenaltyEntry>,
    /// Per-server failure-pattern tracker. Sits on top of the single-shot
    /// `penalty` box: when failures repeat within a window, the breaker
    /// trips OPEN with an exponentially-doubled cooldown so we don't
    /// hammer a flapping server.
    breakers: CircuitBreakerRegistry,
    /// Sequence number for datagram validation (matches C EPICS
    /// lastReceivedSeqNo).  Embedded in VERSION header CID field;
    /// servers echo it back, letting us reject stale responses.
    dgram_seq: u32,
    /// Last validated sequence number from a VERSION response.
    last_valid_seq: Option<u32>,
    /// Per-destination last UDP send-error kind. Mirrors libca cae597d
    /// (`udpiiu::SearchDestUDP::_lastError`): a persistent sendto()
    /// failure (e.g. firewall, unreachable broadcast) repeats at search
    /// rate (~30 ms) and would otherwise spam logs. We log on first
    /// occurrence, on errno change, and on recovery; suppress repeats.
    send_errors: HashMap<SocketAddr, std::io::ErrorKind>,
}

impl SearchEngineState {
    fn new() -> Self {
        Self {
            pending: HashMap::new(),
            buckets: (0..N_SEARCH_BUCKETS).map(|_| Vec::new()).collect(),
            current_bucket: 0,
            fast_ticks_remaining: 0,
            penalty: HashMap::new(),
            breakers: CircuitBreakerRegistry::new(),
            dgram_seq: 0,
            last_valid_seq: None,
            send_errors: HashMap::new(),
        }
    }

    /// Remove a channel entirely.
    fn remove_channel(&mut self, cid: u32) {
        if let Some(p) = self.pending.remove(&cid) {
            self.buckets[p.bucket].retain(|x| *x != cid);
        }
    }

    /// pvxs `client.cpp:713 poke()` parity: reset every pending
    /// search's attempt + holdoff counters and start the engine's
    /// fast-tick revolution. Searches stay in their assigned buckets;
    /// fast-tick (200 ms) covers the full ring in 6 s so each pending
    /// search retries once within that window.
    fn poke(&mut self) {
        for p in self.pending.values_mut() {
            p.attempt = 0;
            p.holdoff_cycles = 0;
            p.last_attempt = None;
        }
        self.fast_ticks_remaining = N_SEARCH_BUCKETS as u32;
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

pub(crate) async fn run_search_engine(
    mut addr_list: Vec<SocketAddr>,
    nameserver_addrs: Vec<SocketAddr>,
    mut request_rx: mpsc::UnboundedReceiver<SearchRequest>,
    response_tx: mpsc::UnboundedSender<SearchResponse>,
) {
    // libca-style multi-NIC: one bound socket per IPv4 interface so
    // `255.255.255.255` and per-subnet broadcasts each leave via the
    // matching NIC. SO_REUSEADDR + (Linux) IP_MULTICAST_ALL=0 are
    // applied to every per-NIC socket inside `AsyncUdpV4::bind`.
    let socket = match AsyncUdpV4::bind(0, true) {
        Ok(s) => s,
        Err(_) => return,
    };
    // Larger receive buffer absorbs multi-PV SEARCH response bursts.
    let _ = socket.set_recv_buffer_size(256 * 1024);

    // Spawn a connection task per EPICS_CA_NAME_SERVERS entry.
    // Each task auto-reconnects with exponential backoff and forwards
    // outgoing search bytes to its TCP socket. Incoming responses are
    // queued via tcp_response_tx for the main loop to process through
    // the shared handle_udp_response parser.
    let (tcp_response_tx, mut tcp_response_rx) = mpsc::unbounded_channel::<ParsedDatagram>();
    let mut nameserver_send_txs: Vec<mpsc::UnboundedSender<Vec<u8>>> = Vec::new();
    for addr in nameserver_addrs {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        nameserver_send_txs.push(tx);
        let resp_tx = tcp_response_tx.clone();
        epics_base_rs::runtime::task::spawn(async move {
            run_nameserver_connection(addr, rx, resp_tx).await;
        });
    }

    let mut state = SearchEngineState::new();
    let mut recv_buf = [0u8; 65536];

    // pvxs `client.cpp::tickSearch`: a single steady tick advances the
    // bucket cursor. fast_tick is engaged after a beacon poke for one
    // full revolution, then we revert to NORMAL_TICK.
    let mut tick = interval(NORMAL_TICK);
    tick.tick().await; // skip immediate fire
    let mut tick_is_fast = false;

    loop {
        tokio::select! {
            req = request_rx.recv() => {
                let Some(req) = req else { return };
                let mut immediate: Vec<u32> = Vec::new();
                if let Some(cid) = handle_request_or_addr(&mut state, &mut addr_list, req) {
                    immediate.push(cid);
                }
                // Drain any additional queued requests so a burst of
                // Schedule messages all land before the next tick.
                while let Ok(req) = request_rx.try_recv() {
                    if let Some(cid) = handle_request_or_addr(&mut state, &mut addr_list, req) {
                        immediate.push(cid);
                    }
                }
                // pvxs `clientdiscover.cpp` parity: send the first SEARCH
                // packet right now instead of waiting up to one tick for
                // the bucket to come around. The bucket placement still
                // governs all subsequent retries.
                if !immediate.is_empty() {
                    fire_searches(&mut state, &immediate, &addr_list, &socket, &nameserver_send_txs).await;
                }
            }

            result = socket.recv_from(&mut recv_buf) => {
                let Ok((len, src)) = result else { continue };
                handle_udp_response(&mut state, &recv_buf[..len], src, &response_tx);
            }

            tcp_dgram = tcp_response_rx.recv() => {
                let Some((bytes, src)) = tcp_dgram else { continue };
                handle_udp_response(&mut state, &bytes, src, &response_tx);
            }

            _ = tick.tick() => {
                process_bucket(&mut state, &addr_list, &socket, &nameserver_send_txs).await;
                if state.fast_ticks_remaining > 0 {
                    state.fast_ticks_remaining -= 1;
                }
            }
        }

        // Tick-cadence transitions are evaluated outside the select! arm so
        // every event path (Schedule, response, tick) gets the same chance
        // to flip the engine in/out of fast mode based on the current
        // `fast_ticks_remaining`.
        if state.fast_ticks_remaining > 0 && !tick_is_fast {
            tick = interval(FAST_TICK);
            tick.tick().await; // skip immediate fire
            tick_is_fast = true;
        } else if state.fast_ticks_remaining == 0 && tick_is_fast {
            tick = interval(NORMAL_TICK);
            tick.tick().await; // skip immediate fire
            tick_is_fast = false;
        }
    }
}

/// Long-lived task: maintain a TCP connection to one nameserver, forward
/// outgoing search bytes from `outgoing_rx`, and feed parsed response
/// frames into `response_tx`. Reconnects with exponential backoff on
/// failure.
async fn run_nameserver_connection(
    addr: SocketAddr,
    mut outgoing_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    response_tx: mpsc::UnboundedSender<ParsedDatagram>,
) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        let stream =
            match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(addr)).await {
                Ok(Ok(s)) => s,
                _ => {
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(max_backoff);
                    continue;
                }
            };
        let _ = stream.set_nodelay(true);
        backoff = Duration::from_secs(1);

        let (mut reader, mut writer) = stream.into_split();

        // Send initial VERSION + HOST_NAME + CLIENT_NAME so the nameserver
        // accepts our search frames (mirrors transport.rs handshake).
        let mut handshake = Vec::new();
        let mut version = CaHeader::new(CA_PROTO_VERSION);
        version.count = CA_MINOR_VERSION;
        handshake.extend_from_slice(&version.to_bytes());
        let host_payload = pad_string(&epics_base_rs::runtime::env::hostname());
        let mut host = CaHeader::new(CA_PROTO_HOST_NAME);
        host.postsize = host_payload.len() as u16;
        handshake.extend_from_slice(&host.to_bytes());
        handshake.extend_from_slice(&host_payload);
        let user = epics_base_rs::runtime::env::get("USER")
            .or_else(|| epics_base_rs::runtime::env::get("USERNAME"))
            .unwrap_or_else(|| "unknown".to_string());
        let user_payload = pad_string(&user);
        let mut client = CaHeader::new(CA_PROTO_CLIENT_NAME);
        client.postsize = user_payload.len() as u16;
        handshake.extend_from_slice(&client.to_bytes());
        handshake.extend_from_slice(&user_payload);
        if writer.write_all(&handshake).await.is_err() {
            tokio::time::sleep(backoff).await;
            continue;
        }

        let resp_tx = response_tx.clone();
        let read_task = epics_base_rs::runtime::task::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let mut accumulated: Vec<u8> = Vec::new();
            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                accumulated.extend_from_slice(&buf[..n]);
                // Forward only the prefix that contains complete CA
                // messages. Without this framing, kernel splitting a
                // server response across read syscalls causes the
                // dispatcher to miss leading frames (when the partial
                // buffer is < 16 bytes) and misalign subsequent
                // parses. Each CA message is 16-byte header +
                // align8(postsize) — no extended-postsize support
                // here because the dispatcher itself ignores it.
                let mut consumed = 0usize;
                loop {
                    if accumulated.len() - consumed < CaHeader::SIZE {
                        break;
                    }
                    // CR-11: handle extended postsize (postsize=0xFFFF,
                    // count=0 → 8 extra header bytes + true u32 size).
                    // Pure 16-byte parse would consume 65,540 bytes for
                    // a frame whose true size is 24 + payload.
                    let (hdr, hdr_size) =
                        match CaHeader::from_bytes_extended(&accumulated[consumed..]) {
                            Ok(v) => v,
                            Err(_) => break,
                        };
                    let msg_size = hdr_size + align8(hdr.actual_postsize());
                    if accumulated.len() - consumed < msg_size {
                        break;
                    }
                    consumed += msg_size;
                }
                if consumed > 0 {
                    let frame_bytes = accumulated[..consumed].to_vec();
                    let _ = resp_tx.send((frame_bytes, addr));
                    accumulated.drain(..consumed);
                }
            }
        });

        // Pipe outgoing search frames to the TCP writer until the reader
        // task ends or the channel closes.
        let mut writer_failed = false;
        // Closed outgoing channel = client shutdown. Track it so we
        // fall through to read_task cleanup, then exit the outer
        // reconnect loop. Earlier code `return`-ed directly which
        // skipped the cleanup and leaked the read task per
        // nameserver on every shutdown.
        let mut shutdown = false;
        'pump: loop {
            tokio::select! {
                msg = outgoing_rx.recv() => {
                    let Some(bytes) = msg else {
                        shutdown = true;
                        break 'pump;
                    };
                    if writer.write_all(&bytes).await.is_err() {
                        writer_failed = true;
                        break 'pump;
                    }
                }
                _ = epics_base_rs::runtime::task::sleep(Duration::from_secs(60)) => {
                    // Periodic noop keeps the connection warm.
                    let echo = CaHeader::new(CA_PROTO_ECHO);
                    if writer.write_all(&echo.to_bytes()).await.is_err() {
                        writer_failed = true;
                        break 'pump;
                    }
                }
            }
            if read_task.is_finished() {
                break 'pump;
            }
        }
        read_task.abort();
        let _ = read_task.await;

        if shutdown {
            // Outgoing channel closed → no more senders ever → don't
            // reconnect; exit the per-nameserver task.
            return;
        }

        if writer_failed {
            // Brief pause before reconnect to avoid a spin loop when the
            // nameserver is fully unreachable.
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max_backoff);
        }
    }
}

// ---------------------------------------------------------------------------
// Request handling
// ---------------------------------------------------------------------------

/// Wrapper that handles the address-list mutation variants
/// inline (they need mutable access to `addr_list` which
/// `handle_request` doesn't have) and delegates everything else.
fn handle_request_or_addr(
    state: &mut SearchEngineState,
    addr_list: &mut Vec<SocketAddr>,
    req: SearchRequest,
) -> Option<u32> {
    match req {
        SearchRequest::AddAddress(addr) => {
            if !addr_list.contains(&addr) {
                addr_list.push(addr);
                tracing::info!(?addr, "ca-rs: addr_list += (programmatic)");
            }
            None
        }
        SearchRequest::SetAddressList(list) => {
            tracing::info!(count = list.len(), "ca-rs: addr_list replaced");
            *addr_list = list;
            None
        }
        other => handle_request(state, other),
    }
}

/// Process a search request. Returns `Some(cid)` when the new entry
/// needs an immediate first-attempt SEARCH packet sent (matches pvxs
/// `clientdiscover.cpp` immediate-broadcast on Find). The bucket
/// scheduler controls only retries; without immediate fire the first
/// attempt waits up to one full tick, which is the gap that made
/// ca-rs single-channel reconnect feel slower than pva-rs.
///
/// `None` means no immediate fire — either the request didn't add a
/// new pending entry (Cancel / ConnectResult) or it was a BeaconAnomaly
/// poke for an already-pending channel (counters reset only; fast-tick
/// mode handles the retransmit).
fn handle_request(state: &mut SearchEngineState, req: SearchRequest) -> Option<u32> {
    match req {
        SearchRequest::Schedule {
            cid,
            pv_name,
            reason,
        } => {
            // pvxs `poke()` semantic: BeaconAnomaly for an ALREADY-pending
            // channel must NOT move it to a new bucket. The whole point of
            // bucket distribution is lost if a mass-anomaly piles every
            // pending search into bucket=current+1. Just reset its retry
            // counters and engage fast-tick mode; the search fires within
            // ~6 s when its existing bucket comes around in fast cadence.
            if reason == SearchReason::BeaconAnomaly && state.pending.contains_key(&cid) {
                if let Some(p) = state.pending.get_mut(&cid) {
                    p.attempt = 0;
                    p.holdoff_cycles = 0;
                    p.last_attempt = None;
                }
                state.fast_ticks_remaining = N_SEARCH_BUCKETS as u32;
                return None;
            }

            let search_payload = build_search_payload(cid, &pv_name);

            // Drop any stale entry before re-scheduling.
            state.remove_channel(cid);

            // Bucket assignment by reason:
            //   Initial / BeaconAnomaly (new): fire on the very next tick.
            //   Reconnect: spread across the ring by cid hash so a
            //              mass-disconnect event doesn't pile every
            //              channel into the same firing bucket.
            // For a beacon poke we additionally engage fast-tick mode
            // (200 ms) so every pending search retries within ~6 s.
            let bucket = match reason {
                SearchReason::Initial | SearchReason::BeaconAnomaly => {
                    (state.current_bucket + 1) % N_SEARCH_BUCKETS
                }
                SearchReason::Reconnect => {
                    let offset = (cid as usize) % N_SEARCH_BUCKETS;
                    (state.current_bucket + 1 + offset) % N_SEARCH_BUCKETS
                }
            };
            let p = PendingSearch {
                cid,
                pv_name,
                search_payload,
                bucket,
                attempt: 0,
                holdoff_cycles: 0,
                last_attempt: None,
            };
            state.buckets[bucket].push(cid);
            state.pending.insert(cid, p);

            if reason == SearchReason::BeaconAnomaly {
                state.poke();
            }

            // Immediate first-attempt SEARCH only on `Initial` (typical
            // single-channel `find()`). Skipping it for `Reconnect` is the
            // whole point of the cid-hashed bucket spread above — without
            // this gate a TCP-close affecting N channels would batch N
            // immediate sends from the main loop's `try_recv` drain
            // (`fire_searches` at the top of `run`), defeating the spread
            // and producing the very burst the bucket scheduler exists to
            // avoid. `BeaconAnomaly` for a NEW cid likewise relies on
            // fast-tick mode (`poke()` above) to retransmit within ~6 s
            // instead of firing right away.
            match reason {
                SearchReason::Initial => Some(cid),
                SearchReason::Reconnect | SearchReason::BeaconAnomaly => None,
            }
        }

        SearchRequest::Cancel { cid } => {
            state.remove_channel(cid);
            None
        }

        SearchRequest::ConnectResult {
            cid,
            success,
            server_addr,
        } => {
            if success {
                state.remove_channel(cid);
                state.penalty.remove(&server_addr);
                state.breakers.record_success(server_addr);
            } else {
                state.penalty.insert(
                    server_addr,
                    PenaltyEntry {
                        until: Instant::now() + PENALTY_DURATION,
                    },
                );
                let was_open = state.breakers.is_open(server_addr);
                state.breakers.record_failure(server_addr);
                if !was_open && state.breakers.is_open(server_addr) {
                    tracing::warn!(server = %server_addr, "circuit breaker tripped OPEN");
                    metrics::counter!("ca_client_circuit_breaker_open_total",
                        "server" => server_addr.to_string())
                    .increment(1);
                }
            }
            None
        }
        // Address-list variants are intercepted by
        // `handle_request_or_addr` before they reach this match.
        // Defensive no-op so adding new variants doesn't crash if
        // future code paths plumb them straight to handle_request.
        SearchRequest::AddAddress(_) | SearchRequest::SetAddressList(_) => None,
    }
}

// ---------------------------------------------------------------------------
// UDP response handling
// ---------------------------------------------------------------------------

fn handle_udp_response(
    state: &mut SearchEngineState,
    data: &[u8],
    src: SocketAddr,
    response_tx: &mpsc::UnboundedSender<SearchResponse>,
) {
    if data.len() < CaHeader::SIZE {
        return;
    }

    let recv_time = Instant::now();
    let mut offset = 0;

    while offset + CaHeader::SIZE <= data.len() {
        let Ok(hdr) = CaHeader::from_bytes(&data[offset..]) else {
            break;
        };

        match hdr.cmmd {
            CA_PROTO_VERSION => {
                // Any VERSION in the datagram marks subsequent SEARCH
                // responses as fresh.  If the server echoed our
                // sequenceNoIsValid flag, record the exact seq_no.
                if hdr.data_type & 0x8000 != 0 {
                    state.last_valid_seq = Some(hdr.cid);
                } else {
                    // Server didn't echo our seq — still accept
                    // responses in this datagram (older servers,
                    // or our own Rust IOC, don't echo the flag).
                    state.last_valid_seq = Some(0);
                }
                offset += CaHeader::SIZE + align8(hdr.postsize as usize);
                continue;
            }
            CA_PROTO_SEARCH => {
                let server_port = hdr.data_type;
                // CA v4.8+: cid contains server IP. Both 0 (INADDR_ANY)
                // and 0xFFFFFFFF (~0u32, libca's "address unknown" sentinel
                // — see udpiiu.cpp searchRespAction) mean "use UDP source
                // address". Without handling both, real C softIoc replies
                // (cid=~0u32) get rerouted to 255.255.255.255 and the
                // search appears to fail.
                let server_ip = if hdr.cid == 0 || hdr.cid == u32::MAX {
                    src.ip()
                } else {
                    std::net::IpAddr::V4(Ipv4Addr::from(hdr.cid.to_be_bytes()))
                };
                metrics::counter!("ca_client_search_responses_total").increment(1);
                let server_addr = SocketAddr::new(server_ip, server_port as u16);
                let cid = hdr.available;

                // Check penalty box — skip penalized servers so the channel
                // can potentially find a non-penalized one.
                let penalized = state
                    .penalty
                    .get(&server_addr)
                    .map(|p| p.until > recv_time)
                    .unwrap_or(false);

                // Circuit breaker OPEN → reject responses from this server
                // entirely. allow() also performs OPEN→HALF_OPEN transition
                // when the cooldown has elapsed, permitting one probe.
                let breaker_blocked = !state.breakers.allow(server_addr);

                if penalized || breaker_blocked {
                    // Don't consume this response — let the channel keep
                    // searching for a better server.
                    offset += CaHeader::SIZE + align8(hdr.postsize as usize);
                    continue;
                }

                // Reject stale responses from previous search rounds.
                // A valid VERSION with our sequence must precede SEARCH
                // responses in the same datagram.
                if state.last_valid_seq.is_none() {
                    offset += CaHeader::SIZE + align8(hdr.postsize as usize);
                    continue;
                }

                if let Some(p) = state.pending.remove(&cid) {
                    state.buckets[p.bucket].retain(|x| *x != cid);
                    tracing::debug!(
                        pv = %p.pv_name, cid, server = %server_addr,
                        "PV search resolved"
                    );
                    let _ = response_tx.send(SearchResponse::Found { cid, server_addr });
                }
            }
            CA_PROTO_NOT_FOUND => {
                // Server explicitly told us the PV is not on it. We don't
                // remove the channel — another server in the addr list may
                // still answer Found.
            }
            _ => {}
        }

        offset += CaHeader::SIZE + align8(hdr.postsize as usize);
    }
}

// ---------------------------------------------------------------------------
// Per-tick bucket processing
// ---------------------------------------------------------------------------

/// Process exactly one search bucket. Pending searches in this bucket
/// either get a UDP retransmit (then re-armed into the same bucket so
/// they fire again after one full N-tick revolution) or, if a retry
/// holdoff is in effect, get pushed forward one bucket and the
/// holdoff counter decrements.
///
/// Steady-state UDP search load = O(1) datagrams per tick regardless
/// of how many channels are pending — the bucket distributes load
/// across the ring. The previous lane-based scheduler had every channel
/// fire on its own deadline and relied on AIMD to dampen storms after
/// the fact; the bucket scheduler prevents storms by construction.
async fn process_bucket(
    state: &mut SearchEngineState,
    addr_list: &[SocketAddr],
    socket: &AsyncUdpV4,
    nameserver_txs: &[mpsc::UnboundedSender<Vec<u8>>],
) {
    let now = Instant::now();

    // Expire old penalties.
    state.penalty.retain(|_, entry| entry.until > now);

    let bucket_idx = state.current_bucket;
    let bucket_ids = std::mem::take(&mut state.buckets[bucket_idx]);

    // Walk bucket: skip-on-holdoff or queue for sending.
    let mut to_send: Vec<u32> = Vec::new();
    for sid in bucket_ids {
        let Some(p) = state.pending.get_mut(&sid) else {
            continue;
        };
        if p.holdoff_cycles > 0 {
            p.holdoff_cycles -= 1;
            // Re-push to NEXT tick's bucket so the holdoff counter
            // decrements once per tick (matching pvxs intent that
            // RETRY_HOLDOFF_CYCLES is a per-tick countdown, not a
            // per-revolution count).
            let next = (state.current_bucket + 1) % N_SEARCH_BUCKETS;
            p.bucket = next;
            state.buckets[next].push(sid);
            continue;
        }

        // Queue for transmission. Re-place in the same bucket — a
        // full revolution (30 ticks at 1 s = 30 s normal cadence) is
        // the steady-state retry interval. Subsequent retries (attempt
        // > 1) get an extra RETRY_HOLDOFF_CYCLES of skip-cycles before
        // they actually transmit again.
        p.last_attempt = Some(now);
        p.attempt = p.attempt.saturating_add(1);
        if p.attempt > 1 {
            p.holdoff_cycles = RETRY_HOLDOFF_CYCLES;
        }
        p.bucket = state.current_bucket;
        state.buckets[state.current_bucket].push(sid);
        to_send.push(sid);
    }

    state.current_bucket = (state.current_bucket + 1) % N_SEARCH_BUCKETS;

    if to_send.is_empty() {
        return;
    }

    fire_searches(state, &to_send, addr_list, socket, nameserver_txs).await;
}

/// Build batched UDP SEARCH datagrams for `cids` and send via every
/// destination + nameserver channel. One VERSION header per datagram
/// carries the rolling sequence number so stale responses are
/// rejected (matches C EPICS dgSeqNoAtTimerExpire). Used both by the
/// per-tick bucket processor and by the immediate-fire path that
/// runs right after handle_request to avoid the up-to-1-tick wait
/// on the first attempt.
async fn fire_searches(
    state: &mut SearchEngineState,
    cids: &[u32],
    addr_list: &[SocketAddr],
    socket: &AsyncUdpV4,
    nameserver_txs: &[mpsc::UnboundedSender<Vec<u8>>],
) {
    state.dgram_seq = state.dgram_seq.wrapping_add(1);
    let version_hdr = {
        let mut h = CaHeader::new(CA_PROTO_VERSION);
        h.count = CA_MINOR_VERSION;
        h.data_type = 0x8000;
        h.cid = state.dgram_seq;
        h.to_bytes()
    };

    // Build batched UDP datagrams (multi-search per packet, MTU-bounded).
    // Bucket distribution caps per-tick load at ~pending/N_SEARCH_BUCKETS,
    // so no AIMD throttling is needed.
    let mut current_frame = Vec::with_capacity(MAX_UDP_SEND);
    current_frame.extend_from_slice(&version_hdr);

    for sid in cids {
        let Some(p) = state.pending.get(sid) else {
            continue;
        };
        let payload = p.search_payload.clone();

        if current_frame.len() + payload.len() > MAX_UDP_SEND
            && current_frame.len() > CaHeader::SIZE
        {
            for addr in addr_list {
                send_with_fanout(
                    socket,
                    &current_frame,
                    *addr,
                    "bucket",
                    &mut state.send_errors,
                )
                .await;
            }
            for ns_tx in nameserver_txs {
                let _ = ns_tx.send(current_frame.clone());
            }
            current_frame.clear();
            current_frame.extend_from_slice(&version_hdr);
        }

        if CaHeader::SIZE + payload.len() > MAX_UDP_SEND {
            // Single payload exceeds MTU — solo send.
            let mut solo = Vec::with_capacity(CaHeader::SIZE + payload.len());
            solo.extend_from_slice(&version_hdr);
            solo.extend_from_slice(&payload);
            for addr in addr_list {
                send_with_fanout(socket, &solo, *addr, "solo", &mut state.send_errors).await;
            }
            for ns_tx in nameserver_txs {
                let _ = ns_tx.send(solo.clone());
            }
        } else {
            current_frame.extend_from_slice(&payload);
        }
    }

    // Flush the final frame.
    if current_frame.len() > CaHeader::SIZE {
        for addr in addr_list {
            send_with_fanout(
                socket,
                &current_frame,
                *addr,
                "flush",
                &mut state.send_errors,
            )
            .await;
        }
        for ns_tx in nameserver_txs {
            let _ = ns_tx.send(current_frame.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build per-channel search payload (SEARCH header + padded PV name).
/// Does NOT include the VERSION header — that is prepended once per datagram.
fn build_search_payload(cid: u32, pv_name: &str) -> Vec<u8> {
    let pv_payload = pad_string(pv_name);

    let mut search_hdr = CaHeader::new(CA_PROTO_SEARCH);
    search_hdr.postsize = pv_payload.len() as u16;
    search_hdr.data_type = CA_DO_REPLY;
    search_hdr.count = CA_MINOR_VERSION;
    search_hdr.cid = cid;
    search_hdr.available = cid;

    let mut payload = Vec::with_capacity(CaHeader::SIZE + pv_payload.len());
    payload.extend_from_slice(&search_hdr.to_bytes());
    payload.extend_from_slice(&pv_payload);
    payload
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule_initial(state: &mut SearchEngineState, cid: u32, pv_name: &str) {
        handle_request(
            state,
            SearchRequest::Schedule {
                cid,
                pv_name: pv_name.to_string(),
                reason: SearchReason::Initial,
            },
        );
    }

    #[test]
    fn build_search_payload_size() {
        let payload = build_search_payload(42, "TEST:PV");
        // CaHeader::SIZE (16) + pad_string("TEST:PV") = 16 + 8 = 24
        assert_eq!(payload.len(), 24);
    }

    #[test]
    fn build_search_payload_alignment() {
        let payload = build_search_payload(1, "A");
        // pad_string("A") = 8 bytes (1 char + null + 6 padding)
        assert_eq!(payload.len(), CaHeader::SIZE + 8);
        assert_eq!(payload.len() % 8, 0);
    }

    #[test]
    fn schedule_places_into_next_bucket() {
        let mut state = SearchEngineState::new();
        state.current_bucket = 5;
        schedule_initial(&mut state, 1, "PV:1");
        let p = state.pending.get(&1).unwrap();
        assert_eq!(p.bucket, 6);
        assert_eq!(state.buckets[6], vec![1]);
        assert_eq!(state.buckets[5], Vec::<u32>::new());
    }

    #[test]
    fn cancel_removes_from_bucket() {
        let mut state = SearchEngineState::new();
        schedule_initial(&mut state, 1, "PV:1");
        let bucket = state.pending.get(&1).unwrap().bucket;
        handle_request(&mut state, SearchRequest::Cancel { cid: 1 });
        assert!(state.pending.is_empty());
        assert!(state.buckets[bucket].is_empty());
    }

    #[test]
    fn poke_resets_attempts_and_engages_fast_mode() {
        let mut state = SearchEngineState::new();
        schedule_initial(&mut state, 1, "PV:1");
        // Simulate one prior attempt with active holdoff.
        if let Some(p) = state.pending.get_mut(&1) {
            p.attempt = 3;
            p.holdoff_cycles = 7;
        }
        state.poke();
        let p = state.pending.get(&1).unwrap();
        assert_eq!(p.attempt, 0);
        assert_eq!(p.holdoff_cycles, 0);
        assert_eq!(state.fast_ticks_remaining, N_SEARCH_BUCKETS as u32);
    }

    #[test]
    fn beacon_anomaly_for_pending_channel_keeps_bucket() {
        // pvxs poke() semantic: a BeaconAnomaly Schedule for an
        // already-pending channel must NOT move it to a new bucket.
        // Otherwise a mass-anomaly piles every pending search into
        // bucket=current+1 and defeats bucket distribution.
        let mut state = SearchEngineState::new();
        // Use Reconnect so it's placed into a non-current+1 bucket.
        handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 7,
                pv_name: "PV:7".into(),
                reason: SearchReason::Reconnect,
            },
        );
        let original_bucket = state.pending.get(&7).unwrap().bucket;
        // Pretend prior attempts happened.
        if let Some(p) = state.pending.get_mut(&7) {
            p.attempt = 4;
            p.holdoff_cycles = 8;
        }
        // Now apply a BeaconAnomaly poke for cid=7.
        handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 7,
                pv_name: "PV:7".into(),
                reason: SearchReason::BeaconAnomaly,
            },
        );
        let p = state.pending.get(&7).unwrap();
        assert_eq!(p.bucket, original_bucket, "poke must not relocate bucket");
        assert_eq!(p.attempt, 0);
        assert_eq!(p.holdoff_cycles, 0);
        assert_eq!(state.fast_ticks_remaining, N_SEARCH_BUCKETS as u32);
        // And the bucket vector still has the cid exactly once.
        let count = state.buckets[original_bucket]
            .iter()
            .filter(|x| **x == 7)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn beacon_anomaly_schedule_pokes_engine() {
        let mut state = SearchEngineState::new();
        schedule_initial(&mut state, 1, "PV:1");
        // Pretend channel #1 had multiple prior failures.
        if let Some(p) = state.pending.get_mut(&1) {
            p.attempt = 2;
            p.holdoff_cycles = 5;
        }
        handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 2,
                pv_name: "PV:2".into(),
                reason: SearchReason::BeaconAnomaly,
            },
        );
        // Both channels should now be at attempt=0 and the engine in fast mode.
        assert_eq!(state.pending.get(&1).unwrap().attempt, 0);
        assert_eq!(state.pending.get(&2).unwrap().attempt, 0);
        assert_eq!(state.fast_ticks_remaining, N_SEARCH_BUCKETS as u32);
    }

    #[test]
    fn connect_success_clears_pending_and_penalty() {
        let mut state = SearchEngineState::new();
        let server: SocketAddr = "127.0.0.1:5064".parse().unwrap();
        schedule_initial(&mut state, 1, "PV:1");
        state.penalty.insert(
            server,
            PenaltyEntry {
                until: Instant::now() + Duration::from_secs(60),
            },
        );
        handle_request(
            &mut state,
            SearchRequest::ConnectResult {
                cid: 1,
                success: true,
                server_addr: server,
            },
        );
        assert!(state.pending.is_empty());
        assert!(!state.penalty.contains_key(&server));
    }

    #[test]
    fn connect_failure_inserts_penalty() {
        let mut state = SearchEngineState::new();
        let server: SocketAddr = "127.0.0.1:5064".parse().unwrap();
        schedule_initial(&mut state, 1, "PV:1");
        handle_request(
            &mut state,
            SearchRequest::ConnectResult {
                cid: 1,
                success: false,
                server_addr: server,
            },
        );
        // Pending entry stays — channel still searching for another server.
        assert!(state.pending.contains_key(&1));
        assert!(state.penalty.contains_key(&server));
    }

    #[test]
    fn n_search_buckets_is_30() {
        // Sanity: pvxs uses 30, our bucket vector must match.
        let state = SearchEngineState::new();
        assert_eq!(state.buckets.len(), N_SEARCH_BUCKETS);
        assert_eq!(N_SEARCH_BUCKETS, 30);
    }

    #[test]
    fn fast_tick_revolution_covers_full_ring() {
        // FAST_TICK * N_SEARCH_BUCKETS should be ~6 s (matches pvxs poke cadence).
        let revolution = FAST_TICK * N_SEARCH_BUCKETS as u32;
        assert!(revolution >= Duration::from_secs(5));
        assert!(revolution <= Duration::from_secs(7));
    }

    /// `Initial` is the only reason that earns the immediate-fire
    /// `Some(cid)` return — `Reconnect` and `BeaconAnomaly` must
    /// return `None` so the main loop's `try_recv` drain doesn't
    /// batch a 5000-channel disconnect cascade into a single-tick
    /// burst (review finding HIGH#1).
    #[test]
    fn reconnect_and_beacon_anomaly_skip_immediate_fire() {
        let mut state = SearchEngineState::new();
        // Initial → Some(cid)
        let cid_initial = handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 100,
                pv_name: "PV:Initial".into(),
                reason: SearchReason::Initial,
            },
        );
        assert_eq!(
            cid_initial,
            Some(100),
            "Initial must return Some for immediate fire"
        );
        // Reconnect → None (bucket-spread, no burst)
        let cid_reconnect = handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 101,
                pv_name: "PV:Reconnect".into(),
                reason: SearchReason::Reconnect,
            },
        );
        assert_eq!(cid_reconnect, None, "Reconnect must NOT immediately fire");
        // BeaconAnomaly (NEW cid) → None (fast-tick handles retransmit)
        let cid_anomaly = handle_request(
            &mut state,
            SearchRequest::Schedule {
                cid: 102,
                pv_name: "PV:Anomaly".into(),
                reason: SearchReason::BeaconAnomaly,
            },
        );
        assert_eq!(
            cid_anomaly, None,
            "BeaconAnomaly NEW must NOT immediately fire"
        );
    }

    /// `Reconnect` schedules must spread across the bucket ring by
    /// cid hash, not collapse into `current+1` like `Initial` does.
    /// Replicates the bucket-placement formula and asserts a
    /// contiguous block of cids touches every bucket.
    #[test]
    fn reconnect_bucket_spread() {
        let mut state = SearchEngineState::new();
        state.current_bucket = 0;
        let mut hit = [false; N_SEARCH_BUCKETS];
        for cid in 1_000u32..(1_000 + N_SEARCH_BUCKETS as u32) {
            handle_request(
                &mut state,
                SearchRequest::Schedule {
                    cid,
                    pv_name: format!("PV:{cid}"),
                    reason: SearchReason::Reconnect,
                },
            );
            hit[state.pending.get(&cid).unwrap().bucket] = true;
        }
        assert!(
            hit.iter().all(|h| *h),
            "Reconnect cids must hit every bucket in the ring"
        );
    }
}
