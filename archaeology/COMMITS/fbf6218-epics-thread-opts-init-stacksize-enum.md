---
sha: fbf62189cbabdc96b9e431ed60b0557e0c6079ea
short_sha: fbf6218
date: 2019-07-02
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [thread-options, stack-size, enum, initialization, portability]
---
# epicsThreadOptsDefaults passes enum directly as stack size causing wrong thread stack allocation

## Root Cause
`epicsThreadOpts.stackSize` documented "do not pass enum values directly" but
`epicsThreadOptsDefaults()` initialized `stackSize` with raw platform constants
(e.g., `STACK_SIZE(1)`, `5000`, `4000*ARCH_STACK_FACTOR`). Callers that then
set `opts.stackSize = epicsThreadStackSmall` (an enum integer, 0-2) passed a
tiny raw byte value to `pthread_attr_setstacksize`, resulting in threads with
dangerously small stacks (or failures on platforms with minimum stack
requirements). The fix also introduces `EPICS_THREAD_OPTS_INIT` macro that
initializes `stackSize` with `epicsThreadStackMedium` (enum value), with the
OS-specific `epicsThreadCreateOpt` now translating enum values via
`epicsThreadGetStackSize()` before use.

## Symptoms
A thread created with `epicsThreadStackSmall` passed directly to
`epicsThreadOpts.stackSize` would receive a stack of 0, 1, or 2 bytes (the
raw enum integer), causing immediate stack overflow, SIGSEGV, or task-create
failure depending on platform.

## Fix
- Replace `epicsThreadOptsDefaults()` function with `EPICS_THREAD_OPTS_INIT`
  macro that sets `stackSize = epicsThreadStackMedium` (enum).
- All `epicsThreadCreateOpt` OS implementations now check:
  `if (stackSize <= epicsThreadStackBig) stackSize = epicsThreadGetStackSize(stackSize);`
  before calling the OS thread creation API. This transparently translates enum
  values to actual byte sizes.
- `epicsThreadCreate()` (legacy path) similarly uses `EPICS_THREAD_OPTS_INIT`.

## Rust Applicability
Rust uses `tokio::runtime::Builder::thread_stack_size()` which takes bytes
directly; there is no enum-vs-bytes ambiguity. The `epicsThread` abstraction
is replaced wholesale by `tokio::spawn`/`std::thread::Builder`. Eliminated.

## Audit Recommendation
No direct Rust analog. Verify that any thread spawning in base-rs or ca-rs
that wraps C EPICS threads does not pass raw enum values where byte counts
are expected.

## C Locations
- `modules/libcom/src/osi/epicsThread.h` — replace `epicsThreadOptsDefaults()` with `EPICS_THREAD_OPTS_INIT` macro
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — add enum-to-bytes translation
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsThreadCreateOpt` — add enum-to-bytes translation
- `modules/libcom/src/osi/os/RTEMS/osdThread.c:epicsThreadCreateOpt` — add enum-to-bytes translation
- `modules/libcom/src/osi/os/vxWorks/osdThread.c:epicsThreadCreateOpt` — add enum-to-bytes translation
- `modules/database/src/ioc/db/dbCa.c:dbCaLinkInitImpl` — use `EPICS_THREAD_OPTS_INIT`
- `modules/database/src/ioc/db/dbEvent.c:db_start_events` — use `EPICS_THREAD_OPTS_INIT`
