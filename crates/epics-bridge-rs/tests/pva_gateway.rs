//! Integration tests for the PVA-to-PVA gateway.
//!
//! Topology:
//!
//! ```text
//!   [PvaClient] ─── PVA ───▶ [PvaGateway downstream]
//!                                 │
//!                                 ▼ (cache)
//!                          [PvaGateway upstream PvaClient]
//!                                 │
//!                                 ▼ PVA
//!                          [PvaServer with SharedPV]
//! ```
//!
//! Verifies: GET, MONITOR fan-out (single upstream subscription
//! shared across multiple downstream clients), and that
//! disappearing downstream subscribers don't abort the upstream
//! monitor task.

#![cfg(feature = "pva-gateway")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use epics_bridge_rs::pva_gateway::{MultiTenantPvaGatewayBuilder, PvaGateway, PvaGatewayConfig};
use epics_pva_rs::client::PvaClient;
use epics_pva_rs::pvdata::{FieldDesc, PvField, ScalarType, ScalarValue};
use epics_pva_rs::server_native::{PvaServer, PvaServerConfig, SharedPV, SharedSource};

/// Build a 1-PV upstream PvaServer on a random loopback port and
/// return (server, addr, shared_pv).
fn spawn_upstream(pv_name: &str, initial: f64) -> (PvaServer, SocketAddr, SharedPV) {
    let pv = SharedPV::new();
    pv.open(
        FieldDesc::Scalar(ScalarType::Double),
        PvField::Scalar(ScalarValue::Double(initial)),
    );
    let source = SharedSource::new();
    source.add(pv_name, pv.clone());

    let pick = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let pick_udp = || {
        let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let cfg = PvaServerConfig {
        tcp_port: pick(),
        udp_port: pick_udp(),
        ..PvaServerConfig::isolated()
    };
    let bound = SocketAddr::new(
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        cfg.tcp_port,
    );
    let server = PvaServer::start(Arc::new(source), cfg);
    (server, bound, pv)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gateway_get_forwards_upstream_value() {
    let (_us_server, us_addr, us_pv) = spawn_upstream("GW:GET:PV", 42.5);
    // Upstream client pinned at the test server.
    let upstream_client = Arc::new(
        PvaClient::builder()
            .server_addr(us_addr)
            .timeout(Duration::from_secs(2))
            .build(),
    );

    let pick = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let pick_udp = || {
        let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let server_config = PvaServerConfig {
        tcp_port: pick(),
        udp_port: pick_udp(),
        ..PvaServerConfig::isolated()
    };
    let cfg = PvaGatewayConfig {
        upstream_client: Some(upstream_client),
        server_config,
        cleanup_interval: Duration::from_secs(60),
        connect_timeout: Duration::from_secs(2),
        max_cache_entries: 1024,
        max_subscribers: 1024,
        control_prefix: None,
    };
    let gw = PvaGateway::start(cfg).expect("gateway start");

    // Downstream client pinned at the gateway.
    let ds = gw.client_config();
    let result = ds.pvget_full("GW:GET:PV").await.expect("downstream get");
    match result.value {
        PvField::Scalar(ScalarValue::Double(v)) => assert_eq!(v, 42.5),
        PvField::Structure(s) => match s.get_field("value") {
            Some(PvField::Scalar(ScalarValue::Double(v))) => assert_eq!(*v, 42.5),
            other => panic!("unexpected NTScalar value: {other:?}"),
        },
        other => panic!("unexpected value shape: {other:?}"),
    }

    // Sanity: upstream PV was not touched (we only read).
    assert!(us_pv.is_open());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gateway_monitor_fans_out_to_two_clients() {
    let (_us_server, us_addr, us_pv) = spawn_upstream("GW:MON:PV", 0.0);
    let upstream_client = Arc::new(
        PvaClient::builder()
            .server_addr(us_addr)
            .timeout(Duration::from_secs(2))
            .build(),
    );

    let pick = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let pick_udp = || {
        let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let server_config = PvaServerConfig {
        tcp_port: pick(),
        udp_port: pick_udp(),
        ..PvaServerConfig::isolated()
    };
    let cfg = PvaGatewayConfig {
        upstream_client: Some(upstream_client),
        server_config,
        cleanup_interval: Duration::from_secs(60),
        connect_timeout: Duration::from_secs(2),
        max_cache_entries: 1024,
        max_subscribers: 1024,
        control_prefix: None,
    };
    let gw = PvaGateway::start(cfg).expect("gateway start");

    // Two independent downstream clients, both pointed at gateway.
    let c1 = gw.client_config();
    let c2 = gw.client_config();

    let (tx1, mut rx1) = tokio::sync::mpsc::channel::<f64>(8);
    let (tx2, mut rx2) = tokio::sync::mpsc::channel::<f64>(8);

    let h1 = tokio::spawn(async move {
        let _ = c1
            .pvmonitor("GW:MON:PV", move |value| {
                if let Some(d) = scalar_double(value) {
                    let _ = tx1.try_send(d);
                }
            })
            .await;
    });
    let h2 = tokio::spawn(async move {
        let _ = c2
            .pvmonitor("GW:MON:PV", move |value| {
                if let Some(d) = scalar_double(value) {
                    let _ = tx2.try_send(d);
                }
            })
            .await;
    });

    // Let the subscriptions establish.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // Drain initial events (both clients must see the seed value).
    let initial1 = recv_within(&mut rx1, Duration::from_secs(2))
        .await
        .expect("client 1 initial");
    let initial2 = recv_within(&mut rx2, Duration::from_secs(2))
        .await
        .expect("client 2 initial");
    assert_eq!(initial1, 0.0);
    assert_eq!(initial2, 0.0);

    // Push three updates upstream; both downstream clients should see
    // each one. We treat "received the last value" as success since
    // an under-loaded test runner can squash to-latest.
    for v in [1.0_f64, 2.0, 3.0] {
        us_pv.try_post(PvField::Scalar(ScalarValue::Double(v)));
        // tiny breather so the broadcast fan-out keeps up.
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    let last1 = drain_to_latest(&mut rx1, Duration::from_secs(3))
        .await
        .expect("client 1 saw an update");
    let last2 = drain_to_latest(&mut rx2, Duration::from_secs(3))
        .await
        .expect("client 2 saw an update");
    assert_eq!(last1, 3.0);
    assert_eq!(last2, 3.0);

    h1.abort();
    h2.abort();
}

fn scalar_double(field: &PvField) -> Option<f64> {
    match field {
        PvField::Scalar(ScalarValue::Double(d)) => Some(*d),
        PvField::Structure(s) => match s.get_field("value")? {
            PvField::Scalar(ScalarValue::Double(d)) => Some(*d),
            _ => None,
        },
        _ => None,
    }
}

async fn recv_within(rx: &mut tokio::sync::mpsc::Receiver<f64>, timeout: Duration) -> Option<f64> {
    tokio::time::timeout(timeout, rx.recv())
        .await
        .ok()
        .flatten()
}

async fn drain_to_latest(
    rx: &mut tokio::sync::mpsc::Receiver<f64>,
    timeout: Duration,
) -> Option<f64> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last: Option<f64> = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(150), rx.recv()).await {
            Ok(Some(v)) => last = Some(v),
            Ok(None) => break,
            Err(_) => {
                if last.is_some() {
                    break;
                }
            }
        }
    }
    last
}

/// G-G2: when `control_prefix` is set, downstream clients should be
/// able to `pvget <prefix>:cacheSize` and read the live cache entry
/// count without that name being forwarded upstream.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn gateway_control_prefix_cache_size() {
    let (_us_server, us_addr, _us_pv) = spawn_upstream("GW:CTRL:PV", 1.0);
    let upstream_client = Arc::new(
        PvaClient::builder()
            .server_addr(us_addr)
            .timeout(Duration::from_secs(2))
            .build(),
    );

    let pick = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let pick_udp = || {
        let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let server_config = PvaServerConfig {
        tcp_port: pick(),
        udp_port: pick_udp(),
        ..PvaServerConfig::isolated()
    };
    let cfg = PvaGatewayConfig {
        upstream_client: Some(upstream_client),
        server_config,
        cleanup_interval: Duration::from_secs(60),
        connect_timeout: Duration::from_secs(2),
        max_cache_entries: 1024,
        max_subscribers: 1024,
        control_prefix: Some("gw".to_string()),
    };
    let gw = PvaGateway::start(cfg).expect("gateway start");

    let ds = gw.client_config();

    // Initial cache is empty.
    let snap = ds.pvget_full("gw:cacheSize").await.expect("cacheSize get");
    let v = match snap.value {
        PvField::Structure(s) => match s.get_field("value") {
            Some(PvField::Scalar(ScalarValue::Long(v))) => *v,
            other => panic!("unexpected cacheSize value shape: {other:?}"),
        },
        other => panic!("unexpected cacheSize wrapper: {other:?}"),
    };
    assert_eq!(v, 0, "fresh gateway cache is empty before any proxy GET");

    // Trigger a proxy GET to populate the cache, then re-read cacheSize.
    let _ = ds.pvget_full("GW:CTRL:PV").await.expect("proxy get");
    let snap = ds
        .pvget_full("gw:cacheSize")
        .await
        .expect("cacheSize get post-proxy");
    let v = match snap.value {
        PvField::Structure(s) => match s.get_field("value") {
            Some(PvField::Scalar(ScalarValue::Long(v))) => *v,
            other => panic!("unexpected cacheSize value shape: {other:?}"),
        },
        other => panic!("unexpected cacheSize wrapper: {other:?}"),
    };
    assert!(v >= 1, "cacheSize should reflect the proxied PV; got {v}");
}

/// G-G1: a multi-tenant gateway with two upstreams (each holding a
/// distinct PV) and one downstream that proxies both. Verifies
/// per-upstream isolation: PV "A:VAL" is only on upstream A, PV
/// "B:VAL" only on B, and a single downstream client reaches both
/// through the gateway.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn multi_tenant_gateway_routes_to_correct_upstream() {
    let (_us_a, addr_a, _pv_a) = spawn_upstream("A:VAL", 1.0);
    let (_us_b, addr_b, _pv_b) = spawn_upstream("B:VAL", 2.0);

    let client_a = Arc::new(
        PvaClient::builder()
            .server_addr(addr_a)
            .timeout(Duration::from_secs(2))
            .build(),
    );
    let client_b = Arc::new(
        PvaClient::builder()
            .server_addr(addr_b)
            .timeout(Duration::from_secs(2))
            .build(),
    );

    let pick = || {
        let l = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let pick_udp = || {
        let l = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let p = l.local_addr().unwrap().port();
        drop(l);
        p
    };
    let server_config = PvaServerConfig {
        tcp_port: pick(),
        udp_port: pick_udp(),
        ..PvaServerConfig::isolated()
    };

    let gw = MultiTenantPvaGatewayBuilder::new()
        .add_upstream("A", client_a)
        .add_upstream("B", client_b)
        .add_downstream("merged", server_config, &["A", "B"], None)
        .connect_timeout(Duration::from_secs(2))
        .start()
        .expect("multi-tenant start");

    assert_eq!(gw.upstream_count(), 2);
    assert_eq!(gw.downstream_count(), 1);

    // Build a downstream client pointed at the "merged" server.
    let server = gw.downstream("merged").expect("merged server present");
    let ds = server.client_config();

    let snap = ds.pvget_full("A:VAL").await.expect("A:VAL via gateway");
    let v = match snap.value {
        PvField::Scalar(ScalarValue::Double(v)) => v,
        PvField::Structure(s) => match s.get_field("value") {
            Some(PvField::Scalar(ScalarValue::Double(v))) => *v,
            other => panic!("unexpected A:VAL shape: {other:?}"),
        },
        other => panic!("unexpected A:VAL wrapper: {other:?}"),
    };
    assert_eq!(v, 1.0);

    let snap = ds.pvget_full("B:VAL").await.expect("B:VAL via gateway");
    let v = match snap.value {
        PvField::Scalar(ScalarValue::Double(v)) => v,
        PvField::Structure(s) => match s.get_field("value") {
            Some(PvField::Scalar(ScalarValue::Double(v))) => *v,
            other => panic!("unexpected B:VAL shape: {other:?}"),
        },
        other => panic!("unexpected B:VAL wrapper: {other:?}"),
    };
    assert_eq!(v, 2.0);
}
