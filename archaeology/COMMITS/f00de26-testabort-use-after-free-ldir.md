---
sha: f00de26be1c7b23b2be797a327a27e5740ba1e0a
short_sha: f00de26
date: 2024-08-15
author: Hinko Kocevar
category: leak
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [use-after-free, test-harness, string-lifetime, dangling-pointer]
---
# testAbort uses freed ldir pointer in error message format string

## Root Cause

`dbOpenFile()` returns a `const char *ldir` that points into an internal path
buffer owned by the database library.  The immediately following call to
`dbReadDatabaseFP()` may release or overwrite that buffer, leaving `ldir` as a
dangling pointer.  If `dbReadDatabaseFP` fails, `testAbort` formats the error
message using the already-freed `ldir`, producing undefined behaviour (typically
a garbage path string or a crash in the format-string processing).

This happens twice: once for `dbStaticTest.db` and once for
`dbStaticTestRemove.db`.

## Symptoms

- Spurious characters or empty string in the `testAbort` error message when
  loading fails, making it hard to diagnose the actual missing file.
- Potential crash inside the test harness `testAbort` vprintf path if the freed
  memory is reused before the format call.
- Only manifests on a failed database load; normal runs are unaffected.

## Fix

Duplicate the returned string with `epicsStrDup(ldir)` before calling
`dbReadDatabaseFP`, use the duplicate in `testAbort`, then `free()` the
duplicate afterwards.  The pattern is applied identically for both load sites.

## Rust Applicability

Eliminated.  In Rust the borrow checker enforces that `ldir` (a `&str` or
`String` slice returned by a function) cannot be used after a subsequent call
that would invalidate the underlying buffer, because that call would require a
mutable borrow of the same structure.  A Rust test helper returning a string
would either return an owned `String` (making the lifetime explicit) or a `&str`
tied to a live reference, preventing the dangling-pointer scenario entirely at
compile time.

## Audit Recommendation

No audit needed in Rust crates — the pattern is structurally impossible in safe
Rust.  Note for completeness: any `unsafe` block in `epics-base-rs` or test
harnesses that calls FFI functions returning raw `*const c_char` pointing into a
mutable C structure should be reviewed for a similar lifetime hazard.

## C Locations

- `modules/database/test/ioc/db/dbStaticTest.c:testAbort` — `ldir` used after
  `dbReadDatabaseFP` which frees the path buffer; fix duplicates string before
  the call
