---
sha: 4966baf423a2800347e55630841df19170c01d15
short_sha: 4966baf
date: 2024-05-19
author: DW
category: type-system
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/records/printf_record.rs
    function: init_record
tags: [SIZV, field_size, signed-unsigned, string-record, printf-record]
---

# SIZV Field Uncapped at 32767: Signed field_size Overflow

## Root Cause
The `SIZV` field in `lsi`, `lso`, and `printf` records is declared as
`epicsUInt16` (unsigned, max 65535). However, `dbAddr::field_size` is a signed
`short` (max 32767). When `SIZV > 32767` is set, `field_size` stores a
negative value (overflow), causing any code that reads `field_size` as the
string buffer size to interpret it as a negative or wrong size, potentially
leading to zero-length or huge buffer operations.

For `printf` record specifically: the `init_record` function allocated
`callocMustSucceed(1, sizv, ...)` correctly using the unsigned value, but
`dbAddr::field_size = sizv` stored it as a signed short — so `field_size`
would be negative for SIZV > 32767.

## Symptoms
- CA clients reading the record via `dbGetField` see a zero or negative
  `field_size`, causing them to allocate zero bytes for the string.
- Possible buffer overflow if a CA client uses `field_size` to determine copy
  length and gets a wrapped negative value interpreted as a large positive.

## Fix
Added a cap in `printf` (and documented for `lsi`/`lso`) record `init_record`:
```c
} else if (sizv > 0x7fff) {
    sizv = 0x7fff;  /* SIZV is unsigned, but dbAddr::field_size is signed */
    prec->sizv = sizv;
}
```
Updated documentation to state max is 32767 (not 65535).

## Rust Applicability
Applies. In base-rs, string record types that expose a `sizv`-equivalent
field must cap the user-configured size at `i16::MAX` (32767) when storing
it in a `field_size` field of type `i16`. Any `TryFrom<u16> for i16` conversion
must check for overflow. Search for `field_size` assignments from unsigned
record configuration fields.

## Audit Recommendation
In `base-rs/src/server/database/records/printf_record.rs` (or lsi/lso
equivalents): verify that `sizv` is capped at `0x7FFF` before being stored
in `DbAddr::field_size` (an `i16`). Check that `u16::try_into::<i16>()` is
used with proper error handling rather than `as i16`.

## C Locations
- `modules/database/src/std/rec/printfRecord.c:init_record` — missing SIZV > 0x7fff cap
- `modules/database/src/std/rec/lsiRecord.c:init_record` — same issue (fixed in next commit)
- `modules/database/src/std/rec/lsoRecord.c:init_record` — same issue (fixed in next commit)
