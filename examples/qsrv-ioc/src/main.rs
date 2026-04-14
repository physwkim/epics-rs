//! qsrv-ioc — QSRV demo IOC with st.cmd-style startup.
//!
//! Loads records from a `.db` file and group PV definitions from JSON,
//! then serves everything over pvAccess via the QSRV bridge.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p qsrv-ioc --features ioc -- ioc/st.cmd
//! ```
//!
//! # Test
//!
//! ```text
//! pvget  DEMO:AI           # NTScalar — simulated temperature
//! pvget  DEMO:BI           # NTEnum  — beam status (Off/On)
//! pvget  DEMO:GROUP        # Group PV — composite structure
//! pvmonitor DEMO:GROUP     # Live group updates
//! pvput  DEMO:AO 42.5      # Write setpoint
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use epics_base_rs::error::CaResult;
use epics_base_rs::server::database::PvDatabase;
use epics_base_rs::types::EpicsValue;
use epics_bridge_rs::qsrv::{BridgeProvider, QsrvPvStore};
use epics_pva_rs::server::PvaServer;
use spvirit_server::PvStore;

#[tokio::main]
async fn main() -> CaResult<()> {
    let args: Vec<String> = std::env::args().collect();

    let port: u16 = std::env::var("EPICS_PVA_SERVER_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5075);

    // Set QSRV_IOC so st.cmd can reference $(QSRV_IOC)/db/*.
    epics_base_rs::runtime::env::set_default("QSRV_IOC", env!("CARGO_MANIFEST_DIR"));

    // Build server from .db file specified in st.cmd (or fallback to defaults)
    let (server, group_file) = if args.len() > 1 && !args[1].starts_with('-') {
        // st.cmd mode: parse the script for dbLoadRecords / qsrvGroupLoadConfig
        let script = std::fs::read_to_string(&args[1])
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", args[1]));
        parse_and_build(port, &script).await?
    } else {
        // No st.cmd: fall back to programmatic setup
        let macros: HashMap<String, String> = HashMap::new();
        let db_path = format!("{}/db/qsrv_demo.db", env!("CARGO_MANIFEST_DIR"));
        let server = PvaServer::builder()
            .port(port)
            .db_file(&db_path, &macros)?
            .build()
            .await?;
        let group_path = format!("{}/db/group.json", env!("CARGO_MANIFEST_DIR"));
        (server, Some(group_path))
    };

    let db: Arc<PvDatabase> = server.database().clone();

    // Load group PV config
    let mut provider = BridgeProvider::new(db.clone());
    if let Some(ref path) = group_file {
        provider
            .load_group_file(path)
            .unwrap_or_else(|e| panic!("failed to load group config {path}: {e}"));
    }
    let provider = Arc::new(provider);
    let store = Arc::new(QsrvPvStore::new(provider));

    let pv_list = store.list_pvs().await;
    eprintln!(
        "qsrv-ioc: serving {} PV(s) over pvAccess (port {port})",
        pv_list.len()
    );
    for pv in &pv_list {
        eprintln!("  {pv}");
    }

    // Simulator: sine wave on AI, toggle BI
    spawn_simulator(db);

    server.run_with_store(store).await
}

/// Parse a simplified st.cmd for dbLoadRecords and qsrvGroupLoadConfig.
async fn parse_and_build(port: u16, script: &str) -> CaResult<(PvaServer, Option<String>)> {
    let mut macros: HashMap<String, String> = HashMap::new();
    let mut builder = PvaServer::builder().port(port);
    let mut group_file: Option<String> = None;

    for raw_line in script.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // epicsEnvSet("KEY", "VALUE")
        if line.starts_with("epicsEnvSet") {
            if let Some((key, val)) = parse_env_set(line) {
                // Expand $(VAR) in value
                let expanded = expand_macros(&val, &macros);
                macros.insert(key, expanded);
            }
            continue;
        }

        // dbLoadRecords("path", "macro=val,...")
        if line.starts_with("dbLoadRecords") {
            if let Some((path, mac_str)) = parse_db_load(line) {
                let path = expand_macros(&path, &macros);
                let mut db_macros = macros.clone();
                for pair in mac_str.split(',') {
                    if let Some((k, v)) = pair.split_once('=') {
                        db_macros.insert(k.trim().to_string(), expand_macros(v.trim(), &macros));
                    }
                }
                builder = builder.db_file(&path, &db_macros)?;
            }
            continue;
        }

        // qsrvGroupLoadConfig("path")
        if line.starts_with("qsrvGroupLoadConfig") {
            if let Some(path) = parse_single_arg(line) {
                group_file = Some(expand_macros(&path, &macros));
            }
            continue;
        }

        // iocInit() — ignored (we init at build time)
    }

    let server = builder.build().await?;
    Ok((server, group_file))
}

fn spawn_simulator(db: Arc<PvDatabase>) {
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
            let bi_val = if (tick as u64) % 10 < 5 { 0 } else { 1 };
            let _ = db
                .put_record_field_from_ca("DEMO:BI", "VAL", EpicsValue::Enum(bi_val))
                .await;
        }
    });
}

// ── Minimal st.cmd parser helpers ──────────────────────────────────────

/// Parse `epicsEnvSet("KEY", "VALUE")` → (KEY, VALUE)
fn parse_env_set(line: &str) -> Option<(String, String)> {
    let inner = line.strip_prefix("epicsEnvSet(")?.strip_suffix(')')?;
    let (k, v) = inner.split_once(',')?;
    Some((unquote(k.trim()), unquote(v.trim())))
}

/// Parse `dbLoadRecords("path", "macros")` → (path, macros)
fn parse_db_load(line: &str) -> Option<(String, String)> {
    let inner = line.strip_prefix("dbLoadRecords(")?.strip_suffix(')')?;
    let (path, mac) = inner.split_once(',')?;
    Some((unquote(path.trim()), unquote(mac.trim())))
}

/// Parse `someCommand("arg")` → arg
fn parse_single_arg(line: &str) -> Option<String> {
    let start = line.find('(')?;
    let inner = line[start + 1..].strip_suffix(')')?;
    Some(unquote(inner.trim()))
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Expand `$(VAR)` references in a string.
fn expand_macros(s: &str, macros: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    // Iterate until no more substitutions happen
    for _ in 0..10 {
        let prev = result.clone();
        for (key, val) in macros {
            result = result.replace(&format!("$({key})"), val);
        }
        if result == prev {
            break;
        }
    }
    result
}
