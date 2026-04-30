---
sha: 37a76b433a9e7d5a8d26a13fd21ad62f20a0c1c1
short_sha: 37a76b4
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, event-task, shutdown, semaphore, lifecycle]
---
# dbEvent db_close_events(): replace pexitsem signal/wait with thread join

## Root Cause
`db_close_events()` used a hand-rolled exit-synchronization protocol: a
`pexitsem` semaphore (signalled by the worker at exit), a global `stopSync`
mutex (to prevent the semaphore from being destroyed before the signal returns),
and a `pendexit` flag initialized to TRUE (cleared when the task starts).
This is a textbook example of re-implementing join with semaphores — but with
a race: if `db_close_events()` was called before the worker had cleared
`pendexit`, the caller would skip the wait entirely, then destroy the semaphore
while the worker was still alive and about to signal it.

## Symptoms
- Worker thread could use a destroyed semaphore (`pexitsem`) after the caller of
  `db_close_events()` freed it, causing silent corruption or crash at shutdown.
- `stopSync` mutex intentionally leaked ("intentionally leak to avoid shutdown
  races") — a design smell acknowledging the underlying fragility.

## Fix
Switch `db_start_events()` to use `epicsThreadCreateOpt()` with `joinable=1`.
Remove `pexitsem`, `stopSync`, and the `pendexit` initialization dance.
`db_close_events()` now sets `pendexit`, signals `ppendsem`, then calls
`epicsThreadMustJoin(evUser->taskid)`. The worker destructs `evUser` itself
(freed inside the worker after clearing its own resources) so no double-free
is possible.

## Rust Applicability
Eliminated. In ca-rs/base-rs the event-task pattern is replaced by tokio tasks
(`JoinHandle`). Dropping or aborting a `JoinHandle` plus `.await`ing the handle
is the exact analogue of `epicsThreadMustJoin`. The semaphore-based shutdown
race cannot occur in async Rust.

## Audit Recommendation
No direct action needed. If any ca-rs/base-rs shutdown path uses a manual
"done" channel + `recv()` instead of `JoinHandle::await`, verify it cannot
race on the sender being dropped before `recv()` returns.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_close_events` — replaced semaphore wait with `epicsThreadMustJoin`
- `modules/database/src/ioc/db/dbEvent.c:db_start_events` — switched to `epicsThreadCreateOpt` with `joinable=1`
- `modules/database/src/ioc/db/dbEvent.c:event_task` — now frees `evUser` and destroys resources before returning
