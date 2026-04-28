//! Auto-restart master process supervisor.
//!
//! Re-exports [`epics_base_rs::runtime::supervise`] so existing
//! ca-gateway-rs callers don't break. The actual logic now lives in
//! base-rs because the same sliding-window NRESTARTS pattern is
//! used by `epics-tools-rs::procserv` too — keeping one canonical
//! implementation prevents drift.
//!
//! Corresponds to C++ ca-gateway's master process pattern (NRESTARTS=10,
//! RESTART_INTERVAL=10*60s, RESTART_DELAY=10s in `gateway.cc:22-24`).

pub use epics_base_rs::runtime::supervise::{
    RestartPolicy, RestartTracker, SuperviseError, supervise,
};
