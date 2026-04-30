---
short_sha: f4be9da
status: not-applicable
files_changed: []
---
The audit target `src/server/database/callback.rs::callback_request` does not exist in `epics-base-rs`. There is no callback subsystem (`callbackInit`/`callbackStop`/`callback_request`/CALLBACK struct/three-priority worker queues) in the rewrite — searched the crate for `callback_request`/`callbackInit`/`AsyncCallback`/`CallbackQueue` and found no matches. Async work in base-rs is dispatched directly via `tokio::spawn` of typed async closures, so a CALLBACK struct with a NULL function pointer is unrepresentable: the callable is always a `Future` produced from a concrete function/closure, and the type system rejects "uninitialized" function fields at compile time. No fix needed.
