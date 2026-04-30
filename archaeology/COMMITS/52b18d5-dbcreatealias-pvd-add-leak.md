---
sha: 52b18d56a068b97d745be1ade02fc6b09780f73c
short_sha: 52b18d5
date: 2023-01-15
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [alias, memory-leak, error-path, pvd, dbcreatealias]
---
# dbCreateAlias Leaks pnewnode and Adds to List Before PVD Succeeds

## Root Cause
`dbStaticLib.c:dbCreateAlias()` had two bugs:

1. **Premature list insertion**: `ellAdd(&precordType->recList, ...)` and
   `precordType->no_aliases++` were executed *before* `dbPvdAdd()`. If
   `dbPvdAdd()` failed (returns NULL), the alias was already visible in the
   record list with `no_aliases` incremented, leaving the database in an
   inconsistent state. The subsequent `return -1` path did not remove the
   node from the list.

2. **Memory leak on PVD failure**: When `dbPvdAdd()` returned `NULL`, the
   code printed an error and returned -1, but did not `free(pnewnode)`. The
   allocated `dbRecordNode` was leaked.

3. **Duplicate-check resource leak**: The call to `dbFindRecord(&tempEntry,
   alias)` was followed by `dbFinishEntry(&tempEntry)` only on the success
   path (when the alias was not found). The original code returned early on
   `!status` before `dbFinishEntry`, leaking the `tempEntry` resources.

## Symptoms
- Failed alias creation leaves `no_aliases` over-counted and a dangling node
  in the record list.
- Memory leak of `pnewnode` on `dbPvdAdd` failure.
- Potential resource leak of `tempEntry` on the duplicate-detection path.

## Fix
1. Save `dbFindRecord` return value, call `dbFinishEntry` unconditionally,
   then test the status.
2. Move `ellAdd` and `no_aliases++` to after successful `dbPvdAdd`.
3. Add `free(pnewnode)` on the `dbPvdAdd` failure path.

## Rust Applicability
`eliminated` — Rust RAII ensures that `pnewnode` (an owned `Box<DbRecordNode>`)
is dropped on early returns. The list-insertion ordering is enforced by the
borrow checker: you cannot add a node to a list before validating insertion
prerequisites without unsafe code. Error paths using `?` cannot skip `Drop`.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbCreateAlias` — premature list insert + pnewnode leak on PVD failure
