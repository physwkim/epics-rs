//! qsrv-ioc — demo of the QSRV bridge under the spvirit PVA server.
//!
//! Exposes individual records AND a group PV over pvAccess.
//!
//! # PV list
//!
//! | PV name         | Type       | Description                |
//! |-----------------|------------|----------------------------|
//! | `DEMO:AI`       | NTScalar   | Simulated sine wave        |
//! | `DEMO:AO`       | NTScalar   | Writable setpoint          |
//! | `DEMO:BI`       | NTEnum     | Binary input (On/Off)      |
//! | `DEMO:GROUP`    | Group PV   | Composite of AI + AO + BI  |
//!
//! # Usage
//!
//! ```text
//! # Start the IOC
//! cargo run -p qsrv-ioc
//!
//! # — In another terminal —
//!
//! # Single records (pvxs tools or spvirit tools)
//! pvget  DEMO:AI
//! pvput  DEMO:AO 42.5
//! pvget  DEMO:BI
//!
//! # Group PV — returns all members in one structure
//! pvget  DEMO:GROUP
//!
//! # Monitor the group (updates when any member changes)
//! pvmonitor DEMO:GROUP
//! ```

use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::server::records::{ai::AiRecord, ao::AoRecord, bi::BiRecord};
use epics_bridge_rs::qsrv::{BridgeProvider, QsrvPvStore};
use epics_pva_rs::server::PvaServer;

/// JSON group PV definition — pvxs/QSRV compatible format.
///
/// This defines `DEMO:GROUP` as a composite of three members:
/// - `temperature`: full NTScalar from DEMO:AI (scalar mapping)
/// - `setpoint`:    value-only from DEMO:AO (plain mapping)
/// - `status`:      full NTEnum from DEMO:BI (scalar mapping)
/// - `version`:     constant integer (const mapping)
const GROUP_CONFIG: &str = r#"{
    "DEMO:GROUP": {
        "+id": "demo:group/v1",
        "+atomic": true,
        "temperature": {
            "+channel": "DEMO:AI",
            "+type": "scalar",
            "+trigger": "*"
        },
        "setpoint": {
            "+channel": "DEMO:AO",
            "+type": "plain",
            "+trigger": "setpoint"
        },
        "status": {
            "+channel": "DEMO:BI",
            "+type": "scalar",
            "+trigger": "status"
        },
        "version": {
            "+type": "const",
            "+value": 1
        }
    }
}"#;

#[tokio::main]
async fn main() -> CaResult<()> {
    let port: u16 = std::env::var("EPICS_PVA_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5075);

    let server = PvaServer::builder()
        .port(port)
        .record("DEMO:AI", AiRecord::new(0.0))
        .record("DEMO:AO", AoRecord::default())
        .record("DEMO:BI", {
            let mut bi = BiRecord::new(0);
            bi.znam = "Off".into();
            bi.onam = "On".into();
            bi
        })
        .build()
        .await?;

    let db: Arc<PvDatabase> = server.database().clone();

    // Load group PV definitions
    let mut provider = BridgeProvider::new(db.clone());
    provider
        .load_group_config(GROUP_CONFIG)
        .expect("failed to parse group config");

    let provider = Arc::new(provider);
    let store = Arc::new(QsrvPvStore::new(provider));

    spawn_simulator(db);

    eprintln!("qsrv-ioc: PVA server on port {port}");
    eprintln!("  Records: DEMO:AI, DEMO:AO, DEMO:BI");
    eprintln!("  Group:   DEMO:GROUP (temperature + setpoint + status + version)");
    eprintln!();
    eprintln!("Test with:");
    eprintln!("  pvget DEMO:AI          # single NTScalar");
    eprintln!("  pvget DEMO:GROUP       # group composite");
    eprintln!("  pvmonitor DEMO:GROUP   # live updates");
    eprintln!("  pvput DEMO:AO 42.5     # write setpoint");

    server.run_with_store(store).await
}

fn spawn_simulator(db: Arc<PvDatabase>) {
    use epics_base_rs::types::EpicsValue;

    tokio::spawn(async move {
        let mut tick = 0.0_f64;
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            tick += 0.1;
            let temp = 22.0 + (tick * 0.5).sin();
            let _ = db
                .put_record_field_from_ca("DEMO:AI", "VAL", EpicsValue::Double(temp))
                .await;

            // Toggle BI every 5 seconds
            let bi_val = if (tick as u64) % 10 < 5 { 0 } else { 1 };
            let _ = db
                .put_record_field_from_ca("DEMO:BI", "VAL", EpicsValue::Enum(bi_val))
                .await;
        }
    });
}
