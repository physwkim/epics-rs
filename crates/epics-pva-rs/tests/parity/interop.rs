//! Cross-implementation interop matrix against pvxs `softIocPVX` / `pvget` /
//! `pvput` / `pvmonitor`.
//!
//! These tests are **ignored by default** because they require a built
//! `pvxs` available locally. Run with:
//!
//! ```bash
//! PVXS_HOME=/path/to/pvxs cargo test -p epics-pva-rs --test parity_interop -- --ignored
//! ```
//!
//! The harness probes `PVXS_HOME/bundle/usr/local/bin` first, then
//! `PVXS_HOME/bin`, then `$PATH`. If `softIocPVX` is missing the test
//! self-skips (with a tracing message) so the matrix degrades gracefully.

#![cfg(test)]

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, Command};

/// Find a pvxs binary by name. Returns `None` when the file isn't found.
fn find_pvxs_bin(name: &str) -> Option<PathBuf> {
    if let Ok(home) = std::env::var("PVXS_HOME") {
        let home = PathBuf::from(home);
        for sub in &["bundle/usr/local/bin", "bin"] {
            let p = home.join(sub).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // Fallback: PATH
    if let Some(found) = which_binary(name) {
        return Some(found);
    }
    None
}

fn which_binary(name: &str) -> Option<PathBuf> {
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in path_env.split(':') {
            let p = Path::new(dir).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// True iff every named binary is available locally.
fn pvxs_available(names: &[&str]) -> bool {
    names.iter().all(|n| find_pvxs_bin(n).is_some())
}

#[tokio::test]
#[ignore]
async fn rust_client_to_pvxs_softiocpvx_get() {
    if !pvxs_available(&["softIocPVX", "pvget"]) {
        eprintln!("pvxs not found; set PVXS_HOME and rerun");
        return;
    }

    let softioc = find_pvxs_bin("softIocPVX").unwrap();

    // Spawn softIocPVX with a minimal db file.
    let dbfile = tempfile::NamedTempFile::new().expect("temp db file");
    std::fs::write(
        dbfile.path(),
        "record(ai, \"INTEROP:VAL\") { field(VAL, \"42.5\") }\n",
    )
    .unwrap();

    let port = 25075u16;
    let udp = 25076u16;

    let mut cmd = Command::new(&softioc);
    cmd.env("EPICS_PVA_SERVER_PORT", port.to_string())
        .env("EPICS_PVA_BROADCAST_PORT", udp.to_string())
        .arg("-d")
        .arg(dbfile.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn softIocPVX");
    let _killer = ChildKiller(&mut child);

    // Give the IOC time to start.
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Native client GET against the pvxs IOC.
    use epics_pva_rs::client_native::context::PvaClient;
    let addr = std::net::SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        port,
    );
    let client = PvaClient::builder()
        .timeout(Duration::from_secs(3))
        .server_addr(addr)
        .build();

    let v = tokio::time::timeout(Duration::from_secs(5), client.pvget("INTEROP:VAL"))
        .await
        .expect("pvget timeout")
        .expect("pvget failed");

    eprintln!("INTEROP got: {v}");
    assert!(matches!(v, epics_pva_rs::pvdata::PvField::Structure(_)));
}

#[tokio::test]
#[ignore]
async fn pvxs_pvget_to_rust_server_get() {
    if !pvxs_available(&["pvget"]) {
        eprintln!("pvxs pvget not found; set PVXS_HOME and rerun");
        return;
    }
    let pvget = find_pvxs_bin("pvget").unwrap();

    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::sync::{mpsc, Mutex};

    use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};
    use epics_pva_rs::server_native::{run_pva_server, ChannelSource, PvaServerConfig};

    #[derive(Clone)]
    struct Source {
        v: Arc<Mutex<f64>>,
    }
    impl ChannelSource for Source {
        fn list_pvs(&self) -> impl std::future::Future<Output = Vec<String>> + Send {
            async { vec!["INTEROP:RS".into()] }
        }
        fn has_pv(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
            let n = name.to_string();
            async move { n == "INTEROP:RS" }
        }
        fn get_introspection(
            &self,
            _: &str,
        ) -> impl std::future::Future<Output = Option<FieldDesc>> + Send {
            async {
                Some(FieldDesc::Structure {
                    struct_id: "epics:nt/NTScalar:1.0".into(),
                    fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Double))],
                })
            }
        }
        fn get_value(
            &self,
            _: &str,
        ) -> impl std::future::Future<Output = Option<PvField>> + Send {
            let v = self.v.clone();
            async move {
                let val = *v.lock().await;
                let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
                s.fields
                    .push(("value".into(), PvField::Scalar(ScalarValue::Double(val))));
                Some(PvField::Structure(s))
            }
        }
        fn put_value(
            &self,
            _: &str,
            _: PvField,
        ) -> impl std::future::Future<Output = Result<(), String>> + Send {
            async { Ok(()) }
        }
        fn is_writable(&self, _: &str) -> impl std::future::Future<Output = bool> + Send {
            async { false }
        }
        fn subscribe(
            &self,
            _: &str,
        ) -> impl std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send {
            async { None }
        }
    }

    static NEXT: AtomicU32 = AtomicU32::new(28075);
    let port = NEXT.fetch_add(2, Ordering::Relaxed) as u16;

    let source = Arc::new(Source {
        v: Arc::new(Mutex::new(123.5)),
    });
    let cfg = PvaServerConfig {
        tcp_port: port,
        udp_port: port + 1,
        ..Default::default()
    };
    let h = tokio::spawn(async move {
        let _ = run_pva_server(source, cfg).await;
    });
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Run pvxs `pvget` against our server. We bypass UDP search by
    // forcing EPICS_PVA_ADDR_LIST.
    let output = Command::new(&pvget)
        .env(
            "EPICS_PVA_ADDR_LIST",
            format!("127.0.0.1:{}", port + 1),
        )
        .env("EPICS_PVA_AUTO_ADDR_LIST", "NO")
        .arg("-w")
        .arg("3")
        .arg("INTEROP:RS")
        .output()
        .await
        .expect("pvget run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    eprintln!("pvxs pvget stdout:\n{stdout}");
    assert!(
        stdout.contains("INTEROP:RS") && stdout.contains("123"),
        "expected pvxs pvget to print PV name + value, got:\n{stdout}"
    );

    h.abort();
}

/// RAII wrapper that kills the child process on drop.
struct ChildKiller<'a>(&'a mut Child);
impl<'a> Drop for ChildKiller<'a> {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}
