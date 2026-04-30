---
sha: 379a68b93c8e4e4440de15392b1d58383ccfa24e
short_sha: 379a68b
date: 2021-05-02
author: Ralph Lange
category: timeout
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, timing, callback, imprecise-timing, sleep]
---

# simmTest: Replace Blind Sleep With Callback-Synchronized Wait

## Root Cause
`testSimmDelay()` in `simmTest.c` used `epicsThreadSleep(1.75 * delay)` to
wait for an asynchronous record processing callback to complete, then
surrounded the check with `testTodoBegin("imprecise")` / `testTodoEnd()` to
mark expected failures on targets with imprecise timing. The sleep-based
approach is inherently racy: on slow targets (VxWorks, RTEMS), `1.75 × 10ms`
may not be enough; on fast targets, it's wasteful.

## Symptoms
Intermittent `simmTest` failures on embedded targets with imprecise timers
(VxWorks, RTEMS, RTEMS-score). CI reports sporadic PACT=1 (record still active)
when the test expects PACT=0 (processing complete).

## Fix
Replace the sleep with a `callbackRequestDelayed(&cb, 1.5 * delay)` + 
`epicsEventWait(poked)`. The callback fires only after the delay, and the
`ping` callback function signals `poked`. This ensures the test proceeds only
after the exact callback-system delay has elapsed, regardless of platform
timer precision.

## Rust Applicability
Eliminated. In Rust tests using tokio, async delays are expressed as
`tokio::time::sleep(Duration::from_millis(...)).await`, which is tokio-timer-
backed and does not race against a fixed wall-clock sleep. The pattern of using
a `oneshot::channel` or `Notify` to synchronize test completion with an async
callback is idiomatic and not error-prone.

## Audit Recommendation
No audit needed. Confirm that Rust integration tests for async record
processing use `tokio::time::sleep` + awaiting a channel signal rather than
`std::thread::sleep` which could race.

## C Locations
- `modules/database/test/std/rec/simmTest.c:testSimmDelay` — replaced `epicsThreadSleep` with `callbackRequestDelayed` + `epicsEventWait`
