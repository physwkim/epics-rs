#![allow(
    clippy::collapsible_if,
    clippy::map_entry,
    clippy::new_without_default,
    clippy::redundant_closure,
    clippy::single_match,
    clippy::type_complexity,
    clippy::unnecessary_cast
)]

//! EPICS pvAccess protocol — client and server.
//!
//! This crate provides the pvAccess wire protocol implementation,
//! separated from the core IOC infrastructure in `epics-base-rs`.

pub mod client;
pub mod codec;
pub mod error;
pub mod pv_request;
pub mod server;

pub use error::{PvaError, PvaResult};

// Re-export commonly used types from epics-base-rs
pub use epics_base_rs::types::{DbFieldType, EpicsValue};

// Re-export spvirit-codec types used in the public API
pub use spvirit_codec::spvd_decode::{
    DecodedValue, FieldType, PvdDecoder, StructureDesc, TypeCode,
};
