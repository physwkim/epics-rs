---
sha: b1d9c57101557a41df4a41ec02e4bc8c3ca65266
short_sha: b1d9c57
date: 2021-10-03
author: Michael Davidsaver
category: wire-protocol
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_event.rs
    function: db_post_events
tags: [event-mask, db_field_log, monitor, DBE_VALUE, DBE_ALARM]
---

# db_field_log::mask overwritten with actual event mask on post

## Root Cause
`db_create_event_log()` initializes the `db_field_log` struct including setting
`pLog->mask = pevent->select` (the subscription's registered event mask, e.g.
`DBE_VALUE | DBE_ALARM`). When `db_post_events()` posted to multiple subscriptions
with different masks, the `pLog->mask` was set to the first subscription's
`select` mask but **not updated** for subsequent subscriptions. The pre-chain
filter (run on each subscription's `pLog`) therefore saw the wrong mask —
the mask of the first subscriber rather than the mask of the current subscriber.

This caused filter chains (e.g., `dbnd` deadband filter) to make incorrect
decisions: `pLog->mask` did not reflect the actual event bits that triggered the
post for the current subscription.

## Symptoms
- `dbnd` filter and other `pre_chain` filters saw wrong `pLog->mask` bits for
  subscriptions beyond the first.
- Filters that gated behavior on `DBE_ALARM` or `DBE_PROPERTY` bits could miss
  or incorrectly pass events for subscriptions with different masks.
- Monitor update behavior differed between the first and subsequent subscriptions
  on the same channel with different event masks.

## Fix
After `db_create_event_log()`, immediately overwrite `pLog->mask` with the
intersection of the actual posted mask and the subscription's select mask:

```c
if (pLog)
    pLog->mask = caEventMask & pevent->select;
```

This ensures that the `pLog` passed to the pre-chain filter accurately reflects
which event bits caused this particular subscription's post.

## Rust Applicability
In `base-rs`, the event posting path should pass the per-subscription effective
mask to the filter chain. If a `FieldLog` (analogous to `db_field_log`) is
created from a posted event, its `mask` field must be set to
`posted_mask & subscription.select_mask`, not just `subscription.select_mask`.

This is a protocol-correctness issue: the filter chain needs to know which
specific bits fired, not just which bits the subscription requested.

## Audit Recommendation
- In `base-rs/src/server/database/db_event.rs:db_post_events`: after creating a
  `FieldLog` for a subscription, set `field_log.mask = event_mask & sub.select`.
- In any `pre_chain` filter equivalent: verify it reads the per-post mask, not
  the subscription's registered mask.

## C Locations
- `modules/database/src/ioc/db/dbEvent.c:db_post_events` — `pLog->mask = caEventMask & pevent->select` added
