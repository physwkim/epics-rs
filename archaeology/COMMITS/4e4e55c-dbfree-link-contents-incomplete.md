---
sha: 4e4e55ca894ea7cbfcbca2502b237d6403f4b0a4
short_sha: 4e4e55c
date: 2024-06-19
author: Hinko Kocevar
category: leak
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_static_lib.rs
    function: delete_record_links
tags: [memory-leak, dbFreeLinkContents, record-deletion, link-cleanup]
---

# dbDeleteRecordLinks only freed plink->text, skipping full link contents cleanup

## Root Cause

The `dbDeleteRecordLinks()` helper introduced in the previous commit (39b0301)
manually freed only `plink->text` while leaving a large commented-out block of
code indicating uncertainty about how to handle different link types. EPICS
already provides `dbFreeLinkContents()` which performs the complete, type-aware
cleanup for any `DBLINK` (freeing the appropriate union member based on
`plink->type`).

By only freeing `plink->text`, the function left link-type-specific allocations
(e.g., `PV_LINK.pvname`, `JSON_LINK.json.string`, `INST_IO.instio.string`) intact,
resulting in partial leak on record deletion.

## Symptoms

- On record deletion, link-internal strings for non-text link types (PV_LINK,
  JSON_LINK, hardware I/O links) are leaked.
- Valgrind / ASAN reports leak from `dbDeleteRecord` for records with non-CONSTANT
  link types.

## Fix

Replaced the hand-rolled `free(plink->text)` loop body with a single call to
`dbFreeLinkContents(plink)`, which is the authoritative cleanup function for
`DBLINK` objects. The function return type was also changed from `long` to `void`
since it cannot fail.

## Rust Applicability

In base-rs, when deleting a record, the Rust equivalent of each `DBLINK` field
must be properly dropped. Since Rust link objects should implement `Drop`, this
is automatic if links are stored as owned values. However, if links are stored
as raw-pointer or unsafe structs (e.g., for FFI interop), the drop must call the
appropriate cleanup for each link type — not just a single string field.

## Audit Recommendation

1. Find the Rust `delete_record` path — verify that all link fields in the
   record struct are properly dropped (not just the text/name field).
2. If links are represented as enums (CONSTANT / PV_LINK / JSON_LINK / etc.),
   ensure each variant's `Drop` implementation frees the variant-specific allocation.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbDeleteRecordLinks` — replaced manual free with dbFreeLinkContents()
