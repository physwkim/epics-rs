---
sha: 8fdaa13c9755aec0ca2efae4160cf062409589b1
short_sha: 8fdaa13
date: 2021-02-22
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/log/errlog.rs
    function: eltc
tags: [errlog, flush, eltc, log-sync, data-loss]
---

# errlog: restore errlogFlush() call in eltc()

## Root Cause
`eltc(yesno)` toggles whether the errlog system echoes log messages to the
console. A previous commit had removed the `errlogFlush()` call from `eltc()`.
This flush was necessary to ensure that any messages buffered in the errlog
queue were delivered before the "to console" flag was changed. Without the
flush, messages buffered while console output was disabled could be silently
lost if `eltc(1)` was called before the errlog task had a chance to process
the queue.

On RTEMS, the errlog task and test harness have specific synchronization
requirements: `dbCaLinkTest` calls `eltc(1)` to re-enable console output after
a test, expecting all preceding log messages to be delivered. Without
`errlogFlush()`, messages from the test were lost.

## Symptoms
- Log messages buffered before `eltc(1)` call could be lost or reordered.
- On RTEMS: `dbCaLinkTest` synchronization failure — test framework not seeing
  expected log output because messages were not flushed.
- Potential data loss in any code path that disables then re-enables console
  logging (`eltc(0)` ... `eltc(1)`) without an explicit flush in between.

## Fix
Re-added `errlogFlush()` call at the end of `eltc()` after setting
`pvt.toConsole`. This ensures the errlog worker thread processes all buffered
messages before `eltc()` returns.

## Rust Applicability
In `base-rs` log system, if there is a buffered/async log sink with a
"to console" toggle (analogous to `eltc`), toggling the flag must be followed
by a flush/drain of the buffer to avoid message loss. The Rust pattern:

```rust
pub fn eltc(enabled: bool) {
    let mut state = LOG_STATE.lock().unwrap();
    state.to_console = enabled;
    drop(state);
    errlog_flush();  // drain the queue synchronously
}
```

If the log is fully synchronous (no internal queue), no flush is needed.
If it uses `tokio::sync::mpsc` internally, `errlog_flush()` should await
the channel to drain (e.g., send a `Flush` sentinel and await its ack).

## Audit Recommendation
- In `base-rs/src/log/errlog.rs:eltc`: verify that after toggling `to_console`,
  the internal log queue is flushed before returning.
- If errlog uses an async mpsc channel, check that the flush is awaited, not
  just a non-blocking signal.

## C Locations
- `modules/libcom/src/error/errlog.c:eltc` — errlogFlush() re-added
