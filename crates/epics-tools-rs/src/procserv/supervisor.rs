//! Central supervisor task — the heart of the procserv daemon.
//!
//! ## Hub-and-spoke architecture
//!
//! C procServ uses a single `poll(2)` loop that iterates a linked
//! list of `connectionItem*`s and dispatches `readFromFd()` /
//! `Send()` virtuals. Output fan-out goes through `SendToAll(buf,
//! count, sender)` which excludes the originator from the
//! party-line.
//!
//! The Rust port keeps the same party-line semantics but maps it
//! onto tokio with a hub-and-spoke shape:
//!
//! ```text
//!                       ┌──────────────────┐
//!                       │   Supervisor     │
//!                       │   (single task)  │
//!                       └────┬────┬────┬───┘
//!     inbound_rx (mpsc)      │    │    │      outbound_tx (mpsc, one per peer)
//!     ┌──────────────────────┘    │    └──────────────────┐
//!     │                           │                       │
//! ┌───▼──────┐               ┌────▼─────┐           ┌─────▼─────┐
//! │ Client A │               │ Client B │           │ ChildPTY  │
//! └──────────┘               └──────────┘           └───────────┘
//! ```
//!
//! When client A types: A's read task → `inbound_tx` → supervisor
//! receives, scans for menu keys, then forwards the bytes to every
//! OTHER peer's `outbound_tx`. The "exclude sender" property comes
//! for free because the supervisor knows which `ClientId` produced
//! the message.
//!
//! When the PTY emits output: child task → `inbound_tx` → supervisor
//! → all clients' `outbound_tx`s + log file.
//!
//! ## TODO: implementation
//!
//! Skeleton only. The real run loop is approximately:
//!
//! 1. Spawn listener tasks (TCP / UNIX) feeding [`Roster`] add events
//! 2. Spawn child task feeding [`InboundEvent::ChildOutput`] events
//! 3. `tokio::select!` over: roster events, inbound events, restart
//!    timer, shutdown signal
//! 4. On inbound bytes: scan menu keys → dispatch action OR forward
//!    to party-line
//! 5. On child exit: consult [`super::restart::RestartPolicy`] +
//!    `RestartMode`, schedule respawn or shut down
//! 6. On graceful shutdown: signal child, drain log, close listeners,
//!    disconnect clients

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::procserv::child::ChildHandle;
use crate::procserv::client::{ClientId, InboundEvent, OutboundFrame};
use crate::procserv::config::ProcServConfig;
use crate::procserv::error::ProcServResult;

/// Top-level handle. Construct via [`Self::new`], drive with [`Self::run`].
pub struct ProcServ {
    config: Arc<ProcServConfig>,
    // Implementation will hold:
    // - roster: HashMap<ClientId, mpsc::Sender<OutboundFrame>>
    // - child: Option<ChildHandle>
    // - restart_tracker: super::restart::RestartTracker
    // - inbound_tx / inbound_rx: mpsc for all peers
}

impl ProcServ {
    /// Construct from validated config. Does not yet open listeners
    /// or spawn the child — call [`Self::run`].
    pub fn new(config: ProcServConfig) -> ProcServResult<Self> {
        config
            .validate()
            .map_err(crate::procserv::error::ProcServError::Config)?;
        Ok(Self {
            config: Arc::new(config),
        })
    }

    /// Run until shutdown (signal, quit-key, or fatal error).
    ///
    /// # TODO
    /// - spawn listeners (use `self.config.listen.tcp_port` etc.)
    /// - spawn first child via [`ChildHandle::spawn`] unless
    ///   `wait_for_manual_start`
    /// - main `tokio::select!` loop (see module docs)
    pub async fn run(self) -> ProcServResult<()> {
        // TODO: real implementation
        let _ = self.config;
        std::future::pending().await
    }
}

/// Internal supervisor → peer message routing.
///
/// Not part of the public API; collected here so the supervisor's
/// state and message envelope stay in one file.
#[allow(dead_code)]
struct SupervisorState {
    inbound_tx: mpsc::Sender<(PeerId, InboundEvent)>,
    inbound_rx: mpsc::Receiver<(PeerId, InboundEvent)>,
    clients: std::collections::HashMap<ClientId, mpsc::Sender<OutboundFrame>>,
    child: Option<ChildHandle>,
}

/// Source identifier for inbound events: any peer can emit input.
/// Used by the supervisor's "exclude sender" fan-out.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PeerId {
    Client(ClientId),
    /// The PTY child (output direction).
    Child,
}
