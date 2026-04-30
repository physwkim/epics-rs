---
sha: f134abb84e9f6a475cae9cbed302d2e0fdf255bb
short_sha: f134abb
date: 2019-07-08
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [race, mutex, thread-join, check-then-act, concurrency]
---

# epicsThread data race on joined flag causes double-join

## Root Cause
`epicsThread::exitWait` contained a classic check-then-act race on the `joined`
boolean flag:

```cpp
// BEFORE (racy):
if(!joined) {
    {
        epicsGuard<epicsMutex> guard(this->mutex);
        joined = true;
    }
    epicsThreadMustJoin(this->id);
}
```

The check `if(!joined)` was performed *outside* the mutex lock. Two threads
calling `exitWait` concurrently could both see `joined == false`, both enter the
`if` body, and both call `epicsThreadMustJoin`. Calling `pthread_join` twice on
the same thread is undefined behavior (typically a crash or deadlock).

## Symptoms
Sporadic crashes or hangs on IOC shutdown when two threads simultaneously called
`exitWait` on the same `epicsThread` object — most commonly seen during
multi-threaded test teardown or IOC shutdown sequences.

## Fix
Read and set `joined` atomically under the mutex, then act on the *old* value
after releasing the lock:

```cpp
// AFTER (correct):
bool j;
{
    epicsGuard<epicsMutex> guard(this->mutex);
    j = joined;
    joined = true;
}
if(!j) {
    epicsThreadMustJoin(this->id);
}
```

This ensures exactly one caller observes `j == false` and performs the join.

## Rust Applicability
Rust's `JoinHandle` is owned and can only be joined once by moving it into
`join()`. The type system enforces single-join; no runtime flag is needed.
tokio `JoinHandle` is similarly owned. Eliminated.

## Audit Recommendation
No action needed in Rust. The ownership model prevents double-join entirely.
If any epics-rs code tracks "already joined" with a `bool` + `Mutex`, audit
for this exact check-then-act pattern.

## C Locations
- `modules/libcom/src/osi/epicsThread.cpp:epicsThread::exitWait` — race on `joined` flag
