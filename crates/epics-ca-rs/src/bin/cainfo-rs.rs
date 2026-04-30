use clap::Parser;
use epics_ca_rs::client::CaClient;
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "cainfo",
    about = "Show EPICS PV channel information and client diagnostics"
)]
struct Args {
    /// CA timeout in seconds (default: $EPICS_CLI_TIMEOUT or 1.0).
    /// C ref: modules/ca/src/tools/tool_lib.c:use_ca_timeout_env (commit 1d056c6).
    #[arg(short = 'w', long = "wait")]
    timeout: Option<f64>,

    /// PV names to query (omit for diagnostics only)
    pv_names: Vec<String>,

    /// Show client diagnostic counters and event history
    #[arg(short, long)]
    diag: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = CaClient::new().await.expect("failed to create CA client");
    let timeout =
        Duration::from_secs_f64(args.timeout.unwrap_or_else(epics_ca_rs::cli::env_default_timeout));

    let mut failed = false;
    for pv_name in &args.pv_names {
        let ch = client.create_channel(pv_name);
        match ch.wait_connected(timeout).await {
            Ok(()) => match ch.info().await {
                Ok(info) => {
                    println!("{}:", info.pv_name);
                    println!("    Server:         {}", info.server_addr);
                    println!("    Type:           {:?}", info.native_type);
                    println!("    Element count:  {}", info.element_count);
                    println!("    Access:         {}", info.access_rights);
                }
                Err(e) => {
                    eprintln!("{pv_name}: {e}");
                    failed = true;
                }
            },
            Err(_) => {
                eprintln!("{pv_name}: Channel connect timed out: '{pv_name}' not found.");
                failed = true;
            }
        }
    }

    if args.diag || args.pv_names.is_empty() {
        if !args.pv_names.is_empty() {
            println!();
        }
        println!("--- Client Diagnostics ---");
        println!("{}", client.diagnostics());
    }

    if failed {
        std::process::exit(1);
    }
}
