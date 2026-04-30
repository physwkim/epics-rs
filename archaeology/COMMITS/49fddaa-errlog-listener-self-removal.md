---
sha: 49fddaa13e10b3c524fa1d294f6d31392aa288af
short_sha: 49fddaa
date: 2022-11-15
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/errlog.rs
    function: errlog_remove_listeners
tags: [errlog, listener, self-removal, use-after-free, callback-reentrancy]
---
# errlogRemoveListeners: self-removal during callback causes use-after-free

## Root Cause
The errlog worker thread iterated its `listenerList` (an intrusive linked list) and called each listener's callback while holding `listenerLock`. If a listener callback called `errlogRemoveListeners()` to deregister itself, `ellDelete` immediately freed the node currently being iterated — the worker then read `ellNext()` from a freed pointer, causing heap corruption or a crash.

The fix adds two bit-fields to `listenerNode`: `active` (set while the callback is executing) and `removed` (set if removal was requested while active). When `errlogRemoveListeners` finds a node that is `active`, it sets `removed = 1` instead of freeing it. After the callback returns, the worker checks `removed` and performs the deferred free.

## Symptoms
Heap corruption or segfault when a log listener deregisters itself from within its own callback. Most commonly triggered during IOC shutdown when listeners call `errlogRemoveListeners` in response to an exit message they receive.

## Fix
Add `active:1` and `removed:1` bit fields to `listenerNode`. In `errlogRemoveListeners()`, check `active` before freeing; if active, set `removed = 1`. In `errlogThread()`, set/clear `active` around the callback call and perform deferred free if `removed` is set. Commit `49fddaa`.

## Rust Applicability
Rust prevents iterator invalidation at compile time (mutable borrow prevents concurrent modification). However, if base-rs uses a listener/callback chain backed by `Arc<Mutex<Vec<...>>>` or a linked list, a callback that removes itself through a shared reference could still trigger deadlock (not use-after-free, but lock re-entrance). The `removed` deferred-free pattern maps to a `pending_removal` flag or collecting indices to remove after the iteration loop completes.

## Audit Recommendation
In `base-rs/src/log/errlog.rs`, check the errlog listener dispatch loop: ensure that if a listener removes itself during iteration, the removal is deferred until after the iteration completes. Using a `Vec` with `retain()` after the callback loop (not during) is the idiomatic Rust fix.

## C Locations
- `modules/libcom/src/error/errlog.c:errlogRemoveListeners` — missing `active` check, premature free
- `modules/libcom/src/error/errlog.c:errlogThread` — missing active/removed guards around callback dispatch
- `modules/libcom/src/error/errlog.h:errlogRemoveListeners` — API doc added noting safe self-removal since this fix
