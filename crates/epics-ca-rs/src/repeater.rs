use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket as StdUdpSocket};

use tokio::net::UdpSocket;

use crate::protocol::*;

/// Per-client connected UDP socket, matching C EPICS repeaterClient.
/// Using a connected socket lets the OS detect dead clients via
/// ECONNREFUSED on send().
struct RepeaterClient {
    sock: StdUdpSocket,
    addr: SocketAddr,
}

impl RepeaterClient {
    fn new(addr: SocketAddr) -> io::Result<Self> {
        let sock = StdUdpSocket::bind("0.0.0.0:0")?;
        sock.connect(addr)?;
        sock.set_nonblocking(true)?;
        Ok(Self { sock, addr })
    }

    fn send_confirm(&self) -> bool {
        let mut confirm = CaHeader::new(CA_PROTO_REPEATER_CONFIRM);
        if let SocketAddr::V4(v4) = self.addr {
            confirm.available = u32::from_be_bytes(v4.ip().octets());
        }
        self.sock.send(&confirm.to_bytes()).is_ok()
    }

    fn send_message(&self, data: &[u8]) -> bool {
        match self.sock.send(data) {
            Ok(_) => true,
            Err(e) => {
                // ECONNREFUSED means client is gone
                matches!(e.kind(), io::ErrorKind::ConnectionRefused)
                    .then_some(false)
                    .unwrap_or(false)
            }
        }
    }

    /// Check if client is still alive by trying to bind to its port.
    /// If bind succeeds, the client has released the port (dead).
    fn verify(&self) -> bool {
        let port = match self.addr {
            SocketAddr::V4(v4) => v4.port(),
            _ => return false,
        };
        match StdUdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)) {
            Ok(_) => false,                                    // port free → client gone
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => true, // port in use → alive
            Err(_) => true,                                    // other error → assume alive
        }
    }
}

/// Run the CA repeater daemon.
/// Binds to UDP 5065, accepts client registrations, and fans out beacons.
pub async fn run_repeater() -> io::Result<()> {
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, CA_REPEATER_PORT);
    let socket = UdpSocket::bind(bind_addr).await?;

    let mut clients: HashMap<u16, RepeaterClient> = HashMap::new();
    let mut buf = [0u8; 4096];

    loop {
        let (len, src) = socket.recv_from(&mut buf).await?;

        // C CA clients send a zero-length UDP packet for repeater
        // registration (backward compat with pre-3.12 repeaters).
        if len == 0 {
            register_client(&mut clients, src);
            continue;
        }

        if len < CaHeader::SIZE {
            continue;
        }

        let Ok(hdr) = CaHeader::from_bytes(&buf[..len]) else {
            continue;
        };

        match hdr.cmmd {
            CA_PROTO_REPEATER_REGISTER => {
                register_client(&mut clients, src);
            }
            _ => {
                // Beacon or other message from server — fan out to all
                // registered clients.  Fill in available=0 with source IP
                // so clients can identify the server (matches C repeater).
                let mut data = buf[..len].to_vec();
                if len >= CaHeader::SIZE {
                    let avail_offset = 12; // available field at bytes 12..16
                    let avail = u32::from_be_bytes([
                        data[avail_offset],
                        data[avail_offset + 1],
                        data[avail_offset + 2],
                        data[avail_offset + 3],
                    ]);
                    if avail == 0 {
                        if let SocketAddr::V4(v4) = src {
                            data[avail_offset..avail_offset + 4]
                                .copy_from_slice(&v4.ip().octets());
                        }
                    }
                }

                let src_port = src.port();
                let mut dead = Vec::new();
                for (port, client) in &clients {
                    // Don't reflect back to sender
                    if *port == src_port {
                        continue;
                    }
                    if !client.send_message(&data) {
                        if !client.verify() {
                            dead.push(*port);
                        }
                    }
                }
                for p in dead {
                    clients.remove(&p);
                }
            }
        }
    }
}

fn register_client(clients: &mut HashMap<u16, RepeaterClient>, src: SocketAddr) {
    let port = src.port();

    // Already registered — just re-send confirm
    if let Some(client) = clients.get(&port) {
        client.send_confirm();
        return;
    }

    // Create per-client connected socket (matches C EPICS repeater)
    let client = match RepeaterClient::new(src) {
        Ok(c) => c,
        Err(_) => return,
    };

    if !client.send_confirm() {
        return;
    }

    clients.insert(port, client);

    // Send VERSION noop to all other clients so we don't accumulate
    // sockets when there are no beacons (matches C EPICS).
    let noop = CaHeader::new(CA_PROTO_VERSION);
    let noop_bytes = noop.to_bytes();
    let mut dead = Vec::new();
    for (p, c) in clients.iter() {
        if *p == port {
            continue;
        }
        if !c.send_message(&noop_bytes) {
            if !c.verify() {
                dead.push(*p);
            }
        }
    }
    for p in dead {
        clients.remove(&p);
    }
}

/// Try to register with an existing repeater. If none is running, spawn one
/// as a background process using the current executable's `ca-repeater` binary,
/// then register again.
pub async fn ensure_repeater() {
    if try_register().await.is_ok() {
        return;
    }

    // No repeater running — spawn one
    spawn_repeater();

    // Give it a moment to start, then register
    epics_base_rs::runtime::task::sleep(std::time::Duration::from_millis(50)).await;
    let _ = try_register().await;
}

/// Send a REPEATER_REGISTER to localhost:5065 and wait for CONFIRM.
async fn try_register() -> Result<(), ()> {
    let socket = UdpSocket::bind("0.0.0.0:0").await.map_err(|_| ())?;

    let local_ip = match socket.local_addr().ok() {
        Some(SocketAddr::V4(v4)) => *v4.ip(),
        _ => Ipv4Addr::LOCALHOST,
    };

    let mut hdr = CaHeader::new(CA_PROTO_REPEATER_REGISTER);
    hdr.available = u32::from_be_bytes(local_ip.octets());

    let repeater_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, CA_REPEATER_PORT);
    socket
        .send_to(&hdr.to_bytes(), repeater_addr)
        .await
        .map_err(|_| ())?;

    // Wait for confirm with short timeout
    let mut buf = [0u8; 64];
    let result = tokio::time::timeout(std::time::Duration::from_millis(200), async {
        loop {
            let (len, _) = socket.recv_from(&mut buf).await.map_err(|_| ())?;
            if len >= CaHeader::SIZE {
                if let Ok(resp) = CaHeader::from_bytes(&buf[..len]) {
                    if resp.cmmd == CA_PROTO_REPEATER_CONFIRM {
                        return Ok::<(), ()>(());
                    }
                }
            }
        }
    })
    .await;

    match result {
        Ok(Ok(())) => Ok(()),
        _ => Err(()),
    }
}

/// Spawn the repeater as a detached background process.
/// Falls back to an in-process repeater thread if the binary is not found.
fn spawn_repeater() {
    let exe = std::env::current_exe().unwrap_or_default();
    let repeater_bin = exe.parent().map(|p| p.join("ca-repeater-rs"));

    // Try external binary first
    if let Some(ref bin) = repeater_bin {
        if bin.exists() {
            use std::process::{Command, Stdio};
            if Command::new(bin)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
            {
                return;
            }
        }
    }

    // Fallback: run repeater in-process on a background thread.
    // This ensures beacon reception works even without the external binary.
    std::thread::spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("repeater runtime");
        let _ = rt.block_on(run_repeater());
    });
}
