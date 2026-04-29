//! UDP search responder + (very simple) beacon broadcaster.
//!
//! Listens on the configured UDP port for SEARCH requests and replies with
//! SEARCH_RESPONSE messages naming our TCP endpoint. Beacons are emitted
//! periodically to advertise our presence.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::net::AsyncUdpV4;
use tracing::debug;

use crate::error::{PvaError, PvaResult};
use crate::proto::{
    ByteOrder, Command, PvaHeader, ReadExt, WriteExt, decode_size, decode_string,
    encode_string_into, ip_to_bytes,
};

use super::source::DynSource;

/// Generate a 12-byte server GUID.
pub fn random_guid() -> [u8; 12] {
    let mut buf = [0u8; 12];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    buf[..8].copy_from_slice(&now.to_le_bytes());
    let pid = std::process::id().to_le_bytes();
    buf[8..12].copy_from_slice(&pid);
    buf
}

fn bind_udp(port: u16) -> PvaResult<AsyncUdpV4> {
    let sock = AsyncUdpV4::bind(port, true).map_err(PvaError::Io)?;
    // pvxs server.cpp joins multicast groups listed in
    // EPICS_PVAS_INTF_ADDR_LIST / EPICS_PVA_ADDR_LIST so SEARCH packets
    // sent to those groups reach the responder. We do the same here —
    // the call is idempotent on each restart and silently skips
    // non-multicast entries.
    crate::client_native::search_engine::join_addr_list_multicast(&sock);
    Ok(sock)
}

/// Run the UDP search responder + beacon emitter until the runtime is dropped.
///
/// `tcp_port` is advertised in SEARCH_RESPONSE so clients know where to
/// open the virtual circuit. `protocol` is normally `"tcp"`; set to
/// `"tls"` when the TCP listener requires TLS so pvxs clients with TLS
/// configured will connect over `pvas://`.
pub async fn run_udp_responder_proto(
    source: DynSource,
    udp_port: u16,
    tcp_port: u16,
    guid: [u8; 12],
    protocol: &'static str,
) -> PvaResult<()> {
    run_udp_responder_with_config(
        source,
        udp_port,
        tcp_port,
        guid,
        protocol,
        Duration::from_secs(15),
        Duration::from_secs(180),
        10,
        Vec::new(),
        true,
        Vec::new(),
    )
    .await
}

/// Like [`run_udp_responder_proto`] but configurable: explicit beacon
/// period, explicit destinations, and an auto-NIC-broadcast flag. When
/// `destinations` is empty AND `auto_beacon` is true, beacons fan out
/// to per-NIC broadcasts (via [`crate::config::env::list_broadcast_addresses`]).
/// When `destinations` is non-empty, exactly those addresses are used.
#[allow(clippy::too_many_arguments)]
pub async fn run_udp_responder_with_config(
    source: DynSource,
    udp_port: u16,
    tcp_port: u16,
    guid: [u8; 12],
    protocol: &'static str,
    beacon_period: Duration,
    beacon_period_long: Duration,
    beacon_burst_count: u8,
    destinations: Vec<SocketAddr>,
    auto_beacon: bool,
    ignore_addrs: Vec<(IpAddr, u16)>,
) -> PvaResult<()> {
    let socket = bind_udp(udp_port)?;
    let socket = Arc::new(socket);
    debug!(?udp_port, "UDP search responder started");

    // Resolve beacon destinations once at startup. pvxs re-resolves
    // on interface change but we keep it static for now; restart the
    // server to pick up new NICs.
    let beacon_destinations: Vec<SocketAddr> = if !destinations.is_empty() {
        destinations
    } else if auto_beacon {
        crate::config::env::list_broadcast_addresses(udp_port)
    } else {
        // Final fallback: limited broadcast.
        vec![format!("255.255.255.255:{}", udp_port).parse().unwrap()]
    };
    debug!(
        ?beacon_destinations,
        ?beacon_period,
        "beacon emitter config"
    );

    let beacon_socket = socket.clone();
    let beacon_guid = guid;
    let beacon_source = source.clone();
    // F2: bind the JoinHandle to an AbortOnDrop guard scoped to this
    // function's stack so the beacon task is aborted when the parent
    // UDP responder unwinds (PvaServer Drop, listener task panic).
    // Without this the bound socket-cloning beacon task lingered
    // until runtime shutdown across server restart cycles.
    struct AbortOnDrop(tokio::task::AbortHandle);
    impl Drop for AbortOnDrop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let beacon_join = tokio::spawn(async move {
        // Burst-then-slowdown cadence (P-G17): emit `beacon_burst_count`
        // beacons at `beacon_period` (default 15s × 10), then drop to
        // `beacon_period_long` (default 180s) for steady state. Mirrors
        // pvxs `server.cpp:826-832`: after the burst every receiver in
        // earshot has had multiple chances to notice the new server, so
        // 12× more steady-state beacons just burn UDP without
        // information gain. Per-emitter monotonically advancing
        // beacon_seq + change_count let clients detect missed beacons
        // and topology changes regardless of cadence.
        let mut beacon_seq: u8 = 0;
        let mut change_count: u16 = 0;
        let mut last_set_hash: u64 = 0;
        let mut emitted: u32 = 0;
        loop {
            let cur_period = if emitted < beacon_burst_count as u32 {
                beacon_period
            } else {
                beacon_period_long
            };
            tokio::time::sleep(cur_period).await;
            // Compute a stable hash of the current PV set so we don't
            // hold an allocated Vec across the await above.
            let pvs = beacon_source.list_pvs().await;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            let mut sorted = pvs;
            sorted.sort();
            sorted.hash(&mut h);
            let cur_hash = h.finish();
            let topology_changed = cur_hash != last_set_hash && last_set_hash != 0;
            if topology_changed {
                change_count = change_count.wrapping_add(1);
                // pvxs doesn't reset to short-burst on topology change,
                // but we also don't lose anything by re-burst on real
                // PV-set churn (rare event); we leave the counter
                // alone to keep parity with pvxs.
            }
            last_set_hash = cur_hash;

            let beacon = build_beacon(
                beacon_guid,
                tcp_port,
                ByteOrder::Little,
                beacon_seq,
                change_count,
            );
            for dest in &beacon_destinations {
                // Limited broadcast / multicast destinations need
                // explicit per-NIC fanout. Per-subnet broadcast and
                // unicast route via AsyncUdpV4::send_to's NIC pick.
                let needs_fanout = match dest {
                    SocketAddr::V4(v4) => v4.ip().is_broadcast() || v4.ip().is_multicast(),
                    SocketAddr::V6(_) => false,
                };
                let _ = if needs_fanout {
                    beacon_socket.fanout_to(&beacon, *dest).await.map(|_| ())
                } else {
                    beacon_socket.send_to(&beacon, *dest).await.map(|_| ())
                };
            }
            beacon_seq = beacon_seq.wrapping_add(1);
            emitted = emitted.saturating_add(1);
        }
    });
    let _beacon_guard = AbortOnDrop(beacon_join.abort_handle());

    // 64 KB receive buffer — IPv4 maximum. The previous 1500-byte
    // (Ethernet MTU) cap silently truncated large multi-PV searches:
    // pvxs clients pack many SEARCH messages into one datagram and a
    // gateway-restart storm can easily exceed 1500 bytes. 64 KB
    // matches the kernel ceiling without truncation. Heap-allocated
    // because 64 KB on the per-task stack is large; one allocation
    // amortized across the listener's lifetime.
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        // pvxs 57f9468 (2025-11): receive metadata tells us which NIC
        // the SEARCH arrived on, so the corresponding SEARCH_RESPONSE
        // can be sent from a socket bound to that same NIC. Without
        // this, replies to multicast/broadcast SEARCHes go via the OS
        // default route — wrong NIC on multi-homed hosts.
        let meta = match socket.recv_with_meta(&mut buf).await {
            Ok(m) => m,
            Err(e) => {
                debug!("udp recv error: {e}");
                continue;
            }
        };
        let n = meta.n;
        let peer = meta.src;
        let reply_iface_ip = meta.iface_ip;
        // pvxs `udp_collector.cpp::handle_one`: silently drop UDP
        // datagrams whose source IP is itself a multicast group. Such
        // packets are necessarily forged (mcast is dest-only) and
        // replying to one would amplify a DDoS.
        if let std::net::IpAddr::V4(v4) = peer.ip() {
            if v4.is_multicast() {
                debug!("ignoring UDP with mcast source {peer}");
                continue;
            }
        }
        // ignore_addrs: drop search packets from blocklisted peers
        // *before* we spend time decoding. Mirrors pvxs serverconn.cpp
        // ignoreAddrs check on inbound search.
        let ignored = ignore_addrs
            .iter()
            .any(|(ip, port)| peer.ip() == *ip && (*port == 0 || peer.port() == *port));
        if ignored {
            continue;
        }
        // pvxs `udp_collector.cpp::process_one` (L329) loops over a
        // single datagram parsing PVA messages until the buffer is
        // drained — clients pack many SEARCH messages per datagram
        // when many channels are searching concurrently. Without the
        // drain loop we'd silently miss N-1 of N searches.
        let frame = &buf[..n];
        let mut pos = 0usize;
        while pos + PvaHeader::SIZE <= frame.len() {
            let chunk = &frame[pos..];
            // Consume one message + advance pos. Bail out of the
            // drain loop if the next message header doesn't decode
            // (truncation / non-PVA padding) — what we have is what
            // we got.
            let consumed = match parse_search_request(chunk) {
                Some(req) => {
                    let consumed = req.consumed;
                    // Pack ALL matches for this single search into
                    // ONE response datagram. pvxs udp_collector.cpp:570
                    // `reply()` does the same; without it the gateway
                    // amplifies an N-cid search into N reply datagrams.
                    let mut matched_cids: Vec<u32> = Vec::with_capacity(req.queries.len());
                    for (cid, name) in &req.queries {
                        if source.has_pv(name).await {
                            matched_cids.push(*cid);
                        }
                    }
                    if !matched_cids.is_empty() {
                        let resp = build_search_response_proto(
                            guid,
                            req.seq,
                            tcp_port,
                            &matched_cids,
                            req.byte_order,
                            protocol,
                        );
                        // pvxs 57f9468: send the reply from the socket
                        // bound to the NIC the request arrived on, so
                        // the source IP on the wire matches the
                        // interface that received the multicast /
                        // broadcast SEARCH. Falls back to plain
                        // routing-decided send if that NIC is no
                        // longer in the bundle (rare).
                        let send = socket.send_via(&resp, peer, reply_iface_ip).await;
                        let send = match send {
                            Ok(n) => Ok(n),
                            Err(e) if e.kind() == std::io::ErrorKind::AddrNotAvailable => {
                                socket.send_to(&resp, peer).await
                            }
                            Err(e) => Err(e),
                        };
                        if let Err(e) = send {
                            debug!("udp send to {peer} via {reply_iface_ip}: {e}");
                        }
                    }
                    consumed
                }
                None => {
                    // Header didn't decode as a SEARCH request — try
                    // to advance past it so a later beacon/etc. in the
                    // same datagram can still be parsed. We use the
                    // header's payload_length when we can read the
                    // header at all; otherwise stop.
                    match PvaHeader::decode(&mut Cursor::new(chunk)) {
                        Ok(h) => PvaHeader::SIZE + h.payload_length as usize,
                        Err(_) => break,
                    }
                }
            };
            if consumed == 0 {
                break;
            }
            pos = pos.saturating_add(consumed);
        }
    }

    // Beacon task is aborted via the AbortOnDrop guard (`_beacon_guard`)
    // when this function unwinds.
    #[allow(unreachable_code)]
    Ok(())
}

/// Build a (one-PV) SEARCH_RESPONSE frame with explicit protocol name.
fn build_search_response_proto(
    guid: [u8; 12],
    seq: u32,
    tcp_port: u16,
    cids: &[u32],
    order: ByteOrder,
    protocol: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&guid);
    payload.put_u32(seq, order);
    let addr = ip_to_bytes(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    payload.extend_from_slice(&addr);
    payload.put_u16(tcp_port, order);
    encode_string_into(protocol, order, &mut payload);
    payload.put_u8(1); // found
    payload.put_u16(cids.len() as u16, order);
    for &cid in cids {
        payload.put_u32(cid, order);
    }
    let header = PvaHeader::application(
        true,
        order,
        Command::SearchResponse.code(),
        payload.len() as u32,
    );
    let mut out = Vec::new();
    header.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

/// Backwards-compat wrapper: protocol = "tcp".
pub async fn run_udp_responder(
    source: DynSource,
    udp_port: u16,
    tcp_port: u16,
    guid: [u8; 12],
) -> PvaResult<()> {
    run_udp_responder_proto(source, udp_port, tcp_port, guid, "tcp").await
}

fn build_beacon(
    guid: [u8; 12],
    tcp_port: u16,
    order: ByteOrder,
    sequence: u8,
    change_count: u16,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&guid);
    // pvxs server.cpp::doBeacons: flags(u8) + seq(u8) + change(u16) = 4 bytes
    payload.put_u8(0); // flags / QoS (undefined, 0)
    payload.put_u8(sequence);
    payload.put_u16(change_count, order);
    let addr = ip_to_bytes(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    payload.extend_from_slice(&addr);
    payload.put_u16(tcp_port, order);
    encode_string_into("tcp", order, &mut payload);
    payload.put_u8(0xFF); // null serverStatus marker (matches pvxs)
    let header = PvaHeader::application(true, order, Command::Beacon.code(), payload.len() as u32);
    let mut out = Vec::new();
    header.write_into(&mut out);
    out.extend_from_slice(&payload);
    out
}

#[derive(Debug)]
struct SearchRequest {
    seq: u32,
    byte_order: ByteOrder,
    queries: Vec<(u32, String)>,
    /// Total bytes consumed from the input slice (header + payload),
    /// used by the multi-message drain loop to advance to the next
    /// chained message in the same datagram.
    consumed: usize,
}

fn parse_search_request(frame: &[u8]) -> Option<SearchRequest> {
    if frame.len() < PvaHeader::SIZE {
        return None;
    }
    let mut cur = Cursor::new(frame);
    let header = PvaHeader::decode(&mut cur).ok()?;
    if header.command != Command::Search.code() || header.flags.is_control() {
        return None;
    }
    let order = header.flags.byte_order();
    let payload_len = header.payload_length as usize;
    let avail = frame.len().saturating_sub(PvaHeader::SIZE);
    if avail < payload_len {
        return None;
    }
    let payload = &frame[PvaHeader::SIZE..PvaHeader::SIZE + payload_len];
    let mut p = Cursor::new(payload);
    let seq = p.get_u32(order).ok()?;
    let _flags = p.get_u8().ok()?;
    let _ = p.get_bytes(3).ok()?;
    let _addr = p.get_bytes(16).ok()?;
    let _port = p.get_u16(order).ok()?;
    let n_proto = decode_size(&mut p, order).ok().flatten()? as usize;
    for _ in 0..n_proto {
        let _ = decode_string(&mut p, order).ok()?;
    }
    let n = p.get_u16(order).ok()? as usize;
    // P-G22 follow-up: cap pre-alloc against attacker-announced
    // count. Each (cid u32, String) consumes >= 5 wire bytes; in
    // practice n is u16-bounded so the worst case is ~1.5MB but
    // capping at remaining-bytes keeps the small-datagram common
    // case tight.
    let remaining = p.get_ref().len().saturating_sub(p.position() as usize);
    let mut queries = Vec::with_capacity(n.min(remaining));
    for _ in 0..n {
        let cid = p.get_u32(order).ok()?;
        let name = decode_string(&mut p, order).ok().flatten()?;
        queries.push((cid, name));
    }
    Some(SearchRequest {
        seq,
        byte_order: order,
        queries,
        consumed: PvaHeader::SIZE + payload_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// build_beacon writes the supplied sequence + change_count into
    /// the payload at the documented offsets (after the 12-byte GUID +
    /// flags byte). Locks in the field order so a refactor cannot swap
    /// them silently.
    #[test]
    fn beacon_payload_carries_sequence_and_change_count() {
        let guid = [0x11; 12];
        let bytes = build_beacon(guid, 5075, ByteOrder::Little, 42, 0xBEEF);
        // 8-byte PVA header + 12-byte GUID = 20 bytes prefix.
        let payload = &bytes[8..];
        assert_eq!(&payload[0..12], &guid);
        assert_eq!(payload[12], 0); // flags byte
        assert_eq!(payload[13], 42, "beacon sequence at offset 13");
        assert_eq!(
            u16::from_le_bytes([payload[14], payload[15]]),
            0xBEEF,
            "change_count at offset 14"
        );
    }
}
