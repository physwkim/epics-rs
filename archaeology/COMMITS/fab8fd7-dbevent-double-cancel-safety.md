---
sha: fab8fd7102143a661a81cf7163dd889601eb015c
short_sha: fab8fd7
date: 2023-09-14
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_cancel_event
tags: [double-cancel, use-after-free, event-subscription, lifecycle, dbEvent]
---

# dbEvent: handle multiple db_event_cancel() calls safely

## Root Cause
`db_event_cancel()` in `dbEvent.c` had two intertwined problems:

1. **Use-after-free on double cancel**: The first call freed the `evSubscrip` via
   `freeListFree()`. A second concurrent or sequential call re-entered the same
   pointer, reading freed memory (`pevent->npend`, `pevent->callBackInProgress`).

2. **Suicide-event race**: The old design used a `pSuicideEvent` pointer on
   `event_user` to hand off deallocation to the event_task. This created a window
   where `event_read()` could run the callback (`callBackInProgress = TRUE`),
   unlock, and the cancel path would free the memory, leaving the task with a
   dangling pointer when it re-locked and read `ev_que->evUser->pSuicideEvent`.

3. **Incorrect `nCanceled` tracking**: A sentinel `canceledEvent` entry was used
   in the ring buffer to mark canceled slots. The counter `nCanceled` affected
   `eventsRemaining`, causing spurious "no more events" signals.

## Symptoms
- Crash or memory corruption when `db_cancel_event()` was called twice on the
  same subscription (e.g. from different threads or on cleanup paths).
- `callBackInProgress` flag read after `freeListFree()` → heap corruption.
- Silent double-free on subscriptions that were pending in the ring buffer at
  cancellation time.

## Fix
Replaced the suicide-event mechanism with a two-phase deferred free:

- Cancel sets `user_sub = NULL` and checks `callBackInProgress`/`npend`:
  - If callback in progress: set `sync=1`, wait for `pflush_sem` (keyed on a
    monotonic `pflush_seq` counter so spurious wakeups are harmless).
  - If `npend > 0`: defer free to `event_read()` (which will call
    `freeListFree` after draining the queue).
  - Otherwise: free immediately under the lock.
- `event_task` increments `pflush_seq` and signals `pflush_sem` after each
  full iteration of event queues, providing the synchronization point.
- `event_read()` checks `user_sub == NULL && npend == 0` after each callback
  and calls `freeListFree()` inline — no more sentinel entries or `nCanceled`.

## Rust Applicability
In `base-rs`, if `db_event.rs` implements a subscription cancel path, it must
guard against:
- Double-cancel: canceling a subscription that is already being torn down.
- Callback-in-progress: the async task delivering callbacks must not read
  subscription state after the subscription is dropped.

The Rust analog is an `Arc<Mutex<SubscriptionState>>` where the cancel sets a
`cancelled` flag and the delivery task checks it before/after each callback.
If a `tokio::task` is used for delivery, `task.abort()` + `JoinHandle::await`
provides the equivalent of the `pflush_sem` wait.

## Audit Recommendation
- Audit `base-rs/src/server/database/db_event.rs` for any subscription cancel
  path that frees state while the delivery task may still reference it.
- Check that `CancellationToken` or equivalent is used so that a second
  cancel is a no-op.
- Verify that the delivery task does not hold an `Arc` clone into subscription
  state after the subscription is logically cancelled.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_event_cancel` — double-free / UAF fix
- `modules/database/src/ioc/db/dbEvent.c:event_read` — deferred-free after callback
- `modules/database/src/ioc/db/dbEvent.c:event_task` — pflush_seq + pflush_sem signal
- `modules/database/src/ioc/db/dbChannel.h:evSubscrip` — added callBackInProgress semantics
