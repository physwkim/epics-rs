//! PTY-based child process management.
//!
//! Wraps `forkpty(3)` (via `nix::pty::forkpty`) to launch the
//! supervised child with its stdin/stdout/stderr connected to a
//! pseudo-terminal. The supervisor owns the master fd and
//! reads/writes through it. Mirrors C `processClass` (procServ's
//! `processFactory.cc`).
//!
//! ## TODO: implementation
//!
//! Skeleton only. Real implementation will:
//! 1. `forkpty` the child, `execvp` argv, set process group via `setsid`
//! 2. In parent, wrap PTY-master fd in `tokio::io::unix::AsyncFd`
//!    so reads/writes integrate with the runtime
//! 3. Spawn a task that reads PTY → forwards into supervisor's mpsc
//!    (party-line input)
//! 4. Provide a `write_to_child(bytes)` method that writes to the
//!    PTY-master fd (used by the supervisor when forwarding party-line
//!    output to the child stdin direction)
//! 5. Emit a [`ChildEvent::Exited`] when the child terminates so the
//!    supervisor can apply the restart policy

use std::path::PathBuf;
use std::process::ExitStatus;

use crate::procserv::error::ProcServResult;

/// Lifecycle event emitted by [`ChildHandle`] over its event channel.
#[derive(Debug)]
pub enum ChildEvent {
    /// PTY produced a chunk of output (child stdout/stderr).
    Output(Vec<u8>),
    /// Child process terminated. The supervisor consults the restart
    /// policy and either re-spawns or exits.
    Exited { status: Option<ExitStatus> },
}

/// Configuration for one child launch. Mirrors the subset of
/// [`crate::procserv::config::ChildConfig`] this module needs.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub ignore_chars: Vec<u8>,
}

/// Handle to a running child process. Cloning is cheap (Arc inside).
#[derive(Debug)]
pub struct ChildHandle {
    // Implementation:
    // - pid: nix::unistd::Pid
    // - master_fd: tokio::io::unix::AsyncFd<OwnedFd>
    // - event_tx: mpsc::Sender<ChildEvent>
    // - join_handle: tokio::task::JoinHandle<()>
    // (kept private; access is via methods)
    _placeholder: (),
}

impl ChildHandle {
    /// Spawn a new child via `forkpty` + `execvp`, returning the
    /// handle plus the receiver for [`ChildEvent`]s. The receiver is
    /// closed when the child exits and its PTY drains.
    ///
    /// # Errors
    /// Returns [`crate::procserv::error::ProcServError::Forkpty`] if
    /// the fork or exec fails. The child path is not validated up
    /// front; an `execvp` failure surfaces as `Exited` shortly after
    /// `spawn` returns.
    pub fn spawn(
        _spec: &ChildSpec,
    ) -> ProcServResult<(Self, tokio::sync::mpsc::Receiver<ChildEvent>)> {
        // TODO: real implementation
        let (_tx, rx) = tokio::sync::mpsc::channel(64);
        Ok((Self { _placeholder: () }, rx))
    }

    /// Write bytes to the child's stdin (PTY-master write). Called
    /// by the supervisor when a non-readonly client's input is
    /// being forwarded to the child via the party-line.
    ///
    /// `ignore_chars` from [`ChildSpec`] are filtered out before the
    /// write — matches C procServ's `--ignore` flag handled in
    /// `processClass::Send`.
    pub async fn write_stdin(&self, _bytes: &[u8]) -> ProcServResult<()> {
        // TODO: real implementation
        Ok(())
    }

    /// Send a signal to the child process group. Negative pid means
    /// "all processes in pgid", which is what we want — `setsid`
    /// makes the child its own group leader so we can signal the
    /// whole tree of grandchildren too.
    pub fn signal(&self, _signo: i32) -> ProcServResult<()> {
        // TODO: real implementation
        Ok(())
    }

    /// Whether the child is currently alive. The menu-key dispatch
    /// (`evaluate(child_alive=…)`) consults this each keystroke.
    pub fn is_alive(&self) -> bool {
        // TODO: real implementation
        false
    }
}
