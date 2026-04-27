//! Native pvAccess server runtime — no `spvirit_server` dependency.

pub mod runtime;
pub mod source;
pub mod tcp;
pub mod udp;

pub use runtime::{run_pva_server, PvaServerConfig};
pub use source::{ChannelSource, ChannelSourceObj, DynSource};
