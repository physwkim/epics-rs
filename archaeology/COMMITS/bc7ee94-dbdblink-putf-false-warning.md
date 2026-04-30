---
sha: bc7ee94e2c7903dc83fec7b18e9f64439b68216e
short_sha: bc7ee94
date: 2019-01-03
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_link.rs
    function: process_target
tags: [putf, dbDbLink, false-warning, link-processing, flag-propagation]
---
# Remove spurious warning when PUTF is set on target with PACT false

## Root Cause
`processTarget()` in `dbDbLink.c` emitted an `errlogPrintf` warning whenever
the destination record had `putf` set while `pact` was false.  After the fix
in `3fb10b6` (dbNotify sets PUTF), the sequence of events legitimately
produces this state: `dbProcessNotify` sets `putf=TRUE` on the first record
before processing starts, so `pact` is still false at the moment the next
link in the chain reads the field.  The warning was therefore a false
positive produced by the preceding bug fix exposing a now-incorrect
diagnostic.

## Symptoms
After `3fb10b6` landed, IOC logs were flooded with:
```
Warning: '<record>.PUTF' found true with PACT false
```
for every CA/PVA put that caused chained record processing via dbDbLink,
even though the behavior was now correct.

## Fix
Deleted the four-line diagnostic block that checked `pdst->putf` and emitted
the errlog warning.  The remaining code (`pdst->putf = psrc->putf`) correctly
propagates the flag to the target.

## Rust Applicability
In `base-rs` `db_link.rs`, if a similar `process_target` / link-follow
function is implemented, do not add a defensive warning when the target's
put-flag is set while it is not yet processing; the initiator sets the flag
before the chain starts processing.

## Audit Recommendation
When implementing link traversal in `process_target` (or equivalent), ensure
no diagnostic fires for the combination `putf=true` + `pact=false` on the
destination — this is the normal state at the start of a notify chain.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — removed erroneous warning block
