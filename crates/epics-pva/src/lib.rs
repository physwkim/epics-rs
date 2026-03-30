//! EPICS pvAccess protocol — client (experimental).
//!
//! This crate provides the pvAccess wire protocol implementation,
//! separated from the core IOC infrastructure in `epics-base-rs`.

pub mod client;
pub mod codec;
pub mod error;
pub mod protocol;
pub mod pvdata;
pub mod serialize;

pub use error::{PvaError, PvaResult};

// Re-export commonly used types from epics-base-rs
pub use epics_base_rs::types::{DbFieldType, EpicsValue};
