---
short_sha: faac1df
status: not-applicable
files_changed: []
---
The C bug: `dbAccess.c::dbPut` posted `DBE_PROPERTY` unconditionally on any write to a field marked `pfldDes->prop != 0`, even when the new value matched the existing field content — causing every CA put of an unchanged property field (e.g. re-writing the current EGU) to fan a wasteful `DBE_PROPERTY` event to all subscribers. The fix introduces a `propertyUpdate` flag gated by a `memcmp` of old vs new bytes.

`epics-base-rs` does not post a separate `DBE_PROPERTY` event from the put path at all. `crates/epics-base-rs/src/server/database/field_io.rs::put_pv_and_post_with_origin` posts at most one combined `EventMask::VALUE | LOG | ALARM` event, and only when `value_changed || alarm_changed` (an `EpicsValue` `PartialEq` comparison of `old_value` vs `new_value` at lines 224 and 252–256). Property-class metadata is surfaced through the per-record metadata cache (`RecordInstance::cached_metadata` / `notify_field_written`); invalidating the cache is cheap and does not push an event to subscribers — the next monitor delivery just sees fresh metadata.

Because there is no unconditional property-event post, there is no spurious-event regression to suppress. The change-gating the C fix introduces is already the only behavior in the Rust path (and it gates the unified value/alarm event, not a separate property event).
