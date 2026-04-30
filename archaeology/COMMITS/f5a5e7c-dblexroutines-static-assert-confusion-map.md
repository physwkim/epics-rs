---
sha: f5a5e7c5f7087ea13a577ae424f0294afb61a0d6
short_sha: f5a5e7c
date: 2025-09-01
author: Dirk Zimoch
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbLexRoutines, STATIC_ASSERT, confusion-map, field-names, compile-time]
---
# dbLexRoutines: STATIC_ASSERT that confusion-map array has even element count

## Root Cause
`dbFieldConfusionMap` is a parallel array of string pairs (field name →
suggested replacement). The loop that searches it treats indices as pairs
using `i^1` (XOR with 1 to get the partner index). If the array has an odd
number of elements, the last element is unpaired, and the loop's `i^1`
access reads one element past the valid entries — an out-of-bounds read.
The original array had a trailing `NULL` sentinel at an odd index,
effectively making the array length odd and the loop logic unreliable.

## Symptoms
Latent out-of-bounds read in the field-name suggestion code path (triggered
when a database `.db` file contains an unrecognized field name). Manifests
as a garbage suggestion string or a crash in debug builds with array bounds
checking enabled.

## Fix
- Remove the trailing `NULL` sentinel.
- Add `STATIC_ASSERT(NELEMENTS(dbFieldConfusionMap) % 2 == 0)` to enforce
  the even-count invariant at compile time.
- Refactor the loop to use `NELEMENTS` instead of a NULL-termination check.
- Minor: rename loop variables for clarity; fix `buf` size from `10` to `8`.

## Rust Applicability
`eliminated` — In Rust, parallel-pair arrays would be expressed as
`&[(&str, &str)]` (a slice of tuples), making the pairing structural and
compile-time safe. Out-of-bounds reads are caught by the type system and
runtime bounds checks. No analog in epics-rs.

## Audit Recommendation
No audit needed.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbRecordField` — NULL-terminated odd-length confusion map with XOR pairing
