---
sha: f902d7000600f8a7f03274e21eb853828f8ee5f2
short_sha: f902d70
date: 2022-07-30
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread-creation, joinable, callback, refactor, prerequisite]
---
# callback.c: switch thread creation to epicsThreadCreateOpt()

## Root Cause
`callback.c:callbackInit()` created callback threads using the legacy
`epicsThreadCreate()` API, which does not expose the `joinable` option. The
companion fix `2ff44cb` (which adds actual thread joining in `callbackStop()`)
requires threads to be created with `joinable=1` via `epicsThreadCreateOpt()`.

This commit is a preparatory refactor: it replaces `epicsThreadCreate()` with
`epicsThreadCreateOpt()` using `opts.joinable = 0` (still non-joinable at
this point). The next commit `2ff44cb` then sets `joinable = 1` and adds the
join calls.

## Symptoms
Not a direct bug fix — this is a prerequisite refactor. No behavior change.

## Fix
Replaced `epicsThreadCreate(name, priority, stackSize, func, arg)` with:
```c
epicsThreadOpts opts = EPICS_THREAD_OPTS_INIT;
opts.joinable = 0;
opts.priority = threadPriority[i];
opts.stackSize = epicsThreadStackBig;
tid = epicsThreadCreateOpt(name, func, arg, &opts);
```

## Rust Applicability
In Rust, `tokio::spawn` always returns a `JoinHandle`. There is no separate
"create with join support" step. Entirely eliminated.

## Audit Recommendation
None — eliminated by Rust's task model.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackInit` — epicsThreadCreate → epicsThreadCreateOpt
