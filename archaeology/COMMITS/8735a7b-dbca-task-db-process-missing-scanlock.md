---
sha: 8735a7b17cce8b2d39c6a9bcefa5ece005e64623
short_sha: 8735a7b
date: 2025-06-16
author: Michael Davidsaver
category: race
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: dbca_task
tags: [scanlock, db_process, race, ca-link, locking]
---

# dbCa: Acquire dbScanLock around db_process() in CA link task

## Root Cause
In `dbCa.c`, the `dbCaTask` loop processes CA monitor updates by calling
`db_process(prec)` directly without holding the record's scan lock. `db_process`
reads and writes record fields (`pact`, alarm state, timestamps, output values)
that must be protected by `dbScanLock`. Any concurrent scan task or CA put
arriving at the same time could race on those fields.

## Symptoms
- Corrupted alarm severity or status on CA-linked records.
- `pact` left set if a second thread processed the record between the read and
  reset, causing the record to become permanently stuck with `pact=1`.
- Intermittent crashes or wrong values on records driven by CA input links.

## Fix
Wrapped `db_process(prec)` in `dbCaTask` with `dbScanLock(prec)` /
`dbScanUnlock(prec)`:
```c
dbScanLock(prec);
db_process(prec);
dbScanUnlock(prec);
```

## Rust Applicability
Applies directly. In base-rs, the equivalent of `dbCaTask` is the async task
that receives CA monitor updates and drives record processing. Before calling
any `record.process()` path, the task must hold the record's scan-lock (the
async mutex or equivalent). Verify that `db_ca.rs::dbca_task` (or the monitor
callback handler) acquires and releases the scan-lock around every `process()`
call.

## Audit Recommendation
Search `base-rs/src/server/database/db_ca.rs` for any `process()` call site
not wrapped in a scan-lock guard. This is a high-severity race — the omission
is trivially reproducible under any CA link workload.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaTask` — added dbScanLock/dbScanUnlock around db_process
