---
sha: d6f8e9038c5a246448bcf05cc5a26b03ffb653b1
short_sha: d6f8e90
date: 2020-05-12
author: Michael Davidsaver
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbentry, hash-lookup, optimization, dbaccess, performance]
---

# getLinkValue: Use dbInitEntryFromAddr() to skip redundant hash lookup

## Root Cause
`getLinkValue()` (called from `dbGet()` on LINK-type fields) previously called
`dbInitEntry` + `dbFindRecord` + `dbFindField` to reconstruct a `DBENTRY` from
a `DBADDR`. This involved a hash table lookup on the record name and a binary
search on the field name — O(log N) work that was unnecessary because the
`DBADDR` already contains the resolved `pfldDes` and `precord` pointers. On
IOCs with thousands of records and frequent LINK-field reads, this added
measurable overhead.

## Symptoms
- No bug; this is a performance/correctness refactor. The old path was
  functionally correct but wasteful.
- If `dbFindRecord` somehow failed (theoretically impossible for a valid
  `DBADDR`), the function returned an error status that was misleading.

## Fix
Replaced the `dbInitEntry` + `dbFindRecord` + `dbFindField` sequence with a
single `dbInitEntryFromAddr(paddr, &dbEntry)` call, which directly populates
the entry from the already-resolved address structure. The error check around
the old lookup was removed since the new path always succeeds for a valid
`DBADDR`. Return value changed from `status` to hardcoded `0`.

## Rust Applicability
Eliminated. In Rust, `DBADDR` is represented as a typed reference/handle that
already carries resolved field metadata. There is no hash-table lookup pattern
to optimize away; the field accessor goes directly through the handle.

## Audit Recommendation
No action required.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:getLinkValue` — dbFindRecord/dbFindField replaced with dbInitEntryFromAddr
