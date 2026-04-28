//! UDP search responder + (very simple) beacon broadcaster.
//!
//! Listens on the configured UDP port for SEARCH requests and replies with
//! SEARCH_RESPONSE messages naming our TCP endpoint. Beacons are emitted
//! periodically to advertise our presence.

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::Arc;
use std::time::Duration;

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::time::interval;
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

fn bind_udp(port: u16) -> PvaResult<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    sock.set_broadcast(true)?;
    sock.set_nonblocking(true)?;
    let bind: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    sock.bind(&bind.into())?;
    // pvxs server.cpp joins multicast groups listed in
    // EPICS_PVAS_INTF_ADDR_LIST / EPICS_PVA_ADDR_LIST so SEARCH packets
    // sent to those groups reach the responder. We do the same here —
    // the call is idempotent on each restart and silently skips
    // non-multicast entries.
    crate::client_native::search_engine::join_addr_list_multicast(&sock);
    let std_sock: StdUdpSocket = sock.into();
    UdpSocket::from_std(std_sock).map_err(PvaError::Io)
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
    let _beacon = tokio::spawn(async move {
        let mut tick = interval(beacon_period);
        // Per-emitter monotonically advancing beacon sequence + change
        // counter. Matches pvxs `server.cpp::doBeacons` so clients can
        // detect missed beacons (sequence gaps) and topology changes
        // (change_count mismatch). Sequence is u8 with natural wrap.
        //
        // change_count tracks PV-set churn: incremented whenever the
        // set of names returned by `list_pvs()` differs from the
        // previous tick. Clients re-issue searches on change_count
        // mismatch even when their beacon sequence is in lock-step.
        let mut beacon_seq: u8 = 0;
        let mut change_count: u16 = 0;
        let mut last_set_hash: u64 = 0;
        loop {
            tick.tick().await;
            // Compute a stable hash of the current PV set so we don't
            // hold an allocated Vec across the await above.
            let pvs = beacon_source.list_pvs().await;
            let mut h = std::collections::hash_map::DefaultHasher::new();
            use std::hash::{Hash, Hasher};
            let mut sorted = pvs;
            sorted.sort();
            sorted.hash(&mut h);
            let cur_hash = h.finish();
            if cur_hash != last_set_hash && last_set_hash != 0 {
                change_count = change_count.wrapping_add(1);
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
                let _ = beacon_socket.send_to(&beacon, dest).await;
            }
            beacon_seq = beacon_seq.wrapping_add(1);
        }
    });

    // 64 KB receive buffer — IPv4 maximum. The previous 1500-byte
    // (Ethernet MTU) cap silently truncated large multi-PV searches:
    // pvxs clients pack many SEARCH messages into one datagram and a
    // gateway-restart storm can easily exceed 1500 bytes. 64 KB
    // matches the kernel ceiling without truncation. Heap-allocated
    // because 64 KB on the per-task stack is large; one allocation
    // amortized across the listener's lifetime.
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let (n, peer) = match socket.recv_from(&mut buf).await {
            Ok(t) => t,
            Err(e) => {
                debug!("udp recv error: {e}");
                continue;
            }
        };
        // ignore_addrs: drop search packets from blocklisted peers
        // *before* we spend time decoding. Mirrors pvxs serverconn.cpp
        // ignoreAddrs check on inbound search.
        let ignored = ignore_addrs
            .iter()
            .any(|(ip, port)| peer.ip() == *ip && (*port == 0 || peer.port() == *port));
        if ignored {
            continue;
        }
        let frame = &buf[..n];
        if let Some(req) = parse_search_request(frame) {
            for cid_name in &req.queries {
                let exists = source.has_pv(&cid_name.1).await;
                if !exists {
                    continue;
                }
                let resp = build_search_response_proto(
                    guid,
                    req.seq,
                    tcp_port,
                    &[cid_name.0],
                    req.byte_order,
                    protocol,
                );
                if let Err(e) = socket.send_to(&resp, peer).await {
                    debug!("udp send to {peer}: {e}");
                }
            }
        }
    }

    #[allow(unreachable_code)]
    {
        _beacon.abort();
        Ok(())
    }
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
    let mut queries = Vec::with_capacity(n);
    for _ in 0..n {
        let cid = p.get_u32(order).ok()?;
        let name = decode_string(&mut p, order).ok().flatten()?;
        queries.push((cid, name));
    }
    Some(SearchRequest {
        seq,
        byte_order: order,
        queries,
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
