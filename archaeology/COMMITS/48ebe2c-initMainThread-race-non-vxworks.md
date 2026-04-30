---
sha: 48ebe2c64eef99c93e234c7d9ca39d8f639f7546
short_sha: 48ebe2c
date: 2023-01-09
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [initMainThread, isOkToBlock, TLS, race, startup]
---
# epicsThread: move isOkToBlock to per-OSD struct, eliminate initMainThread race

## Root Cause
`epicsThread.cpp` contained a file-scope static:
```c
epicsThreadId epicsThreadMainId = initMainThread();
```
This executed the static initializer `initMainThread()` at dynamic library load
time (before `main`). It called `epicsThreadSetOkToBlock(1)`, which itself
called `epicsThreadOnce` to initialize a TLS key (`okToBlockPrivate`). On
non-vxWorks platforms, static initializer ordering is undefined; if another
static initializer from a different TU called any EPICS thread function before
`epicsThreadMainId` was initialized, the TLS key was not yet created and the
lookup returned a stale/uninitialized pointer.

Additionally, TLS (thread-local storage) was used for `isOkToBlock` on all
platforms, even though POSIX/Win32/RTEMS threads already carry a per-thread OSD
struct (`epicsThreadOSD`) that can hold this flag directly.

## Symptoms
On platforms with static-init ordering issues: spurious `epicsThreadIsOkToBlock`
returns 0 for the main thread early in startup. On vxWorks: no change (TLS
retained for compatibility). On embedded RTEMS with parallel static inits:
potential NULL deref via `epicsThreadPrivateGet(okToBlockPrivate)` before the
key was created.

## Fix
Move `isOkToBlock` into `epicsThreadOSD` for POSIX, Win32, RTEMS, and Linux.
Set `isOkToBlock = 1` in `createImplicit()` (called when the main thread first
uses the EPICS thread API) instead of via a static initializer. Remove the
global `epicsThreadMainId = initMainThread()` static entirely. Keep TLS only
for vxWorks (where per-task OSD is managed differently).

## Rust Applicability
Tokio/async Rust does not use `epicsThreadIsOkToBlock`; blocking detection is
handled by `tokio::task::block_in_place` and `#[tokio::main]`. The static-init
race pattern cannot arise. Eliminated.

## Audit Recommendation
None — Rust's async executor startup is deterministic; no equivalent static-init
ordering hazard exists.

## C Locations
- `modules/libcom/src/osi/epicsThread.cpp` — remove `initMainThread()` static, remove TLS-based `okToBlockPrivate`
- `modules/libcom/src/osi/os/posix/osdThread.c` — add `isOkToBlock` to `epicsThreadOSD`, implement `epicsThreadIsOkToBlock/SetOkToBlock`
- `modules/libcom/src/osi/os/WIN32/osdThread.c` — same for Win32
- `modules/libcom/src/osi/os/RTEMS-score/osdThread.c` — same for RTEMS
- `modules/libcom/src/osi/os/vxWorks/osdThread.c` — retain TLS approach, explicit `epicsThreadSetOkToBlock(1)` in `epicsThreadInit`
