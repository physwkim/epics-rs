---
sha: 32d76623f26223b51be5f8759127f28525b53aa1
short_sha: 32d7662
date: 2021-07-21
author: JJL772
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [memory-leak, error-path, free, semaphore, posix-thread]
---

# Memory Leaks on Error Paths in Four Subsystems

## Root Cause
Four separate error-handling paths forgot to `free()` a previously allocated
object before returning:

1. **osdThread.c** (`epicsThreadPrivateCreate`): `pthread_key_create` failure
   returns early without `free(key)`.
2. **iocLogServer.c** (`main`): `fdmgr_init()` returning NULL exits without
   `free(pserver)`.
3. **dbBkpt.c** (`dbb`): semaphore creation failure returns without
   `free(pnode)` (the breakpoint node), causing a breakpoint node leak on
   out-of-memory.
4. **devSiSoftCallback.c** (`add_record`): linked-record-not-found error path
   returns without `free(pdevPvt)`.

## Symptoms
Small heap leaks on rare error conditions. In the thread-private-create case,
the leak occurs at IOC startup if the system is out of thread-private-data
slots. For `iocLogServer`, the leak occurs on every failed startup.

## Fix
Add the missing `free()` call immediately before each early return in the
affected error paths.

## Rust Applicability
Eliminated. Rust's ownership model and RAII mean that allocated values are
automatically freed when they go out of scope at any return point, including
error returns via `?`. There is no analog of forgetting to `free()` on an
early-return path.

## Audit Recommendation
No audit needed. Confirm that Rust equivalents use `Box<T>` or similar RAII
wrappers so that all error paths free resources automatically.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadPrivateCreate` — added `free(key)` before null return
- `modules/libcom/src/log/iocLogServer.c:main` — added `free(pserver)` before error return
- `modules/database/src/ioc/db/dbBkpt.c:dbb` — added `free(pnode)` on semaphore create failure
- `modules/database/src/std/dev/devSiSoftCallback.c:add_record` — added `free(pdevPvt)` on link-not-found error
