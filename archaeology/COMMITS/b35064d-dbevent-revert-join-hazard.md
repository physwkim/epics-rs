---
sha: b35064d26ccc2d94e95331225f8638a5f02777d0
short_sha: b35064d
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_close_events
tags: [dbEvent, shutdown, use-after-free, thread-sync, exit-semaphore]
---

# dbEvent: Revert join, Implement Safe Exit Semaphore Shutdown Protocol

## Root Cause
A previous attempt to join the dbEvent worker thread (commit ca2ea14) used
`epicsThreadMustJoin` but introduced a hazard: the worker thread's final
action was `epicsEventSignal(pexitsem)`, but if `db_close_events()` raced
ahead and called `epicsEventDestroy(pexitsem)` before the worker's
`epicsEventSignal` returned, the signal would operate on freed memory.

The fundamental issue: with a non-joinable thread, neither the worker nor the
caller can safely free `evUser`; both sides race for ownership.

## Symptoms
Intermittent crash or memory corruption during IOC shutdown: `epicsEventSignal`
on a destroyed semaphore, or `freeListFree(evUser)` while the worker thread is
still accessing `evUser` fields.

## Fix
Use a `stopSync` mutex to coordinate:

1. Worker thread: acquires `stopSync`, signals `pexitsem`, releases `stopSync`.
2. `db_close_events()`: waits on `pexitsem` (so worker has signaled), then
   acquires `stopSync` (so worker's `epicsEventSignal` has returned), then
   destroys `pexitsem` + other resources + `freeListFree(evUser)`.
3. `stopSync` itself is intentionally leaked (never freed) to avoid races on
   its own destruction.

The `pendexit` flag is set to `TRUE` at init so that if no worker thread
starts, `db_close_events()` skips the wait path entirely.

## Rust Applicability
Applies. This is the canonical "safe shutdown" problem for Rust async tasks:
a task that signals completion via a `Notify` or `oneshot::Sender` must not
have the Notify/channel destroyed before the signal completes. The Rust
solution is to use `tokio::sync::oneshot`: the worker sends `()` on
`tx: oneshot::Sender<()>`, the shutdown caller awaits `rx: oneshot::Receiver<()>`,
then drops shared state. The `oneshot` sender can be used even after the
receiver has been dropped (it just returns `Err`) — no destroy-before-signal
race.

## Audit Recommendation
In `base-rs/src/server/database/db_event.rs`, verify the shutdown handshake:
- Worker task signals completion via a `oneshot::Sender` or `Notify`.
- Shutdown path awaits the signal before dropping shared `Arc<EventUser>` state.
- No `Mutex`/`Condvar`/channel is destroyed before the worker task exits.
The Rust `JoinHandle::await` pattern (if `abort()` is not used) naturally
prevents the signal-before-destroy race if the JoinHandle is awaited.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_close_events` — use `stopSync` + `pexitsem` protocol instead of `epicsThreadMustJoin`
- `modules/database/src/ioc/db/dbEvent.c:event_task` — acquires `stopSync` before `epicsEventSignal(pexitsem)` to prevent destroy-before-signal race
- `modules/database/src/ioc/db/dbEvent.c:db_init_event_freelists` — initializes `stopSync` mutex
