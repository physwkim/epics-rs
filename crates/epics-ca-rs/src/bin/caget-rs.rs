use clap::Parser;
use epics_ca_rs::CaError;
use epics_ca_rs::client::CaClient;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "caget", about = "Read EPICS PV values")]
struct Args {
    /// CA timeout in seconds (default: $EPICS_CLI_TIMEOUT or 1.0).
    /// C ref: modules/ca/src/tools/tool_lib.c:use_ca_timeout_env (commit 1d056c6).
    #[arg(short = 'w', long = "wait")]
    timeout: Option<f64>,

    /// PV names to read
    #[arg(required = true)]
    pv_names: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = CaClient::new().await.expect("failed to create CA client");
    let timeout =
        Duration::from_secs_f64(args.timeout.unwrap_or_else(epics_ca_rs::cli::env_default_timeout));

    // Create all channels first (C: ca_create_channel for all PVs)
    let channels: Vec<_> = args
        .pv_names
        .iter()
        .map(|name| (name.clone(), client.create_channel(name)))
        .collect();

    // Connect + read all PVs in parallel within single timeout window
    // (C: connect_pvs → ca_pend_io → ca_array_get → ca_pend_io)
    let mut handles = Vec::new();
    for (name, ch) in &channels {
        let name = name.clone();
        let t = timeout;
        let ch = ch.clone();
        handles.push(tokio::spawn(async move {
            let connect = ch.wait_connected(t).await;
            if connect.is_err() {
                return (name, Err("not connected".to_string()));
            }
            match ch.get_with_timeout(t).await {
                Ok((_dbr, value)) => (name, Ok(value.to_string())),
                Err(CaError::Timeout) => (name, Err("timeout".to_string())),
                Err(e) => (name, Err(format!("{e}"))),
            }
        }));
    }

    // Collect results preserving PV order
    let mut results = Vec::with_capacity(handles.len());
    for h in handles {
        results.push(h.await.unwrap());
    }

    // Print results
    let mut failed = false;
    for (pv_name, result) in &results {
        match result {
            Ok(value) => {
                println!("{pv_name} {value}");
            }
            Err(e) if e.contains("not connected") || e.contains("isconnect") => {
                println!("{pv_name} *** Not connected (PV not found)");
                failed = true;
            }
            Err(e) if e.contains("timeout") => {
                println!("{pv_name} *** no data available (timeout)");
                failed = true;
            }
            Err(e) => {
                println!("{pv_name} *** no data available ({e})");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
