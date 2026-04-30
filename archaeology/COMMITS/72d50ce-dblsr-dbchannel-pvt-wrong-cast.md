---
sha: 72d50ce2749bbc791907a32c1ed77af1333a21f5
short_sha: 72d50ce
date: 2024-06-12
author: Michael Davidsaver
category: type-system
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [wrong-cast, pvt-pointer, dbChannel, DBADDR, dblsr, lock-show]
---

# dblsr() Casts pvt Pointer to DBADDR Instead of dbChannel

## Root Cause
In `dbLock.c:dblsr()`, when walking DB links to display lock set membership,
the code cast `plink->value.pv_link.pvt` directly to `DBADDR *`:
```c
pdbAddr = (DBADDR *)(plink->value.pv_link.pvt);
```
However, `pv_link.pvt` stores a `dbChannel *` pointer (since EPICS Base 3.16),
not a raw `DBADDR *`. The `dbChannel` struct contains a `dbAddr` field (`addr`)
as its first member, so on many platforms this worked by accident. But it is
technically a wrong cast — accessing fields past `addr` via the wrong pointer
type is UB and broke if the struct layout changed.

## Symptoms
- `dblsr()` iocsh command produced incorrect or crashing output for records
  with DB_LINK type links ("clearly doesn't get called very often" per commit).
- Diagnostic tool silent corruption: wrong record addresses printed.

## Fix
Changed to correctly extract the `dbAddr` from the `dbChannel`:
```c
pdbAddr = &((dbChannel *)(plink->value.pv_link.pvt))->addr;
```

## Rust Applicability
Eliminated. In base-rs, the Rust type system enforces that `pvt`-equivalent
fields carry the correct type through `Arc<dyn Channel>` or typed newtype
wrappers. There is no raw `void *` pvt field to miscast.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbLock.c:dblsr` — cast `pvt` to `DBADDR *` instead of `dbChannel *`
