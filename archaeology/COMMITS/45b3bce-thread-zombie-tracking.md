---
sha: 45b3bce51534178a0baaf3912a17827caa1b000d
short_sha: 45b3bce
date: 2023-05-23
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread, zombie, lifecycle, join, epicsThreadShow]
---

# epicsThreadShow does not flag zombie threads needing join

## Root Cause
When an EPICS thread returned from its entry function, its tracking struct (`epicsThreadOSD`) remained in the thread list until joined. `epicsThreadShow()` would display such threads as "OK" or "SUSPEND" — identical to live threads — giving operators no indication that the thread had exited and was pending cleanup. This made it impossible to diagnose thread leak / unjoinable-thread scenarios via the standard diagnostic command.

## Symptoms
`epicsThreadShow()` displays threads that have already exited (returned from their function) as "OK", concealing the fact that they are zombies (in POSIX sense: resources not freed because not yet joined). Operators cannot detect thread resource leaks via the diagnostic interface.

## Fix
Added an `isRunning` flag (atomic int) to the thread tracking structs on POSIX (osdThread.h), Linux (osdThread.h), WIN32, and RTEMS. Set to `1` at thread creation, cleared to `0` via `epicsAtomicSetIntT()` at thread exit (before or after exit-at-thread-exit callbacks). `epicsThreadShowInfo()` now appends " ZOMBIE" to the display line when `!isRunning`.

## Rust Applicability
Eliminated. Tokio tasks and `std::thread` handles are managed by the runtime. Tokio `JoinHandle` is explicitly joinable and its drop behavior is well-defined (task is detached, but not forgotten). Rust's `thread::JoinHandle` must be explicitly joined or detached — there are no implicit zombies. `epicsThreadShow` is a diagnostic command with no Rust equivalent.

## Audit Recommendation
No action needed. If `ca-rs` or `base-rs` spawn raw `std::thread` threads and store `JoinHandle`s, verify they are joined at shutdown rather than dropped (which detaches them). This is a lifecycle concern but not the same class of bug.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.h:epicsThreadOSD` — added `isRunning` field
- `modules/libcom/src/osi/os/posix/osdThread.c:start_routine` — clears `isRunning` at thread exit
- `modules/libcom/src/osi/os/posix/osdThreadExtra.c:epicsThreadShowInfo` — prints " ZOMBIE" if `!isRunning`
- `modules/libcom/src/osi/os/Linux/osdThreadExtra.c:epicsThreadShowInfo` — same
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsWin32ThreadEntry` — clears `isRunning`
- `modules/libcom/src/osi/os/RTEMS-score/osdThread.c:threadWrapper` — clears `isRunning`
