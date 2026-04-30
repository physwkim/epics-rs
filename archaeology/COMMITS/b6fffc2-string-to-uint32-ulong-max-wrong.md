---
sha: b6fffc2225300477acfcf9d07131ec9318f4537a
short_sha: b6fffc2
date: 2024-08-12
author: Dirk Zimoch
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_convert.rs
    function: get_string_ulong
  - crate: base-rs
    file: src/server/database/db_convert.rs
    function: put_string_ulong
  - crate: base-rs
    file: src/server/database/db_const_link.rs
    function: cvt_st_uint32
  - crate: base-rs
    file: src/server/database/db_fast_link_conv.rs
    function: cvt_st_ul
tags: [type-system, uint32, ulong-max, conversion, double]
---

# String-to-epicsUInt32 conversion uses ULONG_MAX bound instead of UINT_MAX

## Root Cause

Four conversion functions that convert a string to `epicsUInt32` (a 32-bit
unsigned integer) parsed the string via `epicsParseFloat64` into a `double`,
then checked `dval <= ULONG_MAX` before assigning `*to = dval`. On 64-bit
platforms, `ULONG_MAX` is `2^64 - 1` (18446744073709551615). Doubles can
represent this exactly (it rounds to 2^64 in IEEE 754), so the comparison
passes for values between `UINT_MAX + 1` (4294967296) and `ULONG_MAX`.

Assigning such a double to `*to` (a `uint32_t`) silently truncates or produces
implementation-defined behavior (C UB for floating-point-to-integer conversion
out of range). Values like "5000000000" would pass the bound check and silently
wrap or saturate to a 32-bit value different from the intended one.

## Symptoms

- Writing a string like "5000000000" to a `DBF_ULONG` field via CA/PV link
  produces an incorrect value (0 or garbage) instead of a range error.
- No error is reported; the field silently receives a truncated value.
- The bug is architecture-dependent: on 32-bit platforms where
  `ULONG_MAX == UINT_MAX`, it is harmless.

## Fix

Changed all four `dval <= ULONG_MAX` checks to `dval <= UINT_MAX` (4294967295).
This correctly rejects any double value that exceeds the target type's range,
leaving the conversion status as an error instead of silently truncating.

Affected sites:
- `dbConstLink.c:cvt_st_UInt32`
- `dbConvert.c:getStringUlong` and `putStringUlong`
- `dbFastLinkConv.c:cvt_st_ul`

## Rust Applicability

In base-rs, any string-to-u32 conversion that goes through `f64` as an
intermediate (e.g., `s.parse::<f64>()?.try_into::<u32>()`) must use
`u32::MAX` (4294967295.0) as the upper bound, not `u64::MAX`. The idiomatic
Rust fix is to parse directly as `u64` first, then `try_into::<u32>()` which
performs a checked range conversion. Avoid the float-intermediate path entirely
to prevent precision loss for large integer values.

## Audit Recommendation

1. Find all string-to-`u32` conversion sites in base-rs (`get_string_ulong`,
   `put_string_ulong`, `cvt_st_uint32` equivalents).
2. If conversion goes through `f64`, ensure the upper bound is `u32::MAX`
   (4294967295.0_f64), not `u64::MAX`.
3. Prefer `s.trim().parse::<u32>()` (direct integer parse) over the float-
   intermediate path when the format is known to be integer.

## C Locations
- `modules/database/src/ioc/db/dbConstLink.c:cvt_st_UInt32` — `ULONG_MAX` → `UINT_MAX`
- `modules/database/src/ioc/db/dbConvert.c:getStringUlong` — `ULONG_MAX` → `UINT_MAX`
- `modules/database/src/ioc/db/dbConvert.c:putStringUlong` — `ULONG_MAX` → `UINT_MAX`
- `modules/database/src/ioc/db/dbFastLinkConv.c:cvt_st_ul` — `ULONG_MAX` → `UINT_MAX`
