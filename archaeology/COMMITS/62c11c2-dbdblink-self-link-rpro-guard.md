---
sha: 62c11c22c98e4883307cea741a693a13a527d203
short_sha: 62c11c2
date: 2019-02-02
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dbDbLink.rs
    function: process_target
tags: [self-link, RPRO, PUTF, dbDbLink, infinite-loop, lifecycle]
---
# dbDbLink processTarget: self-link must not set RPRO (infinite reprocess loop)

## Root Cause
`processTarget()` determined whether to set `pdst->rpro` (request reprocessing
after async completion) based on `psrc->putf && dbRec2Pvt(pdst)->procThread!=self`.
When `psrc == pdst` (a self-link: a record linking to its own field), both
conditions could be simultaneously true: the record was marked as being processed
by self (`procThread == self`), but the `dstset` computation used
`dbRec2Pvt(pdst)->procThread==NULL`, which was FALSE (since src==dst and src
already claimed the slot) — yet the `else if` guard checking `procThread!=self`
used the live value, which would be `self`. So the guard evaluated differently
than expected.

More critically, `int dstset = dbRec2Pvt(pdst)->procThread==NULL` for a
self-link would be TRUE on entry (since `srcset` had not yet claimed it), so
`dstset=1` was set, but then after claiming, both src and dst point to the same
record; the claim happened via `srcset`. The `else if (psrc->putf ...)` guard
did not account for `src==dst`.

The real bug (fixed here): when `src == dst`, `dstset` should be forced to 0
because you cannot "newly claim" the destination if it's the same record as the
source — and crucially, you must not set `rpro`, or the record will indefinitely
reprocess itself.

## Symptoms
- A `calcout` record with `OUT` linking to its own field (self-link via `PP`)
  sets `rpro=TRUE` on itself, causing an infinite async reprocessing loop.
- Test case: `chain5_cnt` (`calcout` with `OUT="chain5_cnt.A PP"`).

## Fix
Change `int dstset = dbRec2Pvt(pdst)->procThread==NULL` to:
```c
int dstset = psrc!=pdst && dbRec2Pvt(pdst)->procThread==NULL;
```
This ensures self-links never claim the dst slot separately, and the
`else if (psrc->putf && dstset)` branch correctly skips `rpro` for self-links.

## Rust Applicability
Applies. In base-rs `process_target`, if self-links are possible (src record ==
dst record), the RPRO/reprocess flag must not be set. Verify that the
Rust equivalent checks `psrc != pdst` before marking the record for async
reprocessing.

## Audit Recommendation
In `base-rs/src/server/database/dbDbLink.rs::process_target`: verify the
self-link guard (`src_id != dst_id`) is present before setting any
"reprocess-after-async" flag. Add a regression test with a self-linking record.

## C Locations
- `modules/database/src/ioc/db/dbDbLink.c:processTarget` — `dstset = psrc!=pdst && ...`
- `modules/database/test/std/rec/asyncproctest.db` — added `chain5_cnt` self-link test record
- `modules/database/test/std/rec/asyncproctest.c:MAIN` — added chain5 test
