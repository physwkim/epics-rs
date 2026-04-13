use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Local};
use clap::Parser;
use epics_ca_rs::client::{CaClient, ConnectionEvent};

#[derive(Parser)]
#[command(name = "camonitor", about = "Monitor EPICS PVs for changes")]
struct Args {
    /// PV names to monitor
    #[arg(required = true)]
    pv_names: Vec<String>,

    /// Wait time for initial connections (seconds)
    #[arg(short = 'w', long = "wait", default_value_t = 1.0)]
    timeout: f64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = CaClient::new().await.expect("failed to create CA client");

    let connected_flags: Vec<Arc<AtomicBool>> = args
        .pv_names
        .iter()
        .map(|_| Arc::new(AtomicBool::new(false)))
        .collect();

    let mut handles = Vec::new();

    for (i, pv_name) in args.pv_names.iter().enumerate() {
        let channel = client.create_channel(pv_name);
        let pv = pv_name.clone();
        let flag = connected_flags[i].clone();
        let handle = tokio::spawn(async move {
            monitor_pv(channel, &pv, flag).await;
        });
        handles.push(handle);
    }

    // Initial connection wait (C: ca_pend_event(caTimeout))
    tokio::time::sleep(Duration::from_secs_f64(args.timeout)).await;

    // Print NOT CONNECTED for PVs that didn't connect
    for (i, pv_name) in args.pv_names.iter().enumerate() {
        if !connected_flags[i].load(Ordering::Acquire) {
            println!("{pv_name} *** Not connected (PV not found)");
        }
    }

    // Run forever (C: ca_pend_event(0))
    for handle in handles {
        let _ = handle.await;
    }
}

fn format_server_timestamp(ts: std::time::SystemTime) -> String {
    let dt: DateTime<Local> = ts.into();
    dt.format("%Y-%m-%d %H:%M:%S%.6f").to_string()
}

async fn monitor_pv(
    channel: epics_ca_rs::client::CaChannel,
    pv_name: &str,
    connected_flag: Arc<AtomicBool>,
) {
    // Disconnect monitoring (separate task)
    let mut conn_rx = channel.connection_events();
    let pv = pv_name.to_string();
    let flag = connected_flag.clone();
    tokio::spawn(async move {
        while let Ok(evt) = conn_rx.recv().await {
            match evt {
                ConnectionEvent::Connected => {
                    flag.store(true, Ordering::Release);
                }
                ConnectionEvent::Disconnected => {
                    let now = Local::now().format("%Y-%m-%d %H:%M:%S%.6f");
                    println!("{pv} {now} *** disconnected");
                }
                _ => {}
            }
        }
    });

    let Ok(mut monitor) = channel.subscribe().await else {
        return;
    };

    while let Some(result) = monitor.recv().await {
        match result {
            Ok(snap) => {
                let ts = format_server_timestamp(snap.timestamp);
                println!("{pv_name} {ts} {}", snap.value);
            }
            Err(e) => {
                eprintln!("{pv_name}: {e}");
            }
        }
    }
}
