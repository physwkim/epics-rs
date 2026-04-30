---
sha: b460c2659e8907f2aa7de9db44b1acf9ddb5a3d6
short_sha: b460c26
date: 2022-11-01
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_fast_link_conv.rs
    function: cvt_menu_st
tags: [DBF_MENU, epicsEnum16, out-of-range, type-conversion, SSCN]
---
# Menu field conversion returns error for out-of-range enum index instead of numeric string

## Root Cause
`cvt_menu_st()` in `dbFastLinkConv.c` and `dbGetStringNum()` in `dbStaticLib.c` both returned `S_db_badChoice` when `*from >= pdbMenu->nChoice` — i.e., when the stored `epicsEnum16` index did not correspond to any defined menu choice. This is a valid condition: the `SSCN` (scan skip count) field defaults to `65535`, which is well outside any menu's nChoice. The error caused CA get operations on such fields to return an error status to the client, making the field appear unreadable.

## Symptoms
CA `caget` on `<pv>.SSCN` (or any menu field whose value is not a valid menu index) returns an error or an empty string instead of a useful value. The `dbGetStringNum` path returned `NULL`, causing downstream NULL pointer issues.

## Fix
- `cvt_menu_st`: instead of returning `S_db_badChoice`, call `epicsSnprintf(to, MAX_STRING_SIZE, "%u", *from)` to produce a numeric string.
- `dbGetStringNum`: similarly, use `dbMsgPrint(pdbentry, "%u", choice_ind)` for out-of-range indices.
- Also fix: check `pfield == NULL` before the switch statement (was checked inside only some cases).
Commit `b460c26`.

## Rust Applicability
In base-rs, the DBR type conversion for `DBF_MENU` fields should handle `epicsEnum16` values that exceed `nChoice` by formatting them as decimal strings rather than returning an error. An `Option<&str>` pattern (returning `None` → numeric fallback) or a match with a catch-all arm is appropriate.

## Audit Recommendation
In `base-rs/src/server/database/db_fast_link_conv.rs` (or equivalent): verify `cvt_menu_st` / `db_get_string_num` handles `value >= n_choice` by formatting the raw integer, not returning an error.

## C Locations
- `modules/database/src/ioc/db/dbFastLinkConv.c:cvt_menu_st` — `*from >= pdbMenu->nChoice` → error instead of numeric string
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbGetStringNum` — DBF_MENU and DBF_DEVICE cases with NULL-dereference risk on out-of-range index
