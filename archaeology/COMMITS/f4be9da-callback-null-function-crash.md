---
sha: f4be9daf4d3ab03ebc6c9d9c546c87a0efb83608
short_sha: f4be9da
date: 2023-11-03
author: Jeremy Lorelli
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: callback_request
tags: [null-pointer, callback, crash, lifecycle, database]
---

# Null callback function pointer crash in callbackRequest

## Root Cause
`callbackRequest(pcallback)` checked `pcallback != NULL` but did not check `pcallback->callback != NULL`. If a caller set up a `CALLBACK` struct without initializing the function pointer (e.g., zero-initialized or partially initialized), calling `callbackRequest()` would successfully enqueue the callback. When the callback thread dequeued and invoked `pcallback->callback(pcallback)`, it would call through a NULL function pointer, causing an immediate crash (SIGSEGV).

## Symptoms
Crash on one of the three EPICS callback threads (low/medium/high priority) whenever `callbackRequest()` was called with a `CALLBACK` struct whose `callback` field was NULL. The crash would appear asynchronously on the callback thread, making it hard to trace to the originating `callbackRequest()` call site.

## Fix
Added a NULL check for `pcallback->callback` immediately after the existing `pcallback` NULL check. Returns `S_db_notInit` and logs an error via `epicsInterruptContextMessage()` (interrupt-safe logging) rather than crashing.

## Rust Applicability
Applies. `base-rs` implements a callback/task dispatch system equivalent to EPICS `dbCallback`. If the Rust equivalent allows enqueueing a callback with no associated function (e.g., a `Box<dyn Fn()>` that is somehow `None` in an `Option<Box<dyn Fn()>>`), the same crash could occur. The Rust type system makes this less likely (non-nullable function pointers via `fn` types), but `Option<Box<dyn Fn()>>` patterns need to check for `None` before dispatching.

## Audit Recommendation
In `base-rs/src/server/database/callback.rs`, verify that `callback_request()` validates the callback function is present before enqueueing. If `CALLBACK` structs are translated to a Rust struct with `callback: Option<fn(...)>`, ensure the dispatcher panics gracefully or returns an error rather than unwrapping `None`.

## C Locations
- `modules/database/src/ioc/db/callback.c:callbackRequest` — added `if (!pcallback->callback)` guard
