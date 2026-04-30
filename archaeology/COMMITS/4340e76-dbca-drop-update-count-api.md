---
sha: 4340e7644557dff847fdc839e203e4c7fc695b35
short_sha: 4340e76
date: 2021-11-02
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dead-code, API-cleanup, dbCa, update-count, dbCaGetUpdateCount]
---

# Drop unused dbCaGetUpdateCount() API

## Root Cause
`dbCaGetUpdateCount()` was a private test API that read the `nUpdate` counter
from a `caLink` without holding `dbScanLock`, resulting in a potential race:
the `pca` pointer was read from `plink->value.pv_link.pvt` without the scan
lock, which is the lock that protects link teardown. A concurrent `dbCaRemoveLink`
could free `pca` while this function was using it.

The fix in the companion commit (`e9e576f`) added `dbScanLock` to this function
and introduced `testdbCaWaitForUpdateCount()` as a properly synchronized
replacement. `dbCaGetUpdateCount()` was then left with no callers (the wait
helper supersedes polling the count manually), so this commit removes it.

## Symptoms
- No current crash (callers were in tests only), but the API was fundamentally
  racy: reading `pca` without `dbScanLock` is a use-after-free if the CA link
  is being cleared concurrently.

## Fix
Removed `dbCaGetUpdateCount()` declaration from `dbCa.h` and implementation
from `dbCa.c`.

## Rust Applicability
In Rust, there is no equivalent of an unguarded counter read that could race
with deallocation — the borrow checker prevents accessing freed data. If
`base-rs` exposes a test API to read CA link update counts, it should be done
through a proper `Arc<AtomicU64>` update counter exposed via the link handle,
not by reaching into a raw pointer. This dead-code removal pattern is
eliminated by Rust's ownership model.

## Audit Recommendation
None — eliminated.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaGetUpdateCount` — removed
- `modules/database/src/ioc/db/dbCa.h:dbCaGetUpdateCount` — declaration removed
