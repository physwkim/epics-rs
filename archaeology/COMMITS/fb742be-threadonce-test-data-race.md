---
sha: fb742beae3f92b83251814dd000e52d93d4e4995
short_sha: fb742be
date: 2023-01-06
author: Andrew Johnson
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [epicsThreadOnce, test, data-race, mutex, sleep-race]
---
# epicsThreadOnceTest: main thread reads runCount without lock (data race)

## Root Cause
The test used `epicsThreadSleep(0.1)` as a synchronization barrier before reading `runCount`, then read the variable without holding `lock`. This is a classic sleep-based synchronization anti-pattern: the sleep is not guaranteed to be long enough on a loaded system, and the read of `runCount` outside the mutex constitutes a data race (undefined behavior in C11). On a fast machine the sleep may expire before all threads have incremented `runCount`.

## Symptoms
Intermittent test failures: `runCount == NUM_ONCE_THREADS` assertion fails on loaded systems or fast hosts where threads haven't finished by the time the sleep expires.

## Fix
- Replace the fixed sleep with a polling loop that calls `getRunCount()` (which takes the lock) until `runCount == NUM_ONCE_THREADS`.
- Read `doneCount` under the mutex before the assertion.
- Remove the now-redundant `runCount` test case (testPlan adjusted from 3 to 2).
Commit `fb742be`.

## Rust Applicability
Test-only fix. Rust's ownership/borrowing model prevents unsynchronized reads of shared counters at compile time. No production code affected.

## Audit Recommendation
No production audit needed. Pattern serves as a reminder that any Rust test using `std::thread::sleep` for synchronization should be replaced with `Condvar`/`Barrier`/`AtomicUsize`.

## C Locations
- `modules/libcom/test/epicsThreadOnceTest.c:MAIN(epicsThreadOnceTest)` — unsynchronized read of runCount
