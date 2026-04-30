---
sha: 5ecf7d18a8a7dd2a13296400930beb2c9014610e
short_sha: 5ecf7d1
date: 2023-12-12
author: Andrew Johnson
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [sprintf, snprintf, buffer-overflow, bounds, Clang15]
---

# Replace unsafe sprintf with snprintf in libcom and ca

## Root Cause
Multiple call sites in `cac.cpp`, `epicsTime.cpp`, and test files used `sprintf()` to write into fixed-size buffers. `sprintf()` performs no bounds checking — if the formatted output exceeds the buffer, it writes past the end, causing a stack buffer overflow. Clang 15 introduced deprecation warnings for `sprintf`, flagging these sites.

## Symptoms
Stack buffer overflows on pathologically long hostnames, channel names, or accumulation strings in `cac::defaultExcep` (`buf[512]`) and `cac::pvMultiplyDefinedNotify` (`buf[256]`). In practice the format specifiers use precision limits (e.g., `%.400s`) that bound the field width, but the total formatted length can still exceed the buffer if multiple fields combine. The `epicsTime.cpp` call used `sprintf` into a 32-byte `fracFormat` buffer with an unbounded integer conversion.

## Fix
All `sprintf` calls replaced with `snprintf` (or `epicsSnprintf` in the subsequent commit). The buffer size is explicitly passed to prevent overruns.

## Rust Applicability
Eliminated. Rust `format!()` uses heap-allocated `String` and cannot overflow a fixed buffer. All string formatting in Rust is bounds-safe by construction.

## Audit Recommendation
No action needed. Verify that `ca-rs` does not use any `unsafe` C FFI string formatting functions.

## C Locations
- `modules/ca/src/client/cac.cpp:cac::defaultExcep` — `sprintf` → `snprintf`
- `modules/ca/src/client/cac.cpp:cac::pvMultiplyDefinedNotify` — `sprintf` → `snprintf`
- `modules/libcom/src/osi/epicsTime.cpp:epicsTimeToStrftime` — `sprintf` → `snprintf`
- `modules/ca/src/client/acctst.c:monitorUpdateTest` — dead `printf` guarded with `if(0)`
