//! # epics-bridge-rs
//!
//! EPICS protocol bridge/adapter hub.
//!
//! This crate hosts multiple bridge implementations as feature-gated
//! sub-modules. Each bridge connects EPICS data sources to network
//! protocols (CA or PVA).
//!
//! ## Sub-modules
//!
//! | Module | Feature | Description |
//! |--------|---------|-------------|
//! | [`qsrv`] | `qsrv` (default) | Record → pvAccess channels (C++ QSRV equivalent) |
//! | `ca_gateway` | `ca-gateway` | CA fan-out gateway (C++ ca-gateway equivalent) — *planned* |
//! | `pvalink` | `pvalink` | PVA links for record INP/OUT — *planned* |
//! | `pva_gateway` | `pva-gateway` | PVA-to-PVA proxy — *planned* |
//!
//! ## QSRV (Record ↔ PVA bridge)
//!
//! ```text
//! PVA Client ←→ [epics-pva-rs server] ←→ BridgeProvider ←→ PvDatabase
//! ```
//!
//! - [`BridgeProvider`] implements [`ChannelProvider`] — the PVA server calls
//!   into it to resolve channel names and create channels.
//! - [`BridgeChannel`] serves single-record PVs (NTScalar, NTEnum, NTScalarArray).
//! - [`GroupChannel`] serves multi-record composite PVs from JSON config.
//! - [`BridgeMonitor`] / [`GroupMonitor`] bridge `DbSubscription` events to PVA monitor updates.
//!
//! The `ChannelProvider`, `Channel`, and `PvaMonitor` traits are defined here
//! temporarily. They will move to `epics-pva-rs` once the PVA server is
//! implemented by the spvirit maintainer.

pub mod error;
pub use error::{BridgeError, BridgeResult};

#[cfg(feature = "qsrv")]
pub mod qsrv;

#[cfg(feature = "ca-gateway")]
pub mod ca_gateway;

// Convenience re-exports for the QSRV bridge (default feature).
// External users can write `epics_bridge_rs::BridgeProvider` directly.
#[cfg(feature = "qsrv")]
pub use qsrv::{
    AccessContext, AccessControl, AllowAllAccess, AnyChannel, AnyMonitor, BridgeChannel,
    BridgeMonitor, BridgeProvider, Channel, ChannelProvider, FieldMapping, GroupChannel,
    GroupMonitor, GroupPvDef, NtType, ProcessMode, PutOptions, PvaMonitor,
};
