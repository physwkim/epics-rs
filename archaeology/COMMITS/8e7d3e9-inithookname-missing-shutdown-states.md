---
sha: 8e7d3e9216885ae56b5135aa158ae77059374e3a
short_sha: 8e7d3e9
date: 2021-06-30
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/init_hooks.rs
    function: init_hook_name
tags: [initHook, shutdown, lifecycle, enum-sync, state-machine]
---

# initHookName: Shutdown States Missing from Name Table

## Root Cause
`initHookName()` returns a string name for a given `initHookState` enum value
by indexing into a static string array. The `initHookState` enum was extended
with 7 new shutdown states (`initHookAtShutdown`, `initHookAfterCloseLinks`,
`initHookAfterStopScan`, `initHookAfterStopCallback`, `initHookAfterStopLinks`,
`initHookBeforeFree`, `initHookAfterShutdown`) plus `initHookAfterInterruptAccept`
and `initHookAtEnd` were reordered. The string array in `initHookName()` was
never updated to match, so any lookup for a shutdown state returned either an
out-of-bounds read (UB) or the wrong string from a neighboring slot.

## Symptoms
Any code calling `initHookName(initHookAtShutdown)` or subsequent shutdown
states gets a wrong or garbage string. Log output and debug tools that print
hook state names show wrong names for shutdown lifecycle events.

## Fix
Add the 7 missing shutdown state strings in the correct position in the static
array inside `initHookName()`, using blank separator lines to match the visual
grouping in the enum definition.

## Rust Applicability
Applies. In base-rs, `InitHookState` is likely a Rust enum with a `Display`
or `name()` impl. If shutdown variants were added to the enum but the `match`
arm (or `name()` array) was not updated, Rust's exhaustive match check would
catch the omission at compile time — unlike C's unguarded array indexing.
However, if the mapping uses an explicit `as usize` index into a `&[&str]`
array (to match the C layout), the same OOB bug can occur in Rust.

## Audit Recommendation
In `base-rs/src/server/database/init_hooks.rs`, verify that `init_hook_name()`
or equivalent uses an exhaustive `match` on the enum rather than numeric
indexing. If it uses a `&[&str]` array, confirm the array length equals
`InitHookState::COUNT` (or equivalent) and that shutdown variants are included.

## C Locations
- `modules/libcom/src/iocsh/initHooks.c:initHookName` — added 7 missing shutdown state strings to the name array
