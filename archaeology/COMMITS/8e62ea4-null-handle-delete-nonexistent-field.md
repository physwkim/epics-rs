---
sha: 8e62ea4965d2a375fbe212b6501d76da0edfe478
short_sha: 8e62ea4
date: 2024-08-15
author: Hinko Kocevar
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [dbLexRoutines, delete-record, field, null-deref, dbEntry]
---
# Null handle and double-free when deleting record with non-existent field

## Root Cause
In `dbRecordHead` (called during database loading), when a record
definition with a field override is encountered for an already-existing
record, the code attempts to delete the existing record first.  If
`dbFindRecord` succeeds, `dbDeleteRecord` is called and then
`popFirstTemp` and `dbFreeEntry` are called to clean up the temp entry.
If `dbFindRecord` fails (record not found), the old code entered the
`else` branch and printed a warning, but then fell through without
calling `popFirstTemp` and `dbFreeEntry`.  The fix moved `popFirstTemp`
and `dbFreeEntry` outside both branches so they always execute,
preventing a resource leak and potential crash on the error path.

## Symptoms
When a database file attempts to delete-and-redefine a record that does
not exist (e.g., a typo in the record name), `popFirstTemp` and
`dbFreeEntry` were skipped, leaking the temp entry and potentially
corrupting the temp-entry stack on subsequent parses.

## Fix
Moved `popFirstTemp()` and `dbFreeEntry(pdbentry)` out of the
`if (status == 0)` branch (success only) to after the entire
if-else block, so they execute unconditionally regardless of whether the
record was found.  Also moved `duplicate = TRUE` to after the if-else.

## Rust Applicability
Eliminated.  In Rust, a database-loading parser would use RAII guard
types (`ScopeGuard` / `defer!`) to ensure cleanup actions execute on all
exit paths without duplicating the calls in each branch.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbRecordHead` — moved popFirstTemp/dbFreeEntry/duplicate=TRUE outside if-else to always execute
