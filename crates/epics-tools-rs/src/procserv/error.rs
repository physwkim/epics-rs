//! Error types for the procserv supervisor.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProcServError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("forkpty failed: {0}")]
    Forkpty(String),

    #[error("listener bind failed: {0}")]
    ListenerBind(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("child exited with status {0:?}")]
    ChildExited(Option<i32>),

    #[error("restart limit exceeded ({attempts} in {window_secs}s)")]
    RestartLimitExceeded { attempts: u32, window_secs: u64 },

    #[error("shutdown requested")]
    Shutdown,
}

pub type ProcServResult<T> = Result<T, ProcServError>;
