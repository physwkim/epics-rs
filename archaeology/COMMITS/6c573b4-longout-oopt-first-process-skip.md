---
sha: 6c573b496a2d387583e6ef603530a504b68aaa0a
short_sha: 6c573b4
date: 2021-03-10
author: Joao Paulo Martins
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/records/longout.rs
    function: init_record
tags: [longout, OOPT, on-change, first-process, record-lifecycle]
---

# longout with OOPT=On Change skips output write on first process

## Root Cause

The `longout` record uses a private field `outpvt` as a state variable to decide
whether to force an output write when `OOPT = On Change`. On initialization
(`init_record`), `outpvt` was set to `OUT_LINK_UNCHANGED (0)`. This caused the
`conditional_write()` function to treat the first process identically to a
repeat-value process: since no OUT link change had been detected and the value
had not changed from `pval` (also initialized to `val`), the output write was
silently skipped.

The correct behavior is that the very first processing should always write to the
device support so the hardware/downstream link is initialized to the record's
current value, even when `OOPT = On Change`.

Additionally, the `special()` function (called when the OUT field is modified via
`dbPutField`) was setting `outpvt = OUT_LINK_CHANGED` when `after == FALSE` (i.e.,
*before* the link was updated), not after. This meant the flag could be cleared
before the write triggered by the link change happened.

## Symptoms

- A `longout` record with `OOPT = On Change` and `PINI = NO` never writes to its
  device support on startup, leaving hardware in an indeterminate state.
- A change to the OUT field may or may not trigger a write depending on race
  between the `special()` pre-call and the subsequent processing.

## Fix

- `init_record`: initialize `outpvt = EXEC_OUTPUT (1)` so that the first
  processing always causes a write regardless of value comparison.
- `special()`: changed condition from `if (!after)` to `if ((after) && (prec->ooch == menuYesNoYES))`
  so the forced-write flag is only set *after* the OUT link is updated, and only
  when OOCH is YES.
- `conditional_write()`: simplified the `On Change` branch to
  `if (prec->outpvt == EXEC_OUTPUT)` — encompasses both the first-process case
  and the OUT-link-change case, then falls through to value-change check.
- After write: reset `outpvt = DONT_EXEC_OUTPUT`.

## Rust Applicability

Any Rust implementation of the `longout` record (or any output record with an
equivalent OOPT/On-Change pattern) must initialize the `outpvt`-equivalent state
to "force write" at `init_record` time. The `special()` hook for OUT field changes
must set the flag *after* (`after == true`) the link is updated, not before.

## Audit Recommendation

1. Find the Rust `longout` record `init` function — verify the write-state is
   initialized to "execute" (not "skip").
2. Check that any equivalent of `special()` sets the force-write flag only after
   the link change completes.
3. Check all other output record types (`longoutRecord`, `aoRecord`, `boRecord`,
   etc.) for the same pattern.

## C Locations
- `modules/database/src/std/rec/longoutRecord.c:init_record` — outpvt initial value
- `modules/database/src/std/rec/longoutRecord.c:special` — after-flag condition check
- `modules/database/src/std/rec/longoutRecord.c:conditional_write` — On Change branch logic
