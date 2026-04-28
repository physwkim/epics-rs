//! Runtime-reconfigurable log filtering.
//!
//! Mirrors pvxs's `logger_config_env` / `logger_level_set` /
//! `logger_config_str` (log.cpp:343-388) — at startup the application
//! installs a `tracing_subscriber::EnvFilter` wrapped in a
//! `reload::Layer`, and any later call to [`set_log_filter`] swaps
//! the filter atomically without restarting the process.
//!
//! Typical usage:
//!
//! ```ignore
//! use epics_pva_rs::log;
//! use tracing_subscriber::{fmt, prelude::*};
//!
//! // Once at startup. Reads RUST_LOG / EPICS_PVA_LOG, falls back to
//! // "info" when neither is set.
//! let (filter, handle) = log::init_filter();
//! tracing_subscriber::registry()
//!     .with(filter)
//!     .with(fmt::layer())
//!     .init();
//! log::set_global_handle(handle);
//!
//! // Later, e.g., from an admin RPC:
//! log::set_log_filter("info,epics_pva_rs::client_native=debug").ok();
//! ```
//!
//! All knobs are crate-global. There's exactly one reload handle per
//! process — pvxs has the same constraint (logger registry is a
//! global singleton).

use std::sync::OnceLock;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::reload;

/// Type alias for the reload handle. The first generic is the layer
/// type; the second is the registry it sits on. We type-erase the
/// registry side via `dyn` so callers don't have to thread the full
/// subscriber stack through their signatures.
pub type FilterReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

/// Process-wide reload handle. Set by [`set_global_handle`] once the
/// caller has installed the filter into a registry. Subsequent
/// [`set_log_filter`] / [`set_log_level`] calls go through this.
static GLOBAL_HANDLE: OnceLock<FilterReloadHandle> = OnceLock::new();

/// Build an `EnvFilter` reload-layer pair seeded from the standard
/// EPICS log env vars. Honours `EPICS_PVA_LOG` first (matches
/// pvxs `PVXS_LOG`), then `RUST_LOG`, then defaults to `"info"`.
///
/// Returns the wrapped layer (install into your subscriber) and a
/// handle for runtime reconfiguration.
pub fn init_filter() -> (
    reload::Layer<EnvFilter, tracing_subscriber::Registry>,
    FilterReloadHandle,
) {
    let initial_spec = std::env::var("EPICS_PVA_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    let filter = EnvFilter::try_new(&initial_spec).unwrap_or_else(|_| EnvFilter::new("info"));
    reload::Layer::new(filter)
}

/// Register the reload handle returned by [`init_filter`] as the
/// process-wide handle. Idempotent — subsequent calls are no-ops so
/// applications with multiple wiring entry points don't conflict.
pub fn set_global_handle(handle: FilterReloadHandle) {
    let _ = GLOBAL_HANDLE.set(handle);
}

/// Replace the active log filter spec. `spec` follows the standard
/// `tracing_subscriber::EnvFilter` syntax — same as pvxs's
/// `logger_config_str` (log.cpp:343), e.g.,
/// `"info,epics_pva_rs::client_native=debug"`.
///
/// Returns Err when no global handle is installed (caller forgot to
/// call [`set_global_handle`]) or when `spec` fails to parse.
pub fn set_log_filter(spec: &str) -> Result<(), LogFilterError> {
    let handle = GLOBAL_HANDLE.get().ok_or(LogFilterError::NoHandle)?;
    let new_filter = EnvFilter::try_new(spec).map_err(|e| LogFilterError::Parse(e.to_string()))?;
    handle
        .reload(new_filter)
        .map_err(|e| LogFilterError::Reload(e.to_string()))
}

/// Set a single target's level. Mirrors pvxs `logger_level_set(name,
/// Level)`. Internally builds an `EnvFilter` of the form
/// `"<base>,<target>=<level>"` where `<base>` is the current
/// `RUST_LOG` (or `"info"` fallback).
pub fn set_log_level(target: &str, level: LevelFilter) -> Result<(), LogFilterError> {
    let base = std::env::var("EPICS_PVA_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| "info".to_string());
    let level_str = match level {
        LevelFilter::OFF => "off",
        LevelFilter::ERROR => "error",
        LevelFilter::WARN => "warn",
        LevelFilter::INFO => "info",
        LevelFilter::DEBUG => "debug",
        LevelFilter::TRACE => "trace",
    };
    let spec = if base.is_empty() {
        format!("{target}={level_str}")
    } else {
        format!("{base},{target}={level_str}")
    };
    set_log_filter(&spec)
}

/// Errors from [`set_log_filter`] / [`set_log_level`].
#[derive(Debug, thiserror::Error)]
pub enum LogFilterError {
    #[error("no reload handle registered; call log::set_global_handle() first")]
    NoHandle,
    #[error("invalid filter spec: {0}")]
    Parse(String),
    #[error("reload failed: {0}")]
    Reload(String),
}
