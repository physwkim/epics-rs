---
sha: 27918cb7a1a5c3c9335897f73d1f6ed7c99f6d6d
short_sha: 27918cb
date: 2021-02-04
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/static_run.rs
    function: put_string_num
tags: [error-message, menu, device-support, dbputstring, parse-error]
---
# dbPutString: insufficient error message for DBF_MENU/DEVICE invalid choice

## Root Cause
In `dbPutStringNum`, when parsing a string value for a `DBF_MENU` or
`DBF_DEVICE` field failed (not a valid integer, not a valid choice string),
the function returned the raw `status` from `epicsParseUInt16` which was
`S_stdlib_noConversion` — not `S_db_badChoice`. The error message printed
by the caller only showed the error code, not the menu name or device type,
leaving operators with no context about which menu was invalid or what device
types are available.

Additionally, `dbMsgPrint` was a `static` function in `dbStaticLib.c` that
was not accessible from `dbStaticRun.c`, preventing it from being used in the
more specific error reporting path.

## Fix
1. Made `dbMsgPrint` non-static and declared it in `dbStaticPvt.h` for shared
   use across translation units.
2. In `dbPutStringNum`, on parse failure:
   - Set `status = S_db_badChoice` (correct error code).
   - For `DBF_MENU`: call `dbMsgPrint(pdbentry, "using menu %s", pdbMenu->name)`.
   - For `DBF_DEVICE`: call `dbMsgPrint(pdbentry, "no such device support for '%s' record type", ...)`.
3. Caller `dbRecordField` now prints `pdbentry->message` as part of the error
   line (prepared in the subsequent commit `2c1c352`).

## Rust Applicability
Partial. base-rs field-value parsing should return a typed error with context
(which menu, which device type) rather than a raw parse error code. Not a
correctness/safety bug but affects operator debuggability.

## Audit Recommendation
If base-rs implements DBD field parsing, ensure error types carry the menu
name or device type name when an invalid choice is rejected, rather than only
surfacing a raw "parse error" code.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticRun.c:dbPutStringNum` — sets S_db_badChoice and calls dbMsgPrint for context
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbMsgPrint` — made non-static
- `modules/database/src/ioc/dbStatic/dbStaticPvt.h` — dbMsgPrint declared for cross-TU use
