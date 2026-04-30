---
sha: ac6eb5e212270efbd8dcc95bb73ea9ac46ae7c32
short_sha: ac6eb5e
date: 2021-06-20
author: Dirk Zimoch
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callback_request
tags: [callback, initialization, null-deref, lifecycle, guard]
---

# callbackRequest: No Guard Against Uninitialized Callback Queue

## Root Cause
`callbackRequest()` indexes into `callbackQueue[priority]` and calls
`epicsRingPointerPush(mySet->queue, pcallback)`. If `callbackInit()` was never
called (or if `callbackCleanup()` already destroyed the queues), `mySet->queue`
is `NULL`. The push would dereference a null pointer, crashing the IOC.

This is especially dangerous because `callbackRequest()` can be called from
interrupt context (ISR), where a crash is unrecoverable.

The companion fix in `callbackCleanup()` sets `mySet->semWakeUp = NULL` and
`mySet->queue = NULL` after destroying them, so subsequent spurious calls to
`callbackRequest()` after shutdown can be detected.

## Symptoms
- Calling any device support or record-processing code that triggers
  `callbackRequest()` before IOC startup completes → null dereference crash.
- Calling `callbackRequest()` from a driver interrupt after `callbackCleanup()`
  has run during IOC shutdown → null dereference crash.

## Fix
In `callbackCleanup()`: set `mySet->semWakeUp = NULL` and `mySet->queue = NULL`
after destroying each. In `callbackRequest()`: add an early guard
`if (!mySet->queue) { epicsInterruptContextMessage(...); return S_db_notInit; }`.

## Rust Applicability
Applies. In base-rs, the callback queue equivalent is likely an `mpsc` channel
sender or a tokio `Notify`. The question is: can a device driver or record
processing function call `callback_request()` before the runtime channel is
initialized, or after the receiver has been dropped (closing the channel)? If
the sender is stored in an `Option<Sender>` or behind a `Mutex<Option<...>>`,
a missing `None` check is the Rust analog.

## Audit Recommendation
In `base-rs/src/server/database/callback.rs`, verify that `callback_request()`
checks that the queue/channel is initialized (not `None`/dropped) before
attempting to push. Confirm that `callback_cleanup()` sets the queue reference
to `None` so post-shutdown calls are detected rather than panicking.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackCleanup` — added null-assign for queue/semWakeUp after destroy
- `modules/database/src/ioc/db/callback.c:callbackRequest` — added `if (!mySet->queue)` guard returning `S_db_notInit`
