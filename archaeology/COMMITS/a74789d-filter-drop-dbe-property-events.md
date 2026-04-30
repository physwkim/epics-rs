---
sha: a74789d9c0e55f6499b66f99cf0a745c681d884f
short_sha: a74789d
date: 2023-05-03
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/filters/decimate.rs
    function: filter
  - crate: base-rs
    file: src/server/database/filters/sync.rs
    function: filter
tags: [dbe-property, filter, monitor, decimate, sync]
---
# Decimate and Sync Filters Incorrectly Drop DBE_PROPERTY Monitor Events

## Root Cause
Both the `decimate` and `sync` channel filters' `filter()` functions had an
early-return guard that only let events pass through when
`pfl->ctx == dbfl_context_read` (i.e. a direct DB read, not a subscription
event). For subscription events with `DBE_PROPERTY` in `pfl->mask` — which
signal metadata changes like EGU, HOPR/LOPR, alarm limits — the filter
applied its decimation/synchronisation logic. In the decimate case, a
`DBE_PROPERTY` update posted while the counter was mid-cycle would be
dropped entirely. In the sync case it would be held until the state gate
opened.

`DBE_PROPERTY` events MUST be delivered immediately and unconditionally; a
client that misses an EGU or limits update will display stale metadata until
the next reconnect.

## Symptoms
- Monitors subscribed with `DBE_PROPERTY` through a `decimate` or `sync`
  filter may never deliver the initial or updated property metadata.
- Engineering unit changes, display limits or alarm severity threshold
  changes are silently dropped by these filters.

## Fix
Add `|| (pfl->mask & DBE_PROPERTY)` to the pass-through guard in both
`decimate.c:filter` and `sync.c:filter`, so `DBE_PROPERTY` events bypass
the decimation/synchronisation logic entirely.

## Rust Applicability
`applies` — If `base-rs` implements channel filter plugins (`decimate`,
`sync`, or future equivalents), each filter's `filter()` must pass through
any event with `DBE_PROPERTY` set unconditionally, before applying any
rate-limiting or gate logic. This is a semantic correctness requirement for
the EPICS monitor protocol, not just a C-ism.

## Audit Recommendation
In `base-rs` filter implementations, verify that the event dispatch loop
checks `event.mask & DBE_PROPERTY` before applying any deadband, decimation,
or synchronisation logic. The pattern is: if the event carries property data,
forward it immediately regardless of filter state.

## C Locations
- `modules/database/src/std/filters/decimate.c:filter` — missing `DBE_PROPERTY` pass-through
- `modules/database/src/std/filters/sync.c:filter` — missing `DBE_PROPERTY` pass-through
