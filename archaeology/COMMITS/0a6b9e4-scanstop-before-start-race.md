---
sha: 0a6b9e4511ea455625566b41627e991ca0fa6e13
short_sha: 0a6b9e4
date: 2024-06-14
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/scan.rs
    function: scan_stop
tags: [lifecycle, scanStop, scanStart, ordering, ctlInit, crash]
---

# scanStop() Before scanStart() Causes Crash or Hang

## Root Cause
`dbScan.c:scanStop()` checked only `if (scanCtl == ctlExit) return;` before
proceeding to stop all periodic scan tasks. If `scanStop()` was called before
`scanStart()` had ever been called (i.e., `scanCtl` was still `ctlInit`), the
function would attempt to signal semaphores for scan tasks that had never been
created, causing a crash or hang in the epicsEventWait calls below.

The state machine was:
- `ctlInit` → initial state (no tasks spawned yet)
- `ctlRun` → after `scanStart()`
- `ctlExit` → after first `scanStop()`

The guard only prevented double-stop (`ctlExit`) but not pre-start-stop
(`ctlInit`).

## Symptoms
- IOC crash or hang if `scanStop()` was called during error handling before
  the IOC was fully initialized (e.g., during `iocInit` failure path).
- Manifested as a deadlock waiting on epicsEvents for never-started scan tasks.

## Fix
Added `ctlInit` to the early-return guard:
```c
if (scanCtl == ctlInit || scanCtl == ctlExit) return;
```

## Rust Applicability
Applies. The base-rs scan subsystem should enforce the same state machine: a
`scan_stop()` call issued before `scan_start()` must be a no-op. If the Rust
scan task manager uses `Option<JoinHandle>` or a `ScanState` enum, verify that
a `stop()` on an un-started scanner does not panic or deadlock.

## Audit Recommendation
In `base-rs/src/server/database/scan.rs:scan_stop` (or equivalent): verify
that calling `stop()` before `start()` is handled gracefully (returns early
rather than unwrapping a None JoinHandle or awaiting a channel that was never
opened). Check for `unwrap()` calls on `Option<JoinHandle>` fields.

## C Locations
- `modules/database/src/ioc/db/dbScan.c:scanStop` — missing `ctlInit` check allowed pre-start stop
