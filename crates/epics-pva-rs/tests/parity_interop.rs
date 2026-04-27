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
#[path = "parity/stability_interop.rs"]
mod stability_interop;
#[path = "parity/testbitmask_port.rs"]
mod testbitmask_port;
#[path = "parity/testxcode_port.rs"]
mod testxcode_port;
#[path = "parity/testendian_port.rs"]
mod testendian_port;
#[path = "parity/testpvreq_port.rs"]
mod testpvreq_port;
#[path = "parity/testtype_port.rs"]
mod testtype_port;
#[path = "parity/testdata_port.rs"]
mod testdata_port;
#[path = "parity/testconfig_port.rs"]
mod testconfig_port;
#[path = "parity/testudp_port.rs"]
mod testudp_port;
#[path = "parity/testdiscover_port.rs"]
mod testdiscover_port;
