---
sha: 80da400f9c103d07a2a18ca65b6c5019a4adc769
short_sha: 80da400
date: 2023-01-30
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbLock, cantProceed, errlog, log-ordering, abort]
---
# dbLock: errlogPrintf before cantProceed may lose the message on abort

## Root Cause
Multiple places in `dbLock.c` called `errlogPrintf()` immediately followed by `cantProceed(NULL)`. The `errlog` subsystem is asynchronous: the message is queued to a worker thread. When `cantProceed` aborts the process (via `abort()` or `exit()`), the errlog worker may not have had time to flush and print the queued message, so the diagnostic is silently lost.

`cantProceed()` itself accepts a printf-style format string since EPICS 7.x, so the two-step pattern is unnecessary.

## Symptoms
On a fatal dbLock consistency violation the intended error message may not appear in the log before the process terminates, making post-mortem analysis harder.

## Fix
Replace each `errlogPrintf(msg); cantProceed(NULL)` pair with a single `cantProceed(msg, ...)`. The `cantProceed` implementation calls the underlying abort path synchronously, ensuring the message is emitted before exit.

## Rust Applicability
In Rust, `panic!` / `eprintln!` + `std::process::abort()` are synchronous; there is no async log queue that could be skipped at abort time. This pattern does not apply.

## Audit Recommendation
No audit needed — Rust's panic infrastructure is synchronous.

## C Locations
- `modules/database/src/ioc/db/dbLock.c:dbLockIncRef` — errlogPrintf + cantProceed pair
- `modules/database/src/ioc/db/dbLock.c:dbLockDecRef` — errlogPrintf + cantProceed pair
- `modules/database/src/ioc/db/dbLock.c:dbScanLockMany` — multiple pairs
- `modules/database/src/ioc/db/dbLock.c:dbLockSetMerge` — multiple pairs
- `modules/database/src/ioc/db/dbLock.c:dbLockSetSplit` — multiple pairs
