//! End-to-end CA benchmarks.
//!
//! Spins up an in-process softioc with a handful of PVs, connects a
//! client, and times the operations that show up in real
//! workloads:
//!
//! - **search + connect**: cost of resolving a fresh PV name
//! - **caget**: cost of a one-shot read on an established channel
//! - **caput**: cost of a fire-and-forget write
//!
//! Results land in `target/criterion/`; pull the HTML report out of
//! the `report/` subdir for graphs. Numbers are the *per-operation*
//! cost averaged across many iterations, so stalls show up as
//! standard-deviation widening rather than visible spikes.
//!
//! Tracking baselines: see `BENCHMARKS.md` for the numbers that
//! were current when this file landed. Use them to spot regressions
//! when refactoring hot paths.

use std::sync::Arc;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use tokio::runtime::Runtime;

use epics_base_rs::types::EpicsValue;
use epics_ca_rs::client::CaClient;
use epics_ca_rs::server::CaServer;

fn make_runtime() -> Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime")
}

/// Spin up a softioc populated with N PVs of the given type. Returns
/// `(server_handle, port)`. Server runs forever; the bench drops the
/// handle when done so the runtime cleans up.
async fn boot_softioc(n_pvs: usize) -> (tokio::task::JoinHandle<()>, u16) {
    let mut builder = CaServer::builder().port(0);
    for i in 0..n_pvs {
        builder = builder.pv(&format!("BENCH:PV:{i}"), EpicsValue::Double(i as f64));
    }
    let server = builder.build().await.expect("server build");
    // Capture the actual bound port via env scan: CaServer chooses
    // the port lazily inside run(). We use a static port here for
    // reproducibility in benchmarks. (Production code would pick a
    // dynamic port and read it back; bench uses a fixed offset to
    // avoid collisions when run repeatedly.)
    let port = 5099u16;
    // Override port to the fixed value before starting.
    let server = CaServer::from_parts(server.database().clone(), port, None, None, None);
    let handle = tokio::spawn(async move {
        let _ = server.run().await;
    });
    // Give the listener time to bind.
    tokio::time::sleep(Duration::from_millis(200)).await;
    (handle, port)
}

fn point_addr_list_at(port: u16) {
    // SAFETY: bench runs serially; we set env vars before any client
    // is constructed.
    unsafe {
        std::env::set_var("EPICS_CA_ADDR_LIST", format!("127.0.0.1:{port}"));
        std::env::set_var("EPICS_CA_AUTO_ADDR_LIST", "NO");
        std::env::set_var("EPICS_CA_SERVER_PORT", port.to_string());
    }
}

fn bench_caget(c: &mut Criterion) {
    let rt = make_runtime();
    let (_server, port) = rt.block_on(async { boot_softioc(8).await });
    point_addr_list_at(port);
    let client = Arc::new(rt.block_on(async { CaClient::new().await.expect("client") }));

    // Warm up — establish channels before timing.
    rt.block_on(async {
        for i in 0..8 {
            let _ = client.caget(&format!("BENCH:PV:{i}")).await;
        }
    });

    c.bench_function("e2e_caget_warm_8pvs", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                for i in 0..8 {
                    let _ = client.caget(&format!("BENCH:PV:{i}")).await;
                }
            }
        });
    });
}

fn bench_caput(c: &mut Criterion) {
    let rt = make_runtime();
    let (_server, port) = rt.block_on(async { boot_softioc(1).await });
    point_addr_list_at(port);
    let client = Arc::new(rt.block_on(async { CaClient::new().await.expect("client") }));
    rt.block_on(async {
        let _ = client.caput("BENCH:PV:0", "1.0").await;
    });

    c.bench_function("e2e_caput_warm", |b| {
        let client = client.clone();
        b.to_async(&rt).iter(|| {
            let client = client.clone();
            async move {
                let _ = client.caput("BENCH:PV:0", "1.0").await;
            }
        });
    });
}

criterion_group! {
    name = e2e;
    // Lower sample size — each iteration is an actual TCP round-trip,
    // not a microsecond op.
    config = Criterion::default()
        .sample_size(20)
        .measurement_time(Duration::from_secs(8))
        .warm_up_time(Duration::from_secs(2));
    targets = bench_caget, bench_caput
}
criterion_main!(e2e);
