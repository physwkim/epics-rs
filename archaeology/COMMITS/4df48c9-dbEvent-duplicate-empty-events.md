---
sha: 4df48c91f4e7202899bd859312a2b68148bf0bad
short_sha: 4df48c9
date: 2022-06-27
author: Michael Davidsaver
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_queue_event_log
tags: [dbEvent, flow-control, compaction, duplicate-events, queue]
---
# dbEvent queue accumulates duplicate reference-type events instead of compacting them

## Root Cause
When a field value changes rapidly, `db_queue_event_log` can be called while a
previous event for the same subscription (`evSubscrip`) is already pending in
the queue (`pevent->npend > 0`). If both the queued event and the new event are
reference-type logs (`dbfl_type_ref`, i.e., they reference the live record
field rather than owning a copy), they are effectively duplicates — the client
will only see the most-current value. The event-task does collapse consecutive
reference-type events at processing time, but the old code had removed the
early-exit compaction check from `db_queue_event_log`, allowing unbounded
accumulation of duplicate reference entries.

## Symptoms
Under rapid scan/posting rates, the dbEvent queue fills up with redundant
reference-type events. This leads to queue overflow (`queueOverflow` flag),
dropped events, and unnecessary memory pressure, while the CA/PVA client would
have received the same value anyway.

## Fix
Re-add the early-exit check at the top of `db_queue_event_log`:
if `npend > 0` and both the last queued log and the incoming log have no copy
(`!dbfl_has_copy`), free the incoming log and return without queuing. This
restores the compaction behavior that had been present in older EPICS versions.

## Rust Applicability
In base-rs `db_event.rs`, the `queue_event_log` function or equivalent
subscription queue logic should compact duplicate reference-type events:
if a subscription already has a pending event and both old and new events
reference the live field (no owned copy), the new event should be dropped
and the existing pending event left in place. Without this, high-rate PVs
can overflow mpsc queues.

## Audit Recommendation
In `db_event.rs::queue_event_log` (or `SubscriptionQueue::push`), check
whether compaction of consecutive reference-type events is implemented.
Specifically: `if pending_count > 0 && !last.has_copy() && !new.has_copy() { drop(new); return; }`.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_queue_event_log` — re-add duplicate reference-event compaction before enqueueing
