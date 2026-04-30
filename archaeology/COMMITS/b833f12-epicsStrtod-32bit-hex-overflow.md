---
sha: b833f12129ac9b1058486b64d11f3df6d9da4801
short_sha: b833f12
date: 2025-04-04
author: Dirk Zimoch
category: type-system
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/util/stdlib.rs
    function: epics_strtod
tags: [strtod, hex, 32-bit, overflow, type-truncation]
---
# epicsStrtod: use strtoll/strtoull for hex parsing on 32-bit architectures

## Root Cause
`epicsStrtod` in `epicsStdlib.c` parsed hexadecimal floating-point literals
(e.g., `0xDEADBEEF`) via `strtol`/`strtoul`. On 32-bit platforms `long` is
32 bits wide, so values above `INT_MAX` or `UINT_MAX` would overflow and
silently wrap. This affected channel-access PUT of large hex values to `double`
or `float` fields on 32-bit IOCs (e.g., Raspberry Pi, older ARM).

## Symptoms
Writing a large hex value (`0xDEADBEEF`, `0x80000000`, etc.) to a numeric field
on a 32-bit IOC resulted in a wrong/wrapped value being stored. The error was
silent — `epicsStrtod` returned without error status.

## Fix
Replace `strtol`/`strtoul` with `strtoll`/`strtoull` which are always 64-bit
regardless of platform word size.

## Rust Applicability
Rust uses `i64::from_str_radix` / `u64::from_str_radix` for hex parsing, which
are always 64-bit. However, if `base-rs` has a hand-rolled `epics_strtod`
wrapper for parsing field values from strings (e.g., from CA PUT text), verify
it uses the 64-bit parse path for hex input. On 32-bit Rust targets `usize` is
32-bit but numeric parsing functions like `u64::from_str_radix` remain 64-bit.

## Audit Recommendation
In `base-rs/src/util/stdlib.rs::epics_strtod` (or the string-to-double parse
utility), verify that hex prefixed strings (`0x…`) are parsed via 64-bit integer
conversion before casting to `f64`, not via `usize` or pointer-sized types.

## C Locations
- `modules/libcom/src/misc/epicsStdlib.c:epicsStrtod` — replace `strtol`/`strtoul` with `strtoll`/`strtoull` for hex branch
