---
sha: ca2ea14082bdadb3ea8f7b7ad967fd42b41e6a41
short_sha: ca2ea14
date: 2021-04-02
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_close_events
tags: [dbEvent, thread-join, shutdown, lifecycle, worker-thread]
---

# dbEvent: Worker Thread Must Be Joined on Close

## Root Cause
`db_close_events()` sent a shutdown signal to the event worker thread
(`evUser->pendexit = TRUE` + `epicsEventSignal(ppendsem)`) but did NOT join
the thread before freeing the `evUser` struct. If the worker thread was still
running — reading from `evUser` fields — when the caller freed the struct,
this was a use-after-free.

The original code used `epicsThreadCreate` (non-joinable thread) so there was
no join mechanism; the worker thread owned its own cleanup and freed `evUser`
itself. This is an anti-pattern: the caller has no guarantee the thread has
exited before it proceeds.

## Symptoms
Use-after-free of `evUser` fields in the event worker thread during IOC
shutdown. Race between `db_close_events()` freeing `evUser` and the worker
thread's final reads of `evUser->pendexit`. In practice masked by timing but
can manifest as crashes or memory corruption under ASAN.

## Fix
Switch to `epicsThreadCreateOpt` with `opts.joinable = 1`. Add
`epicsThreadMustJoin(evUser->taskid)` in `db_close_events()` after
`epicsEventMustWait(pexitsem)`.

Note: This commit was later reverted (see b35064d) due to a discovered
threading hazard when joining; a more careful synchronization scheme using
`stopSync` mutex + exit semaphore was substituted.

## Rust Applicability
Applies. In base-rs, the dbEvent worker is likely a `tokio::task::JoinHandle`.
`db_close_events()` should call `join_handle.await` (or `abort()` +
`join_handle.await`) to ensure the task has fully exited before dropping the
shared state. If the JoinHandle is simply dropped without `abort()`, the task
continues to run and may access freed/dropped state.

## Audit Recommendation
In `base-rs/src/server/database/db_event.rs`, verify that `db_close_events()`
(or the equivalent `Drop` impl) awaits the worker JoinHandle. Confirm the
JoinHandle is not just dropped — tokio tasks are not automatically cancelled
on JoinHandle drop (unlike `abort()`).

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_close_events` — added `epicsThreadMustJoin`
- `modules/database/src/ioc/db/dbEvent.c:db_start_events` — switched to `epicsThreadCreateOpt` with `joinable=1`
