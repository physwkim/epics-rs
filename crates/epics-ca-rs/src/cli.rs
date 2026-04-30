//! Helpers shared across the `caget` / `caput` / `cainfo` / `camonitor`
//! command-line binaries.

/// Read `EPICS_CLI_TIMEOUT` from the environment, falling back to 1.0 s
/// when unset or unparsable. Mirrors C `tool_lib.c:use_ca_timeout_env`
/// (commit 1d056c6) — the env var is consulted only when the caller
/// did not pass `-w`/`--wait` on the command line.
pub fn env_default_timeout() -> f64 {
    std::env::var("EPICS_CLI_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0)
}
