---
sha: 8488c9e8910297930afe066af53091c5993622e4
short_sha: 8488c9e
date: 2023-09-03
author: Michael Davidsaver
category: bounds
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/init_hooks.rs
    function: init_hook_name
tags: [init-hooks, array-bounds, static-assert, compile-time-check]
---
# initHookName() Missing Compile-Time Array Length Consistency Check

## Root Cause
`initHookName()` maps an `initHookState` enum integer to a string by
indexing into a `stateName[]` C string array. The array length and the
`initHookAtEnd` enum value must stay in sync; if a new init hook state is
added to the enum without adding the corresponding string, the runtime
bounds check (`state >= NELEMENTS(stateName)`) would pass for the new
enum value while the returned pointer is actually out of bounds.

No compile-time assertion enforced that `NELEMENTS(stateName) ==
initHookAtEnd + 1`, so the mismatch could be silently introduced.

## Symptoms
- Adding a new `initHookState` value without a matching `stateName[]` entry
  would produce an out-of-bounds string lookup at runtime, returning
  "Not an initHookState" for valid states — or UB if the runtime guard were
  absent.
- No build-time error to catch the desync.

## Fix
Added `STATIC_ASSERT(NELEMENTS(stateName)==initHookAtEnd+1)` immediately
before the bounds check. This converts a potential runtime bug into a
build-time error whenever the enum and the name table go out of sync.

## Rust Applicability
`partial` — Rust's exhaustive `match` on an enum eliminates this entire
class of bug: adding a new `InitHookState` variant without updating a match
arm is a compile error. However, if `init_hook_name()` is implemented as
a lookup into a `&[&str]` slice indexed by `state as usize` (rather than as
a match), the same array/enum length mismatch is possible in Rust. Audit the
implementation to confirm it uses `match` rather than a raw slice lookup.

## Audit Recommendation
In `base-rs` `init_hooks.rs`, confirm `init_hook_name()` uses an exhaustive
`match` on the `InitHookState` enum. If it uses a `&[&str]` slice with
numeric indexing, add either a `const` assertion on slice length or convert
to `match`.

## C Locations
- `modules/libcom/src/iocsh/initHooks.c:initHookName` — missing `STATIC_ASSERT` on array length
