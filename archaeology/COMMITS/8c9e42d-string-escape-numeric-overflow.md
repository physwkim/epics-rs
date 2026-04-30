---
sha: 8c9e42d15ed181b4a1bea2a49637d5e1bb769625
short_sha: 8c9e42d
date: 2020-07-28
author: Andrew Johnson
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/iocsh/registry.rs
    function: null
  - crate: base-rs
    file: src/server/db_loader/mod.rs
    function: null
tags: [escape-sequence, overflow, UB, string-parsing, bounds]
---

# Numeric overflow in epicsStrnRawFromEscaped octal/hex escape parsing

## Root Cause
`epicsStrnRawFromEscaped()` parsed octal (`\NNN`) and hex (`\xXX...`)
escape sequences with greedy accumulation that could overflow a `char`
(unsigned value > 0xFF). The octal parser consumed a third digit even
when the accumulated value exceeded `0377` (255), producing undefined
behavior when storing the result as `char`. The hex parser consumed
arbitrarily many hex digits (`while (isxdigit(c))`), accumulating
values > 0xFF and also causing UB on store.

## Symptoms
Parsing string literals with escape sequences like `"\400"` or `"\x088"`
produced incorrect byte values due to integer overflow, or undefined
behavior when the computed value was stored into a `char`. Downstream
consumers (IOC shell, db_loader) that use escape-decoded strings could
receive corrupted data.

## Fix
For octal: added a guard `u > 037` before consuming the third digit
(max two-digit accumulated value before third digit is `077` = 63, so
adding a third digit can overflow only if `u > 037`). This stops at the
third digit if overflow would occur.
For hex: changed from greedy multi-digit to strict 2-digit (`\xXX`).
The first non-hex or third character is left in the input stream.

## Rust Applicability
In base-rs, string escape decoding may exist in `db_loader/mod.rs`
(for parsing db file string literals) or `iocsh/registry.rs` (for
command string parsing). Rust's standard escape parsing is correct, but
any custom `epicsString`-style parser ported from C must apply the
same 2-digit hex / overflow-guarded octal constraints. An unconstrained
hex accumulator in Rust would panic on `as u8` cast overflow in debug
mode, or silently wrap in release mode.

## Audit Recommendation
Search `src/server/db_loader/mod.rs` and `src/server/iocsh/registry.rs`
for escape sequence parsing (look for `\\x`, `isxdigit`, or `parse_escape`
patterns). If a custom parser is used, verify: (1) hex escapes are
limited to 2 digits, and (2) octal escapes check for overflow before
consuming the third digit. If `std::str::from_utf8` or Rust string
literals are used directly, this is eliminated.

## C Locations
- `modules/libcom/src/misc/epicsString.c:epicsStrnRawFromEscaped` — fixed octal 3rd-digit overflow guard and limited hex to 2 digits
