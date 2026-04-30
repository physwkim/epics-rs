---
sha: 059d32a975a67093dbc9a6359ef6a765c8659da8
short_sha: 059d32a
date: 2023-05-25
author: Ralph Lange
category: race
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/channel.rs
    function: channel_open
tags: [uninitialized, db-field-log, memset, struct-init, dbchannel]
---
# dbChannel Type Probe Struct Has Uninitialized Members

## Root Cause
`dbChannel.c:dbChannelOpen()` constructs a `db_field_log probe` struct for
use as a type probe when opening a channel filter chain. The struct was
manually initialized field-by-field:

```c
probe.type = dbfl_type_val;
probe.ctx  = dbfl_context_read;
probe.field_type  = ...;
probe.no_elements = ...;
probe.field_size  = ...;
probe.sevr = NO_ALARM;
probe.stat = NO_ALARM;
probe.time.secPastEpoch = 0;
probe.time.nsec = 0;
```

Several fields were missing from this list (the struct contains more members
than those listed). The uninitialized fields would be read by filter plugins'
`channel_register_pre` / `channel_register_post` callbacks that inspect the
probe, producing undefined behavior or incorrect filter configuration.

Found by static analysis (cppcheck / SonarQube).

## Symptoms
- Filter plugins that read probe fields beyond those explicitly initialized
  receive indeterminate values.
- Could lead to incorrect filter state, wrong type probing, or crashes in
  plugins that dereference pointer members left as garbage.

## Fix
Replace the explicit field-by-field initialization with `memset(&probe, 0,
sizeof(probe))` followed by only the non-zero fields. This ensures all
padding and unlisted members are zero-initialized.

## Rust Applicability
`partial` — Rust's `Default::default()` or struct initialization with `..
Default::default()` provides the same guarantee as `memset` to zero. When
implementing `dbChannelOpen` equivalent in `base-rs`, the `DbFieldLog`
probe struct should use `DbFieldLog { field_type, no_elements, field_size,
.. Default::default() }` to ensure all fields start from a known state.

## Audit Recommendation
In `base-rs` `channel.rs`, verify that the `DbFieldLog` probe struct used
during channel filter chain construction is fully zero-initialized before
being passed to filter `register` callbacks. Prefer `Default::default()`
construction over manual field assignment.

## C Locations
- `modules/database/src/ioc/db/dbChannel.c:dbChannelOpen` — partial manual init of `db_field_log probe`
