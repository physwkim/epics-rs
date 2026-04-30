---
sha: 88bfd6f378dac38bf2751621d2c8633379aa092c
short_sha: 88bfd6f
date: 2025-11-05
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/convert.rs
    function: put_string_to_integer
tags: [dbConvert, hex, octal, base, string-parse, CA-put]
---
# dbConvert: allow hex/octal string-to-integer conversion in dbPut/dbGet

## Root Cause
All string-to-integer conversions in `dbConvert.c` and `dbFastLinkConv.c`
hardcoded `base=10` in every `epicsParseIntN`/`epicsParseUIntN` call. A
client CA-putting the string `"0x1A"` or `"017"` to an integer field would
either fail with a conversion error or silently produce the wrong value
(treating leading zeros as decimal). Users writing hex values to EPICS
records via channel access had no supported path.

## Symptoms
- CA put of `"0x1A"` to a `DBF_LONG` field returns a conversion error or
  stores `0` (parse stops at `x`).
- CA put of `"017"` (intended as octal 15) stores `17` decimal.
- Affects all integer field types: `DBF_CHAR`, `DBF_UCHAR`, `DBF_SHORT`,
  `DBF_USHORT`, `DBF_LONG`, `DBF_ULONG`, `DBF_INT64`, `DBF_ENUM`,
  `DBF_MENU`, `DBF_DEVICE`.

## Fix
Replace hardcoded `10` with a global `dbConvertBase` (defaults to `0` =
auto-detect hex/octal/decimal). At `iocBuild` time, if
`EPICS_DB_CONVERT_DECIMAL_ONLY=YES`, `dbConvertBase` is set back to `10` for
backward compatibility. All `epicsParseIntN` and `epicsParseUIntN` calls
in both `dbConvert.c` and `dbFastLinkConv.c` use `dbConvertBase`.

## Rust Applicability
`applies` — base-rs's `put_string_to_integer` (or equivalent conversion
function) almost certainly hardcodes decimal parsing. Rust's `str::parse::<i32>()`
uses decimal only; hex/octal require explicit `i32::from_str_radix(s, 16)`.
Check whether base-rs allows hex/octal CA puts and add auto-detect
(`from_str_radix` with `0` prefix detection) if not.

## Audit Recommendation
Audit `base-rs/src/server/database/convert.rs` and any function that
converts a string field value to an integer type during CA put processing.
Verify that hex (`0x...`) and octal (`0...`) prefixes are handled or that
the omission is a documented deliberate choice.

## C Locations
- `modules/database/src/ioc/db/dbConvert.c:getStringChar` et al. — hardcoded `base=10`
- `modules/database/src/ioc/db/dbFastLinkConv.c:cvt_st_c` et al. — hardcoded `base=10`
- `modules/libcom/src/misc/epicsConvert.c` — new `dbConvertBase` global (default 0)
