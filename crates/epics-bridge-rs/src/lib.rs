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
//! | `ca_gateway` | `ca-gateway` | CA fan-out gateway (C++ ca-gateway equivalent) |
//! | `pvalink` | `pvalink` | PVA links for record INP/OUT — *planned* |
//! | `pva_gateway` | `pva-gateway` | PVA-to-PVA proxy — *planned* |
//!
//! ## Group PVs (QSRV equivalent)
//!
//! Group PV support (composite PVA channels backed by multiple EPICS records)
//! is now provided directly by `epics-pva-rs` via `spvirit-server`'s
//! [`GroupPvStore`](spvirit_server::GroupPvStore). Use
//! [`PvaServerBuilder::group_json()`](epics_pva_rs::server::PvaServerBuilder::group_json)
//! or
//! [`PvaServerBuilder::group_file()`](epics_pva_rs::server::PvaServerBuilder::group_file)
//! to configure group PVs.

pub mod error;
pub use error::{BridgeError, BridgeResult};

#[cfg(feature = "ca-gateway")]
pub mod ca_gateway;
