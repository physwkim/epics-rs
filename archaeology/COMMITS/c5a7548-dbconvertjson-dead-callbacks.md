---
sha: c5a754852656c4fc2b97a5abeadf0833e8fe0403
short_sha: c5a7548
date: 2022-07-27
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbConvertJSON, yajl, JSON-parser, dbLSConvertJSON, refactor]
---
# dbConvertJSON: dblsj_string rejects DBF_STRING type; dead yajl callbacks waste allocations

## Root Cause
Two distinct issues in `dbConvertJSON.c`:

1. `dblsj_string()` — the string handler for `dbLSConvertJSON` — contained a guard `if (parser->dbrType != DBF_STRING) return 0` that always triggered an error for DBF_STRING fields (the only type dbLSConvertJSON is supposed to handle), making the function effectively always reject valid string input.

2. Several yajl callbacks (`dbcj_map_key`, `dbcj_end_map`, `dbcj_end_array`) were defined as stubs returning 0 (illegal) but were explicitly assigned in the callback table. Passing NULL for unneeded yajl callbacks is equivalent (yajl treats NULL as "always illegal"); the explicit stubs added code size and maintenance burden without effect.

3. `yajl_set_default_alloc_funcs` was called with a stack-allocated (uninitialized) `yajl_alloc_funcs` struct; passing `NULL` to `yajl_alloc` is correct and uses the library default.

## Symptoms
For issue 1: dbLSConvertJSON always returned an error when given a valid JSON string value for a DBF_STRING field, causing link initialization failures for long-string constant links.

## Fix
- Remove the `dbrType != DBF_STRING` guard from `dblsj_string`.
- Replace `dblsj_integer` + `dblsj_double` with a unified `dblsj_number` using the yajl number-as-string callback.
- Set unused callbacks to NULL in the callback tables.
- Pass NULL instead of `&dbcj_alloc` to `yajl_alloc`.
Commit `c5a7548`.

## Rust Applicability
Rust-based JSON parsing uses serde/nom/etc., not yajl. The dbrType guard logic maps to a type-dispatch match in the Rust equivalent. Eliminated by design.

## Audit Recommendation
No direct audit needed — yajl is not used in Rust. If base-rs has a JSON-to-DBR converter, verify string-type dispatch doesn't accidentally reject its own type.

## C Locations
- `modules/database/src/ioc/db/dbConvertJSON.c:dblsj_string` — incorrect `dbrType != DBF_STRING` guard
- `modules/database/src/ioc/db/dbConvertJSON.c:dblsj_callbacks` — dead callback stubs replaced with NULL
