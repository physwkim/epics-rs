//! qsrv-ioc — demo of the QSRV bridge under the spvirit PVA server.
//!
//! Spins up an `ai` and an `ao` record, wraps the database in a qsrv
//! `BridgeProvider`, and exposes both records over pvAccess via
//! `QsrvPvStore`. Run a `spget` / `spput` / `spmonitor` against the
//! resulting server to exercise the wire path:
//!
//! ```text
//! cargo run -p qsrv-ioc &
//! cargo run -p spvirit-tools --bin spget    -- DEMO:AI
//! cargo run -p spvirit-tools --bin spput    -- DEMO:AO 42.5
//! cargo run -p spvirit-tools --bin spmonitor -- DEMO:AI
//! ```

use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::records::{ai::AiRecord, ao::AoRecord};
use epics_bridge_rs::qsrv::{BridgeProvider, QsrvPvStore};
use epics_pva_rs::server::PvaServer;

#[tokio::main]
async fn main() -> CaResult<()> {
    let server = PvaServer::builder()
        .record("DEMO:AI", AiRecord::new(0.0))
        .record("DEMO:AO", AoRecord::default())
        .build()
        .await?;
    let db: Arc<PvDatabase> = server.database().clone();

    spawn_simulator(db.clone());

    let provider = Arc::new(BridgeProvider::new(db));
    let store = Arc::new(QsrvPvStore::new(provider));

    eprintln!("qsrv-ioc: serving DEMO:AI, DEMO:AO over pvAccess (port 5075)");
    server.run_with_store(store).await
}

fn spawn_simulator(db: Arc<PvDatabase>) {
    tokio::spawn(async move {
        let mut tick = 0.0_f64;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            tick += 0.1;
            let value = 22.0 + (tick * 0.5).sin();
            let _ = db
                .put_record_field_from_ca(
                    "DEMO:AI",
                    "VAL",
                    epics_base_rs::types::EpicsValue::Double(value),
                )
                .await;
        }
    });
}
