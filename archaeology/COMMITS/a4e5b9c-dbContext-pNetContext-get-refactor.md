---
sha: a4e5b9c52801708bd5c24ab8d5f00b33f7abdf36
short_sha: a4e5b9c
date: 2019-11-12
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [refactor, smart-pointer, dbContext, readability, minor]
---

# dbContext: replace pNetContext.get()->method() with pNetContext->method()

## Root Cause
`dbContext.cpp` used `this->pNetContext.get()->method(guard)` in five places
where `this->pNetContext->method(guard)` is equivalent and idiomatic. The
`get()` call on a smart pointer followed by `->` dereference is redundant;
modern C++ directly supports `operator->` on smart pointers. This was a code
quality / readability issue, not a functional bug.

## Symptoms
- No functional bug. Minor readability degradation.

## Fix
Replace `pNetContext.get()->X()` with `pNetContext->X()` in five methods:
`show`, `flush`, `circuitCount`, `selfTest`, `beaconAnomaliesSinceProgramStart`.

## Rust Applicability
In Rust, `Arc<T>` and `Box<T>` implement `Deref`, so `ptr.method()` already
auto-derefs without needing `.as_ref().method()`. Not applicable to Rust.
Eliminated.

## Audit Recommendation
None — pure refactor with no functional change; not applicable to Rust.

## C Locations
- `modules/database/src/ioc/db/dbContext.cpp:show,flush,circuitCount,selfTest,beaconAnomaliesSinceProgramStart` — get()->X() → ->X()
