---
sha: 5c77c842a43a07d6b9a60e77a23ba06b2f80a0a9
short_sha: 5c77c84
date: 2025-07-31
author: Dirk Zimoch
category: other
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/unit_test.rs
    function: null
tags: [nan, float-comparison, test-harness, dbr-type, diagnostic]
---
# Test Harness Cannot Detect NaN Equality; DBR Type IDs Not Human-Readable

## Root Cause
The `testdbVGetFieldEqual` macro in `dbUnitTest.c` used `expect==pod.val.mem`
to compare expected and actual float/double values. IEEE 754 defines `NaN !=
NaN`, so any test asserting that a field holds NaN would always fail, even
when the field correctly contained NaN. Additionally, the test helper printed
the raw integer DBR type code (e.g. `4`) in failure messages rather than the
symbolic name (e.g. `DBR_FLOAT`), making failures hard to diagnose.

## Symptoms
- `testdbGetFieldEqual("pv", DBR_DOUBLE, NAN)` always reports test failure
  even when the PV contains NaN.
- Test failure output shows opaque DBR integer rather than symbolic type name.
- Float format `%e` produces unnecessarily long exponent notation vs. `%g`.

## Fix
- NaN equality check changed to
  `expect==pod.val.mem || ((expect!=expect) && (pod.val.mem!=pod.val.mem))`.
- Introduced `DBR_NAME(dbrType)` macro: strips `DBF_` prefix and maps to
  the string form of the type.
- Float/double format changed from `%e` to `%g`.

## Rust Applicability
`partial` — any Rust test helpers that compare `f32`/`f64` fields against
expected values must use `f32::is_nan()` / `f64::is_nan()` rather than
`==`. This is a general Rust gotcha since `f64::NAN == f64::NAN` is `false`
by design, and the compiler may warn on `x == x` but not on `expected == got`
when both happen to be NaN. Any base-rs or ca-rs test utilities should use
`approx` or explicit NaN checks.

## Audit Recommendation
Search base-rs and ca-rs test code for float equality assertions (`assert_eq!`
on `f32`/`f64` values) and confirm NaN cases are handled with
`f64::is_nan()` guards or `assert!(result.is_nan())`.

## C Locations
- `modules/database/src/ioc/db/dbUnitTest.c:testdbVGetFieldEqual` — NaN-safe float compare + DBR name display
