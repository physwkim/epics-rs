---
sha: 1c566e21102e254a47974e2526847fa3d7117ecc
short_sha: 1c566e2
date: 2021-02-27
author: Andrew Johnson
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/record_support.rs
    function: init_record
tags: [aai-record, device-support, pass1-init, buffer-allocation, lifecycle]
---
# aai record: allow device support to defer init_record to pass 1

## Root Cause
The Array Analog Input (aai) record called device support's `init_record()`
only during pass 0 of IOC initialization.  Pass 0 runs before links are fully
established, so Soft Channel device support had to manually call
`dbInitLink()` and allocate the array buffer itself.  If the buffer was
allocated before the linked record initialized its buffer, the pointer could
be stale or point to the wrong allocation.

The real bug: when aai was linked to a compress/histogram/subArray record
(which use `bptr` and `cvt_dbaddr`/`get_array_info`), and the aai record
initialized first in pass 0 before the linked record's `bptr` was set, the
aai's copy of `bptr` was NULL or dangling.

## Symptoms
Segfault during IOC startup or first scan when aai is linked (via INP) to
a compress, histogram, or subArray record if aai initializes first in pass 0.

## Fix
Added pass-1 callback mechanism to aai:
- Device support returns `2` (now `AAI_DEVINIT_PASS1`) from `init_record` in
  pass 0 to request a second call in pass 1.
- `aaiRecord.c:init_record` saves this by setting `pact = 2`; in pass 1 it
  calls `init_record` again then sets `pact = FALSE`.
- `devAaiSoft.c:init_record` now returns `2` immediately in pass 0 and in the
  pass-1 call reads INP into the already-allocated buffer (record support
  handles allocation).

## Rust Applicability
In `base-rs`, record initialization likely happens in a single async phase
after all links are resolved, making the pass-0/pass-1 split unnecessary.
However, if a two-phase init is implemented (e.g. to match upstream behavior),
ensure device support can signal a deferred pass-1 callback.

## Audit Recommendation
Verify `base-rs/src/server/database/record_support.rs` `init_record` handles
two-phase initialization for aai-equivalent records.  Ensure device support
cannot capture a buffer pointer before the linked record has allocated it.

## C Locations
- `modules/database/src/std/rec/aaiRecord.c:init_record` — pass1 request handling
- `modules/database/src/std/dev/devAaiSoft.c:init_record` — returns AAI_DEVINIT_PASS1 in pass 0
