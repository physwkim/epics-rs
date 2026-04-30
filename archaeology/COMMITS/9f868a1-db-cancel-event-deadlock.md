---
sha: 9f868a107461b652280721dffdc8b592b9269270
short_sha: 9f868a1
date: 2023-10-23
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_cancel_event
tags: [deadlock, event-queue, cancel, concurrent, condition-variable]
---

# Concurrent db_cancel_event causes hang via shared flush semaphore

## Root Cause
`db_cancel_event()` needed to synchronize with the event task: it waited on `pflush_sem` (a single global semaphore) for the worker to complete a cycle (detected via `pflush_seq` counter change). When multiple threads concurrently called `db_cancel_event()`, they all waited on the same `pflush_sem`. The fix for concurrent cancellation was a re-trigger hack: each waiter unconditionally re-triggered `pflush_sem` after waking, to ensure other waiters would also wake. This "thundering herd" pattern caused one spurious wakeup per cancel, and under heavy concurrency, the re-trigger chain could cause cancels to "miss" the sequence counter change and spin or deadlock.

## Symptoms
Hang in `db_cancel_event()` when multiple threads concurrently cancel subscriptions. The worker task completes its cycle, increments `pflush_seq`, and signals `pflush_sem` once — but multiple waiters each consume/re-post the semaphore in a race, and some waiters never observe the sequence counter change.

## Fix
Replaced the single `pflush_sem` with a per-waiter `event_waiter` struct containing its own `epicsEventId wake`. Waiters register themselves in `evUser->waiters` (an `ELLLIST`) before waiting, and the event task iterates all registered waiters to trigger each one individually after incrementing `pflush_seq`. Waiter registration/deregistration is serialized under `evUser->lock` to prevent the trigger-destroy race.

## Rust Applicability
Applies. `base-rs` implements a database event/subscription system. The equivalent of `db_cancel_event` (subscription cancellation with synchronization) needs to correctly handle concurrent cancellations. If a single `Notify` or `oneshot::Sender` is used as the flush signal, the same race applies. Each concurrent cancel must have its own waker/notify.

## Audit Recommendation
In `base-rs/src/server/database/db_event.rs`, audit the subscription cancellation path:
1. Verify that concurrent `db_cancel_event()` calls each get their own `tokio::sync::Notify` or `oneshot::channel`.
2. The event worker must signal ALL pending cancellers after each cycle, not just one.
3. The waker must not be destroyed while the worker may still be signaling it (hold the lock across trigger+destroy as in the C fix).

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_cancel_event` — replaced `pflush_sem` wait with per-waiter `event_waiter` node
- `modules/database/src/ioc/db/dbEvent.c:event_task` — iterates `evUser->waiters` to trigger all per-waiter events
- `modules/database/src/ioc/db/dbEvent.c:event_user` — removed `pflush_sem`, added `ELLLIST waiters`
