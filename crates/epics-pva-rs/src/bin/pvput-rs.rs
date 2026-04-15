use clap::Parser;
use epics_pva_rs::client::PvaClient;

#[derive(Parser)]
#[command(
    name = "pvput-rs",
    version,
    about = "Write a value to an EPICS PV via pvAccess"
)]
struct Args {
    /// PV name to write to
    pv_name: String,
    /// Value to write
    #[arg(allow_hyphen_values = true)]
    value: String,

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
    let client = PvaClient::new().expect("failed to create PVA client");

    match client.pvput(&args.pv_name, &args.value).await {
        Ok(()) => {
            if !args.quiet {
                println!("{} <- {}", args.pv_name, args.value);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
