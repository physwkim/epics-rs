---
sha: 23d917677221fc9d24421aa02dc63d2767c563e0
short_sha: 23d9176
date: 2018-10-26
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/rec/waveform.rs
    function: process
  - crate: base-rs
    file: src/server/database/rec/aai.rs
    function: init_record
tags: [waveform, aai, nord, status, record-support]
---

# aai/waveform record cleanup: nord initialization and waveform returns readValue status

## Root Cause
Two related bugs in array input records:

1. **waveformRecord::process()** called `readValue()` but discarded its return value, always returning `0` (success). If `readValue()` failed (e.g., device returned error), the failure was silently swallowed and the record appeared healthy.

2. **aaiRecord/waveformRecord::init_record()**: `nord` was initialized as `if (nelm == 1) nord = 1; else nord = 0;` — a stylistic verbosity, but also in waveform's `readValue()`, the `nord` change detection used a stale local `nord` copy pre-read vs. post-read comparison that was wrong when not connected to a constant link (the check `!dbLinkIsConstant(&prec->siol)` was too broad; the fix checks the actual change `nRequest != prec->nord` instead).

## Symptoms
1. A waveform record with a failing device would show no alarm and return `status=0` to the record processing layer, masking the error.
2. waveform records reading from a simulation link could fail to post NORD change events correctly under some link configurations.

## Fix
1. In `waveformRecord::process()`, captured `status = readValue(prec)` and returned `status` at the end instead of `0`.
2. Simplified `nord = (prec->nelm == 1)` initialization.
3. In `waveformRecord::readValue()` sim path, replaced `!dbLinkIsConstant` guard with direct `nRequest != prec->nord` comparison for NORD change event posting.

## Rust Applicability
In base-rs waveform/aai record support, the `process()` function must propagate the `read_value()` return status. Discarding the status is a logic bug that masks device errors. Additionally, NORD change event posting must compare actual read count vs. previous NORD, not use a proxy condition.

## Audit Recommendation
In `base-rs/src/server/database/rec/waveform.rs::process` and `aai.rs::process`, verify that `read_value()` result is captured and returned. Verify that NORD change detection uses direct element-count comparison, not a link-type check.

## C Locations
- `modules/database/src/std/rec/waveformRecord.c:process` — added `status = readValue(prec)` and `return status`
- `modules/database/src/std/rec/waveformRecord.c:readValue` — fixed nord change detection from `!dbLinkIsConstant` to `nRequest != prec->nord`
- `modules/database/src/std/rec/aaiRecord.c:init_record` — simplified nord initialization
