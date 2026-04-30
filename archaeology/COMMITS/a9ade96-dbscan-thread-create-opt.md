---
sha: a9ade9669ac27ee982b432e3a4e3e9fdb751284f
short_sha: a9ade96
date: 2022-07-30
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread-creation, joinable, dbscan, refactor, prerequisite]
---
# dbScan: switch thread creation to epicsThreadCreateOpt()

## Root Cause
`dbScan.c` created scan threads using the legacy `epicsThreadCreate()` API,
which does not support the `joinable` option. Without the `Opts`-capable API,
it was impossible to mark scan threads as joinable (required for
`epicsThreadMustJoin()` to work in the companion fix `bded79f`).

This commit is a preparatory refactor: it switches `initOnce()` and
`spawnPeriodic()` to use `epicsThreadCreateOpt()` with `opts.joinable = 0`
(still not joinable yet). The subsequent commit `bded79f` then sets
`joinable = 1` and adds the actual join calls.

## Symptoms
Not a direct bug — this is a prerequisite refactor to enable later joinability
fixes. Without this change the later `epicsThreadMustJoin()` calls could not
be added.

## Fix
Replaced `epicsThreadCreate(name, priority, stackSize, func, arg)` with
`epicsThreadCreateOpt(name, func, arg, &opts)` where `opts` is initialized via
`EPICS_THREAD_OPTS_INIT` with explicit `priority`, `stackSize`, and
`joinable=0` fields. This is a no-op behavior change but enables future
joinability.

## Rust Applicability
Rust `tokio::spawn` / `std::thread::spawn` always return a `JoinHandle` that
can be joined or aborted. There is no concept of a "non-joinable" thread in
safe Rust (unlike detached pthreads). This pattern is entirely eliminated.

## Audit Recommendation
None — eliminated by Rust's task/thread model. All spawned tasks return handles.

## C Locations
- `modules/database/src/ioc/db/dbScan.c:initOnce` — epicsThreadCreate → epicsThreadCreateOpt
- `modules/database/src/ioc/db/dbScan.c:spawnPeriodic` — epicsThreadCreate → epicsThreadCreateOpt
