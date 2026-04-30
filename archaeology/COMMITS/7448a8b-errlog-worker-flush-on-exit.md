---
sha: 7448a8bfa98fc7153bb1ab8c9b9e4914baba297a
short_sha: 7448a8b
date: 2022-11-14
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/errlog.rs
    function: errlog_thread
tags: [errlog, worker-thread, shutdown, flush, at-exit]
---
# errlog worker exits loop before draining buffer at shutdown

## Root Cause
The errlog worker loop condition was `while (!pvt.atExit)`. When `atExit` was set during shutdown, the worker exited the loop immediately — even if there were messages still queued in the ring buffer (`pvt.log->pos > 0`). Those queued messages were silently discarded. Additionally, any thread waiting on `errlogFlush()` (i.e., with `nFlushers > 0`) would not receive a final wakeup after the worker exited, causing the flush caller to wait indefinitely.

## Symptoms
- Log messages queued just before IOC shutdown are silently dropped.
- Callers of `errlogFlush()` during shutdown hang forever because the worker exits without triggering `waitForSeq`.

## Fix
- Change the loop condition to `while (1)` and move the `atExit` exit check to after the buffer-empty test: exit only when `atExit == true AND buffer is empty`.
- After breaking out of the loop, trigger `waitForSeq` if any flushers are waiting.
- In `msgbufCommit`, suppress queuing new messages (without dropping, they go to console) when `atExit` is set.
Commit `7448a8b`.

## Rust Applicability
If base-rs has an async errlog worker task, the shutdown sequence must ensure the task drains its channel before exiting. A tokio task that simply receives a shutdown signal and returns without draining a `mpsc::Receiver` has the same bug. The fix pattern: on shutdown notification, continue processing items until the channel is empty, then exit. Use `channel.close()` + `while let Some(msg) = rx.recv().await { ... }` drain loop.

## Audit Recommendation
In `base-rs/src/log/errlog.rs`, verify the errlog worker task: (1) drains remaining buffered messages when `atExit` signal arrives, (2) notifies any waiting `flush()` callers after the drain completes.

## C Locations
- `modules/libcom/src/error/errlog.c:errlogThread` — premature exit before buffer drain
- `modules/libcom/src/error/errlog.c:msgbufCommit` — missing atExit guard to avoid queuing post-shutdown
