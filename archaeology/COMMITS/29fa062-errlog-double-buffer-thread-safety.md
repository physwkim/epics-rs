---
sha: 29fa0621d7b1ed51920a5f8de81ee71c2a5935f8
short_sha: 29fa062
date: 2021-02-19
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/log/errlog.rs
    function: errlog_thread
tags: [errlog, double-buffer, thread-safety, lock-contention, listener]
---
# errlog: rewrite with double-buffering to avoid holding lock during print

## Root Cause
The old errlog implementation used a single ring buffer protected by a mutex.
`errlogThread` held `msgQueueLock` while calling listener callbacks (which
include `fprintf` to console). This serialized every producer against the
print I/O, causing starvation under heavy logging load. The original `msgNode`
linked-list design allocated heap nodes per message and required the lock to be
held across the entire dispatch cycle.

## Symptoms
- Under burst logging (e.g. IOC startup), producers block waiting for the queue
  lock while errlogThread is doing slow console I/O.
- Missed messages (`missedMessages` counter increments) even when the logical
  buffer has capacity.
- Deadlock risk: any listener that itself calls `errlogPrintf` re-enters the
  same mutex path.

## Fix
Replaced the linked-list queue with a **double-buffer scheme**: two fixed
`buffer_t` regions (`bufs[2]`) — one for logging (`pvt.log`) and one for
printing (`pvt.print`). Producers write into `pvt.log` under `msgQueueLock`
using `msgbufAlloc()` and commit with `msgbufCommit()`. The worker thread
swaps the pointers under the lock, then iterates the print buffer *without
holding the lock*, allowing producers to proceed concurrently. Each message is
prefixed with a 1-byte state flag (`ERL_STATE_FREE/WRITE/READY`) so the worker
can distinguish in-progress writes from committed ones. A `flushSeq` counter
(atomic) is used by `errlogFlush()` to synchronize flush waiters.

## Rust Applicability
Rust's `base-rs` needs an equivalent errlog/logging subsystem. The bug pattern
— holding a global lock during slow I/O in a consumer thread while producers
queue messages — is equally possible in a Rust `Mutex<VecDeque>` design.
The correct Rust pattern mirrors this fix: use a `tokio::sync::mpsc` channel
(decoupled producer/consumer) or `std::sync::mpsc` so the producer never
blocks on slow I/O. If a synchronous flush is needed, use a generation counter
(`AtomicU32`) that the consumer increments after draining, and have
`errlog_flush()` await the generation advance.

## Audit Recommendation
In `base-rs/src/log/errlog.rs` (or equivalent): verify that the log message
consumer task does NOT hold any shared lock while writing to stderr/file. If
using `Mutex<Queue>`, drain the queue into a local `Vec` first, release the
lock, then call `write_all`. Confirm flush semantics use an atomic generation
counter, not a mutex-gated flag.

## C Locations
- `modules/libcom/src/error/errlog.c:errlogThread` — held msgQueueLock across listener dispatch; replaced with double-buffer swap
- `modules/libcom/src/error/errlog.c:msgbufAlloc` — new lock+alloc primitive; returns NULL if log buffer full (increments nLost)
- `modules/libcom/src/error/errlog.c:msgbufCommit` — swaps buffers and signals waitForWork when buffer was empty
