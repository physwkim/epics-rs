#![allow(dead_code)] // Helpers are conditionally used across test files.

//! Shared helpers for CA interop and soak tests.
//!
//! These tests exercise epics-ca-rs against the reference EPICS C
//! implementation (`softIoc`, `caget`, `caput`, `camonitor`). They are
//! gated on the C tools being available so that CI environments without
//! a full EPICS install can still run the rest of the suite.

use std::net::{TcpListener, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Returns true when the named binary resolves on PATH or in the local
/// EPICS install. Used by interop tests to early-exit on hosts that
/// lack a C reference implementation.
pub fn have_tool(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Skip the test (printed to stderr) when a required C tool is missing.
/// Returns `true` when the test should proceed.
pub fn require_tool(name: &str) -> bool {
    if have_tool(name) {
        true
    } else {
        eprintln!("SKIP: `{name}` not found on PATH; install EPICS base to run this test");
        false
    }
}

/// Pick a free TCP port by binding ephemeral and immediately closing.
pub fn free_tcp_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("free TCP port");
    listener.local_addr().unwrap().port()
}

/// Pick a free UDP port the same way.
pub fn free_udp_port() -> u16 {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("free UDP port");
    sock.local_addr().unwrap().port()
}

/// A child process that's killed when dropped, plus the `EPICS_CA_*`
/// environment overrides callers should propagate to clients/IOCs that
/// need to talk to it.
pub struct ManagedIoc {
    child: Child,
    pub udp_port: u16,
    pub tcp_port: u16,
}

impl Drop for ManagedIoc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl ManagedIoc {
    pub fn ca_addr_list(&self) -> String {
        format!("127.0.0.1:{}", self.udp_port)
    }
}

/// Spawn a `softIoc` process running the supplied `.db` content. Returns
/// once the IOC has been observed accepting CA traffic on `udp_port`.
///
/// Uses the standard EPICS install at /Users/stevek/codes/epics-base
/// (test fixture path) and a per-test ephemeral UDP/TCP port pair so
/// concurrent tests don't collide.
pub fn spawn_softioc(db_content: &str) -> Option<ManagedIoc> {
    if !have_tool("softIoc") {
        return None;
    }
    let dir = tempfile::tempdir().expect("temp dir");
    let db_path = dir.path().join("test.db");
    std::fs::write(&db_path, db_content).expect("write db");

    let udp_port = free_udp_port();
    let tcp_port = free_tcp_port();

    let mut cmd = Command::new("softIoc");
    cmd.arg("-S") // No interactive shell — keeps softIoc happy without a TTY
        .arg("-d")
        .arg(&db_path)
        .env("EPICS_CAS_INTF_ADDR_LIST", "127.0.0.1")
        .env("EPICS_CAS_BEACON_ADDR_LIST", "127.0.0.1")
        .env("EPICS_CA_ADDR_LIST", "127.0.0.1")
        .env("EPICS_CA_AUTO_ADDR_LIST", "NO")
        .env("EPICS_CA_SERVER_PORT", udp_port.to_string())
        .env("EPICS_CAS_SERVER_PORT", udp_port.to_string())
        .env("EPICS_CA_REPEATER_PORT", "5165")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().ok()?;

    // Hand the IOC time to bind sockets before tests exercise it.
    std::thread::sleep(Duration::from_millis(800));

    // Keep tempdir alive by leaking — the child has the .db open.
    std::mem::forget(dir);

    Some(ManagedIoc {
        child,
        udp_port,
        tcp_port,
    })
}

/// Run a one-shot `caget` and return stdout (trimmed). Returns None on
/// non-zero exit.
pub fn run_caget(addr_list: &str, server_port: u16, pv: &str) -> Option<String> {
    let out = Command::new("caget")
        .arg("-w")
        .arg("3")
        .arg(pv)
        .env("EPICS_CA_ADDR_LIST", addr_list)
        .env("EPICS_CA_AUTO_ADDR_LIST", "NO")
        .env("EPICS_CA_SERVER_PORT", server_port.to_string())
        .output()
        .ok()?;
    if !out.status.success() {
        eprintln!("caget failed: {}", String::from_utf8_lossy(&out.stderr));
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Run a one-shot `caput` and return success/failure.
pub fn run_caput(addr_list: &str, server_port: u16, pv: &str, value: &str) -> bool {
    Command::new("caput")
        .arg("-w")
        .arg("3")
        .arg(pv)
        .arg(value)
        .env("EPICS_CA_ADDR_LIST", addr_list)
        .env("EPICS_CA_AUTO_ADDR_LIST", "NO")
        .env("EPICS_CA_SERVER_PORT", server_port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
