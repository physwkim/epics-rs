---
sha: cae597d21c7019da12b7960d70acb795fab72f94
short_sha: cae597d
date: 2018-11-14
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: search_request
tags: [udp, error-log, spam, search, repeated-error]
---

# CA client suppresses repeated UDP send error messages per destination

## Root Cause
`udpiiu::SearchDestUDP::searchRequest()` logged an error via `errlogPrintf` on every failed `sendto()` call. CA search sends go out at a high rate (potentially every 30ms during active searches). A persistent UDP error (e.g., network interface down, unreachable broadcast address) would spam the error log with the same message at high frequency, filling disk and degrading performance.

## Symptoms
Log files flooded with repeated "CAC: error = ... sending UDP msg to ..." lines whenever a CA search destination is unreachable. Operators cannot distinguish a new error from an ongoing condition.

## Fix
Added `_lastError` field to `SearchDestUDP`. On error: only log if the errno changed (`localErrno != _lastError`), then store `_lastError = localErrno`. On success after a prior error: log "CAC: ok sending UDP msg to ..." recovery message and reset `_lastError = 0`. Constructor initializes `_lastError = 0`.

## Rust Applicability
In ca-rs `src/client/udp.rs::search_request` (or equivalent UDP send loop), the same error-dedup pattern is needed. Without it, a persistent `sendto` error floods the log. The `_lastError` state should be per-destination (per search destination struct). Implement as `last_error: Option<io::ErrorKind>` and only log on change.

## Audit Recommendation
Audit `ca-rs/src/client/udp.rs` UDP send path. Confirm that repeated identical send errors are deduplicated (log only on first occurrence and on recovery). If the current implementation logs unconditionally, add a `last_error` field per destination.

## C Locations
- `modules/ca/src/client/udpiiu.cpp:udpiiu::SearchDestUDP::searchRequest` — added `_lastError` field; dedup logic on error; recovery log on success
- `modules/ca/src/client/udpiiu.h:udpiiu::SearchDestUDP` — added `int _lastError` member
