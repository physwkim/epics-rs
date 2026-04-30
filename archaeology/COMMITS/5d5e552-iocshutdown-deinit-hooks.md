---
sha: 5d5e552a7ec6ef69459c97b0081aa775372a6290
short_sha: 5d5e552
date: 2019-11-14
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/ioc_init.rs
    function: ioc_shutdown
tags: [shutdown, lifecycle, init-hooks, iocshutdown, ordering]
---

# Add de-init hook announcements to iocShutdown sequence

## Root Cause
`iocShutdown()` performed a fixed sequence of teardown operations
(close links → stop scan → stop callbacks → dbCaShutdown → free resources)
but did not announce any `initHook` states during shutdown. Third-party code
that registered `initHookRegister` callbacks had no way to know when
individual shutdown phases were entered. This forced drivers and support modules
to use ad-hoc polling or race conditions to detect when it was safe to
release their own resources during IOC shutdown.

## Symptoms
- Race between driver teardown and the IOC scan/callback subsystems: a driver
  cleaning up in its `epicsAtExit` handler might dereference records that had
  already been freed by `scanCleanup`.
- No defined point after which CA links are guaranteed to be closed, causing
  use-after-free in CA link cleanup in multi-threaded shutdown scenarios.

## Fix
Added `initHookAnnounce()` calls at each significant phase of `iocShutdown`:
- `initHookAtShutdown` — at entry
- `initHookAfterCloseLinks` — after `iterateRecords(doCloseLinks)`
- `initHookAfterStopScan` — after `scanStop()` (isolated mode only)
- `initHookAfterStopCallback` — after `callbackStop()` (isolated mode only)
- `initHookAfterStopLinks` — after `dbCaShutdown()`
- `initHookBeforeFree` — before `scanCleanup()`/`callbackCleanup()` (isolated)
- `initHookAfterShutdown` — at return

Corresponding new enum values added to `initHooks.h`.

## Rust Applicability
Partial. In base-rs, the IOC shutdown sequence should publish lifecycle events
through a broadcast channel or hook registry so that subsystems (CA links,
scan tasks, record processors) can await a specific phase before releasing
resources. Without explicit shutdown-phase signals, tasks may race during
`JoinHandle::abort()`.

## Audit Recommendation
In `base-rs/src/server/database/ioc_init.rs:ioc_shutdown`, verify that:
1. Scan tasks are fully stopped (awaited) before record memory is freed.
2. CA link tasks are cancelled before the record db is dropped.
3. Any registered shutdown callbacks are invoked in the correct order relative
   to scan/callback teardown.
Consider a structured teardown using `tokio::sync::broadcast` shutdown signals
mirroring the `initHook` phase sequence.

## C Locations
- `modules/database/src/ioc/misc/iocInit.c:iocShutdown` — added initHookAnnounce calls at each phase
- `modules/libcom/src/iocsh/initHooks.h` — added initHookAtShutdown and 6 subsequent phase enums
