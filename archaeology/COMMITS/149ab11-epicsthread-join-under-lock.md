---
sha: 149ab1186ad6e8b8e7a8879bf40d56c905ccce3a
short_sha: 149ab11
date: 2018-04-21
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, race, mutex, epicsThread, lifecycle]
---

# Fix epicsThread::exitWait() to set joined flag under lock before pthread_join

## Root Cause
In `epicsThread::exitWait()`, the `joined = true` flag was set *after* `epicsThreadJoin()` returned. Two concurrent callers of `exitWait()` could both see `joined == false`, both enter the `epicsThreadJoin()` call, and double-join the same thread — which is undefined behavior (potential deadlock or crash on POSIX). Additionally, the late-exit path checked `!joined` but did not hold the lock when doing so, creating a TOCTOU race.

## Symptoms
Rare deadlock or crash when two threads concurrently call `exitWait()` on the same `epicsThread` object, or when a timeout-path and a success-path race in `exitWait()`.

## Fix
Set `joined = true` inside a lock guard *before* calling `epicsThreadJoin()`. Used `epicsGuardRelease` to drop the lock during the actual `pthread_join()` call (to avoid holding the lock across a blocking join). The late-exit path was also tightened to check `this->terminated && !joined` before acquiring the join.

## Rust Applicability
In Rust, `JoinHandle::join()` consumes the handle (moves it), making double-join structurally impossible. Tokio's `JoinHandle::await` is also a one-shot consuming operation. This entire race class is eliminated by Rust's ownership model.

## Audit Recommendation
None. Rust ownership prevents double-join races structurally.

## C Locations
- `modules/libcom/src/osi/epicsThread.cpp:epicsThread::exitWait` — `joined = true` moved inside lock guard before `epicsThreadJoin()`; late-exit path checks `this->terminated` first
