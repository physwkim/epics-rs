---
short_sha: b6fffc2
status: not-applicable
files_changed: []
---
The `db_convert.rs`, `db_const_link.rs`, and `db_fast_link_conv.rs` files do not exist in base-rs. The current `DbFieldType` enum (`crates/epics-base-rs/src/types/dbr.rs`) has no `UInt32`/`ULong` variants — `Long` is `i32` only — so the specific "double-bound checked against ULONG_MAX before assignment to uint32" pattern has no analog. String→Long conversion in `EpicsValue::convert_to` goes through `to_f64() as i32`, which saturates rather than wraps and is well-defined Rust semantics. If `UInt32`/`ULong` are added later, string conversion must use `u32::MAX` (4294967295.0_f64) as the bound, not `u64::MAX`, or parse as `u64` directly and `try_into::<u32>()`.
