---
sha: e4a81bb361558769817d55c162c301735131b6b4
short_sha: e4a81bb
date: 2022-01-04
author: Andrew Johnson
category: timeout
severity: low
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/mod.rs
    function: ca_pend_io
  - crate: ca-rs
    file: src/client/mod.rs
    function: ca_pend_event
tags: [timeout, NaN, semantics, documentation, ca-pend]
---

# Document zero and NaN timeout semantics for CA and epicsEvent APIs

## Root Cause
The `ca_pend_event()`, `ca_pend_io()`, `epicsEventWaitWithTimeout()`, and
`epicsMessageQueueReceiveWithTimeout()` APIs accepted `double timeout` but
the behavior for zero, NaN, and overflow values was undocumented. This led
to inconsistent assumptions by callers:

- Zero: should be equivalent to `tryWait` (non-blocking)
- NaN or value too large for the target OS: should wait forever (no timeout)

Without documentation, some callers passed `0.0` expecting non-blocking and
others expecting a short sleep. The inconsistency was masked because the
RTEMS-score backend had a pre-existing NaN/overflow bug (fixed in the companion
commit 1655d68).

## Symptoms
- No runtime crash from this commit alone — documentation gap only.
- Callers may have assumed different semantics, leading to subtle timing issues
  in test suites.

## Fix
Added formal API documentation to `cadef.h`, `epicsEvent.h`, and
`epicsMessageQueue.h` specifying:
- `timeout == 0` → equivalent to non-blocking tryWait/trySend/tryReceive
- `NaN or too large` → equivalent to infinite wait (no timeout)

Also renamed parameter `timeOut` → `timeout` consistently across all files.

## Rust Applicability
In `ca-rs`, `pend_io` and `pend_event` accept `std::time::Duration`. The
documented contract (zero = non-blocking, very large = wait forever) should be
honored by `tokio::time::timeout(duration, ...)`:
- `Duration::ZERO` → `tokio::time::timeout(Duration::ZERO, fut)` returns
  immediately if fut is not ready.
- `Duration::MAX` → effectively infinite wait (wraps to ~584 years).

Verify that `ca-rs` does not pass `Duration::ZERO` to `tokio::time::sleep`
expecting a non-blocking poll — use `futures::poll!` or `tokio::time::timeout`
with a zero duration instead.

## Audit Recommendation
- In `ca-rs/src/client/mod.rs`, verify that `ca_pend_io` / `ca_pend_event`
  with `timeout=Duration::ZERO` behave as non-blocking poll, not a sleep.
- Check that any conversion from CA's `f64` timeout to `Duration` handles NaN
  by mapping to `Duration::MAX` (infinite wait), not panicking.

## C Locations
- `modules/ca/src/client/cadef.h:ca_pend_event` — zero/NaN doc + rename
- `modules/ca/src/client/cadef.h:ca_pend_io` — zero/NaN doc + rename
- `modules/libcom/src/osi/epicsEvent.h:epicsEventWaitWithTimeout` — zero/NaN doc
- `modules/libcom/src/osi/epicsMessageQueue.h` — zero/NaN doc
