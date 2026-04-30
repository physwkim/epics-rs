---
sha: 02a24a144d0c062311212c769926c1e2df5a1a52
short_sha: 02a24a1
date: 2020-03-08
author: Michael Davidsaver
category: race
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [race, pthread, refcount, double-free, joinable]
---

# POSIX thread joinable refcount incremented after pthread_create — double-free race

## Root Cause
In `osdThread.c::epicsThreadCreateOpt`, the extra reference count for
`epicsThreadMustJoin()` was incremented **after** `pthread_create`. This
creates a race: if the new thread runs immediately and self-joins before
`pthread_create` returns, the join path calls `free_threadInfo` first, then
the main thread increments the refcount on already-freed memory, and later
decrements it, causing a double-free or use-after-free.

Additionally, `epicsThreadMustJoin` read and cleared the `joinable` flag
non-atomically (`if(!id->joinable)` then `id->joinable = 0`), allowing two
concurrent callers to both observe `joinable==1` and both attempt
`pthread_join`, producing undefined behaviour.

## Symptoms
- Intermittent crash / heap corruption on fast-starting joinable threads.
- Double `pthread_join` on the same tid when two threads call
  `epicsThreadMustJoin` concurrently.

## Fix
1. Move the refcount increment to **before** `pthread_create`; if
   `pthread_create` fails, immediately decrement it back.
2. Canonicalise `joinable` to 0/1 at init time so an atomic
   compare-and-swap (`epicsAtomicCmpAndSwapIntT(&id->joinable, 1, 0)`)
   can be used in `epicsThreadMustJoin` to atomically claim the join right
   and prevent double-join.
3. Remove the trailing `id->joinable = 0` after the join (now redundant).

## Rust Applicability
Rust uses `std::thread::JoinHandle` / `tokio::task::JoinHandle` which are
single-owner types. The ownership rules (move semantics) prevent double-join:
once `join()` / `await` is called on a `JoinHandle` the handle is consumed.
There is no shared `joinable` flag. Eliminated.

## Audit Recommendation
None — Rust ownership model eliminates this class of bug.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadCreateOpt` — refcount increment timing bug
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadMustJoin` — non-atomic joinable flag check
