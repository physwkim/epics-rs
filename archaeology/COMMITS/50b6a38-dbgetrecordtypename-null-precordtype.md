---
sha: 50b6a3801ae4c7824314877835764a2bd6cc38fd
short_sha: 50b6a38
date: 2024-08-15
author: Hinko Kocevar
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [null-deref, dbentry, record-type, defensive-null-check]
---
# dbGetRecordTypeName null-deref when precordType is unset in DBENTRY

## Root Cause

`dbGetRecordTypeName(DBENTRY *pdbentry)` unconditionally dereferenced
`pdbentry->precordType->name` without first verifying that `precordType` is
non-NULL.  A `DBENTRY` freshly initialised or positioned before any record type
(e.g. after `dbFirstRecordType` returns non-zero, or before iteration begins)
leaves `precordType` as NULL.  Any caller that reaches `dbGetRecordTypeName` in
that state triggers a null pointer dereference and a segfault.

## Symptoms

- Segmentation fault (SIGSEGV) in any tool or IOC startup code that calls
  `dbGetRecordTypeName` without first successfully navigating to a record type
  via `dbFirstRecordType` / `dbNextRecordType`.
- The crash is silent and hard to attribute because the DBENTRY itself is a
  valid heap pointer; only the embedded `precordType` field is NULL.

## Fix

Extract `pdbentry->precordType` into a local variable and guard with an explicit
NULL check, returning NULL to the caller when the entry has no associated record
type.  Callers are already expected to handle a NULL return (consistent with
other `dbGet*` accessors in the same file).

## Rust Applicability

Eliminated.  In Rust the equivalent of `precordType` would be typed as
`Option<&RecordType>` (or `Option<Arc<RecordType>>`).  Accessing `.name` without
an explicit `match` / `.map()` / `?` is a compile-time error, so the
unconditional dereference pattern cannot be written in safe Rust.  The
`record_type()` method in `epics-base-rs` (e.g.
`src/server/database/processing.rs:164`) returns a `&str` derived from a
concrete owned field on a live record instance, bypassing the nullable-pointer
problem entirely.

## Audit Recommendation

No direct Rust audit needed — the type system prevents this class of defect.
If `epics-base-rs` ever exposes a C-FFI boundary that calls back into
`dbGetRecordTypeName`, verify the returned `*const c_char` is checked for NULL
before calling `CStr::from_ptr`.

## C Locations

- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbGetRecordTypeName` —
  unconditional `precordType->name` dereference; fix adds NULL guard returning
  NULL
