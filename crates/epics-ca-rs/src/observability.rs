//! Optional helpers for wiring up tracing + metrics observability.
//!
//! Compiled only with the `observability` feature so the default build
//! retains zero overhead when not used.
//!
//! # Quick start
//!
//! ```ignore
//! // In a binary that depends on epics-ca-rs with the `observability` feature:
//! epics_ca_rs::observability::init_tracing();
//! let _exporter = epics_ca_rs::observability::serve_prometheus("0.0.0.0:9090".parse()?)?;
//!
//! let client = epics_ca_rs::client::CaClient::new().await?;
//! // ... your code; metrics + structured logs are emitted automatically.
//! ```
//!
//! # Metric names
//!
//! All metrics emitted by epics-ca-rs are prefixed with `ca_client_*`
//! (client side) or `ca_server_*` (server side). See
//! `doc/10-observability.md` for the full schema.

use std::net::SocketAddr;

#[cfg(feature = "observability")]
use metrics_exporter_prometheus::PrometheusBuilder;

/// Initialize a `tracing` subscriber that reads `RUST_LOG` and writes to
/// stderr. Idempotent — calling twice is harmless. Returns true on first
/// call, false on subsequent calls.
#[cfg(feature = "observability")]
pub fn init_tracing() -> bool {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,epics_ca_rs=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(false))
        .try_init()
        .is_ok()
}

/// Install the `metrics` global recorder backed by a Prometheus exporter.
/// Spawns an HTTP listener on `addr` that serves `/metrics`.
///
/// Returns the exporter's drop guard. Drop it to stop the listener.
#[cfg(feature = "observability")]
pub fn serve_prometheus(
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()?;
    Ok(())
}

/// Stub when the feature is disabled — returns `false` so callers can
/// log "observability not compiled in" without conditional compilation
/// at the call site.
#[cfg(not(feature = "observability"))]
pub fn init_tracing() -> bool {
    false
}

/// Stub when the feature is disabled.
#[cfg(not(feature = "observability"))]
pub fn serve_prometheus(
    _addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    Err("epics-ca-rs built without `observability` feature".into())
}
