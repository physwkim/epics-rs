---
sha: d381a936b56a13f8b2c77b1e53f86d2e9aa49cce
short_sha: d381a93
date: 2020-07-06
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [YAJL, va_start, UB, enum, JSON-parser]
---

# YAJL va_start with enum parameter is undefined behavior

## Root Cause
`yajl_config()` and `yajl_gen_config()` declared their variadic parameter
as `yajl_option opt` (an enum type). The C standard (C11 §6.5.2.2p7 /
§7.16.1.4p4) states it is undefined behavior to use `va_start` with a
parameter that is not the last non-variadic parameter, AND it is UB to
pass an enum as the `parmN` argument of `va_start()` because enum types
are promoted to `int` in varargs but the named parameter has enum type,
which may have different size/alignment on some ABIs.

## Symptoms
With strict C compilers (GCC with `-Wpedantic`, Clang with UB sanitizer),
`va_start(ap, opt)` where `opt` is an enum may produce incorrect variadic
argument extraction, leading to corrupted JSON parse configuration or
silent incorrect option settings. The UB is typically benign on x86-64
where `enum` and `int` are the same size, but can misfire on strict-ABI
platforms.

## Fix
Changed the function signatures to accept `int option` as the named
parameter, assigned to a local `yajl_option opt = option` inside the
function body, and changed `va_start(ap, opt)` to `va_start(ap, option)`.
This makes the va_start parameter a plain `int` (well-defined) while
preserving enum type safety for the switch.

## Rust Applicability
Rust does not use C varargs for configuration. JSON parsing in epics-rs
uses `serde_json` or similar, where this C UB has no analog. Fully
eliminated.

## Audit Recommendation
No audit needed. If epics-rs wraps libyajl via FFI, confirm the wrapper
does not pass enums as `va_start` parameters. Any Rust-native JSON
parser is unaffected.

## C Locations
- `modules/libcom/src/yajl/yajl.c:yajl_config` — changed `yajl_option opt` parameter to `int option`
- `modules/libcom/src/yajl/yajl_gen.c:yajl_gen_config` — same fix
