---
sha: 839f764bcb37a628b22ee988460fb4e8f04de27c
short_sha: 839f764
date: 2024-03-22
author: Simon Rose
category: leak
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [leak, taskwdRemove, watchdog, perpetual-thread, asCa, rsrv]
---

# Missing taskwdRemove in Perpetual Threads Leaks Watchdog Registration

## Root Cause
Three perpetual server threads (`asCaTask`, `req_server`, `rsrv_online_notify_task`)
registered with the EPICS task watchdog (`taskwdInsert`) at startup but never
called `taskwdRemove` at exit. Since these threads are "perpetual" (they only
exit when the process exits), this was not an operational problem. However,
for future-proofing and in test environments where the IOC may be
initialized/shutdown multiple times in one process, the watchdog entries would
accumulate as leaks.

Additionally, for defensive programming, `cantProceed("Unreachable...")` was
added to document that these exit paths are never expected to be reached in
normal operation.

## Symptoms
- No runtime crash or functional failure in production.
- In test environments with repeated IOC init/shutdown cycles, the watchdog
  task list would grow without bound (one entry per init cycle per thread).

## Fix
Added `taskwdRemove(0)` after the unreachable `cantProceed` call in each
perpetual thread function for completeness.

## Rust Applicability
Eliminated. In ca-rs and base-rs, server tasks are Tokio async tasks managed
by `JoinHandle`. There is no watchdog registration system; task lifecycle is
managed by `tokio::select!` and `CancellationToken`. Dropping/aborting a
JoinHandle does not leak.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/as/asCa.c:asCaTask` — missing `taskwdRemove`
- `modules/database/src/ioc/rsrv/caservertask.c:req_server` — missing `taskwdRemove`
- `modules/database/src/ioc/rsrv/online_notify.c:rsrv_online_notify_task` — missing `taskwdRemove`
