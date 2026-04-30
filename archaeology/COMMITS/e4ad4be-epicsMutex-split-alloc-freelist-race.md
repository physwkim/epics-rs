---
sha: e4ad4becde5ebe76b2251bef0fdfccd4fd1101ea
short_sha: e4ad4be
date: 2023-09-30
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [epicsMutex, free-list, split-allocation, initialization, race]
---
# epicsMutex: eliminate split allocation and free-list race at shutdown

## Root Cause
The original `epicsMutex` implementation used a split-allocation design:
- `epicsMutexOSD* id` — the OS mutex object (allocated by the OS)
- `epicsMutexParm` — a separate heap node holding the `id` pointer plus
  metadata (file/line, list linkage)

These two allocations were tracked separately and linked via `pmutexNode->id`.
A global `freeList` (a valgrind-instrumented ELLLIST) cached freed
`epicsMutexParm` nodes to avoid repeated `malloc`/`free` on hot paths.

The race: `epicsMutexCleanup()` (called specially from `epicsExitCallAtExits`
to avoid a circular reference with the exit handler list lock) drained the
`freeList` and freed nodes. If another thread was simultaneously creating a
mutex (allocating from the free list) during shutdown, the drain and allocation
raced without adequate locking. Additionally, `epicsMutexGlobalLock` itself was
a pointer initialized lazily via `epicsMutexOsiInit` (a `epicsThreadOnce`);
before that once-block ran, any call to `epicsMutexOsiCreate` was unsafe.

The bootstrap problem: using `epicsThreadOnce` to initialize the global mutex
lock means the mutex system itself depended on the thread-once system being
initialized first — a circular dependency on some platforms.

## Symptoms
On shutdown: heap corruption or crash in `epicsMutexCleanup` due to free-list
drain racing with concurrent mutex creation. On startup: potential NULL deref
if `epicsMutexGlobalLock` was accessed before `epicsMutexOsiInit` completed.

## Fix
Unify `epicsMutexParm` and `epicsMutexOSD` into a single allocation: the
`epicsMutexParm` now IS the OSD struct (via `epicsMutexImpl.h`). Eliminate
`freeList` entirely. Change `epicsMutexGlobalLock` from a pointer to a static
struct with platform-specific static initialization (POSIX static mutex init,
or `epicsMutexOsdSetup()` fallback). Remove `epicsMutexCleanup()` and its
special circular-reference exemption from `epicsExitCallAtExits`. Eliminate
pre-XP Win32 CRITICAL_SECTION path and RTEMS non-fast mutex path.

## Rust Applicability
Rust uses `std::sync::Mutex<T>` or `parking_lot::Mutex<T>` — single allocation,
no free list, no bootstrap hazard. The split-allocation pattern and free-list
race cannot arise. Rust mutexes are statically initialized (no `Once` needed).
Eliminated.

## Audit Recommendation
None — Rust's mutex implementation avoids all of these structural hazards by
design.

## C Locations
- `modules/libcom/src/osi/epicsMutex.cpp` — unified allocation, remove `freeList`, remove `epicsMutexGlobalLock` pointer
- `modules/libcom/src/misc/epicsExit.c:epicsExitCallAtExits` — remove `epicsMutexCleanup()` special call
- Platform `osdMutex.c` files — adopt unified `epicsMutexParm`/OSD struct via `epicsMutexImpl.h`
