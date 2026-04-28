//! Side-car file management — log file, info file, pid file, env vars.
//!
//! Mirrors C `openLogFile()`, `writeInfoFile()`, `writePidFile()`,
//! and `setEnvVar()` from `procServ.cc`. These exist to support the
//! `procServUtils/manage-procs` tooling, which inspects pid + info
//! files in a known directory to enumerate / attach / restart
//! procserv instances.
//!
//! Convention preserved exactly (per kodex commit-history note
//! `4d2aee67`): `PROCSERV_INFO` env-var carries `KEY=value` pairs
//! mirroring the info file content.

use std::path::Path;
use std::path::PathBuf;

use crate::procserv::error::ProcServResult;

/// Per-line writer to the supervisor log. Wraps a file with timestamp
/// prefixing and rotation (TODO).
pub struct LogFile {
    _placeholder: (),
}

impl LogFile {
    /// Open / create the log at `path`. Appends if it exists.
    ///
    /// # TODO
    /// - `tokio::fs::OpenOptions::new().append(true).create(true)`
    /// - line buffering (flush on `\n`)
    /// - timestamp prefix per line using config.logging.time_format
    /// - rotation hook (size or daily) — defer to v1.1
    pub async fn open(_path: &Path) -> ProcServResult<Self> {
        // TODO: real implementation
        Ok(Self { _placeholder: () })
    }

    pub async fn write(&self, _line: &[u8]) -> ProcServResult<()> {
        // TODO: real implementation
        Ok(())
    }
}

/// Write the supervisor's pid to the configured pid file.
///
/// # TODO
/// - atomic write (tmp + rename) so concurrent reads never see a
///   partial file
/// - cleanup hook to delete on graceful exit
pub fn write_pid_file(_path: &Path, _pid: i32) -> ProcServResult<()> {
    // TODO: real implementation
    Ok(())
}

/// Status info file. Format matches C procServ + `manage-procs`:
///
/// ```text
/// procservpid=NNNN
/// childpid=NNNN
/// childexe=/path/to/foo
/// childargs=arg1 arg2
/// ```
///
/// Updated whenever the child respawns.
pub fn write_info_file(_path: &Path, _info: &InfoSnapshot) -> ProcServResult<()> {
    // TODO: real implementation
    Ok(())
}

/// Snapshot of supervisor + child state, serialized into the info
/// file and the `PROCSERV_INFO` env var. Construct a fresh one on
/// each child respawn.
#[derive(Debug, Clone)]
pub struct InfoSnapshot {
    pub procserv_pid: i32,
    pub child_pid: Option<i32>,
    pub child_exe: PathBuf,
    pub child_args: Vec<String>,
}

/// Build the `KEY=value` form for `PROCSERV_INFO` env var (passed to
/// the child on `execvp` so the IOC can introspect its supervision
/// context).
pub fn render_procserv_info(info: &InfoSnapshot) -> String {
    let mut out = String::new();
    out.push_str(&format!("procservpid={}\n", info.procserv_pid));
    if let Some(p) = info.child_pid {
        out.push_str(&format!("childpid={p}\n"));
    }
    out.push_str(&format!("childexe={}\n", info.child_exe.display()));
    out.push_str(&format!("childargs={}\n", info.child_args.join(" ")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_info_keys_match_c_procserv_convention() {
        let info = InfoSnapshot {
            procserv_pid: 1234,
            child_pid: Some(1235),
            child_exe: PathBuf::from("/usr/bin/softIoc"),
            child_args: vec!["-d".into(), "test.db".into()],
        };
        let rendered = render_procserv_info(&info);
        assert!(rendered.contains("procservpid=1234"));
        assert!(rendered.contains("childpid=1235"));
        assert!(rendered.contains("childexe=/usr/bin/softIoc"));
        assert!(rendered.contains("childargs=-d test.db"));
    }
}
