---
sha: 86a942872aea66e06cc8aa5478da74f8f5d3fb77
short_sha: 86a9428
date: 2019-06-23
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, thread-join, sleep-removal, race, lifecycle]
---
# epicsThreadTest: use explicit join instead of sleep to await threads

## Root Cause
The thread test used `epicsThreadSleep(2.0)` after spawning test threads, then
checked results. If the machine was loaded and threads hadn't run yet, the
results would be wrong (flaky). More critically, the threads were not joinable,
so there was no way to be certain they had completed.

## Symptoms
- Flaky tests: thread body (`run()`) sometimes didn't execute before the sleep
  expired, especially under heavy CI load.
- Race between thread checking `pinfo->isOkToBlock` and main reading it.

## Fix
Create threads with `joinable=1` via `epicsThreadCreateOpt`. Remove all
`epicsThreadSleep` calls from thread bodies. Add `didSomething` flag to
`info` struct. After spawning, call `epicsThreadMustJoin` (joining B before A
to increase detection chance), then check `didSomething`.

## Rust Applicability
Eliminated. Test infrastructure. Rust tests use `JoinHandle::await` or
`thread::join()` natively; no sleep-based synchronization needed.

## Audit Recommendation
None required. Test-only change.

## C Locations
- `modules/libcom/test/epicsThreadTest.cpp:MAIN(epicsThreadTest)` — replaced sleeps with joinable threads + MustJoin
