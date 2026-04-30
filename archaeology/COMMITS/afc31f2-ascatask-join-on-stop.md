---
sha: afc31f2f064974e97ef61a9dc6cc58692a1b0a5f
short_sha: afc31f2
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, asCa, shutdown, joinable, lifecycle]
---
# asCaStop: join worker thread to prevent post-stop use-after-free

## Root Cause
`asCaStop()` waited on `asCaTaskWait` (a semaphore signalled when the worker
finishes its work item), released the lock, and returned — but the worker
thread was still alive. If `asCaStop()` was called as part of IOC teardown,
subsequent frees of shared state (HAG lists, CA context) could race with the
still-running worker.

## Symptoms
- Access-control worker thread may touch freed memory after `asCaStop()` returns.
- Latent crash at IOC shutdown when AS reconfiguration coincides with shutdown.

## Fix
Spawn `asCaTask` with `epicsThreadCreateOpt()` and `joinable=1`. After the
existing semaphore wait, call `epicsThreadMustJoin(threadid)` and clear
`threadid = 0` to prevent double-join.

## Rust Applicability
Eliminated. asCa is the C-layer access-security CA worker; base-rs uses Rust
ownership and tokio task handles for equivalent teardown.

## Audit Recommendation
No direct action. Confirm base-rs access-security shutdown path awaits any
background resolver tasks before freeing HAG/UAG structures.

## C Locations
- `modules/database/src/ioc/as/asCa.c:asCaStop` — added `epicsThreadMustJoin(threadid)` + clear
- `modules/database/src/ioc/as/asCa.c:asCaStart` — switched to `epicsThreadCreateOpt` with `joinable=1`
