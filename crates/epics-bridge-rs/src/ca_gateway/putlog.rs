//! Put-event logging.
//!
//! Records all client put operations to a log file. Corresponds to
//! C++ ca-gateway's `-putlog` option.
//!
//! Each put generates one line in the log:
//!
//! ```text
//! 2026-04-09T14:35:21.123Z user@host TEMP:setpoint 25.0 OK
//! 2026-04-09T14:35:22.456Z guest@1.2.3.4 PRESSURE:cmd 100.0 DENIED
//! ```

use std::path::PathBuf;

use chrono::Utc;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use crate::error::BridgeResult;

/// Outcome of a put attempt for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutOutcome {
    /// Put accepted and forwarded upstream.
    Ok,
    /// Put rejected (read-only mode, ACL deny, etc.).
    Denied,
    /// Put forwarded but upstream returned an error.
    Failed,
}

impl PutOutcome {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::Denied => "DENIED",
            Self::Failed => "FAILED",
        }
    }
}

/// Put-event logger.
///
/// Writes to a file with line-buffered async I/O. Multiple concurrent
/// writers are serialized via an internal mutex.
pub struct PutLog {
    path: PathBuf,
    /// Mutex around the file handle so concurrent writers are serialized.
    file: Mutex<Option<tokio::fs::File>>,
}

impl PutLog {
    /// Create a new logger writing to `path`. Opens (or creates) the file
    /// in append mode lazily on the first write.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: Mutex::new(None),
        }
    }

    /// Log a put event.
    pub async fn log(
        &self,
        user: &str,
        host: &str,
        pv: &str,
        value: &str,
        outcome: PutOutcome,
    ) -> BridgeResult<()> {
        let timestamp = Utc::now().to_rfc3339();
        let line = format!(
            "{} {}@{} {} {} {}\n",
            timestamp,
            user,
            host,
            pv,
            value,
            outcome.as_str()
        );

        let mut guard = self.file.lock().await;
        if guard.is_none() {
            let f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .await?;
            *guard = Some(f);
        }

        if let Some(f) = guard.as_mut() {
            f.write_all(line.as_bytes()).await?;
            f.flush().await?;
        }
        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn log_to_temp_file() {
        let temp =
            std::env::temp_dir().join(format!("ca_gateway_putlog_test_{}.log", std::process::id()));
        // Cleanup any leftover from previous test runs
        let _ = std::fs::remove_file(&temp);

        let log = PutLog::new(temp.clone());
        log.log("alice", "host1", "TEMP", "25.0", PutOutcome::Ok)
            .await
            .unwrap();
        log.log("bob", "host2", "PRESS", "100", PutOutcome::Denied)
            .await
            .unwrap();
        log.log("eve", "host3", "VAC", "1e-6", PutOutcome::Failed)
            .await
            .unwrap();

        // Read back
        let content = std::fs::read_to_string(&temp).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("alice@host1 TEMP 25.0 OK"));
        assert!(lines[1].contains("bob@host2 PRESS 100 DENIED"));
        assert!(lines[2].contains("eve@host3 VAC 1e-6 FAILED"));

        let _ = std::fs::remove_file(&temp);
    }

    #[test]
    fn outcome_as_str() {
        assert_eq!(PutOutcome::Ok.as_str(), "OK");
        assert_eq!(PutOutcome::Denied.as_str(), "DENIED");
        assert_eq!(PutOutcome::Failed.as_str(), "FAILED");
    }
}
