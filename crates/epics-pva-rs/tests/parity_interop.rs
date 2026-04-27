//! Wire-level parity / interop matrix entry point.
//!
//! Cargo only picks up `tests/*.rs` automatically; this file exists so the
//! sub-modules under `tests/parity/` get compiled and registered.

#[path = "parity/interop.rs"]
mod interop;
