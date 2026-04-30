---
sha: 1d056c6fe42bc645f50c823d8b5861faa2f1b554
short_sha: 1d056c6
date: 2022-12-06
author: Doug Murray
category: timeout
severity: low
rust_verdict: partial
audit_targets:
  - crate: ca-rs
    file: src/tools/tool_lib.rs
    function: use_ca_timeout_env
tags: [timeout, env-var, ca-tools, epics-cli-timeout, default-timeout]
---
# CA Command-Line Tools Ignore EPICS_CLI_TIMEOUT Environment Variable

## Root Cause
The CA command-line tools (`caget`, `caput`, `camonitor`, `cainfo`) all
support a `-w <seconds>` flag to override the default connection timeout.
However, there was no mechanism to set the timeout from the environment,
requiring every operator invocation to pass `-w` explicitly.

Additionally, the `-w` parsing had a bug: when `epicsScanDouble` failed to
parse the user-supplied string, the code reset `caTimeout = DEFAULT_TIMEOUT`
rather than preserving the value already set (which could be a valid env-var
value). This meant that a bad `-w` argument after a valid `EPICS_CLI_TIMEOUT`
env var would silently revert to the hardcoded default.

## Symptoms
- No way to configure per-site/per-user default timeout without modifying
  startup scripts to add `-w N` to every CA tool invocation.
- A bad `-w` argument after setting `EPICS_CLI_TIMEOUT` silently reset the
  timeout to 1.0s.

## Fix
- Added `use_ca_timeout_env(double *timeout)` to `tool_lib.c` which reads
  `EPICS_CLI_TIMEOUT` from the environment and calls `epicsScanDouble`.
- Each tool calls `use_ca_timeout_env(&caTimeout)` before `getopt` parsing
  (so command-line `-w` overrides the env var).
- Error message for invalid `-w` now shows the current (preserved) timeout
  rather than assuming DEFAULT_TIMEOUT.
- Documented in `epicsStdlib.h` that `epicsScanDouble` only modifies the
  target on successful conversion.

## Rust Applicability
`partial` — ca-rs CLI tools should implement equivalent env-var priority:
`EPICS_CLI_TIMEOUT` → default timeout, overridable by `-w`. Rust equivalent
is `std::env::var("EPICS_CLI_TIMEOUT").ok().and_then(|s| s.parse().ok())`.
The priority chain (env-var first, then CLI override) should be preserved.

## Audit Recommendation
In `ca-rs` tool implementations, check for `EPICS_CLI_TIMEOUT` env var
before processing command-line arguments. Ensure that a parse failure for
`-w` preserves the env-var-derived timeout rather than reverting to a
hardcoded default.

## C Locations
- `modules/ca/src/tools/tool_lib.c:use_ca_timeout_env` — new function reading `EPICS_CLI_TIMEOUT`
- `modules/ca/src/tools/caget.c:main` — calls `use_ca_timeout_env` before `getopt`
- `modules/ca/src/tools/caput.c:main` — same
- `modules/ca/src/tools/camonitor.c:main` — same
- `modules/ca/src/tools/cainfo.c:main` — same
