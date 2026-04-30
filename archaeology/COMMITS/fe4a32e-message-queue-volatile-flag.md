---
sha: fe4a32e425ab0a5b8fd080dae1a5f2f26a591714
short_sha: fe4a32e
date: 2023-06-13
author: Michael Davidsaver
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [volatile, race, memory-order, message-queue, unlock]
---

# Volatile flag misuse in epicsMessageQueue receive path

## Root Cause
`threadNode::eventSent` was declared `volatile bool`, which in C++ does not provide any memory ordering guarantees — it only prevents compiler optimization of reads/writes, not CPU reordering. The field was read *after* `epicsMutexUnlock()` in `myReceive()`, meaning the compiler (or CPU) could reorder the load to before the unlock. This is a classic volatile-vs-atomic mistake: the lock that protects `eventSent` was released before the field was consumed.

## Symptoms
In theory, the receive function could read a stale (pre-write) value of `eventSent` after the sender had already set it, causing a spurious "message not sent" return (-1) even when a message was successfully transmitted. Under normal operation this is very unlikely, but under high contention or on weakly-ordered architectures it becomes more probable.

## Fix
Removed `volatile`. Captured `threadNode.eventSent` into a local `bool wasSent` while still holding the mutex (before `epicsMutexUnlock`), then used `wasSent` after the unlock. This correctly places the read inside the critical section where the mutex provides full memory ordering.

## Rust Applicability
Eliminated. Rust has no `volatile` keyword for data races. Cross-thread data sharing requires `Arc<Mutex<T>>` or `Arc<Atomic*>`, both of which have correct memory ordering. The pattern of "read under lock, use after unlock" is naturally expressed in Rust via `let val = guard.field; drop(guard); use(val)`.

## Audit Recommendation
No action needed. As a general Rust pattern, watch for any `Arc<UnsafeCell<T>>` with manual unlock sequencing in `ca-rs` or `base-rs` — but standard Rust idioms prevent this class of bug.

## C Locations
- `modules/libcom/src/osi/os/default/osdMessageQueue.cpp:myReceive` — read `eventSent` before unlock into local `wasSent`
- `modules/libcom/src/osi/os/default/osdMessageQueue.cpp:threadNode` — removed `volatile` from `eventSent`
