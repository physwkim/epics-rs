---
sha: d47fa4caa404aeff8ed5a218b057b64e08499857
short_sha: d47fa4c
date: 2022-08-08
author: Michael Davidsaver
category: other
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/records/asub_record.rs
    function: fetch_values
tags: [aSub, dbGetLink, constant-link, dbLinkIsConstant, record-processing]
---
# aSub record: dbGetLink called on constant input links causing error

## Root Cause
`fetch_values()` in `aSubRecord.c` iterated all 26 input links (INPA..INPZ) and called `dbGetLink()` on each unconditionally. `dbGetLink()` is not valid for constant links (links with no connected PV, initialized with a literal value); calling it on a constant link returns an error status, which caused `fetch_values` to return early with an error — effectively failing the entire record processing for any aSub record that had at least one constant input link.

The same issue was previously fixed for `ai`, `longin`, and `stringin` records (referenced as Launchpad bugs 1699445 / 1887981). This extends the fix to aSub.

## Symptoms
An aSub record with any constant input link (e.g., `field(INPA, "1.0")`) fails to process: `fetch_values` returns error, STAT/SEVR are set to INVALID, and the subroutine is never called.

## Fix
Add `if (dbLinkIsConstant(plink)) continue;` before the `dbGetLink()` call in the `for` loop over inputs. Constant links are initialized during `init_record` (via `dbConstLoadArray`); they do not need runtime fetching. Commit `d47fa4c`.

## Rust Applicability
In base-rs, the aSub record equivalent's `fetch_values` loop must check `link.is_constant()` and skip `db_get_link()` for constant links. Failing to do so would cause the same error-on-constant-link bug.

## Audit Recommendation
In `base-rs/src/server/database/records/asub_record.rs`, verify the input link fetch loop skips constant links (`dbLinkIsConstant` equivalent check) before calling the runtime DB get.

## C Locations
- `modules/database/src/std/rec/aSubRecord.c:fetch_values` — unconditional dbGetLink on all input links including constants
