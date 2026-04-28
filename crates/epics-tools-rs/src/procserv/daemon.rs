//! Daemonize + signal forwarding.
//!
//! Mirrors C `forkAndGo()` (`procServ.cc:870`) and the
//! `OnSig{Pipe,Term,Hup}` handlers. The main goals:
//!
//! 1. **Detach from controlling terminal**: `fork` + `setsid` + close
//!    fd 0/1/2 (or redirect to `/dev/null`).
//! 2. **PID file**: write the supervisor's pid for `manage-procs`
//!    style tooling.
//! 3. **Signal forwarding**:
//!    - `SIGHUP` — reload config
//!    - `SIGTERM`/`SIGINT` — graceful shutdown (signal child, drain, exit)
//!    - `SIGPIPE` — ignored (PTY writes to dead clients raise it)

use crate::procserv::error::ProcServResult;

/// Daemonize the current process. Equivalent to C `forkAndGo()`
/// when `--foreground` is not set.
///
/// # TODO: implementation
/// - `nix::unistd::fork()`; parent exits, child continues
/// - `nix::unistd::setsid()` to become session leader
/// - second fork to ensure no controlling tty (defensive)
/// - `chdir("/")` so the daemon doesn't pin a mount
/// - redirect 0/1/2 → `/dev/null` (or to log file when configured)
pub fn fork_and_go() -> ProcServResult<()> {
    // TODO: real implementation
    Ok(())
}

/// Set up the signal-handling task. Returns a future that resolves
/// when a graceful-shutdown signal arrives; reload signals are
/// handled internally by the returned `ReloadHandle`.
///
/// # TODO: implementation
/// - `tokio::signal::unix::signal(SignalKind::terminate())` for SIGTERM
/// - `SignalKind::interrupt()` for SIGINT
/// - `SignalKind::hangup()` for SIGHUP (treat as reload)
/// - install ignore for SIGPIPE so dead-client writes don't kill us
pub async fn install_signal_handlers() -> ProcServResult<ShutdownSignal> {
    // TODO: real implementation
    Ok(ShutdownSignal { _placeholder: () })
}

/// Future-like handle that resolves when a graceful-shutdown signal
/// is received. The supervisor task `tokio::select!`s on this
/// alongside its other branches.
pub struct ShutdownSignal {
    _placeholder: (),
}
