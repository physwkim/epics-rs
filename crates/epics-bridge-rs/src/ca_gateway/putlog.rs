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

/// Default rotation threshold: 100 MiB. Rolls `path` to `path.1`
/// (overwriting any prior `.1`) and re-opens. Operators typically
/// run logrotate on top; this is the in-process safety net so the
/// gateway doesn't fill the partition between cron ticks.
const DEFAULT_MAX_BYTES: u64 = 100 * 1024 * 1024;

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
/// writers are serialized via an internal mutex. When the file grows
/// past `max_bytes`, it is renamed to `<path>.1` (overwriting any
/// existing `.1`) and a fresh file is opened. Operators are still
/// expected to run logrotate; this is the in-process backstop so a
/// chatty gateway can't fill its disk between rotation ticks.
pub struct PutLog {
    path: PathBuf,
    /// Mutex around the file handle so concurrent writers are serialized.
    file: Mutex<Option<tokio::fs::File>>,
    /// Approximate byte count of the current file (tracked since open
    /// so we don't `metadata()` on every write). Reset to 0 after
    /// rotation.
    bytes_written: Mutex<u64>,
    max_bytes: u64,
}

impl PutLog {
    /// Create a new logger writing to `path`. Opens (or creates) the file
    /// in append mode lazily on the first write. Default rotation
    /// threshold is 100 MiB; override with [`Self::with_max_bytes`].
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            file: Mutex::new(None),
            bytes_written: Mutex::new(0),
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }

    /// Override the rotation threshold (bytes).
    pub fn with_max_bytes(mut self, n: u64) -> Self {
        self.max_bytes = n;
        self
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
            // Initialise byte counter from existing file size so a
            // restart picks up the rotation threshold mid-cycle.
            let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
            *self.bytes_written.lock().await = len;
            *guard = Some(f);
        }

        if let Some(f) = guard.as_mut() {
            f.write_all(line.as_bytes()).await?;
            f.flush().await?;
        }
        let mut counter = self.bytes_written.lock().await;
        *counter = counter.saturating_add(line.len() as u64);

        if *counter >= self.max_bytes {
            // Drop current handle, rename, re-open lazily on next log().
            *guard = None;
            *counter = 0;
            drop(guard);
            drop(counter);
            let backup = self.path.with_extension(
                self.path
                    .extension()
                    .map(|e| format!("{}.1", e.to_string_lossy()))
                    .unwrap_or_else(|| "1".to_string()),
            );
            // Best-effort rename; failure (e.g. backup path on a
            // different fs) just means the next write keeps appending
            // to the original file. Operators get a warning to fix.
            if let Err(e) = tokio::fs::rename(&self.path, &backup).await {
                tracing::warn!(
                    error = %e,
                    src = %self.path.display(),
                    dst = %backup.display(),
                    "putlog rotation rename failed; continuing without rotation"
                );
            }
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
