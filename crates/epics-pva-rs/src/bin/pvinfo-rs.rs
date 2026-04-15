use clap::Parser;
use epics_pva_rs::client::PvaClient;
use epics_pva_rs::format;

#[derive(Parser)]
#[command(name = "pvinfo-rs", version, about = "Show EPICS PV type info via pvAccess")]
struct Args {
    /// PV names to query
    #[arg(required = true)]
    pv_names: Vec<String>,

    /// Wait time in seconds
    #[arg(short = 'w', default_value = "5.0")]
    timeout: f64,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = PvaClient::new().expect("failed to create PVA client");

    let mut failed = false;
    for (i, pv_name) in args.pv_names.iter().enumerate() {
        if i > 0 {
            println!();
        }
        match client.pvinfo_full(pv_name).await {
            Ok((desc, server_addr)) => {
                println!("{pv_name}");
                if server_addr.port() != 0 {
                    println!("Server: {server_addr}");
                }
                println!("Type:");
                print!("{}", format::format_info_indented(&desc, 1));
            }
            Err(e) => {
                eprintln!("{pv_name}: {e}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
