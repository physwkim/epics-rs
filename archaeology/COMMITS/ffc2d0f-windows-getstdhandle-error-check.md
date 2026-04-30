---
sha: ffc2d0f23a39eb9819cd135c776531a9e1e6500b
short_sha: ffc2d0f
date: 2023-10-27
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [Windows, GetStdHandle, INVALID_HANDLE_VALUE, error-check, errlog]
---

# Incorrect GetStdHandle error check uses NULL instead of INVALID_HANDLE_VALUE

## Root Cause
`isATTY()` in `errlog.c` checked `if(hand && ...)` to test whether `GetStdHandle()` succeeded. On Windows, `GetStdHandle()` returns `INVALID_HANDLE_VALUE` (which is `(HANDLE)-1`, a non-NULL pointer) on error, not `NULL`. The `NULL` check was therefore always true even when the handle was invalid, passing the invalid handle to `GetConsoleMode()`. Per the author's note, this is "likely inconsequential" because `GetConsoleMode()` returns 0 for invalid handles — but it is formally wrong and could theoretically interact with edge cases or future API changes.

## Symptoms
On Windows, `GetStdHandle()` failure would not be detected, and an invalid handle value would be passed to `GetConsoleMode()`. In practice `GetConsoleMode()` would return 0 (failure), so the overall effect was the same as detecting the error. No real user-visible impact, but the wrong API contract is used.

## Fix
Changed `if(hand && ...)` to `if(hand!=INVALID_HANDLE_VALUE && ...)`.

## Rust Applicability
Eliminated. Rust's Windows API bindings (via `windows` crate or `winapi`) use `Result`-returning wrappers or typed `Handle` structs that correctly handle `INVALID_HANDLE_VALUE`. Tokio's I/O infrastructure on Windows uses these correct wrappers. The `errlog` functionality is replaced by Rust's `log` crate + `tracing` infrastructure in `base-rs`.

## Audit Recommendation
No action needed. If `base-rs/src/log/` has any Windows-specific console mode detection code, verify it uses `INVALID_HANDLE_VALUE` for error checking.

## C Locations
- `modules/libcom/src/error/errlog.c:isATTY` — `if(hand && ...)` → `if(hand!=INVALID_HANDLE_VALUE && ...)`
