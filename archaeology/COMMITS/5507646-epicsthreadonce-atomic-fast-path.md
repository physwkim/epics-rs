---
sha: 5507646ce72624197782f9c6d32fc2b9901ed579
short_sha: 5507646
date: 2023-01-09
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [once-init, atomic, fast-path, thread-sync, posix]
---
# epicsThreadOnce Fast-Path Read is Non-Atomic; Slow-Path Comparison is Non-Atomic

## Root Cause
`osdThread.c:epicsThreadOnce()` implements a "run once" initializer using a
`epicsThreadOnceId *id` sentinel pointer. The implementation had two issues:

1. **Non-atomic fast-path check**: The check `if (*id != EPICS_THREAD_ONCE_DONE)`
   was a plain pointer read without any atomic or memory-barrier semantics.
   On architectures where pointer reads are not naturally atomic (or where
   the compiler could reorder), a racing thread could read a stale value and
   proceed into the slow path unnecessarily — or, more dangerously, a thread
   could see DONE before the writes from `func(arg)` were visible.

2. **Slow-path comparison uses `epicsThreadGetIdSelf()` twice**: The check
   for recursive initialization compared `*id == epicsThreadGetIdSelf()`,
   calling `epicsThreadGetIdSelf()` once for the active-thread mark and once
   again for the recursive check. This introduced a window where `id` could
   change between the two reads.

3. **Polling loop uses plain `*id` reads**: The busy-wait loop for a
   concurrent initializer (`while (*id != EPICS_THREAD_ONCE_DONE)`) used
   plain reads, making the exit condition subject to compiler reordering.

## Symptoms
- On weakly-ordered architectures (ARM, POWER), the once-init guard may not
  provide the expected happens-before guarantee — a thread can see `DONE`
  without seeing the effects of `func(arg)`.
- Compiler may cache `*id` in a register across the polling loop iterations.

## Fix
- Add an `epicsAtomicGetPtrT` fast-path check: only enter the mutex if the
  ID is not yet `DONE`.
- Use `epicsAtomicCmpAndSwapPtrT` in the slow path to atomically transition
  `INIT → self` (active), ensuring no two threads both claim "first caller".
- Replace the polling loop's plain `*id` read with `epicsAtomicGetPtrT`.
- Pre-compute `self = epicsThreadGetIdSelf()` once and reuse it.

## Rust Applicability
`eliminated` — Rust's `std::sync::OnceLock` and `once_cell::sync::OnceCell`
provide correct, atomic, once-init guarantees on all platforms. They use the
OS's futex or mutex internally with proper memory ordering. epics-rs uses
`OnceLock`/`Lazy` for singleton initialization and does not implement a
custom once-init primitive.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadOnce` — non-atomic id reads in fast and slow paths
