---
sha: d15c8093ecab33e572d11065e3e6215717d65b57
short_sha: d15c809
date: 2019-02-01
author: Andrew Johnson
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [refactor, dbAccess, dbChannel, dbAddr, code-dedup, other]
---
# Extract dbEntryToAddr() from dbChannelCreate() and dbNameToAddr()

## Root Cause
Both `dbNameToAddr()` and `dbChannelCreate()` contained identical ~30-line
blocks that populated a `DBADDR` struct from a `DBENTRY`. This duplication
meant any bug fix to one had to be applied to the other manually — and the
`cvt_dbaddr` SPC_DBADDR special handling was duplicated twice.

## Symptoms
- No runtime bug. Tech-debt: divergence risk between the two implementations.
- Maintenance burden: future changes to DBADDR initialization must be made
  in two places.

## Fix
Extract the shared logic into `dbEntryToAddr(const DBENTRY *pdbentry, DBADDR *paddr)`.
Both `dbNameToAddr` and `dbChannelCreate` now call this single function.
The `$` field-modifier handling (string/link char-array views) is left in the
callers since they have slightly different edge-case handling.

## Rust Applicability
Eliminated. The Rust equivalent (`dbNameToAddr` → `DbAddr::from_name`) is
presumably a single code path since Rust's type system prevents the duplication
pattern. No action needed.

## Audit Recommendation
None required. Pure refactor with no semantic change.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbEntryToAddr` — new function, extracted
- `modules/database/src/ioc/db/dbAccess.c:dbNameToAddr` — now delegates to dbEntryToAddr
- `modules/database/src/ioc/db/dbChannel.c:dbChannelCreate` — now delegates to dbEntryToAddr
- `modules/database/src/ioc/db/dbAccessDefs.h` — new `dbEntryToAddr` declaration
