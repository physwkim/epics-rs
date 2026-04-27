use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use crate::protocol::*;
use epics_base_rs::error::CaResult;
use epics_base_rs::server::database::PvDatabase;

/// Run UDP search responders bound to one or more local interfaces.
///
/// Each interface gets its own task — having a dedicated socket per
/// interface lets the OS keep the broadcast routing straight on multi-NIC
/// hosts (matching C EPICS osiSockDiscoverInterfaces behaviour).
///
/// `ignore_addrs` filters out source addresses that should never receive
/// search replies (EPICS_CAS_IGNORE_ADDR_LIST).
pub async fn run_udp_search_responder(
    db: Arc<PvDatabase>,
    port: u16,
    tcp_port: u16,
    intf_addrs: Vec<Ipv4Addr>,
    ignore_addrs: Vec<Ipv4Addr>,
) -> CaResult<()> {
    let intfs = if intf_addrs.is_empty() {
        vec![Ipv4Addr::UNSPECIFIED]
    } else {
        intf_addrs
    };

    // Spawn one responder task per interface and wait for the first error.
    let mut handles = Vec::with_capacity(intfs.len());
    for ip in intfs {
        let db_t = db.clone();
        let ignore_t = ignore_addrs.clone();
        let handle = epics_base_rs::runtime::task::spawn(async move {
            run_single_responder(db_t, ip, port, tcp_port, ignore_t).await
        });
        handles.push(handle);
    }

    // Propagate the first error, abort the rest.
    let mut handles_iter = handles.into_iter();
    let result = if let Some(first) = handles_iter.next() {
        match first.await {
            Ok(inner) => inner,
            Err(e) => Err(epics_base_rs::error::CaError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))),
        }
    } else {
        Ok(())
    };
    for h in handles_iter {
        h.abort();
    }
    result
}

async fn run_single_responder(
    db: Arc<PvDatabase>,
    bind_ip: Ipv4Addr,
    port: u16,
    tcp_port: u16,
    ignore_addrs: Vec<Ipv4Addr>,
) -> CaResult<()> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    #[cfg(target_os = "macos")]
    sock.set_reuse_port(true)?;
    sock.set_nonblocking(true)?;
    sock.bind(&std::net::SocketAddrV4::new(bind_ip, port).into())?;
    let socket = UdpSocket::from_std(sock.into())?;
    socket.set_broadcast(true)?;

    let mut buf = [0u8; 4096];

    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;
        if len < CaHeader::SIZE {
            continue;
        }

        // Apply ignore list (EPICS_CAS_IGNORE_ADDR_LIST). Any datagram
        // whose source IP appears in the list is silently dropped.
        if let SocketAddr::V4(v4) = src {
            if ignore_addrs.contains(v4.ip()) {
                continue;
            }
        }

        let mut offset = 0;
        while offset + CaHeader::SIZE <= len {
            let hdr = match CaHeader::from_bytes(&buf[offset..]) {
                Ok(h) => h,
                Err(_) => break,
            };
            let payload_size = align8(hdr.postsize as usize);
            let msg_len = CaHeader::SIZE + payload_size;

            if offset + msg_len > len {
                break;
            }

            if hdr.cmmd == CA_PROTO_SEARCH {
                let payload_start = offset + CaHeader::SIZE;
                let payload_end = payload_start + hdr.postsize as usize;
                let payload = &buf[payload_start..payload_end];

                // Extract PV name (null-terminated)
                let pv_name_end = payload
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(payload.len());
                if let Ok(pv_name) = std::str::from_utf8(&payload[..pv_name_end]) {
                    if db.has_name(pv_name).await {
                        let server_ip = local_ip_for(src);
                        let mut resp = CaHeader::new(CA_PROTO_SEARCH);
                        resp.postsize = 8;
                        resp.data_type = tcp_port;
                        resp.count = 0;
                        resp.cid = u32::from_be_bytes(server_ip.octets());
                        resp.available = hdr.available;

                        let mut ver = CaHeader::new(CA_PROTO_VERSION);
                        ver.count = CA_MINOR_VERSION;

                        let mut reply = Vec::with_capacity(CaHeader::SIZE * 2 + 8);
                        reply.extend_from_slice(&ver.to_bytes());
                        reply.extend_from_slice(&resp.to_bytes());
                        let mut search_payload = [0u8; 8];
                        search_payload[0..2].copy_from_slice(&CA_MINOR_VERSION.to_be_bytes());
                        reply.extend_from_slice(&search_payload);

                        let _ = socket.send_to(&reply, src).await;
                    } else if hdr.data_type == CA_DO_REPLY {
                        // Client asked for an explicit negative reply
                        // (search header data_type == CA_DO_REPLY=10).
                        // Send CA_PROTO_NOT_FOUND so it doesn't have to
                        // wait for the search timeout.
                        let mut nf = CaHeader::new(CA_PROTO_NOT_FOUND);
                        nf.data_type = CA_DO_REPLY;
                        nf.count = CA_MINOR_VERSION;
                        nf.cid = hdr.available;
                        nf.available = hdr.available;
                        let _ = socket.send_to(&nf.to_bytes(), src).await;
                    }
                }
            }

            offset += msg_len;
        }
    }
}

/// Determine the local interface IP that would route to `remote`.
/// Creates a temporary unconnected UDP socket and "connects" it (no data
/// is sent — this just lets the OS pick the outgoing interface via routing).
fn local_ip_for(remote: SocketAddr) -> Ipv4Addr {
    let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") else {
        return Ipv4Addr::UNSPECIFIED;
    };
    if sock.connect(remote).is_err() {
        return Ipv4Addr::UNSPECIFIED;
    }
    match sock.local_addr() {
        Ok(SocketAddr::V4(a)) => *a.ip(),
        _ => Ipv4Addr::UNSPECIFIED,
    }
}
