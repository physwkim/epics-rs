---
sha: e1c1bb8b1bacce82c58e133fd34182e5166b38f0
short_sha: e1c1bb8
date: 2023-01-22
author: Michael Davidsaver
category: race
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/event.rs
    function: event_read
tags: [race, event-queue, canceled-events, eventsRemaining, lock]
---
# dbEvent: correct eventsRemaining count — skip canceled events

## Root Cause
In `dbEvent.c:event_read()`, the `eventsRemaining` argument passed to the
user callback was computed as:
```c
ev_que->evque[ev_que->getix] != EVENTQEMPTY
```
This was evaluated **while holding the queue lock** but AFTER `event_remove()`
advanced the get index. The check inspects the next slot but does not account
for canceled events (`nCanceled`). If the next slot contains a canceled event,
`eventsRemaining` would be `1` even though the next callback call would
immediately skip it — creating a false "more events" signal.

Furthermore, the original code re-read `ev_que->evque[ev_que->getix]` and
`ev_que->nCanceled` without a lock (there was a `UNLOCKEVQUE` before the
callback and a `LOCKEVQUE` after). The peek at the next slot happened outside
the lock window.

## Symptoms
- Consumer callbacks are told `eventsRemaining=1` but the queue only contains
  canceled events — the consumer flushes unnecessarily.
- Conversely, when many events are canceled, `eventsRemaining` may be stale,
  causing flow-control signals to be incorrect and potentially contributing to
  queue stalls (see b6626e4).

## Fix
Captured `eventsRemaining` into a local variable immediately after
`event_remove()` / `RNGINC()` (while the queue is still locked):
```c
eventsRemaining = ev_que->evque[ev_que->getix] != EVENTQEMPTY && !ev_que->nCanceled;
```
Then passed this snapshot to the callback. The `nCanceled` guard ensures that
a queue of only canceled events does not assert `eventsRemaining`.

## Rust Applicability
A Rust event queue using `mpsc` or `VecDeque` behind a `Mutex` would compute
the "more items" flag inside the lock, naturally snapshotting the value before
releasing. The pattern of peeking at a queue outside the lock (as the original
C code did by re-reading after `UNLOCKEVQUE`) would be a data race in safe
Rust (not expressible without `unsafe`). However:
- A Rust implementation should verify that "has more items" is computed
  atomically with item removal, not re-read post-release.
- Canceled subscriptions should be excluded from the `eventsRemaining`
  computation.

## Audit Recommendation
In `base-rs` event dispatch: confirm that the "more items" signal to the
CA/PVA server flush logic is snapshotted inside the same mutex guard as the
item dequeue, and that logically-canceled subscriptions are excluded from
the count.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:event_read` — eventsRemaining snapshotted under lock, nCanceled guard added
