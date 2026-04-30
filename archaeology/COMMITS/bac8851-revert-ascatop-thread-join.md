---
sha: bac8851132b28579dd9d85a3f0deed08f8d9a0b1
short_sha: bac8851
date: 2020-03-23
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/as_ca.rs
    function: as_ca_stop
tags: [thread-join, lifecycle, deadlock, shutdown, access-security]
---

# Revert asCaStop() thread join to avoid deadlock on shutdown

## Root Cause
A prior commit made `asCaTask` joinable and added `epicsThreadMustJoin(threadid)`
to `asCaStop()`. The `asCaTask` thread processes CA monitor events for access
security rules. During IOC shutdown, `asCaStop()` is called from a context that
may hold resources that `asCaTask` is waiting on (e.g., the asCaTaskLock mutex
or the CA client context). This caused a deadlock: the shutdown thread blocked
in `epicsThreadMustJoin`, while `asCaTask` blocked waiting for a resource held
by the shutdown thread.

The joinable thread also introduced a subtle issue: if `asCaStop()` was called
before `asCaStart()` had created the thread (e.g., in error recovery paths),
`epicsThreadMustJoin(0)` would invoke undefined behavior.

## Symptoms
- IOC hangs indefinitely during `iocShutdown()` in the `asCaStop()` call.
- Deadlock detected by EPICS watchdog or operator intervention required.
- IOC must be killed (SIGKILL) rather than cleanly exiting.

## Fix
Reverted the commit: replaced `epicsThreadCreateOpt` (joinable) back to
`epicsThreadCreate` (non-joinable), and removed `epicsThreadMustJoin(threadid)`
from `asCaStop()`. The stop function now uses the existing event-signaling
pattern (`epicsEventMustWait(asCaTaskWait)`) to synchronize with the thread
reaching its idle state, without requiring a full join.

## Rust Applicability
Applies. In base-rs, any async task that is cancelled/joined during shutdown
must not hold (or wait on) resources owned by the shutdown caller. The pattern
of joining a task from within a context that the task is waiting on is a
classic async deadlock:

```rust
// DEADLOCK RISK:
let guard = mutex.lock().await;          // shutdown holds mutex
as_ca_handle.await;                      // waits for task to finish
// Meanwhile: as_ca_task waits for mutex.lock().await -- deadlock
```

Use `tokio::task::JoinHandle::abort()` instead of `.await` during shutdown,
or ensure the task is in a cancellation-safe await point before joining.

## Audit Recommendation
In `base-rs/src/server/database/as_ca.rs:as_ca_stop`, verify that:
1. The access security CA task is not joined while the caller holds any mutex
   that the task may be waiting on.
2. Prefer `handle.abort()` for hard-stop, or a dedicated shutdown signal
   (e.g., `CancellationToken`) that allows the task to exit cleanly before
   the join.
3. Check all other task joins in the IOC shutdown sequence for the same
   deadlock pattern.

## C Locations
- `modules/database/src/ioc/as/asCa.c:asCaStop` — removed epicsThreadMustJoin, reverted to non-joinable thread
- `modules/database/src/ioc/as/asCa.c:asCaStart` — reverted epicsThreadCreateOpt back to epicsThreadCreate
