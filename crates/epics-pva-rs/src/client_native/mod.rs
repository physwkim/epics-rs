//! Native pvAccess client — no `spvirit_client` dependency.
//!
//! Layered structure (mirrors pvxs `src/client*.cpp`):
//!
//! - [`decode`]   parses PVA frames coming from the server
//! - [`conn`]     manages a single TCP virtual circuit (handshake +
//!                framed I/O)
//! - [`search`]   handles UDP search broadcast + reply collection
//! - [`ops`]      drives GET / PUT / MONITOR / GET_FIELD operations on
//!                top of an established connection
//! - [`context`]  the public [`PvaClient`] facade
//!
//! The legacy `crate::client` module is a thin re-export of this one (see
//! `client.rs`), so existing callers like `pvget-rs` keep working.

pub mod conn;
pub mod context;
pub mod decode;
pub mod ops;
pub mod search;

pub use context::{PvGetResult, PvaClient, PvaClientBuilder};
