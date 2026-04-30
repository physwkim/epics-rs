---
sha: 5485adacb9b407d8cbcf6ffbb6e41a6690f9025b
short_sha: 5485ada
date: 2022-04-15
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/types/epics_math.rs
    function: null
tags: [NaN, Inf, const, type-safety, cross-platform]
---

# Make epicsNAN and epicsINF truly const on all platforms

## Root Cause
`epicsNAN` and `epicsINF` were declared `extern float` (mutable globals) in all
platform-specific `epicsMath.h` headers and defined as mutable globals in
`epicsMath.cpp`. Any code could inadvertently write to them (e.g.
`epicsNAN = 0.0f`) with no compiler warning, changing what future code using
`epicsNAN` would see — effectively corrupting the sentinel values used
throughout EPICS to represent "undefined" field values.

On some platforms (e.g. MSVC) the lack of `const` also prevented the compiler
from placing them in read-only memory segments, meaning they could be
accidentally modified by a buffer overrun in adjacent globals.

## Symptoms
- Silent mutation of `epicsNAN`/`epicsINF` by any code assigning to them.
- On MSVC: values placed in a writable data segment, increasing attack surface.
- Potential ODR (one-definition-rule) violations across translation units with
  different linkage expectations.

## Fix
Added `const` to both the declarations in all platform headers and the
definitions in `epicsMath.cpp`:
- `const float epicsNAN = NAN;`
- `const float epicsINF = INFINITY;`

## Rust Applicability
In Rust, `f32::NAN` and `f32::INFINITY` are built-in constants — there are no
mutable global sentinels. If `base-rs` or `ca-rs` define any NaN/Inf sentinel
constants (e.g. for "disconnected" field values), they must be declared `const`
or `static` (immutable). No mutable sentinel globals should exist.

Check any `lazy_static!` or `once_cell::sync::Lazy<f32>` initializing NaN/Inf
— these are unnecessary; use `const F32_NAN: f32 = f32::NAN` directly.

## Audit Recommendation
- Search `base-rs` and `ca-rs` for any `static mut` float constants used as
  sentinels (NaN/Inf representations for EPICS undefined values).
- Verify that `DBF_FLOAT`/`DBF_DOUBLE` "undefined" encoding uses `f32::NAN` /
  `f64::NAN` directly, not a stored global.

## C Locations
- `modules/libcom/src/osi/epicsMath.cpp` — definition made const
- `modules/libcom/src/osi/os/*/epicsMath.h` — declarations made const (all 7 platforms)
