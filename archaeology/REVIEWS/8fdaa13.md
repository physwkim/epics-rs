---
short_sha: 8fdaa13
status: not-applicable
files_changed: []
---
The C bug restored an `errlogFlush()` call inside `errlog.c::eltc(yesno)`: toggling the "to-console" flag without first draining the buffered errlog queue could lose messages. In `epics-base-rs` there is no `errlog` subsystem with a buffered queue. The audit target `src/log/errlog.rs` does not exist. Logging is provided by four trivial macros in `src/runtime/log.rs` (`rt_debug!`, `rt_info!`, `rt_warn!`, `rt_error!`) that all expand to a synchronous `eprintln!`. There is no internal queue, no `eltc()` console toggle, no async sink, and therefore no flush ordering hazard — every message is delivered synchronously before the macro returns. The data-loss class of bug is structurally impossible.
