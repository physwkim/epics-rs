---
sha: de7ad13b3c757cbcbfddc111fb8f94c31389e20f
short_sha: de7ad13
date: 2021-11-13
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [fork, pthread, UB, atfork, epicsThread]
---

# posix: warn on epicsThread use from child process after fork()

## Root Cause
POSIX `fork()` creates a child process with only the calling thread; all other
threads are not copied. Any mutexes or thread-state that was locked/owned by
a non-forking thread in the parent is now permanently locked in the child with
no owner thread to release it. Calling EPICS thread APIs (`epicsThreadCreate`,
`epicsThreadGetIdSelf`, etc.) from the child after `fork()` is undefined
behavior — the once-flags, mutexes, and thread keys may be in inconsistent state.

Previously there was no warning; the child would silently deadlock or corrupt
state when using `epicsThread*` APIs.

## Symptoms
- Silent deadlock in child process when `epicsThread*` APIs called after `fork()`.
- No diagnostic — difficult to debug.
- Common scenario: applications that `fork()` to spawn subprocesses and then
  (incorrectly) use EPICS threading before `exec()`.

## Fix
- Registered `pthread_atfork(NULL, NULL, &childHook)` during thread init.
- `childHook` sets `childAfterFork = 1`.
- `epicsThreadInit()` checks `childAfterFork` and emits a one-shot `stderr`
  warning: "Detected use of epicsThread from child process after fork()".
- Uses `epicsAtomicCmpAndSwapIntT` to ensure the warning prints only once.
- Skipped on RTEMS (no fork support).

## Rust Applicability
Rust strongly discourages `fork()` and does not provide `std::process::fork`.
The only Rust `fork()` is through `nix::unistd::fork()` or `libc::fork()` FFI.
Tokio is not fork-safe (async runtime state, I/O driver, timer wheel are all
in inconsistent state after fork). This pattern is effectively eliminated in
idiomatic Rust. If `ca-rs`/`base-rs` must support forking (e.g. for test
isolation), they should use `std::process::Command` (which uses `fork+exec`
internally with careful pre/post-fork cleanup).

## Audit Recommendation
None — eliminated by Rust's lack of raw fork support and Tokio's fork-unsafety
being a documented constraint.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:childHook` — new atfork child handler
- `modules/libcom/src/osi/os/posix/osdThread.c:once` — pthread_atfork registration
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadInit` — warning emission
