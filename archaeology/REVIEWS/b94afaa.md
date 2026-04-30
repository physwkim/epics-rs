---
short_sha: b94afaa
status: not-applicable
files_changed: []
---
The `UTAG` field is not implemented in base-rs (no occurrences of `utag`/`UTAG`/`TimeStampTag` in `crates/epics-base-rs/src`). There is no `db_field_log.rs` module and `db_access.rs::get_options` does not serialize a utag in monitor/get responses. When UTAG support is added later, it must be modeled as `u64`, the wire layout must place 8 bytes immediately after `nsec` with no padding, and any link-layer `get_timestamp_tag` callback must take `&mut u64`.
