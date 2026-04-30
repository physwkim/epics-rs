---
sha: 271f20faa05a3fe1e79ede7532de98db6776de0a
short_sha: 271f20f
date: 2025-08-27
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/event.rs
    function: db_flush_extra_labor_event
tags: [dbEvent, extra-labor, synchronization, busy-wait, race]
---
# dbEvent: expand synchronization — fix busy-wait and labor-pending race

## Root Cause
Two related bugs in `dbEvent.c`:

**Bug 1 — `db_flush_extra_labor_event` busy-waits on `extraLaborBusy`:**
The original implementation polled `extraLaborBusy` with a 100ms sleep
while holding and releasing the lock. This was a classic busy-wait antipattern
that wasted CPU and had a subtle race: `extraLaborBusy` was set to `TRUE`
before the labor function was dispatched, but the check `while (extraLaborBusy)`
could exit early if the worker had not yet set `extraLaborBusy = TRUE`
(before starting the labor), missing a pending labor cycle entirely.

**Bug 2 — `extraLaborBusy` set before `extra_labor` cleared:**
In `event_task`, the code set `extraLaborBusy = TRUE` and read
`pExtraLaborSub = extralabor_sub` before clearing `extra_labor = FALSE`.
A concurrent `db_flush_extra_labor_event` could observe `extraLaborBusy`
becoming `FALSE` after the labor completed, then return — but the next
`db_post_extra_labor` call could have set `extra_labor = TRUE` again during
the labor execution, leaving unprocessed pending labor.

## Symptoms
- `db_flush_extra_labor_event` returns before the labor function has actually
  executed, causing callers (e.g., `db_cancel_event` synchronization) to
  observe stale state from the previous cycle.
- Monitor callbacks observed after `db_cancel_event` returns (use-after-free
  in the subscription struct).
- 100ms sleep visible in IOC shutdown latency.

## Fix
Extract `db_sync_event()` helper that:
1. Adds an `event_waiter` to `evUser->waiters` list.
2. Signals `ppendsem` to wake the worker.
3. Waits on a per-waiter `epicsEvent` (not a sleep).
4. Loops until `pflush_seq` changes (the worker increments this at each cycle).

Replace the `extraLaborBusy` busy-wait in `db_flush_extra_labor_event` with
a call to `db_sync_event` when labor is pending (`extraLaborBusy ||
(extra_labor && extralabor_sub)`).

Move `extra_labor = FALSE` to before `extraLaborBusy = TRUE` is set, so the
flag accurately reflects whether additional labor has been queued since the
current cycle started.

`db_cancel_event` now also uses `db_sync_event` through the same helper.

## Rust Applicability
`applies` — base-rs's event system (`event.rs`) likely has an analogous
`flush_extra_labor` or `cancel_event` synchronization path. Verify:
1. No busy-wait (sleep loop) on a "labor busy" flag.
2. `cancel_subscription` properly awaits a full worker cycle before returning
   (to prevent use-after-free of the subscription struct).
3. If extra labor can be posted concurrently with a flush, the flush correctly
   waits for any labor that was pending at the time of the call.

## Audit Recommendation
Audit `base-rs/src/server/database/event.rs::db_flush_extra_labor_event` and
`db_cancel_event` (or their Rust equivalents) for sleep-based polling and for
the race where a pending-labor flag check exits before the labor has actually
completed.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_flush_extra_labor_event` — 100ms sleep busy-wait on extraLaborBusy
- `modules/database/src/ioc/db/dbEvent.c:event_task` — extraLaborBusy set before extra_labor cleared
- `modules/database/src/ioc/db/dbEvent.c:db_cancel_event` — inline sync loop now factored into db_sync_event
