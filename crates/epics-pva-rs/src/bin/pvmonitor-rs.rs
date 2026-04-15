use clap::Parser;
use epics_pva_rs::client::PvaClient;
use epics_pva_rs::format;

#[derive(Parser)]
#[command(
    name = "pvmonitor-rs",
    version,
    about = "Monitor EPICS PVs via pvAccess"
)]
struct Args {
    /// PV names to monitor
    #[arg(required = true)]
    pv_names: Vec<String>,

    /// Output mode: raw, nt, json
    #[arg(short = 'M', default_value = "nt")]
    mode: String,

    /// Show entire structure (implies raw mode)
    #[arg(short = 'v', action = clap::ArgAction::Count)]
    verbose: u8,

    /// Wait time in seconds
    #[arg(short = 'w', default_value = "5.0")]
    timeout: f64,

    /// Quiet mode, print only error messages
    #[arg(short = 'q')]
    quiet: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut handles = Vec::new();

    for pv_name in args.pv_names {
        let mode = if args.verbose > 0 {
            "raw".to_string()
        } else {
            args.mode.clone()
        };
        let quiet = args.quiet;
        let handle = tokio::spawn(async move {
            let client = PvaClient::new().expect("failed to create PVA client");

            // Get introspection once for typed formatting
            let desc = client.pvinfo(&pv_name).await.ok();

            let result = client
                .pvmonitor(&pv_name, |value| {
                    if quiet {
                        return;
                    }
                    let output = if let Some(ref d) = desc {
                        match mode.as_str() {
                            "json" => format::format_json(&pv_name, value),
                            "raw" => format::format_raw(&pv_name, d, value),
                            _ => format::format_nt(&pv_name, d, value),
                        }
                    } else {
                        format!("{pv_name} {value}\n")
                    };
                    print!("{output}");
                })
                .await;

            if let Err(e) = result {
                eprintln!("{pv_name}: {e}");
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.await;
    }
}
