---
sha: 73b86d49211ac11de35ee654bd3db707951e3bd2
short_sha: 73b86d4
date: 2020-06-22
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [buffer-overflow, dbpf, string-array, bounds, cli]
---

# Prevent buffer overflow in dbpf string array element parsing

## Root Cause
`dbpf()` parsed whitespace-delimited string array elements by walking a character pointer `c` through fixed-size `epicsOldString` slots. There was no bounds check on `c` while copying characters: if an input token exceeded `MAX_STRING_SIZE-1` bytes, `c` would advance past the end of the slot and into the next slot (or beyond the allocation), causing a heap buffer overflow.

## Symptoms
Calling `dbpf <pv> "a_very_long_string_exceeding_40_chars ..."` on a string-array PV would corrupt adjacent heap memory, potentially causing a crash or silent data corruption.

## Fix
Added a bounds check: `if (c >= array[n+1]-1)` (i.e., one byte before the end of the current `MAX_STRING_SIZE` slot), print an error and return `-1`. Also saved `pvalue = p` before the inner loop to correctly report the start of the overflowing token. Also fixed the `pvalue = (void*)array` cast to `pvalue = array[0]` for type correctness.

## Rust Applicability
Rust string handling via `&str` slices and `String` provides bounds safety by construction — there is no `char *` walk that can overflow. This specific bug class is eliminated. The superseding `d1491e0` commit replaced this entire path with JSON parsing anyway.

## Audit Recommendation
None. The JSON-based replacement path in `d1491e0` supersedes this fix, and Rust's string handling prevents this class of overflow.

## C Locations
- `modules/database/src/ioc/db/dbTest.c:dbpf` — added `c >= array[n+1]-1` overflow check with error message and early return
