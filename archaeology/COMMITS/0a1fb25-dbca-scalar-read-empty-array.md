---
sha: 0a1fb25e6bb9523f69b69d845706028c35ac72ca
short_sha: 0a1fb25
date: 2020-06-29
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_ca.rs
    function: dbCaGetLink
tags: [empty-array, scalar, dbCa, LINK_ALARM, boundary]
---

# dbCaGetLink fails with alarm when reading scalar from empty CA-linked array

## Root Cause
`dbCaGetLink()` takes the fast scalar path (`dbFastGetConvertRoutine`) when `nelements` is NULL (i.e., the caller is requesting a scalar read). It did not check `pca->usedelements` before attempting the conversion. If the upstream CA channel had zero elements (e.g., an array PV with `NELM=0` or currently reporting zero elements), `pca->pgetNative` pointed to an empty buffer and the fast-convert read from it unconditionally, producing a garbage or zeroed value with no alarm.

## Symptoms
A scalar AI record linked via a CA link to an array PV that currently has zero elements would read a zero or stale value without raising any alarm, making it appear healthy when the link is effectively broken.

## Fix
Added a `usedelements < 1` check before entering the fast scalar convert path. If the upstream array is empty, sets `pca->sevr = INVALID_ALARM`, `pca->stat = LINK_ALARM`, returns error (`status = -1`).

## Rust Applicability
In base-rs, the CA-link get path for scalar reads must check that the received element count is at least 1 before extracting the value. A zero-element buffer is a valid upstream condition (not a protocol error) that must be surfaced as an alarm rather than silently returning a default value.

## Audit Recommendation
Audit `base-rs/src/server/database/db_ca.rs::dbCaGetLink` (or the equivalent CA-link read path). Confirm the scalar fast-path branch checks `used_elements >= 1` before conversion and raises `LINK_ALARM / INVALID_ALARM` on failure.

## C Locations
- `modules/database/src/ioc/db/dbCa.c:dbCaGetLink` — added `pca->usedelements < 1` guard before fast scalar convert
