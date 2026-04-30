---
sha: 08b741ed056297d22665d576df6b7a4f0d5c1e8a
short_sha: 08b741e
date: 2021-04-19
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/repeater.rs
    function: start_repeater_if_not_installed
tags: [caRepeater, fallback, spawn-process, in-process-thread, lifecycle]
---

# CA Repeater: Fallback to In-Process Thread When exec Fails

## Root Cause
`caStartRepeaterIfNotInstalled()` attempted `osiSpawnDetachedProcess("caRepeater")`.
If the spawn returned `osiSpawnDetachedProcessNoSupport` (unsupported platform),
it fell back to an in-process thread. But if the spawn returned
`osiSpawnDetachedProcessFail` (executable not found / exec failure — common
when `caRepeater` is not in `$PATH`), it only printed a warning and returned
WITHOUT starting a thread. The IOC then operated without any CA repeater,
causing beacon registration to fail silently.

## Symptoms
On host targets where `caRepeater` is not in `$PATH` (e.g., embedded installs,
containers), CA clients never register with a repeater and never receive
server beacons. Discovery is broken; channels that could otherwise connect
fail to do so.

## Fix
Change the condition from `osptr == osiSpawnDetachedProcessNoSupport` (fallback
to thread only on unsupported-platform) to `osptr != osiSpawnDetachedProcessSuccess`
(fallback to thread on ANY non-success, including exec failure). Remove the
separate warning-only `osiSpawnDetachedProcessFail` branch.

The repeater process name is also changed to `"!CA Repeater"` (exclamation
prefix) to distinguish the fallback thread from a properly launched process in
diagnostic output.

## Rust Applicability
Applies. In `ca-rs`, the repeater startup logic should attempt to `spawn` the
`caRepeater` binary and fall back to an in-process `tokio::task` if the spawn
fails for any reason (not just `ENOSYS`). The `std::process::Command::spawn()`
error check should cover all error cases, not just "not supported".

## Audit Recommendation
In `ca-rs/src/client/repeater.rs`, verify that `start_repeater_if_not_installed()`
falls back to an in-process repeater task whenever `Command::spawn()` returns
`Err(...)`, including the common `NotFound` / `PermissionDenied` cases.
Check that the fallback task is actually spawned (not just a warning log).

## C Locations
- `modules/ca/src/client/udpiiu.cpp:caStartRepeaterIfNotInstalled` — changed condition to `!= osiSpawnDetachedProcessSuccess` for thread fallback
