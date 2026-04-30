---
sha: b36e5262c74e7f32bcb80cc52d2066fc452eb471
short_sha: b36e526
date: 2020-08-21
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_loader/mod.rs
    function: null
tags: [const-link, DBF_CHAR, long-string, waveform, type-coercion]
---

# Const link string init fails for DBF_CHAR waveform fields

## Root Cause
`lnkConst_loadArray()` handled the `sc40` case (scalar string constant)
by calling `dbFastPutConvertRoutine[DBF_STRING][dbrType]`, which limits
conversion to 40 characters (the `epicsOldString` length). When `dbrType`
is `DBF_CHAR`, this route truncates any string longer than 40 chars and
does not account for the character-array semantics: a `DBF_CHAR` waveform
expects the string bytes stored directly in the buffer up to `NELM`
elements, with a null terminator.

## Symptoms
Using `{const:"string longer than 40 chars"}` as an INP link for a
`DBF_CHAR` waveform record silently truncates the value to 40 characters.
String values of any length shorter than 40 were also not null-terminated
correctly for multi-element char arrays.

## Fix
Added a `dbrType != DBF_CHAR` check: for char arrays, use `strncpy`
directly into the buffer up to `*pnReq` bytes and compute `nElems` as
`strlen(copied) + 1`. This allows strings up to `NELM` characters
(including the null terminator) and works correctly regardless of the
40-character epicsOldString limit.

## Rust Applicability
In base-rs `db_loader/mod.rs`, the const-link string initialization path
for `CHAR` (or `u8`) array fields must use direct byte-copy semantics,
not the generic type-conversion routine that is limited to 40 bytes. If
a shared conversion function is used for all scalar-string-to-array
conversions, it may silently truncate long-string waveform initializers.

## Audit Recommendation
In `src/server/database/db_loader/mod.rs` or wherever const-link
string values are loaded into waveform/aai records: search for
`DBF_CHAR` or `FieldType::Char` handling. Verify that string-to-char-array
conversion uses the full buffer length (`nelm`) rather than a 40-byte
limit. Look for use of a generic string-to-field converter that may
apply a 40-byte cap.

## C Locations
- `modules/database/src/std/link/lnkConst.c:lnkConst_loadArray` — added `dbrType != DBF_CHAR` branch with `strncpy` for long strings
