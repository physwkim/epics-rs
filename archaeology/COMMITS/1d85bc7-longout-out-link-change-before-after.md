---
sha: 1d85bc7424576fb1f7eb6c890d6a42c8d54254f4
short_sha: 1d85bc7
date: 2021-03-10
author: Joao Paulo Martins
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/records/longout.rs
    function: special
tags: [longout, special, link-change, before-after, ordering]
---

# longout special() sets link-changed flag before OUT link is updated

## Root Cause

The `special()` function in `longoutRecord.c` is called twice for each field
modification: once *before* the change (`after == 0`) and once *after*
(`after == 1`). The code set `prec->outpvt = OUT_LINK_CHANGED` when `!after`
(i.e., in the pre-change call), not in the post-change call.

Because `outpvt` is checked in `conditional_write()` and then reset to
`OUT_LINK_UNCHANGED` after the write, if record processing happened to run
*between* the pre-call setting the flag and the actual link update completing,
the flag would be consumed by an unrelated processing pass and the link-change
write would never happen. Conversely, if no processing ran, the flag would still
be set when the post-update processing came, but the link change itself was
partially applied at flag-set time.

## Symptoms

- With `OOPT = On Change` and `OOCH = YES`, modifying the OUT field does not
  reliably trigger a device support write.
- The behavior is timing-dependent (race between CA put on OUT field and any
  concurrent record processing).

## Fix

Changed `if (!after)` to `if (after)` in `special()` for the `longoutRecordOUT`
case, so `outpvt = OUT_LINK_CHANGED` is only set *after* the new link value is
in place. A subsequent record processing will then correctly see the flag and
write to the new device.

## Rust Applicability

Any Rust record type that uses a `special()` / field-change hook to set a
"force-write" state flag must set that flag in the *post* call (`after == true`),
not the pre-call. Setting state in the pre-call creates a race window where the
flag can be consumed before the link is fully updated.

## Audit Recommendation

1. Find the Rust `longout` record `special` hook — confirm the force-write flag
   is set only in the `after = true` branch.
2. Check other output records with OOPT/On-Change logic for the same pattern.

## C Locations
- `modules/database/src/std/rec/longoutRecord.c:special` — was `!after`, now `after` for the OUT field case
