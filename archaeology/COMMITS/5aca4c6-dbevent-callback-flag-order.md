---
sha: 5aca4c684cc87158ded4c4d4c3303b4c018e43fa
short_sha: 5aca4c6
date: 2023-09-13
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: event_read
tags: [callback, flag-order, cancel, race, dbEvent]
---

# dbEvent: clear callBackInProgress before signaling pflush_sem

## Root Cause
In `event_read()`, after the callback returned and the event queue lock was
re-acquired, the old code conditionally cleared `callBackInProgress` ONLY if
`user_sub == NULL && npend == 0u` (the cancel-pending path). The `else` branch
cleared the flag for the non-cancelled case. The ordering was:

```c
if (pevent->user_sub == NULL && pevent->npend == 0u) {
    pevent->callBackInProgress = FALSE;
    epicsEventSignal(pflush_sem);
} else {
    pevent->callBackInProgress = FALSE;
}
```

This meant the signal was emitted with the flag still `TRUE` in the moment
between the `if` check and the actual `epicsEventSignal`, because
`callBackInProgress` was set to `FALSE` inside the branch just before the
signal. However, `db_event_cancel()` would see `callBackInProgress == FALSE`
only after the signal, so if it had already woken up (spurious or racing),
it could see a stale state.

The specific bug: `callBackInProgress` should be cleared unconditionally
*before* checking whether to signal, to ensure the waiter in `db_event_cancel`
sees a consistent state.

## Symptoms
- In a cancel-while-callback-in-progress race: `db_event_cancel()` could read
  `callBackInProgress == TRUE` even after the event_task had finished the
  callback, causing an unnecessary extra wait cycle.
- In the worst case on weak-memory architectures, the signal could be emitted
  before the flag was visible as cleared, leading to a race where the cancel
  thread re-acquires the lock, checks `callBackInProgress`, and sees stale data.

## Fix
Move `callBackInProgress = FALSE` out of both branches so it is set
unconditionally before the `epicsEventSignal` decision:

```c
pevent->callBackInProgress = FALSE;
if (pevent->user_sub == NULL && pevent->npend == 0u) {
    epicsEventSignal(pflush_sem);
}
```

## Rust Applicability
In `base-rs`, any subscription delivery loop that sets a "callback in progress"
flag must clear it (with appropriate memory ordering — `Release` or
`SeqCst`) before signaling any waiting cancellation task. Use
`AtomicBool::store(false, Ordering::Release)` followed by a `Notify::notify_one()`.

## Audit Recommendation
- In `base-rs/src/server/database/db_event.rs`, verify that any "in-progress"
  atomic flag is cleared with `Release` ordering before the corresponding
  `Notify` or channel send that wakes the cancel waiter.
- Pattern: `flag.store(false, Release); notify.notify_one();` — never interleave
  notify before the store.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:event_read` — flag clear ordering fix
