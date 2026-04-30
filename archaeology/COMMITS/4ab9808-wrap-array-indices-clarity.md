---
sha: 4ab9808180b8a4cc59294b2f52687e5311f50635
short_sha: 4ab9808
date: 2020-03-30
author: Ben Franksen
category: bounds
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/filters/arr.rs
    function: wrap_array_indices
tags: [array-filter, index-wrap, clarity, off-by-one]
---
# arr filter: wrapArrayIndices early-return clarifies empty-slice path

## Root Cause
`wrapArrayIndices` in `arr.c` used a pre-declared `len = 0` variable that was
conditionally assigned inside an `if` block, then returned. The conditional was
not an `else`, so the implicit zero-return was unclear and could confuse future
maintainers into adding a non-zero path by mistake. The logic was functionally
correct but the structure obscured that an empty-array result (return 0) is the
intentional outcome when `*end < *start`.

## Symptoms
No runtime misbehavior. The change is a clarity/maintainability fix to make the
two-outcome structure (positive length vs. zero) explicit via `if/else return`.

## Fix
Remove the `len` variable entirely. Use early return for the positive case and
explicit `return 0` in the `else` branch, making both outcomes unambiguous.

## Rust Applicability
In Rust, equivalent filter logic would naturally use a match or `if/else`
expression that returns a value — the implicit-zero-via-uninitialized-variable
pattern cannot arise. However, any Rust port of `wrapArrayIndices` should
verify it returns `0usize` (not panic) when `end < start`, covering the empty
slice case.

## Audit Recommendation
Verify that `wrap_array_indices` (or equivalent range-clamping helper) in
`base-rs/src/server/database/filters/arr.rs` returns `0` (not an underflow or
panic) when the computed end index is less than start. Add a unit test for the
backward-range case.

## C Locations
- `modules/database/src/std/filters/arr.c:wrapArrayIndices` — removed `len` variable, explicit `if/else return`
