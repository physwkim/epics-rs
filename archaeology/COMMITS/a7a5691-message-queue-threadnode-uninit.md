---
sha: a7a56912ebb6c728316465e079397d3b5cc65d88
short_sha: a7a5691
date: 2023-06-13
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [uninitialized, message-queue, struct, UB, thread]
---

# Uninitialized threadNode struct in epicsMessageQueue

## Root Cause
`struct threadNode` in `osdMessageQueue.cpp` was allocated on the stack without initialization. The `ELLNODE link` member (used for intrusive linked-list operations) was left with garbage values. If the list-traversal code ever inspected or followed the `link` pointers before a message was sent (e.g., during a timeout or early cancellation path), it would dereference garbage pointers — undefined behavior with potential for crash or memory corruption.

## Symptoms
Sporadic crashes or memory corruption during message queue receive operations under contention or timeout conditions. The bug is latent — it may not manifest in normal usage but is a real UB risk under stress.

## Fix
Added an inline constructor to `threadNode` that zero-initializes all members: `evp=NULL`, `buf=NULL`, `size=0`, `eventSent=false`, and `memset(&link, 0, sizeof(link))`. This ensures the struct is always in a consistent initial state regardless of how it is created.

## Rust Applicability
Eliminated. Rust guarantees all stack-allocated values are initialized before use (enforced by the borrow checker and definite-assignment analysis). Stack-allocated structs with `Default` or explicit initialization are always sound. The `tokio::sync::mpsc` and `std::sync::mpsc` channels used as message queue equivalents in Rust don't have this issue.

## Audit Recommendation
No action needed. As a general pattern, if any `ca-rs` or `base-rs` code has `unsafe` blocks that create `MaybeUninit` structs, audit those for proper initialization before use.

## C Locations
- `modules/libcom/src/osi/os/default/osdMessageQueue.cpp:threadNode` — added inline constructor with full initialization
