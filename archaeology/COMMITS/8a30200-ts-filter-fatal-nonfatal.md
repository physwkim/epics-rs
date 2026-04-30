---
sha: 8a3020033e4abe287a1b050172134dca5caf75ad
short_sha: 8a30200
date: 2022-06-15
author: Jure Varlec
category: lifecycle
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/filters/ts.rs
    function: filter
  - crate: base-rs
    file: src/server/database/filters/ts.rs
    function: replace_fl_value
tags: [abort-vs-error, filter, fatal-error, field-log, lifecycle]
---
# ts filter: replace cantProceed with non-fatal error handling

## Root Cause
The `ts` (timestamp) filter in `ts.c` called `cantProceed()` — a hard abort
that terminates the IOC process — on two internal logic-error conditions:
1. In `filter()` when `pvt->mode` is `tsModeInvalid` or `tsModeGenerate`.
2. In `ts_string()` when `pvt->str` is `tsStringInvalid`.

`cantProceed()` calls `abort()`, which crashes the entire IOC for what are
recoverable configuration mistakes or invalid states that might arise from
module misbehavior, not true unrecoverable corruption.

Additionally, the return type of the value-populating callbacks
(`ts_seconds`, `ts_nanos`, `ts_double`, `ts_array`, `ts_string`) was `void`,
so `replace_fl_value()` could not detect failure from allocation or logic
errors; it always returned the (possibly corrupted) `pfl`.

## Symptoms
- IOC process aborts (SIGABRT) when timestamp filter receives an invalid
  mode/string enum value, e.g. from mis-parsed config or future enum extension.
- Crash is not recoverable without IOC restart.
- In `ts_string()` invalid `str` enum sets `fmt = ""` as a silent workaround
  before calling `cantProceed()` — confused control flow.

## Fix
1. Changed the value-callback signature from `void (*func)(...)` to
   `int (*func)(...)`, returning 0 on success and non-zero on error.
2. `replace_fl_value()` now checks the return value; on error it calls
   `db_delete_field_log(pfl)` and returns `NULL`.
3. In `filter()` the `tsModeInvalid`/`tsModeGenerate` branch now logs via
   `errMessage` and returns `NULL` pfl instead of aborting.
4. In `ts_string()` the `tsStringInvalid` check was moved before the malloc
   (so `errMessage` + return 1 avoids any allocation); `cantProceed` removed.
5. In `channelRegisterPost()` the corresponding branch sets `*cb_out = NULL`.

## Rust Applicability
Rust does not use abort-on-logic-error patterns the same way. Rust filter
equivalents would use `Result<Option<FieldLog>, Error>` naturally. However:
- A Rust implementation must ensure it returns `None`/`Err` rather than
  panicking on invalid enum variants (i.e., use exhaustive `match` with a
  proper fallback arm, not `unreachable!()`).
- Any filter trait that processes `FieldLog` should have a fallible interface
  so the caller can delete/skip the log on error.

## Audit Recommendation
Audit `base-rs` filter infrastructure: verify that `filter()` trait methods
return `Result<Option<FieldLog>>` (not bare `FieldLog`) and that an invalid
filter configuration does not cause a panic. Ensure no `unwrap()`/`expect()`
on enum discriminants in timestamp-filter logic.

## C Locations
- `modules/database/src/std/filters/ts.c:filter` — cantProceed replaced with errMessage + NULL return
- `modules/database/src/std/filters/ts.c:replace_fl_value` — now checks int return from func
- `modules/database/src/std/filters/ts.c:ts_string` — tsStringInvalid guard moved before alloc
- `modules/database/src/std/filters/ts.c:channelRegisterPost` — sets cb_out=NULL on invalid mode
