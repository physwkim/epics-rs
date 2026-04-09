//! CA gateway daemon binary.
//!
//! Usage:
//!
//! ```text
//! ca-gateway-rs --pvlist gateway.pvlist [--access gateway.access]
//!               [--preload preload.txt] [--port 5064]
//!               [--read-only] [--no-stats]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use epics_bridge_rs::ca_gateway::{GatewayConfig, GatewayServer, RestartPolicy, supervise};

#[derive(Parser, Debug)]
#[command(
    name = "ca-gateway-rs",
    about = "Pure Rust port of the EPICS CA gateway",
    version
)]
struct Args {
    /// Path to .pvlist access list file
    #[arg(long)]
    pvlist: Option<PathBuf>,

    /// Path to .access ACF file
    #[arg(long)]
    access: Option<PathBuf>,

    /// Path to a file listing literal upstream PV names to pre-subscribe
    /// (one per line, blank/# lines ignored).
    #[arg(long)]
    preload: Option<PathBuf>,

    /// Path to put-event log file (records all client puts).
    #[arg(long)]
    putlog: Option<PathBuf>,

    /// Path to a command file processed when the gateway receives SIGUSR1 (Unix).
    #[arg(long)]
    command: Option<PathBuf>,

    /// CA server port (downstream side). 0 = use default 5064.
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// Read-only mode: reject all puts.
    #[arg(long)]
    read_only: bool,

    /// Disable statistics PV publication.
    #[arg(long)]
    no_stats: bool,

    /// Statistics PV prefix (default: "gateway:").
    #[arg(long, default_value = "gateway:")]
    stats_prefix: String,

    /// Heartbeat interval in seconds (0 = disable).
    #[arg(long, default_value_t = 1)]
    heartbeat_interval: u64,

    /// Cleanup interval in seconds.
    #[arg(long, default_value_t = 10)]
    cleanup_interval: u64,

    /// Statistics refresh interval in seconds.
    #[arg(long, default_value_t = 10)]
    stats_interval: u64,

    /// Run under auto-restart supervisor (NRESTARTS pattern).
    #[arg(long)]
    supervised: bool,

    /// Max restarts within window (default 10).
    #[arg(long, default_value_t = 10)]
    max_restarts: u32,

    /// Restart window in seconds (default 600).
    #[arg(long, default_value_t = 600)]
    restart_window: u64,

    /// Restart delay in seconds (default 10).
    #[arg(long, default_value_t = 10)]
    restart_delay: u64,
}

async fn run_once(config: GatewayConfig) -> Result<(), String> {
    eprintln!("[ca-gateway-rs] starting...");
    let server = GatewayServer::build(config)
        .await
        .map_err(|e| format!("build failed: {e}"))?;
    server
        .run()
        .await
        .map_err(|e| format!("runtime error: {e}"))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();

    let config = GatewayConfig {
        pvlist_path: args.pvlist.clone(),
        pvlist_content: None,
        access_path: args.access.clone(),
        putlog_path: args.putlog.clone(),
        command_path: args.command.clone(),
        preload_path: args.preload.clone(),
        server_port: args.port,
        timeouts: Default::default(),
        stats_prefix: if args.no_stats {
            String::new()
        } else {
            args.stats_prefix.clone()
        },
        cleanup_interval: std::time::Duration::from_secs(args.cleanup_interval),
        stats_interval: std::time::Duration::from_secs(args.stats_interval),
        heartbeat_interval: if args.heartbeat_interval == 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(args.heartbeat_interval))
        },
        read_only: args.read_only,
    };

    if args.supervised {
        let policy = RestartPolicy {
            max_restarts: args.max_restarts,
            window: std::time::Duration::from_secs(args.restart_window),
            delay: std::time::Duration::from_secs(args.restart_delay),
        };
        eprintln!(
            "[ca-gateway-rs] running under supervisor: max {} restarts in {:?}",
            args.max_restarts,
            std::time::Duration::from_secs(args.restart_window),
        );
        let result = supervise(policy, || {
            let cfg = config.clone();
            async move { run_once(cfg).await }
        })
        .await;

        match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[ca-gateway-rs] supervisor exit: {e}");
                ExitCode::FAILURE
            }
        }
    } else {
        match run_once(config).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("[ca-gateway-rs] {e}");
                ExitCode::FAILURE
            }
        }
    }
}
