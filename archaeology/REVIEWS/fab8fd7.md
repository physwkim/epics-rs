---
short_sha: fab8fd7
status: not-applicable
files_changed: []
---
The audit target `src/server/database/db_event.rs::db_cancel_event` does not exist in `epics-base-rs`. There is no `db_event.rs`, no `evSubscrip`, no `freeListFree`, and no suicide-event mechanism. Subscription cancellation in the rewrite is RAII via `DbSubscription::drop` in `src/server/database/db_access.rs:329-356`, which spawns a fire-and-forget `remove_subscriber(sid)` cleanup task. Double-cancel is impossible because `Drop` runs at most once per `DbSubscription`, and use-after-free is impossible because the per-subscriber `mpsc::Sender` is owned by the record's `subscribers` Vec — a `try_send` on a removed slot is a no-op (the slot is gone, not dangling). The `pSuicideEvent`/`callBackInProgress` race the C fix targets has no counterpart in the tokio mpsc + Arc<RwLock<RecordInstance>> model.
