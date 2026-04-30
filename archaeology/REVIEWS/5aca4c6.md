---
short_sha: 5aca4c6
status: not-applicable
title: dbEvent: clear callBackInProgress before signaling pflush_sem
crate: base-rs
---

# Review: 5aca4c6

## Verdict
not-applicable — structurally absent in base-rs.

## Analysis
The C bug is about ordering of `callBackInProgress = FALSE` versus
`epicsEventSignal(pflush_sem)` inside `event_read()` of the dbEvent
subsystem (per-subscription callback flag with cancel-rendezvous).

`epics-base-rs` does not implement that mechanism. PV-level
subscriptions are tokio mpsc-based: subscribers hold a
`mpsc::Receiver<MonitorEvent>`, and notification is fire-and-forget via
`tx.try_send` (`crates/epics-base-rs/src/server/pv.rs:48`,
`record_instance.rs:1397`). Cancellation is by Receiver drop; there is
no "callback in progress" atomic flag, no `pflush_sem`, and no
flag-clear-then-signal handshake to mis-order.

A repo-wide search for `in_progress`, `in_flight`,
`callback_in_progress`, `pflush`, `flush_sem` in the base-rs `src/`
tree yields zero matches. The closest analog (`processing: AtomicBool`
on `RecordInstance`, `record_instance.rs:1138-1178`) is a recursion
guard, not a cancel rendezvous, and its release ordering is already
correct via `ProcessGuard::drop` (`Ordering::Release` store, no signal).

## Files Changed
None.
