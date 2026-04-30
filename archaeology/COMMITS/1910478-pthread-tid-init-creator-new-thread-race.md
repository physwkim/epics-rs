---
sha: 1910478297f7565d291a95d632eebe01e383438d
short_sha: 1910478
date: 2025-11-14
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [pthread, tid, race, memory-barrier, thread-create]
---
# pthread tid initialized by both creator and new thread with missing memory barrier

## Root Cause
After `pthread_create()` writes `pthreadInfo->tid`, the newly spawned thread
also writes `pthreadInfo->tid = pthread_self()` in `start_routine()`. Both
writes produce the same value (`pthread_self()` in the new thread equals the
`pthread_t` returned by `pthread_create()`), so the logical result is
correct. However, neither write was guarded by a memory barrier. On weakly
ordered architectures (ARM, POWER), a reader in a third thread that calls
`epicsThreadGetOSHandle()` could observe an uninitialised `tid` if it reads
between the `pthread_create()` call and the completion of both stores.

Additionally, the previous fix (commit 214b5d9) left `start_routine` without
a barrier, meaning the new thread's own write to `pthreadInfo->tid` could be
reordered after subsequent operations.

## Symptoms
`epicsThreadGetOSHandle()` returns zero/garbage `pthread_t` on ARM/POWER
under thread-creation load. Subsequent `pthread_join` or signal delivery to
the handle may fail or kill the wrong thread.

## Fix
Add `epicsAtomicWriteMemoryBarrier()` after each of the two writes to
`pthreadInfo->tid` — once in `start_routine()` immediately after
`pthreadInfo->tid = pthread_self()`, and once in `epicsThreadCreateOpt()`
after `pthreadInfo->tid = new_tid`. The barriers ensure visibility to any
thread that subsequently reads `pthreadInfo->tid`.

## Rust Applicability
`eliminated` — Rust uses `std::thread::spawn` which returns a `JoinHandle`
holding the thread ID in a `Arc`-protected slot. tokio's `task::spawn`
exposes only an opaque `JoinHandle<T>`. There is no raw `pthread_t` field
that can race with its own initialization.

## Audit Recommendation
No audit needed in epics-rs.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:start_routine` — write barrier missing after `pthreadInfo->tid = pthread_self()`
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — write barrier missing after `pthreadInfo->tid = new_tid`
