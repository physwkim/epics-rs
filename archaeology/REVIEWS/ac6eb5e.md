---
short_sha: ac6eb5e
status: not-applicable
files_changed: []
---
The general-purpose `callbackRequest`/`callbackQueue` subsystem (epicsRingPointer-backed priority queues processed by callback worker threads) does not exist in base-rs. There is no `src/server/database/callback.rs`. Searches for `callback_request`, `callbackInit`, `callbackCleanup` in base-rs return only a single comment in `record_trait.rs:50` mentioning the C equivalent. Record processing is dispatched directly through `tokio::spawn` in scan.rs / processing.rs, and there is no globally-shared callback queue whose `Option<Sender>` could be `None` at request time. The "uninitialized queue null-deref" failure mode therefore has no analog. Nothing to fix.
