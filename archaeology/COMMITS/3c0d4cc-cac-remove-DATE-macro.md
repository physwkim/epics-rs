---
sha: 3c0d4ccf4931b696c82ae678f7dd45740f4eac8c
short_sha: 3c0d4cc
date: 2019-11-15
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [reproducible-build, __DATE__, CA-client, version-string, determinism]
---

# cac.cpp version string embeds __DATE__ — breaks reproducible builds

## Root Cause
The CA client library (`cac.cpp`) embedded `__DATE__` (compile-time date
string) in a static version string `pVersionCAC`. This means every build
produces a different binary regardless of source content, defeating
reproducible-build guarantees and making binary diffing unreliable.

## Symptoms
- Every compilation produces a different `cac.o` even with identical source.
- Reproducible-build checks fail.
- Binary packages cannot be bitwise-compared across rebuild runs.

## Fix
Remove `__DATE__` from the string literal, leaving only the EPICS version
string.

## Rust Applicability
In Rust, `__DATE__` does not exist. Version strings are embedded via
`env!("CARGO_PKG_VERSION")` which is deterministic (derived from `Cargo.toml`).
The ca-rs equivalent (a static version string in `cac.rs` or similar) would
naturally be reproducible. Eliminated.

## Audit Recommendation
None — not applicable to Rust.

## C Locations
- `modules/ca/src/client/cac.cpp:pVersionCAC` — `__DATE__` removed from static version string
