//! `pvxvct-rs` — PV Access Virtual Cable Tester. Mirrors pvxs
//! `tools/pvxvct.cpp`.
//!
//! Listens on the UDP broadcast port for SEARCH (client → server) and
//! BEACON (server → client) frames, decodes the headers + key
//! metadata, and prints to stdout. Useful for diagnosing network
//! configuration issues — replicates `pvxvct` at the operationally
//! relevant level (decoded frames, no raw hex dump).
//!
//! ```text
//! pvxvct-rs                     # listen for both SEARCH and BEACON
//! pvxvct-rs -C                  # only SEARCH
//! pvxvct-rs -S                  # only BEACON
//! pvxvct-rs -H 192.168.1.5      # filter by source IP (repeatable)
//! ```

use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::time::SystemTime;

use clap::Parser;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

use epics_pva_rs::client_native::decode::try_parse_frame;
use epics_pva_rs::proto::{Command, ReadExt, decode_size, decode_string, ip_from_bytes};

#[derive(Parser)]
#[command(name = "pvxvct-rs", version, about = "PVA Virtual Cable Tester")]
struct Args {
    /// Show only client SEARCH packets.
    #[arg(short = 'C', conflicts_with = "server_only")]
    client_only: bool,

    /// Show only server BEACON packets.
    #[arg(short = 'S', conflicts_with = "client_only")]
    server_only: bool,

    /// Filter by source IP. Repeatable; entries OR-combined.
    #[arg(short = 'H', long = "host")]
    hosts: Vec<IpAddr>,

    /// Filter SEARCH frames by PV name. Repeatable; a frame is shown if any
    /// of its names matches any `-P` value. pvxs `pvxvct` parity (commit
    /// bb53bb8 "fix pvxvct: actually apply PV name and host/network filters").
    #[arg(short = 'P', long = "pv")]
    pvnames: Vec<String>,

    /// UDP port to bind. Defaults to EPICS_PVA_BROADCAST_PORT or 5076.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,
}

fn bind_udp(port: u16) -> std::io::Result<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    #[cfg(unix)]
    {
        let _ = sock.set_reuse_port(true);
    }
    sock.set_broadcast(true)?;
    sock.set_nonblocking(true)?;
    let bind: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    sock.bind(&bind.into())?;
    let std_sock: StdUdpSocket = sock.into();
    UdpSocket::from_std(std_sock)
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let frac_us = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_micros())
        .unwrap_or(0);
    // Cheap formatter: "Tssss.uuuuuu" — fine for a debug tool.
    format!("T{secs}.{frac_us:06}")
}

fn fmt_guid(g: &[u8]) -> String {
    let mut s = String::with_capacity(24);
    for b in g {
        use std::fmt::Write;
        write!(&mut s, "{b:02X}").unwrap();
    }
    s
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let port = args.port.unwrap_or_else(|| {
        std::env::var("EPICS_PVA_BROADCAST_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(5076)
    });
    let socket = match bind_udp(port) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("pvxvct-rs: bind 0.0.0.0:{port}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("pvxvct-rs: listening on 0.0.0.0:{port}");

    let mut buf = vec![0u8; 4096];
    loop {
        let (n, peer) = match socket.recv_from(&mut buf).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("pvxvct-rs: recv: {e}");
                continue;
            }
        };
        if !args.hosts.is_empty() && !args.hosts.contains(&peer.ip()) {
            continue;
        }

        let bytes = &buf[..n];
        let Ok(Some((frame, _consumed))) = try_parse_frame(bytes) else {
            continue;
        };
        let cmd = Command::from_code(frame.header.command);
        let order = frame.header.flags.byte_order();

        match cmd {
            Some(Command::Beacon) if !args.client_only => {
                // Re-decode the beacon body to surface the advertised
                // server address + GUID + proto string.
                let mut cur = Cursor::new(frame.payload.as_slice());
                let guid = cur.get_bytes(12).unwrap_or_default();
                let _flags = cur.get_u8().unwrap_or(0);
                let _seq = cur.get_u8().unwrap_or(0);
                let _change = cur.get_u16(order).unwrap_or(0);
                let addr = cur.get_bytes(16).unwrap_or_default();
                let server_port = cur.get_u16(order).unwrap_or(0);
                let proto = decode_string(&mut cur, order)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "tcp".into());

                let mut addr_arr = [0u8; 16];
                addr_arr[..addr.len().min(16)].copy_from_slice(&addr[..addr.len().min(16)]);
                let server_ip =
                    ip_from_bytes(&addr_arr).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                let server_disp = if server_ip.is_unspecified() {
                    peer.ip()
                } else {
                    server_ip
                };
                println!(
                    "{} BEACON   peer={peer:21} server={server_disp}:{server_port} proto={proto} guid={}",
                    now_iso(),
                    fmt_guid(&guid)
                );
            }
            Some(Command::Search) if !args.server_only => {
                // Header + payload-tail decode: SEARCH carries
                // (seq:u32, flags:u8, reserved:u24, response_addr:16,
                // response_port:u16, n_protocols:u8, ...). For
                // operational debug we just surface seq + reply
                // address + first PV name.
                let mut cur = Cursor::new(frame.payload.as_slice());
                let seq = cur.get_u32(order).unwrap_or(0);
                let _flags = cur.get_u8().unwrap_or(0);
                let _ = cur.get_bytes(3); // reserved
                let resp_addr = cur.get_bytes(16).unwrap_or_default();
                let resp_port = cur.get_u16(order).unwrap_or(0);
                let mut addr_arr = [0u8; 16];
                addr_arr[..resp_addr.len().min(16)]
                    .copy_from_slice(&resp_addr[..resp_addr.len().min(16)]);
                let resp_ip = ip_from_bytes(&addr_arr).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                let n_protos = decode_size(&mut cur, order).ok().flatten().unwrap_or(0);
                for _ in 0..n_protos {
                    let _ = decode_string(&mut cur, order);
                }
                let n_search = cur.get_u16(order).unwrap_or(0);
                let mut names = Vec::new();
                for _ in 0..n_search {
                    let _cid = cur.get_u32(order).unwrap_or(0);
                    if let Ok(Some(name)) = decode_string(&mut cur, order) {
                        names.push(name);
                    }
                }
                // -P filter: if any -P values were given, only print
                // frames whose name set overlaps the filter set.
                // Empty `names` means a discover SEARCH (channel
                // count = 0 in pvxs `tickSearch(SearchKind::
                // discover)`); always show those — `-P` is for
                // narrowing per-PV searches, not for hiding the
                // network's discover heartbeat.
                if !args.pvnames.is_empty()
                    && !names.is_empty()
                    && !names.iter().any(|n| args.pvnames.iter().any(|p| p == n))
                {
                    continue;
                }
                println!(
                    "{} SEARCH   peer={peer:21} seq={seq} reply={resp_ip}:{resp_port} pvs={names:?}",
                    now_iso()
                );
            }
            _ => {
                if !args.client_only && !args.server_only {
                    println!(
                        "{} OTHER    peer={peer:21} cmd_code={}",
                        now_iso(),
                        frame.header.command
                    );
                }
            }
        }
    }
}
