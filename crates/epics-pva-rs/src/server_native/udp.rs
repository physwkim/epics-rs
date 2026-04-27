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
    decode_size, decode_string, encode_size_into, encode_string_into, ip_to_bytes, ByteOrder,
    Command, PvaHeader, ReadExt, WriteExt,
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
    )
    .await
}

/// Like [`run_udp_responder_proto`] but configurable: explicit beacon
/// period, explicit destinations, and an auto-NIC-broadcast flag. When
/// `destinations` is empty AND `auto_beacon` is true, beacons fan out
/// to per-NIC broadcasts (via [`crate::config::env::list_broadcast_addresses`]).
/// When `destinations` is non-empty, exactly those addresses are used.
pub async fn run_udp_responder_with_config(
    source: DynSource,
    udp_port: u16,
    tcp_port: u16,
    guid: [u8; 12],
    protocol: &'static str,
    beacon_period: Duration,
    destinations: Vec<SocketAddr>,
    auto_beacon: bool,
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
    debug!(?beacon_destinations, ?beacon_period, "beacon emitter config");

    let beacon_socket = socket.clone();
    let beacon_guid = guid;
    let beacon = tokio::spawn(async move {
        let mut tick = interval(beacon_period);
        loop {
            tick.tick().await;
            let beacon = build_beacon(beacon_guid, tcp_port, ByteOrder::Little);
            for dest in &beacon_destinations {
                let _ = beacon_socket.send_to(&beacon, dest).await;
            }
        }
    });

    let mut buf = vec![0u8; 1500];
    loop {
        let (n, peer) = match socket.recv_from(&mut buf).await {
            Ok(t) => t,
            Err(e) => {
                debug!("udp recv error: {e}");
                continue;
            }
        };
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
        beacon.abort();
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
    let header = PvaHeader::application(true, order, Command::SearchResponse.code(), payload.len() as u32);
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

fn build_beacon(guid: [u8; 12], tcp_port: u16, order: ByteOrder) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&guid);
    // pvxs server.cpp::doBeacons: flags(u8) + seq(u8) + change(u16) = 4 bytes
    payload.put_u8(0); // flags / QoS (undefined, 0)
    payload.put_u8(0); // beacon sequence (u8)
    payload.put_u16(0, order); // change count (u16)
    let addr = ip_to_bytes(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    payload.extend_from_slice(&addr);
    payload.put_u16(tcp_port, order);
    encode_string_into("tcp", order, &mut payload);
    payload.put_u8(0xFF); // null serverStatus marker (matches pvxs)
    let header =
        PvaHeader::application(true, order, Command::Beacon.code(), payload.len() as u32);
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
