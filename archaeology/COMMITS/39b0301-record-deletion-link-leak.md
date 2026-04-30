---
sha: 39b0301062d134601be1593d5df8837980cf62e8
short_sha: 39b0301
date: 2024-06-18
author: Hinko Kocevar
category: leak
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_static_lib.rs
    function: dbDeleteRecord
tags: [memory-leak, record-deletion, dblink, link-cleanup, lifecycle]
---

# Record deletion leaks all link field allocations (dbDeleteRecord)

## Root Cause

`dbDeleteRecord()` in `dbStaticLib.c` removed a record from the PV directory and
from the record type's linked list, then deleted its info entries and freed the
record node — but it never freed the allocations held by the record's link fields
(`DBLINK` structs). Each `DBLINK` may hold heap-allocated strings (e.g.,
`plink->text`, `PV_LINK.pvname`, `JSON_LINK.json.string`) set during
`dbStaticPutString()` or link initialization.

The bug was exposed in IOC configurations that dynamically add/remove records
(e.g., using `dbLoadRecords` followed by `dbDeleteRecord` in testing or runtime
reconfiguration). Every deleted record leaked all its link field memory.

## Symptoms

- Monotonic heap growth in IOCs that call `dbDeleteRecord` (common in test
  harnesses and dynamic-IOC scenarios).
- Valgrind / ASAN reports from `dbDeleteRecord` showing link string allocations
  with no corresponding free.

## Fix

Introduced `dbDeleteRecordLinks()` helper that iterates over `rtyp->link_ind[]`,
gets the `DBLINK*` for each link field, and frees the heap data.

The initial implementation only freed `plink->text` (partial — improved in the
next commit, `4e4e55c`, to call `dbFreeLinkContents()`). The key fix is calling
this helper inside `dbDeleteRecord` after the PVD/list removals but before the
record node free:

```c
dbDeleteRecordLinks(precordType, prec);
```

## Rust Applicability

In base-rs, the equivalent of `dbDeleteRecord` must ensure all link fields are
dropped before the record is freed. If the record struct owns its links as Rust
types, this happens automatically via `Drop`. If the record is managed via unsafe
or FFI pointers, an explicit cleanup step is needed.

Dynamic record deletion is an important scenario in tests and runtime
reconfiguration — ensure the Rust path does not leave link-internal allocations
alive after the record is removed.

## Audit Recommendation

1. Find the Rust `delete_record` / `remove_record` path in base-rs.
2. Verify the record's link fields are fully dropped (not just the record node
   itself) before the containing allocation is freed.
3. Add a test that creates and deletes records with each link type and runs under
   ASAN or checks for zero-leak.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbDeleteRecord` — added dbDeleteRecordLinks() call
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbDeleteRecordLinks` — new helper (partial, improved in 4e4e55c)
