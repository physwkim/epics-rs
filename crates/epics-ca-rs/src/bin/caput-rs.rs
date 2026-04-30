use clap::Parser;
use epics_ca_rs::CaError;
use epics_ca_rs::client::CaClient;
use std::time::Duration;

#[derive(Parser)]
#[command(name = "caput", about = "Write a value to an EPICS PV")]
struct Args {
    /// Wait for completion callback (ca_put_callback)
    #[arg(short = 'c', long = "callback")]
    callback: bool,

    /// CA timeout in seconds (default: $EPICS_CLI_TIMEOUT or 1.0).
    /// C ref: modules/ca/src/tools/tool_lib.c:use_ca_timeout_env (commit 1d056c6).
    #[arg(short = 'w', long = "timeout")]
    timeout: Option<f64>,

    /// PV name to write to
    pv_name: String,

    /// Value to write
    #[arg(allow_hyphen_values = true)]
    value: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let client = CaClient::new().await.expect("failed to create CA client");
    let timeout = Duration::from_secs_f64(
        args.timeout
            .unwrap_or_else(epics_ca_rs::cli::env_default_timeout),
    );

    let ch = client.create_channel(&args.pv_name);
    if let Err(e) = ch.wait_connected(timeout).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    // Read old value with timeout
    let (native_type, old_value) = match ch.get_with_timeout(timeout).await {
        Ok((t, val)) => (t, val.to_string()),
        Err(CaError::Timeout) => {
            eprintln!("Read operation timed out: PV data was not read.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let value = match epics_ca_rs::EpicsValue::parse(native_type, &args.value) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    // Write value
    let result = if args.callback {
        ch.put_with_timeout(&value, timeout).await
    } else {
        ch.put_nowait(&value).await
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    // Re-read new value from server with timeout (C: caget after put)
    let new_value = match ch.get_with_timeout(timeout).await {
        Ok((_, val)) => val.to_string(),
        _ => args.value.clone(),
    };

    println!("Old : {} {}", args.pv_name, old_value);
    println!("New : {} {}", args.pv_name, new_value);
}
