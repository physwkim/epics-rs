---
sha: 64011ba0f3fd77fbc388a0c4dcf2fcab0c15fed7
short_sha: 64011ba
date: 2023-03-09
author: Henrique Silva
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dev/sa_soft.rs
    function: read_sa
tags: [NORD, subArray, device-support, event-layering, duplicate-post]
---
# Remove duplicate NORD db_post_events from subArray device support

## Root Cause
`devSASoft.c:read_sa()` contained `db_post_events(&prec->nord,
DBE_VALUE|DBE_LOG)` — event posting from device support. This is architecturally
incorrect: device support should read/write hardware state and update record
fields; event notification is the record layer's responsibility. The duplicate
posting in device support was a workaround for the missing post in
`subArrayRecord.c:process()` (fixed by the companion commit 51c5b8f). Having
both active would double-post NORD events for the default soft channel device.

## Symptoms
- With the companion fix (51c5b8f) applied, monitors subscribed to NORD would
  receive two DBE_VALUE events per process cycle when using the soft device.
- Without the companion fix, monitors would receive the event only from device
  support (correct behaviour, wrong layer), and custom device support that
  omitted its own posting would see no NORD events.

## Fix
Remove the `nord` snapshot variable and the `db_post_events` call from
`devSASoft.c:read_sa()`. Event posting now happens exclusively in
`subArrayRecord.c:process()` (commit 51c5b8f), ensuring all device support
implementations (not just the soft channel) benefit from the notification.

## Rust Applicability
In `base-rs`, device support traits must NOT call `db_post_events` (or any
monitor notification API) directly. All event posting must flow through the
record support `process()` function. If a device support `read()` impl is
currently calling a notification function, it should be removed. The Rust type
system can enforce this by keeping the monitor notification API out of the
device support trait interface.

## Audit Recommendation
In `base-rs/src/server/database/dev/`: grep for any call to `post_events` or
equivalent monitor notification within device support implementations. These
should not exist; only record support `process()` functions should call them.

## C Locations
- `modules/database/src/std/dev/devSASoft.c:read_sa` — removed `nord` snapshot and `db_post_events` call for NORD
