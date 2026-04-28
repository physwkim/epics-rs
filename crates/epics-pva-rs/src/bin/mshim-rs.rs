//! `mshim-rs` — beacon multicast shim mirroring pvxs `tools/mshim.cpp`.
//!
//! Listens on one or more `-L <ip[:port]>` endpoints and forwards
//! every received UDP datagram to one or more `-F <ip[:port]>`
//! destinations. Used to bridge IPv4 multicast to PVA clients /
//! servers that don't speak multicast natively.
//!
//! ```text
//! # 1. Forward local SEARCH packets to a multicast group:
//! mshim-rs -L 127.0.0.1:15076 -F 224.1.1.1:5076
//!
//! # 2. Forward multicast BEACONs back to local clients:
//! mshim-rs -L 224.1.1.1:5076 -F 127.0.0.1:15076
//! ```
//!
//! Multicast addresses (224.0.0.0/4) on the listen side are joined
//! automatically. On the send side, multicast destinations use the
//! OS-default outbound interface and TTL. The full pvxs syntax
//! (`@iface` interface override, `,ttl#`) is accepted at the parser
//! level but currently logged as advisory — the simpler default
//! routes work for the common cross-subnet scenario.

use std::io::ErrorKind;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};

use clap::Parser;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

#[derive(Parser)]
#[command(name = "mshim-rs", version, about = "PVA beacon/search multicast shim")]
struct Args {
    /// Listen endpoint. Repeat for multiple. Multicast groups are
    /// joined automatically.
    #[arg(short = 'L', long = "listen", required = true)]
    listen: Vec<String>,

    /// Forward destination. Repeat for multiple.
    #[arg(short = 'F', long = "forward", required = true)]
    forward: Vec<String>,

    /// Default UDP port if a `-L` / `-F` entry omits one.
    #[arg(short = 'p', long = "port")]
    port: Option<u16>,
}

#[derive(Debug)]
struct Endpoint {
    ip: IpAddr,
    port: u16,
    /// `@iface` override or `,ttl#` modifiers (parsed but advisory
    /// only — kernel-default routing handles the common case).
    extra: Option<String>,
}

fn parse_endpoint(s: &str, default_port: u16) -> Result<Endpoint, String> {
    // Split off any `,ttl#` or `@iface` suffix first.
    let (head, extra) = match s.find([',', '@']) {
        Some(idx) => (&s[..idx], Some(s[idx..].to_string())),
        None => (s, None),
    };
    let (ip_str, port) = if let Some((a, b)) = head.rsplit_once(':') {
        let port: u16 = b.parse().map_err(|e| format!("port {b:?} invalid: {e}"))?;
        (a, port)
    } else {
        (head, default_port)
    };
    let ip: IpAddr = ip_str
        .parse()
        .map_err(|e| format!("ip {ip_str:?} invalid: {e}"))?;
    Ok(Endpoint { ip, port, extra })
}

fn bind_listen(ep: &Endpoint) -> std::io::Result<UdpSocket> {
    let sock = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    sock.set_reuse_address(true)?;
    #[cfg(unix)]
    {
        let _ = sock.set_reuse_port(true);
    }
    sock.set_broadcast(true)?;
    sock.set_nonblocking(true)?;
    // Multicast groups must bind 0.0.0.0; unicast/broadcast bind to
    // the actual address so packets only show up there.
    let bind_addr = if matches!(ep.ip, IpAddr::V4(v4) if v4.is_multicast()) {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), ep.port)
    } else {
        SocketAddr::new(ep.ip, ep.port)
    };
    sock.bind(&bind_addr.into())?;
    if let IpAddr::V4(v4) = ep.ip
        && v4.is_multicast()
    {
        sock.join_multicast_v4(&v4, &Ipv4Addr::UNSPECIFIED)?;
    }
    let std_sock: StdUdpSocket = sock.into();
    UdpSocket::from_std(std_sock)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let default_port = args
        .port
        .or_else(|| {
            std::env::var("EPICS_PVA_BROADCAST_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(5076);

    let listen: Vec<Endpoint> = match args
        .listen
        .iter()
        .map(|s| parse_endpoint(s, default_port))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mshim-rs: {e}");
            std::process::exit(2);
        }
    };
    let forward: Vec<Endpoint> = match args
        .forward
        .iter()
        .map(|s| parse_endpoint(s, default_port))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!("mshim-rs: {e}");
            std::process::exit(2);
        }
    };

    // Single send socket — the kernel routes per-destination IP.
    // Tokio requires nonblocking sockets when adopting via
    // `from_std`, otherwise the runtime registration panics.
    let send_sock_std = match StdUdpSocket::bind("0.0.0.0:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mshim-rs: bind send socket: {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = send_sock_std.set_nonblocking(true) {
        eprintln!("mshim-rs: send_sock set_nonblocking: {e}");
        std::process::exit(1);
    }
    if let Err(e) = send_sock_std.set_broadcast(true) {
        eprintln!("mshim-rs: set_broadcast: {e}");
    }
    let send_sock = match UdpSocket::from_std(send_sock_std) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("mshim-rs: send socket from_std: {e}");
            std::process::exit(1);
        }
    };
    let send_sock = std::sync::Arc::new(send_sock);

    let forward_targets: Vec<SocketAddr> = forward
        .iter()
        .map(|e| SocketAddr::new(e.ip, e.port))
        .collect();

    eprintln!(
        "mshim-rs: listening on {} endpoint(s), forwarding to {} target(s)",
        listen.len(),
        forward.len()
    );
    for ep in &listen {
        if let Some(extra) = &ep.extra {
            eprintln!(
                "  listen {}:{} (extra {extra:?} — advisory)",
                ep.ip, ep.port
            );
        } else {
            eprintln!("  listen {}:{}", ep.ip, ep.port);
        }
    }
    for ep in &forward {
        if let Some(extra) = &ep.extra {
            eprintln!(
                "  forward → {}:{} (extra {extra:?} — advisory)",
                ep.ip, ep.port
            );
        } else {
            eprintln!("  forward → {}:{}", ep.ip, ep.port);
        }
    }

    let mut handles = Vec::new();
    for ep in listen {
        let sock = match bind_listen(&ep) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("mshim-rs: bind {}:{}: {e}", ep.ip, ep.port);
                std::process::exit(1);
            }
        };
        let targets = forward_targets.clone();
        let send_sock = send_sock.clone();
        let h = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match sock.recv_from(&mut buf).await {
                    Ok((n, peer)) => {
                        let payload = &buf[..n];
                        for tgt in &targets {
                            // Avoid an obvious feedback loop: don't
                            // forward back to the source endpoint of
                            // the same datagram.
                            if *tgt == peer {
                                continue;
                            }
                            if let Err(e) = send_sock.send_to(payload, tgt).await
                                && e.kind() != ErrorKind::WouldBlock
                            {
                                eprintln!("mshim-rs: forward to {tgt}: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("mshim-rs: recv on {}:{}: {e}", ep.ip, ep.port);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        handles.push(h);
    }

    // Wait forever; tokio::signal::ctrl_c gives clean exit.
    tokio::select! {
        _ = futures_join_all(handles) => {}
        _ = tokio::signal::ctrl_c() => {
            eprintln!("mshim-rs: shutting down");
        }
    }
}

async fn futures_join_all(handles: Vec<tokio::task::JoinHandle<()>>) {
    for h in handles {
        let _ = h.await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_endpoint_ipv4_with_port() {
        let ep = parse_endpoint("127.0.0.1:5076", 9999).unwrap();
        assert_eq!(ep.ip.to_string(), "127.0.0.1");
        assert_eq!(ep.port, 5076);
        assert!(ep.extra.is_none());
    }

    #[test]
    fn parse_endpoint_default_port_when_omitted() {
        let ep = parse_endpoint("224.1.1.1", 5076).unwrap();
        assert_eq!(ep.ip.to_string(), "224.1.1.1");
        assert_eq!(ep.port, 5076);
    }

    #[test]
    fn parse_endpoint_ttl_modifier_kept_as_extra() {
        let ep = parse_endpoint("224.1.1.1,255", 5076).unwrap();
        assert_eq!(ep.ip.to_string(), "224.1.1.1");
        assert_eq!(ep.port, 5076);
        assert_eq!(ep.extra.as_deref(), Some(",255"));
    }

    #[test]
    fn parse_endpoint_iface_modifier_kept_as_extra() {
        let ep = parse_endpoint("224.1.1.1@eth0", 5076).unwrap();
        assert_eq!(ep.ip.to_string(), "224.1.1.1");
        assert_eq!(ep.extra.as_deref(), Some("@eth0"));
    }

    #[test]
    fn parse_endpoint_ipv4_port_with_iface() {
        // pvxs syntax: "224.1.1.1:5076@eth0"
        let ep = parse_endpoint("224.1.1.1:5076@eth0", 9999).unwrap();
        assert_eq!(ep.ip.to_string(), "224.1.1.1");
        assert_eq!(ep.port, 5076);
        assert_eq!(ep.extra.as_deref(), Some("@eth0"));
    }

    #[test]
    fn parse_endpoint_rejects_bad_ip() {
        assert!(parse_endpoint("not-an-ip", 5076).is_err());
    }

    #[test]
    fn parse_endpoint_rejects_bad_port() {
        assert!(parse_endpoint("127.0.0.1:notaport", 5076).is_err());
    }
}
