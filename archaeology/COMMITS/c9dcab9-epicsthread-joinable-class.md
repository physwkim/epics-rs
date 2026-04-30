---
sha: c9dcab95a6aa7376edde322d3b9473e9a89c2b38
short_sha: c9dcab9
date: 2018-04-04
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-join, lifecycle, epicsThread, joinable, class]
---

# class epicsThread gains joinable semantics via epicsThreadCreateOpt

## Root Cause
The C++ `epicsThread` class used `epicsThreadCreate()` (non-joinable by default on POSIX). Without joinable semantics, `exitWait()` could not reliably wait for the OS thread to fully exit — it relied on an internal `terminated` flag set by the thread itself, which could have memory ordering issues and did not prevent the thread's stack from being reused before the caller considered the join complete.

## Symptoms
After `exitWait()` returned, the underlying pthread might still be running (in OS cleanup) for a brief window. Resources (stack, TLS) held by the thread could be freed slightly late. On resource-constrained systems, this could manifest as rare use-after-free or valgrind warnings.

## Fix
The `epicsThread` constructor now uses `epicsThreadCreateOpt()` with `opts.joinable = 1`, ensuring the underlying pthread is joinable. A `joined` boolean member is added to track whether `epicsThreadJoin()` has been called. `exitWait()` calls `epicsThreadJoin()` once after detecting `terminated == true`.

## Rust Applicability
Tokio `JoinHandle` and `std::thread::JoinHandle` are always joinable by design. The C-level concept of "optionally joinable" threads does not exist in Rust. Fully eliminated.

## Audit Recommendation
None. Rust thread handles are always joinable.

## C Locations
- `modules/libcom/src/osi/epicsThread.cpp:epicsThread::epicsThread` — switched to `epicsThreadCreateOpt` with `joinable=1`; added `joined` member
- `modules/libcom/src/osi/epicsThread.h:epicsThread` — added `bool joined` field
