//! Native pvAccess server runtime — no `spvirit_server` dependency.

pub mod runtime;
pub mod shared_pv;
pub mod source;
pub mod tcp;
pub mod udp;

pub use runtime::{run_pva_server, PvaServerConfig};
pub use shared_pv::{SharedPV, SharedSource};
pub use source::{ChannelSource, ChannelSourceObj, DynSource};
