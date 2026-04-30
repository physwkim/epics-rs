---
sha: 20e20cbf2b57cd6f6e36e6a705ef4313d787e233
short_sha: 20e20cb
date: 2022-07-27
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbConvertJSON, yajl, silent-failure, json-type-error, diagnostics]
---
# dbConvertJSON: null/boolean/mistype JSON values rejected silently with no message

## Root Cause
Several yajl callbacks in `dbConvertJSON.c` returned 0 (signaling an illegal value to yajl) without printing any diagnostic:
- `dbcj_null`: null JSON tokens rejected silently.
- `dbcj_boolean`: boolean tokens rejected silently.
- `dblsj_integer` and `dblsj_double`: numeric values in a string-expected context rejected silently.
- `dbcj_string`: provided a generic "String provided, numeric expected" message without showing the actual string value.

Additionally, the `yajl_status_error` case in the switch statement had a fall-through to `default` that made the control flow confusing; both the error message printing and the `status = S_db_badField` assignment were in ambiguous positions.

## Symptoms
Malformed JSON in `.db` constant links (e.g., `null`, `true`/`false`, or a string where a number was expected) was rejected without any diagnostic message, leaving operators unable to identify the source of the link initialization failure.

## Fix
- Add `errlogPrintf` calls to `dbcj_null`, `dbcj_boolean`, `dblsj_integer`, `dblsj_double`.
- Improve `dbcj_string` message to include the actual string value: `"String \"%.*s\" provided, numeric value expected"`.
- Clean up the `yajl_status_error / default` switch structure to avoid fall-through confusion.
- Add `errlogPrintf("dbConvertJSON: Invalid dbrType %d\n", dbrType)` for invalid type guard.
Commit `20e20cb`.

## Rust Applicability
Rust JSON parsing with serde produces typed error variants that include field context. No silent-failure risk. Eliminated by design.

## Audit Recommendation
No logic audit needed. Confirm base-rs JSON link parsing logs actionable error messages including the offending value.

## C Locations
- `modules/database/src/ioc/db/dbConvertJSON.c:dbcj_null` — silent rejection
- `modules/database/src/ioc/db/dbConvertJSON.c:dbcj_boolean` — silent rejection
- `modules/database/src/ioc/db/dbConvertJSON.c:dblsj_integer` — silent rejection
- `modules/database/src/ioc/db/dbConvertJSON.c:dblsj_double` — silent rejection
- `modules/database/src/ioc/db/dbConvertJSON.c:dbPutConvertJSON` — yajl status switch fall-through
