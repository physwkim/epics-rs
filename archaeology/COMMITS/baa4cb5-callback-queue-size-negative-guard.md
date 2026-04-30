---
sha: baa4cb5495cf103b4276c61b1e2a4e0077bb0d9b
short_sha: baa4cb5
date: 2025-09-30
author: Michael Davidsaver
category: bounds
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: set_queue_size
tags: [callback, queue-size, sanity-check, iocInit, bounds]
---
# callbackSetQueueSize: reject non-positive queue size before iocInit

## Root Cause
`callbackSetQueueSize(int size)` in `callback.c` only checked whether the
callback system was already initialized (`cbState != cbInit`) before
applying the caller-supplied size. A zero or negative value passed before
`iocInit` was silently accepted and later used to size the ring buffer,
resulting in either a zero-capacity queue (all callbacks dropped) or a
`malloc(0)` / enormous allocation on sign extension to `size_t`.

## Symptoms
- Zero-capacity callback queue: CA monitors never fire after `iocInit`.
- Negative size: implementation-defined behavior in the queue allocator;
  potential heap corruption or crash at startup.

## Fix
Add an early guard at the top of `callbackSetQueueSize`:
```c
if (size <= 0) {
    fprintf(stderr, "Queue size must be positive\n");
    return -1;
}
```
This prevents the nonsensical value from reaching the state machine check
or the allocator.

## Rust Applicability
`partial` — base-rs likely has an equivalent `set_queue_size` or
`CallbackQueue::new(capacity)` path. The Rust version should use `NonZeroUsize`
or a checked conversion to prevent silent zero-capacity construction. If the
capacity parameter is `usize`, a caller passing `0` would not be caught by
the type system.

## Audit Recommendation
Audit `base-rs/src/server/database/callback.rs` (or equivalent queue
initialization) for a `capacity: usize` parameter that can be zero. Add an
explicit `assert!(capacity > 0)` or return `Err` for zero values, consistent
with the C fix.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackSetQueueSize` — missing non-positive guard
