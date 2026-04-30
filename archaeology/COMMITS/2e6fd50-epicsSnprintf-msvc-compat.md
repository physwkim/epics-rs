---
sha: 2e6fd505d20d9c3ecf78f0f9fb9132a89cc8012d
short_sha: 2e6fd50
date: 2023-12-14
author: Andrew Johnson
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [snprintf, MSVC, bounds, format, portability]
---

# Use epicsSnprintf for old MSVC compiler portability

## Root Cause
Old MSVC versions (pre-VS2015) implemented `snprintf()` non-conformantly: it returned -1 on truncation rather than the number of characters that would have been written (as C99 requires). EPICS provides `epicsSnprintf()` as a portable wrapper that normalizes this behavior. Two calls in `cac.cpp` and one in `epicsTime.cpp` still used the raw `snprintf()` / a secondary `snprintf()` for building the format string, which could behave incorrectly on old MSVC.

## Symptoms
On old MSVC compilers, `snprintf()` calls that result in truncation return -1. Any code that checks the return value to detect truncation (or compute total needed size) would misinterpret -1 as an error rather than as "output was truncated". This could affect error message assembly in `cac::defaultExcep` and `cac::pvMultiplyDefinedNotify`.

## Fix
Replaced `snprintf()` with `epicsSnprintf()` in `cac.cpp` and `epicsTime.cpp`. Added `#include "epicsStdio.h"` where missing.

## Rust Applicability
Eliminated. Rust's `format!()` macro and `write!()` family always produce correct-length output with heap allocation, or return `Err` on I/O failure. There is no fixed-buffer formatting that can truncate silently. The MSVC `snprintf` non-conformance does not affect Rust.

## Audit Recommendation
No action needed. If `ca-rs` has any `unsafe` FFI that calls `snprintf` directly on Windows, replace with a safe Rust formatting alternative.

## C Locations
- `modules/ca/src/client/cac.cpp:cac::defaultExcep` — `snprintf` → `epicsSnprintf`
- `modules/ca/src/client/cac.cpp:cac::pvMultiplyDefinedNotify` — `snprintf` → `epicsSnprintf`
- `modules/libcom/src/osi/epicsTime.cpp:epicsTimeToStrftime` — inner `snprintf` → `epicsSnprintf`
