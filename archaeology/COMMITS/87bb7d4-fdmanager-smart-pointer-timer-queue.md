---
sha: 87bb7d430f02b16c8483e503bd032abbb8317fe4
short_sha: 87bb7d4
date: 2025-01-30
author: Dirk Zimoch
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [smart-pointer, memory, lifecycle, fdmanager, unique-ptr]
---

# fdManager: Use smart pointer for pTimerQueue to prevent leak on exception

## Root Cause
`fdManagerPrivate::pTimerQueue` was a raw `epicsTimerQueuePassive*` pointer
initialized to `nullptr` and freed with an explicit `delete` in
`fdManager::~fdManager()`. If any exception was thrown between construction
and the destructor (e.g., during IOC startup failure), the timer queue would be
leaked. Additionally the raw pointer required manual null-check in
`lazyInitTimerQueue()`.

## Symptoms
- Timer queue memory leak on exception during `fdManager` teardown.
- Potential double-delete if `~fdManager` was called more than once in error
  recovery paths (rare but possible under test teardown).

## Fix
Changed `pTimerQueue` from `epicsTimerQueuePassive*` to
`epics::auto_ptr<epicsTimerQueuePassive>` (a compatibility alias for
`std::unique_ptr` under C++11, or `std::auto_ptr` under C++03). Removed the
explicit `delete priv->pTimerQueue` from the destructor. Updated
`lazyInitTimerQueue()` to use `.get()` / `.reset()`.

## Rust Applicability
Eliminated. Rust ownership semantics guarantee deterministic destruction
without manual `delete`. All timer/queue resources in ca-rs and pva-rs are
managed via `Drop` implementations or `JoinHandle::abort()`. No equivalent
raw-pointer lifecycle issue exists.

## Audit Recommendation
No action required.

## C Locations
- `modules/libcom/src/fdmgr/fdManager.cpp:fdManagerPrivate` — pTimerQueue changed to auto_ptr
- `modules/libcom/src/fdmgr/fdManager.h:fdManager` — auto_ptr typedef added
