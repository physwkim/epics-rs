---
sha: 16c3202992600a6b5c0daaaf9f29715f4650c458
short_sha: 16c3202
date: 2021-07-21
author: Andrew Johnson
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/waveform_record.rs
    function: process
tags: [pact, waveform, callback, record-processing, lifecycle]
---

# waveform: PACT=TRUE Lost, Causes Double-Processing on Async Completion

## Root Cause
During a refactor of `waveformRecord.c:process()`, the line `prec->pact = TRUE`
was accidentally removed. The `PACT` (Process ACTive) flag is the standard
EPICS mechanism to mark a record as in-flight for asynchronous device support:
device support sets it before initiating async I/O, and the callback re-enters
`process()` to complete. Without `prec->pact = TRUE` being set at the start of
`process()`, the guard `if (!pact && prec->pact) return 0` never fires on
re-entry. This means a second scan trigger can initiate a concurrent second
processing pass while the first async callback is still pending.

## Symptoms
Waveform records with asynchronous device support (e.g., hardware-backed
waveforms) may process concurrently, leading to data corruption, double-posting
of monitor events, or incorrect UDF handling. GitHub Issue #187.

## Fix
Restore `prec->pact = TRUE` before `prec->udf = FALSE` and
`recGblGetTimeStampSimm(...)`.

## Rust Applicability
Applies. In base-rs, the equivalent of `PACT` is the in-flight flag that
prevents concurrent processing of a single record. If the waveform record
processor sets `udf = false` and calls into device support before marking the
record as in-flight (setting pact/async-active), concurrent scan tasks can
re-enter processing.

## Audit Recommendation
In `base-rs/src/server/database/waveform_record.rs`, verify that the `process()`
function marks the record as active (equivalent of `pact = TRUE`) BEFORE
performing any async device call. Check other record types for the same
pattern: the active-flag assignment must be the first mutation in `process()`.

## C Locations
- `modules/database/src/std/rec/waveformRecord.c:process` — restored `prec->pact = TRUE` before `prec->udf = FALSE`
