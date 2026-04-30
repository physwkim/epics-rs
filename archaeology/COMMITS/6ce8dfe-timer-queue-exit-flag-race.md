---
sha: 6ce8dfec01cdfc2b3ebd9f2a506cf92312bede47
short_sha: 6ce8dfe
date: 2017-11-08
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [race, atomics, timer, exitFlag, shutdown]
---
# timerQueueActive exitFlag is plain bool with data race on multi-threaded shutdown

## Root Cause
`timerQueueActive` used `bool exitFlag` (a plain non-atomic bool) that was
written by the timer worker thread (`run()`) and read by the destructor
(`~timerQueueActive()`) on a different thread. This is a data race per C++11
memory model. TSan would flag the spin loop `while (!this->exitFlag)` in the
destructor as accessing a shared variable without synchronization.

## Symptoms
- TSan data race report on `exitFlag`.
- Without sanitizers: on architectures with weak memory ordering, the
  destructor's spin loop could see a stale `false` and spin indefinitely after
  `run()` had written `true` (though unlikely on x86 due to TSO).

## Fix
Change `exitFlag` from `bool` to `int` and use `epics::atomic::get/set`
(which wraps C11 `_Atomic` or `__atomic_*` builtins) for all accesses.
The destructors spin uses `epics::atomic::get(exitFlag)` and `run()` uses
`epics::atomic::set(exitFlag, 1)`.

## Rust Applicability
Rust's `timerQueueActive` equivalent is a `tokio::time::interval`-driven
`JoinHandle`. The exit mechanism is via `JoinHandle::abort()` or a
`CancellationToken`. There is no `exitFlag` spin loop; the tokio runtime
handles shutdown ordering. This pattern is structurally eliminated.

## Audit Recommendation
No direct Rust audit needed. If any custom timer thread in base-rs or ca-rs
implements a shutdown flag without `AtomicBool`, audit for this pattern.

## C Locations
- `modules/libcom/src/timer/timerPrivate.h:timerQueueActive` — `exitFlag`: `bool` → `int` (atomic)
- `modules/libcom/src/timer/timerQueueActive.cpp:~timerQueueActive` — spin reads via `epics::atomic::get`
- `modules/libcom/src/timer/timerQueueActive.cpp:run` — writes via `epics::atomic::set`
