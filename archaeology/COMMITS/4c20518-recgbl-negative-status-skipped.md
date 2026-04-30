---
sha: 4c205188640502806c7747a9ebe7c0239ffcb468
short_sha: 4c20518
date: 2024-02-26
author: seifalrahman
category: other
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/recgbl.rs
    function: recgbl_record_error
tags: [status-check, negative-error, errSymLookup, recGbl, error-logging]
---

# recGblRecordError Skips Error Symbol Lookup for Negative Status Codes

## Root Cause
`recGbl.c:recGblRecordError` used `if (status)` to decide whether to call
`errSymLookup(status, errMsg, ...)`. The condition is true for any nonzero
status — including negative values (which are valid EPICS error codes, e.g.
`S_db_badField = -1` style codes). However, `errSymLookup` only handles
positive EPICS status codes (status > 0); calling it with a negative value
produces no useful output or garbled output.

The fix changed the condition to `if (status > 0)` so that `errSymLookup` is
only called for positive status codes. Negative status values are still logged
(the `errlogPrintf` below runs unconditionally) but without the symbol lookup
attempting to decode them.

## Symptoms
- For negative EPICS status codes, `recGblRecordError` would call
  `errSymLookup` with a negative value, potentially printing garbage or
  nothing in the error message field, obscuring the true error.
- Operators would see incomplete error messages for record processing failures
  that returned negative status codes.

## Fix
Changed `if (status)` to `if (status > 0)` to guard `errSymLookup` correctly.

## Rust Applicability
Applies (partial). In base-rs, error logging uses Rust's `Result<T, E>` and
the `thiserror`/`anyhow` ecosystem, which display error messages correctly.
However, if base-rs has any code path that translates C EPICS status codes
(positive/negative integers) to error strings and has a similar sign check,
audit it. Search for any numeric status → string conversion that checks `!= 0`
instead of `> 0`.

## Audit Recommendation
In `base-rs/src/server/database/recgbl.rs` or equivalent error reporting:
verify that any mapping from numeric EPICS status to error string correctly
handles both positive and negative error codes. In particular, any call to an
EPICS C FFI `errSymLookup` wrapper should guard with `status > 0`.

## C Locations
- `modules/database/src/ioc/db/recGbl.c:recGblRecordError` — `if (status)` should be `if (status > 0)`
