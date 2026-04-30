---
sha: e5b4829074b296634233dea756d77379749c1bd6
short_sha: e5b4829
date: 2024-05-19
author: Michael Davidsaver
category: bounds
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/records/lsi_record.rs
    function: init_record
  - crate: base-rs
    file: src/server/database/records/lso_record.rs
    function: init_record
tags: [SIZV, field_size, string-record, lsi, lso, bounds]
---

# lsi/lso SIZV Uncapped at 32767: Signed dbAddr::field_size Overflow

## Root Cause
`lsiRecord.c` and `lsoRecord.c` both allocate `sizv` bytes for their `val`
buffer, then store `sizv` into `dbAddr::field_size` (a signed `short`).
When `SIZV > 32767` is configured, the assignment wraps to a negative signed
short, causing CA clients and the database access layer to use a negative or
garbage field size for all string operations on the record.

This is the same root cause as the `printf` record fix (4966baf), applied to
`lsi` and `lso` records which were fixed in a companion commit.

## Symptoms
- CA `DBR_CHAR` array reads of `lsi`/`lso` records with `SIZV > 32767` return
  zero-length or incorrectly sized data.
- `dbAddr::field_size` is negative, causing `dbGetField`/`dbPutField` to
  compute wrong element counts, potentially writing past buffer end.

## Fix
Added cap in both `lsiRecord.c` and `lsoRecord.c` `init_record`:
```c
} else if (sizv > 0x7fff) {
    sizv = 0x7fff;  /* SIZV is unsigned, but dbAddr::field_size is signed */
    prec->sizv = sizv;
}
```

## Rust Applicability
Applies. Same as 4966baf (printf record). In base-rs lsi/lso record
implementations, `sizv` must be capped at `i16::MAX` before storing in
`field_size`. The Rust type system can enforce this with `u16::min(sizv, 0x7FFF)`.

## Audit Recommendation
In `base-rs/src/server/database/records/lsi_record.rs` and `lso_record.rs`:
check `init_record` for any `field_size` assignment from `sizv`. Ensure cap at
`0x7FFF` (32767) is applied. Cross-reference with printf record fix.

## C Locations
- `modules/database/src/std/rec/lsiRecord.c:init_record` — missing SIZV > 0x7fff cap
- `modules/database/src/std/rec/lsoRecord.c:init_record` — missing SIZV > 0x7fff cap
