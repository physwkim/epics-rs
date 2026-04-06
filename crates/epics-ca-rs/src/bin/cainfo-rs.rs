use clap::Parser;
use epics_ca_rs::client::CaClient;

#[derive(Parser)]
#[command(name = "rcainfo", about = "Show EPICS PV channel information and client diagnostics")]
struct Args {
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

    let mut failed = false;
    for pv_name in &args.pv_names {
        match client.cainfo(pv_name).await {
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
