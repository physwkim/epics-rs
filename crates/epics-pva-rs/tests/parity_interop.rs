//! Wire-level parity / interop matrix entry point.
//!
//! Cargo only picks up `tests/*.rs` automatically; this file exists so the
//! sub-modules under `tests/parity/` get compiled and registered.

#[path = "parity/interop.rs"]
mod interop;
#[path = "parity/wire_dump.rs"]
mod wire_dump;
#[path = "parity/tls_interop.rs"]
mod tls_interop;
