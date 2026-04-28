//! `procserv-rs` — Rust port of the EPICS procServ daemon.
//!
//! Drop-in for `epics-modules/procServ` deployments: same flag set,
//! same `PROCSERV_INFO` env contract, same per-line PTY behaviour.
//! See [`epics_tools_rs::procserv`] for architectural notes.

#[cfg(not(unix))]
fn main() {
    eprintln!("procserv-rs is Unix-only (requires forkpty)");
    std::process::exit(2);
}

#[cfg(unix)]
mod app {
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::time::Duration;

    use clap::Parser;

    use epics_tools_rs::procserv::{
        ProcServ, ProcServConfig,
        config::{ChildConfig, KeyBindings, ListenConfig, LoggingConfig},
        restart::{RestartMode, RestartPolicy},
    };

    /// CLI flags chosen to match C procServ verbatim where possible.
    /// Operators porting wrapper scripts should not need to relearn
    /// the surface.
    #[derive(Parser, Debug)]
    #[command(
        name = "procserv-rs",
        about = "Pure-Rust port of the EPICS procServ process supervisor",
        version
    )]
    struct Args {
        /// TCP port to listen on (`-p` / `--port` in C procServ).
        #[arg(short = 'p', long)]
        port: Option<u16>,

        /// Bind to all interfaces; default is localhost only.
        /// (C procServ `--allow`).
        #[arg(long)]
        allow: bool,

        /// UNIX-domain socket path (`--unixpath`).
        #[arg(long = "unixpath")]
        unix_path: Option<PathBuf>,

        /// Run in foreground (don't daemonize) — `-f` in C.
        #[arg(short = 'f', long)]
        foreground: bool,

        /// Log file (`-L` / `--logfile`).
        #[arg(short = 'L', long)]
        logfile: Option<PathBuf>,

        /// PID file (`--pidfile`).
        #[arg(long)]
        pidfile: Option<PathBuf>,

        /// Info file (`--info-file`).
        #[arg(long)]
        info_file: Option<PathBuf>,

        /// Hold-off time between restarts in seconds (`--holdoff`).
        #[arg(long, default_value_t = 15)]
        holdoff: u64,

        /// Wait for manual start (don't launch the child until a
        /// connected user issues the restart key).
        #[arg(short = 'w', long)]
        wait: bool,

        /// chdir to this directory before exec'ing the child.
        #[arg(long)]
        chdir: Option<PathBuf>,

        /// Display name for the child in banners.
        #[arg(long)]
        name: Option<String>,

        /// Program to launch + its arguments. Everything after `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        cmd: Vec<String>,
    }

    fn build_config(args: Args) -> Result<ProcServConfig, String> {
        if args.cmd.is_empty() {
            return Err("missing child command".into());
        }
        let mut iter = args.cmd.into_iter();
        let program = PathBuf::from(iter.next().unwrap());
        let argv: Vec<String> = iter.collect();
        let display_name = args
            .name
            .unwrap_or_else(|| program.file_name().map_or("child".into(), |s| s.to_string_lossy().into()));

        let listen = ListenConfig {
            tcp_port: args.port,
            tcp_bind: args.port.map(|p| {
                if args.allow {
                    std::net::SocketAddr::from(([0, 0, 0, 0], p))
                } else {
                    std::net::SocketAddr::from(([127, 0, 0, 1], p))
                }
            }),
            unix_path: args.unix_path,
        };

        let child = ChildConfig {
            name: display_name,
            program,
            args: argv,
            cwd: args.chdir,
            kill_signal: 9, // SIGKILL — match C default
            ignore_chars: Vec::new(),
        };

        let logging = LoggingConfig {
            log_path: args.logfile,
            pid_path: args.pidfile,
            info_path: args.info_file,
            time_format: "%Y-%m-%d %H:%M:%S".into(),
        };

        Ok(ProcServConfig {
            foreground: args.foreground,
            listen,
            keys: KeyBindings::default(),
            child,
            logging,
            restart: RestartPolicy::default(),
            restart_mode: RestartMode::OnExit,
            holdoff: Duration::from_secs(args.holdoff),
            wait_for_manual_start: args.wait,
        })
    }

    pub async fn entry() -> ExitCode {
        // Initialize tracing — RUST_LOG controls verbosity.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .try_init();

        let args = Args::parse();
        let cfg = match build_config(args) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "procserv-rs: invalid config");
                return ExitCode::FAILURE;
            }
        };

        let server = match ProcServ::new(cfg) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(error = %e, "procserv-rs: build failed");
                return ExitCode::FAILURE;
            }
        };

        match server.run().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                tracing::error!(error = %e, "procserv-rs: runtime error");
                ExitCode::FAILURE
            }
        }
    }
}

#[cfg(unix)]
#[tokio::main(flavor = "multi_thread")]
async fn main() -> std::process::ExitCode {
    app::entry().await
}
