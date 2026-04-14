//! Random Signals IOC — four PVs updated every 10ms with random values.
//!
//! Serves over both Channel Access and pvAccess simultaneously.
//!
//! PV names:
//!   BLM:001:SA:A, BLM:001:SA:B, BLM:001:SA:C, BLM:001:SA:D
//!
//! Usage:
//!   cargo run --release -p random-signals
//!
//! Test:
//!   caget     BLM:001:SA:A          # Channel Access
//!   camonitor BLM:001:SA:A          # CA live updates
//!   pvget     BLM:001:SA:A          # pvAccess
//!   pvmonitor BLM:001:SA:A          # PVA live updates

use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::ioc_builder::IocBuilder;
use epics_base_rs::types::EpicsValue;
use epics_ca_rs::server::CaServer;
use epics_pva_rs::server::PvaServer;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

const PV_NAMES: [&str; 4] = [
    "BLM:001:SA:A",
    "BLM:001:SA:B",
    "BLM:001:SA:C",
    "BLM:001:SA:D",
];

#[tokio::main]
async fn main() -> CaResult<()> {
    let ca_port: u16 = std::env::var("EPICS_CA_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5064);

    let pva_port: u16 = std::env::var("EPICS_PVA_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5075);

    // Build shared database
    let mut ioc = IocBuilder::new();
    for name in &PV_NAMES {
        ioc = ioc.pv(name, EpicsValue::Double(0.0));
    }
    let (db, _) = ioc.build().await?;

    eprintln!(
        "Random signals IOC — {} @ 100 Hz (CA:{ca_port} PVA:{pva_port})",
        PV_NAMES.join(", ")
    );

    spawn_updater(db.clone());

    let ca_server = CaServer::from_parts(db.clone(), ca_port, None, None, None);
    let pva_server = PvaServer::from_parts(db, pva_port, None, None, None);

    let ca_handle = tokio::spawn(async move { ca_server.run().await });
    let pva_handle = tokio::spawn(async move { pva_server.run().await });

    tokio::select! {
        res = ca_handle => { eprintln!("CA server exited: {res:?}"); }
        res = pva_handle => { eprintln!("PVA server exited: {res:?}"); }
    }

    Ok(())
}

fn spawn_updater(db: Arc<PvDatabase>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        let mut rng = SmallRng::from_os_rng();
        loop {
            interval.tick().await;
            for name in &PV_NAMES {
                let val = rng.random_range(-10.0..10.0);
                let _ = db.put_pv(name, EpicsValue::Double(val)).await;
            }
        }
    });
}
