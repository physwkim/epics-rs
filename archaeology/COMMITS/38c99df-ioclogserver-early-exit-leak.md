---
sha: 38c99df2e02554340c1d59bc6493d7317f0fc652
short_sha: 38c99df
date: 2023-05-26
author: Ralph Lange
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [ioclogserver, memory-leak, early-exit, error-path, free]
---
# iocLogServer Early-Exit Error Paths Leak pserver Allocation

## Root Cause
`iocLogServer.c:main()` allocates `pserver` via `malloc()` then enters a
sequence of initialisation steps (fdmgr init, socket bind, listen,
ioctl, file open, callback registration). Each step has an error-exit path
that returned `IOCLS_ERROR` without calling `free(pserver)`. Five such
error paths were identified by static analysis (cppcheck/SonarQube):
1. `fdmgr_init()` failure — `pserver` freed before error print, re-leaked
   in the original code because `free` and `fprintf` were in wrong order.
2. Duplicate server detection (already a server on that port).
3. `listen()` failure.
4. `ioctl(FIONBIO)` failure.
5. Key file access failure.
6. `fdmgr_add_callback()` failure.

## Symptoms
- Each failed `iocLogServer` startup leaks one `pserver` allocation.
- Not critical for a server process that exits on failure, but constitutes
  real memory unsafety detected by static analysis.
- In the `fdmgr_init` case, the original code printed the error message
  *after* `free(pserver)`, accessing the freed struct — technically UAF.

## Fix
Added `free(pserver)` before each `return IOCLS_ERROR` in all six error
paths. Also reordered the `free`/`fprintf` sequence in the first path to
eliminate the use-after-free.

## Rust Applicability
`eliminated` — Rust's ownership model prevents this entire class of bug.
A `Box<PServer>` or equivalent struct dropped at end-of-scope automatically
frees memory. Error paths using `?` or explicit `return Err(...)` release
all owned resources via `Drop`. epics-rs log server code is structured
around `async fn` with `?`-propagation, so early exits clean up
automatically.

## Audit Recommendation
No Rust audit needed. The memory safety guarantee is structural.

## C Locations
- `modules/libcom/src/log/iocLogServer.c:main` — five early-exit paths missing `free(pserver)`
