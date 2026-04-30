---
sha: cb5f68994fbe43ebaf3c06361ae7e793aef8892c
short_sha: cb5f689
date: 2021-06-20
author: Andrew Johnson
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [compiler-warning, caRepeater, chdir, union-init, portability]
---

# Compiler Warning Fixes: chdir Return Value and Union Initialization

## Root Cause
Two classes of compiler warnings in the CA client codebase:

1. **`caRepeater.cpp`**: `chdir("/")` return value was ignored. Recent GCC
   versions and glibc emit `-Wunused-result` for `chdir()` because its failure
   is relevant to security (if it fails, the process stays in an unexpected
   directory). The fix uses `(void)! chdir("/")` to explicitly discard the
   result.

2. **`Cap5.xs`** (Perl CA binding): `union { ... } p;` was declared without
   initialization. GCC warns about using uninitialized union members. The `void*`
   member was moved to the first position (making it the active member for zero-
   initialization) and `p = {0}` was added.

3. **`osdThread.c`**: `#define USE_MEMLOCK (defined(...))` used the `defined()`
   preprocessor operator inside a `#define` body — which is UB per the C
   standard (defined() is only valid inside `#if`/`#elif`). Replaced with an
   `#if / #define / #else / #define / #endif` pattern.

## Symptoms
Build warnings (treated as errors on strict build configurations). No runtime
bug.

## Fix
Applied warning-squishing patterns: `(void)!` cast, union zero-initialization,
and `#if`-based conditional defines.

## Rust Applicability
Eliminated. Rust does not have C preprocessor macros or C union initialization
rules. `chdir` equivalents in Rust return `Result` and the compiler enforces
handling via `#[must_use]`. Uninitialized variables are not permitted.

## Audit Recommendation
No audit needed. This is a pure build-system hygiene fix.

## C Locations
- `modules/ca/src/client/caRepeater.cpp:main` — `(void)! chdir("/")`
- `modules/ca/src/perl/Cap5.xs:CA_put, CA_put_callback` — union zero-init
- `modules/libcom/src/osi/os/posix/osdThread.c` — `USE_MEMLOCK` preprocessor fix
