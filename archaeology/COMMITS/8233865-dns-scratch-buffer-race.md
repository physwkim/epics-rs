---
sha: 823386573f3b9e3630eb79d3943c1d10a7034eb2
short_sha: 8233865
date: 2023-06-13
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [DNS, race, shared-buffer, async, lock-release]
---

# Shared scratch buffer race in async DNS lookup (ipAddrToAsciiGlobal)

## Root Cause
`ipAddrToAsciiGlobal` had a `char nameTmp[1024]` member variable (shared state on the object). The `run()` method called `sockAddrToA()` — a potentially long-running DNS lookup — while holding a released lock (via `epicsGuardRelease`), writing the result into `this->nameTmp`. A second concurrent lookup could start, overwrite `nameTmp` with a different address, and then the callback `transactionComplete(this->nameTmp)` would deliver the wrong hostname. This is a classic TOCTOU/shared-mutable-buffer race.

## Symptoms
Under concurrent DNS lookups (multiple CA channels connecting simultaneously to different hosts), callbacks could receive the wrong hostname string — e.g., channel A's `transactionComplete` would be called with the IP address string that was resolved for channel B. This would cause incorrect host display in `ca_monitor` output and potentially confuse reconnect logic.

## Fix
Moved `nameTmp` from a member variable to a local `std::vector<char> nameTmp(1024)` inside `run()`. Since `run()` is a single worker thread loop, the local buffer is thread-private per-invocation. Each DNS resolution writes into its own stack-local buffer, eliminating the shared-state race.

## Rust Applicability
Eliminated. In Rust, the equivalent pattern would use an owned `String` or `Vec<u8>` created inside the async task, which cannot be accidentally shared. The `async-dns` or `tokio::net::lookup_host` APIs return owned `String` results. There is no shared scratch buffer.

## Audit Recommendation
No action needed for the DNS path itself. If `ca-rs` uses a custom hostname-resolution future, verify that the resolved-name `String` is owned by the future and not stored in a shared location.

## C Locations
- `modules/libcom/src/misc/ipAddrToAsciiAsynchronous.cpp:ipAddrToAsciiGlobal` — removed `nameTmp` member
- `modules/libcom/src/misc/ipAddrToAsciiAsynchronous.cpp:ipAddrToAsciiGlobal::run` — added local `std::vector<char> nameTmp(1024)`
