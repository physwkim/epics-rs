//! UDP forward integration tests for the `mshim-rs` binary,
//! mirroring pvxs `test/testudpfwd.cpp::testFwdVia`.
//!
//! Pattern: spawn `mshim-rs` as a subprocess with `-L 127.0.0.1:A -F
//! 127.0.0.1:B`, bind a sender socket and a receiver socket, send a
//! datagram to the listen port, verify it arrives at the forward
//! port. `CARGO_BIN_EXE_mshim-rs` gives the test the path of the
//! freshly-built binary.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Allocate two ephemeral UDP ports by binding+dropping. There's a
/// micro-window where another process could grab them, but for a
/// single-test loopback scenario the chance is negligible.
fn alloc_two_ports() -> (u16, u16) {
    let a = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let b = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let pa = a.local_addr().unwrap().port();
    let pb = b.local_addr().unwrap().port();
    drop(a);
    drop(b);
    (pa, pb)
}

struct ChildGuard(Option<Child>);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut c) = self.0.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[test]
fn mshim_forwards_loopback_datagram() {
    // Skip on platforms where the binary path env var isn't set
    // (older cargo) — environment_var-based binary lookup is the
    // canonical way to find a sibling bin.
    let bin = match option_env!("CARGO_BIN_EXE_mshim-rs") {
        Some(p) => p,
        None => {
            eprintln!("CARGO_BIN_EXE_mshim-rs not set — skipping");
            return;
        }
    };

    let (listen_port, forward_port) = alloc_two_ports();

    // Start the receiver socket FIRST so we don't miss the forwarded
    // packet.
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, forward_port)).expect("bind receiver");
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    // Spawn mshim-rs. Inherit stderr so a binding failure surfaces
    // in the test log instead of silently failing the wait loop.
    let child = Command::new(bin)
        .arg("-L")
        .arg(format!("127.0.0.1:{listen_port}"))
        .arg("-F")
        .arg(format!("127.0.0.1:{forward_port}"))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn mshim-rs");
    let _guard = ChildGuard(Some(child));

    // Give mshim-rs ≥500ms to bind before the first probe so the
    // initial datagrams aren't tossed at a not-yet-listening port.
    std::thread::sleep(Duration::from_millis(500));

    // Give mshim-rs a moment to bind. The binary prints to stderr
    // when up; we can't easily await that without piping, so we
    // poll-send and poll-recv with a short timeout.
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind sender");
    let listen_addr: SocketAddr = format!("127.0.0.1:{listen_port}").parse().unwrap();

    // Try sending a few times — mshim-rs may not be listening yet.
    let probe = b"PVXS-TEST-FRAME";
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received = false;
    while Instant::now() < deadline {
        sender.send_to(probe, listen_addr).expect("send");
        let mut buf = [0u8; 64];
        if let Ok((n, _)) = receiver.recv_from(&mut buf) {
            assert!(
                buf[..n].starts_with(probe),
                "forwarded payload mismatch: got {:?}",
                &buf[..n]
            );
            received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(received, "mshim-rs did not forward the test datagram");
}

#[test]
fn mshim_rejects_invalid_listen_endpoint() {
    let bin = match option_env!("CARGO_BIN_EXE_mshim-rs") {
        Some(p) => p,
        None => return,
    };
    let out = Command::new(bin)
        .arg("-L")
        .arg("not-an-ip:5076")
        .arg("-F")
        .arg("127.0.0.1:5076")
        .output()
        .expect("spawn");
    // exit code 2 = parse error per our CLI contract.
    assert_eq!(
        out.status.code(),
        Some(2),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
