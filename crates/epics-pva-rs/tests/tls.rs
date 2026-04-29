//! TLS end-to-end test.
//!
//! Generates a self-signed certificate at runtime, spins up a server that
//! accepts TLS-only, then connects a client that trusts that exact cert
//! and performs a GET. Confirms that the TCP-over-TLS plumbing in
//! `client_native::server_conn::connect_tls` and the `tokio_rustls`
//! acceptor in `server_native::tcp` actually shake hands and exchange
//! frames.

#![allow(clippy::manual_async_fn)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio::sync::{Mutex, mpsc};

use epics_pva_rs::auth::{TlsClientConfig, TlsServerConfig};
use epics_pva_rs::client_native::context::PvaClient;
use epics_pva_rs::pvdata::{FieldDesc, PvField, PvStructure, ScalarType, ScalarValue};
use epics_pva_rs::server_native::{ChannelSource, PvaServerConfig, run_pva_server};
use serial_test::file_serial;

// Generate a self-signed cert + matching key pair for tests.
fn generate_self_signed() -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()])
        .expect("self-signed cert");
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der: PrivateKeyDer<'static> =
        PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()).into();
    (cert_der, key_der)
}

#[derive(Clone)]
struct StaticSource {
    inner: Arc<Mutex<std::collections::HashMap<String, PvField>>>,
}

impl StaticSource {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
    async fn put(&self, name: &str, value: f64) {
        let mut s = PvStructure::new("epics:nt/NTScalar:1.0");
        s.fields
            .push(("value".into(), PvField::Scalar(ScalarValue::Double(value))));
        self.inner
            .lock()
            .await
            .insert(name.to_string(), PvField::Structure(s));
    }
}

fn nt_scalar_desc() -> FieldDesc {
    FieldDesc::Structure {
        struct_id: "epics:nt/NTScalar:1.0".into(),
        fields: vec![("value".into(), FieldDesc::Scalar(ScalarType::Double))],
    }
}

impl ChannelSource for StaticSource {
    fn list_pvs(&self) -> impl std::future::Future<Output = Vec<String>> + Send {
        let inner = self.inner.clone();
        async move { inner.lock().await.keys().cloned().collect::<Vec<_>>() }
    }
    fn has_pv(&self, name: &str) -> impl std::future::Future<Output = bool> + Send {
        let inner = self.inner.clone();
        let n = name.to_string();
        async move { inner.lock().await.contains_key(&n) }
    }
    fn get_introspection(
        &self,
        name: &str,
    ) -> impl std::future::Future<Output = Option<FieldDesc>> + Send {
        let inner = self.inner.clone();
        let n = name.to_string();
        async move {
            if inner.lock().await.contains_key(&n) {
                Some(nt_scalar_desc())
            } else {
                None
            }
        }
    }
    fn get_value(&self, name: &str) -> impl std::future::Future<Output = Option<PvField>> + Send {
        let inner = self.inner.clone();
        let n = name.to_string();
        async move { inner.lock().await.get(&n).cloned() }
    }
    fn put_value(
        &self,
        _name: &str,
        _value: PvField,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async { Ok(()) }
    }
    fn is_writable(&self, _name: &str) -> impl std::future::Future<Output = bool> + Send {
        async { false }
    }
    fn subscribe(
        &self,
        _name: &str,
    ) -> impl std::future::Future<Output = Option<mpsc::Receiver<PvField>>> + Send {
        async { None }
    }
}

static NEXT_PORT: AtomicU32 = AtomicU32::new(16075);
fn alloc_port_pair() -> (u16, u16) {
    let base = NEXT_PORT.fetch_add(2, Ordering::Relaxed) as u16;
    (base, base + 1)
}

#[tokio::test]
#[file_serial(pva_listener)]
async fn tls_client_to_tls_server_full_handshake() {
    // Reseed the global rustls crypto provider with ring (otherwise
    // ServerConfig::builder() panics on default-features=false rustls).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let (cert, key) = generate_self_signed();

    // Build server-side TLS config (no client-cert auth).
    let server_cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], key)
        .expect("server tls config");
    let server_tls = Arc::new(TlsServerConfig {
        config: Arc::new(server_cfg),
        require_client_cert: false,
    });

    // Build client-side TLS config trusting the server cert.
    let mut roots = RootCertStore::empty();
    roots.add(cert).unwrap();
    let client_cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let client_tls = Arc::new(TlsClientConfig {
        config: Arc::new(client_cfg),
    });

    // Server source.
    let source = Arc::new(StaticSource::new());
    source.put("TLS:PV", 12.5).await;

    let (tcp, udp) = alloc_port_pair();
    let cfg = PvaServerConfig {
        tcp_port: tcp,
        udp_port: udp,
        idle_timeout: Duration::from_secs(60),
        max_connections: 16,
        max_channels_per_connection: 64,
        monitor_queue_depth: 8,
        tls: Some(server_tls),
        ..Default::default()
    };
    let server_handle = tokio::spawn(async move {
        let _ = run_pva_server(source, cfg).await;
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Client targeting the TLS server explicitly.
    let server_addr =
        std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), tcp);
    let client = PvaClient::builder()
        .timeout(Duration::from_secs(3))
        .server_addr(server_addr)
        .with_tls(client_tls)
        .build();

    let v = tokio::time::timeout(Duration::from_secs(5), client.pvget("TLS:PV"))
        .await
        .expect("pvget timed out")
        .expect("pvget failed");
    match v {
        PvField::Structure(s) => {
            assert_eq!(s.struct_id, "epics:nt/NTScalar:1.0");
            assert!(matches!(
                s.get_value(),
                Some(ScalarValue::Double(d)) if (d - 12.5).abs() < 1e-9
            ));
        }
        other => panic!("expected NTScalar structure, got {other:?}"),
    }

    server_handle.abort();
}
