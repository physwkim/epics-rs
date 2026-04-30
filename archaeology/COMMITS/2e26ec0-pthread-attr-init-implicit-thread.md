---
sha: 2e26ec09a6304c9a58e143c61fef22e259830bbe
short_sha: 2e26ec0
date: 2025-09-01
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [pthread, implicit-thread, uninitialized, attr-init, posix]
---
# pthread_attr_t Uninitialized for Non-EPICS Threads Adopted by EPICS

## Root Cause
When a thread that was not created by EPICS calls into EPICS code,
`createImplicit()` wraps the existing pthread in an `epicsThreadOSD`
struct. The `epicsThreadOSD` contains a `pthread_attr_t attr` field. For
threads created by EPICS (`osdThreadCreate`), `pthread_attr_init` is
called. But `createImplicit()` never called `pthread_attr_init`, leaving
the `attr` field uninitialized.

Any subsequent code that reads `pthreadInfo->attr` (e.g., to query stack
size or scheduling policy for a non-EPICS thread) would operate on garbage
data, yielding undefined behavior or incorrect results.

## Symptoms
- Incorrect or garbage stack-size values when querying an implicitly
  created EPICS thread wrapper.
- Undefined behavior if `pthread_attr_*` query functions are called on the
  uninitialised `attr` field.

## Fix
Add `pthread_attr_init(&pthreadInfo->attr)` immediately after
`pthreadInfo->isOkToBlock = 1` in `createImplicit()`, mirroring what
`osdThreadCreate` already does.

## Rust Applicability
`eliminated` — Rust uses `std::thread` (and tokio). There is no
`pthread_attr_t` struct to initialise; thread attributes are expressed via
the `std::thread::Builder` API at spawn time. Implicit thread adoption
(equivalent to `createImplicit`) is not a pattern used in epics-rs; foreign
threads that call into Rust do so through `extern "C"` boundaries where no
EPICS thread struct is synthesised.

## Audit Recommendation
No Rust audit needed. The pattern does not exist in the Rust codebase.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:createImplicit` — missing `pthread_attr_init` before attr use
