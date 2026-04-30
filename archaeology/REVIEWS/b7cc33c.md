---
short_sha: b7cc33c
status: not-applicable
files_changed: []
---
The C bug ordered `db_post_events(DBE_PROPERTY)` after the second `dbPutSpecial` pass that triggers `DBE_VALUE | DBE_LOG`, so monitors received the value event before the property metadata update — clients formatted the new value using stale EGU/PREC/enum strings.

`epics-base-rs` does not post `DBE_PROPERTY` as a separate event from the put path. `crates/epics-base-rs/src/server/database/field_io.rs::put_pv_and_post_with_origin` posts a single combined `EventMask::VALUE | LOG | ALARM` event, and the snapshot it carries is built via `RecordInstance::make_monitor_snapshot` which pulls fresh display/control/enum metadata through `cached_metadata`. `notify_field_written` invalidates that cache when a metadata-class field is written, so the next monitor snapshot already includes the updated property metadata in the same delivery — there is no second-event ordering window for clients to mis-render.

`EventMask::PROPERTY` exists as a subscription bit (`server/recgbl.rs:35`) and `DbSubscription::subscribe_with_mask` can request it, but nothing in the put/post path emits `PROPERTY`-tagged events that need to precede the value event. The ordering bug the C fix repairs has no analog in the Rust event model.
