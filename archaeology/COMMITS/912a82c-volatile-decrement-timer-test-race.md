---
sha: 912a82c0b521d8e894b0930d25582e98fcfe5686
short_sha: 912a82c
date: 2023-03-08
author: Dirk Zimoch
category: race
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [volatile, atomic, decrement, timer, race-condition]
---
# Timer Test Uses volatile Counter Instead of Atomic for Thread-Safe Decrement

## Root Cause
`epicsTimerTest.cpp` used a `volatile unsigned expireCount` as a shared
counter decremented in the timer expire callback and checked on the main
thread. The `volatile` keyword in C/C++ guarantees that reads and writes
are not optimized away but does NOT provide atomicity or memory ordering
guarantees. On multi-core architectures, the `--expireCount` read-modify-write
and the `== 0u` comparison can be split across cache lines or reordered,
leading to races where:
- Multiple timer callbacks decrement simultaneously and both see non-zero,
  causing the event to never be signaled.
- The main thread reads a stale cached value.

C++11 deprecated decrement of volatile integral types in this pattern.

## Symptoms
- `testAccuracy()` in the timer test could hang indefinitely because
  `expireEvent.signal()` was never called (concurrent decrements racing to zero).
- Compiler warnings about deprecated volatile arithmetic in C++20.

## Fix
Replace `volatile unsigned expireCount` with `static int expireCount` and
use `epics::atomic::decrement(expireCount)` / `epics::atomic::set(expireCount, N)`,
which provide proper atomic read-modify-write with acquire/release semantics.

## Rust Applicability
`eliminated` — Rust does not allow `volatile` arithmetic on shared data
without `unsafe`. Shared counters in Rust use `std::sync::atomic::AtomicU32`
with `fetch_sub(1, Ordering::SeqCst)` or appropriate ordering. epics-rs timer
tests would use `Arc<AtomicU32>` naturally. The anti-pattern cannot be
introduced without `unsafe`.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/test/epicsTimerTest.cpp:expireCount` — `volatile` counter used for multi-thread decrement
- `modules/libcom/test/epicsTimerTest.cpp:delayVerify::expire` — `--expireCount` data race
