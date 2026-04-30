---
short_sha: baa4cb5
status: not-applicable
files_changed: []
---
The C bug is in `callback.c::callbackSetQueueSize`: missing `if (size <= 0)` guard, so a non-positive `size` argument flowed through to ring-buffer allocation, producing zero-capacity queues or huge sign-extended `size_t` allocations.

`crates/epics-base-rs` has no `callback.rs`, no `callbackSetQueueSize` analogue, and no priority-queued callback subsystem at all. Record processing in base-rs runs through `server/database/processing.rs` and the tokio task graph (`runtime/task.rs`, `runtime/supervise.rs`) rather than the C `cbLow/cbMedium/cbHigh` priority queues. The grep for `set_queue_size`/`CallbackQueue` returns no matches in base-rs source. There is no API surface that accepts a queue capacity that could be set to zero, so the guard the C commit adds has no place to live in the current Rust code.
