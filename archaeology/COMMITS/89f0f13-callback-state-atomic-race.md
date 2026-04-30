---
sha: 89f0f1338a97d6b4db704ad5130cd2cbd36b9667
short_sha: 89f0f13
date: 2017-11-08
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callbackInit
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callbackStop
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callbackCleanup
tags: [race, atomics, callback, shutdown, init-ordering]
---
# Callback subsystem uses non-atomic state flag causing data races on init/stop/cleanup

## Root Cause
`callback.c` used two separate state variables: `cbCtl` (an enum controlling
run/pause/exit) and `callbackIsInit` (an int marking initialization). Both
were non-atomic plain `int`/`enum` values accessed from multiple threads
without synchronization. The `shutdown` field in each `cbQueueSet` was
likewise a plain `int`. This caused false-positive data race reports (and
real data races) on TSan-instrumented builds when the main thread called
`callbackStop()` while callback worker threads were reading `cbCtl` /
`mySet->shutdown`.

## Symptoms
- TSan or Helgrind reported data races on `cbCtl` and `callbackIsInit`.
- Without sanitizers: potential torn read of `cbCtl`, so a callback worker
  might miss the `ctlExit` transition and spin indefinitely or process an
  extra batch after shutdown was initiated.
- Double `callbackInit()` guard was racy: two threads calling `callbackInit()`
  simultaneously could both see `callbackIsInit == 0` and double-initialize.

## Fix
- Merge `cbCtl` and `callbackIsInit` into a single atomic `cbState` integer
  holding a new `cbState_t` enum (`cbInit / cbRun / cbStop`).
- Use `epicsAtomicCmpAndSwapIntT` in `callbackInit()` and `callbackStop()` to
  ensure only one thread transitions the state machine.
- Use `epicsAtomicGetIntT` in worker loops to read `mySet->shutdown`.
- Use `epicsAtomicSetIntT` in `callbackStop()` to write `mySet->shutdown = 1`.
- `callbackCleanup()` now guards against calling when not in `cbStop` state.

## Rust Applicability
In base-rs `callback.rs`, the equivalent callback subsystem state must use
`AtomicU8` / `AtomicBool` or a proper `Mutex<State>` for transitions.
If the Rust implementation uses a plain `bool` or `Cell<State>` visible to
multiple threads, the same race applies. CAS-based state transitions map
directly to `AtomicU8::compare_exchange`.

## Audit Recommendation
In `callback.rs`, audit:
1. The init/stop/cleanup state transitions — must be `AtomicU8::compare_exchange` or `Mutex`-guarded.
2. The per-queue shutdown flag — must be `AtomicBool::store(true, Ordering::Release)` with a corresponding `load(Ordering::Acquire)` in the worker.
3. Double-init guard must use CAS, not a plain load+store.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackInit` — use `epicsAtomicCmpAndSwap` for cbInit→cbRun transition
- `modules/database/src/ioc/db/callback.c:callbackStop` — use CAS cbRun→cbStop; atomic set shutdown flags
- `modules/database/src/ioc/db/callback.c:callbackCleanup` — CAS cbStop→cbInit; guard against wrong state
- `modules/database/src/ioc/db/callback.c:callbackTask` — atomic get `mySet->shutdown`
