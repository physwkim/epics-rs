---
sha: 5d1f572ceae8ae88cc35a280ac9d506a1b11ffe5
short_sha: 5d1f572
date: 2023-03-08
author: Henrique Silva
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dev/aai_soft.rs
    function: read_aai
  - crate: base-rs
    file: src/server/database/dev/wf_soft.rs
    function: read_wf
tags: [NORD, aai, waveform, device-support, event-layering]
---
# Remove NORD db_post_events from aai and waveform device support layers

## Root Cause
Same architectural problem as 64011ba (subArray): `devAaiSoft.c:read_aai()` and
`devWfSoft.c:read_wf()` both posted `db_post_events` for NORD changes from
within device support. This was the wrong layer. For `aai`, the companion fix
(aff7463) adds NORD posting to `aaiRecord.c:process()`. For `waveform`, the
`waveformRecord.c:process()` already had NORD posting via `db_post_events`
after `put_array_info` (fix #263 referenced in commit message), making the
device support version redundant and causing double-posting.

## Symptoms
- For `aai` records with soft channel device: NORD monitors received no events
  after this fix until the companion aaiRecord change (aff7463) was applied.
  With both applied: single correct NORD event per process.
- For waveform records: double NORD events per process cycle when using soft
  channel device.

## Fix
Remove `nord` snapshot variable and `db_post_events` call from both
`devAaiSoft.c:read_aai()` and `devWfSoft.c:read_wf()`. The companion commit
(aff7463) ensures `aaiRecord.c:process()` and `aaoRecord.c:process()` handle
NORD posting.

## Rust Applicability
Same as 64011ba: device support in `base-rs` must not post monitor events.
Verify `src/server/database/dev/aai_soft.rs::read_aai` and
`dev/wf_soft.rs::read_wf` do not call any notification API. The record support
`process()` functions in `rec/aai.rs` and `rec/waveform.rs` must own all
NORD event notifications.

## Audit Recommendation
In `base-rs/src/server/database/dev/aai_soft.rs` and `dev/wf_soft.rs`:
confirm no `post_events` calls exist. In `rec/aai.rs::process`,
`rec/waveform.rs::process`: confirm NORD change detection and posting is
present (mirrors fix aff7463 for aai).

## C Locations
- `modules/database/src/std/dev/devAaiSoft.c:read_aai` — removed `nord` snapshot and `db_post_events` for NORD
- `modules/database/src/std/dev/devWfSoft.c:read_wf` — removed `nord` snapshot and `db_post_events` for NORD
