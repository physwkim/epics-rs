//! Errors raised by the PVA gateway internals.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GwError {
    #[error(
        "upstream PV {0:?} did not deliver an initial monitor event before the connect timeout"
    )]
    UpstreamTimeout(String),
    #[error("upstream PVA error: {0}")]
    Upstream(#[from] epics_pva_rs::error::PvaError),
    #[error("gateway not running")]
    NotRunning,
    #[error("channel cache full (cap = {0})")]
    CacheFull(usize),
    #[error("{0}")]
    Other(String),
}

pub type GwResult<T> = Result<T, GwError>;
