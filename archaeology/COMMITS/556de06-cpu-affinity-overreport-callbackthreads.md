---
sha: 556de06ff2516243f71411f58709fa22feaa7819
short_sha: 556de06
date: 2026-02-06
author: Dirk Zimoch
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/callback.rs
    function: get_cpu_count
tags: [CPU-affinity, sched_getaffinity, callback-threads, taskset, overreport]
---
# epicsThreadGetCPUs overreports CPUs when affinity mask is restricted

## Root Cause
`epicsThreadGetCPUs()` on POSIX called `sysconf(_SC_NPROCESSORS_ONLN)` which
returns the number of online CPUs in the system, regardless of the calling
process's CPU affinity mask. When an IOC is launched with a restricted affinity
mask (e.g. `taskset -c 0,1 ./softIoc ...`), `sysconf` still reports all online
CPUs. `callbackParallelThreads(0, ...)` uses this count to spawn N callback
threads, spawning more threads than the process has physical CPU cores available.

## Symptoms
- IOC started with `taskset` on 2 of 32 cores spawns 32 callback threads.
- The extra 30 threads contend for 2 cores, degrading throughput and increasing
  context-switch overhead.
- `callbackParallelThreads(-N, ...)` (subtract N from available) gives
  meaningless results when the base count is inflated.

## Fix
Added a `sched_getaffinity()` probe (guarded by `#ifdef CPU_COUNT`, i.e.
Linux glibc) before the `sysconf` fallback. `CPU_COUNT(&mask)` counts only
the bits set in the affinity mask, which reflects the CPUs actually available
to this process. The `sysconf` path remains as fallback for platforms without
`CPU_COUNT`.

## Rust Applicability
In `base-rs`, any equivalent of `callback_parallel_threads(0)` that spawns
worker threads proportional to CPU count must use the affinity-aware count.
In Rust, `num_cpus::get()` on Linux already uses `sched_getaffinity` via libc.
If using Tokio's default runtime (which calls `num_cpus::get()` internally),
this is handled automatically. If `base-rs` calls `std::thread::available_parallelism()`,
note that as of Rust 1.74 it also uses `sched_getaffinity` on Linux. No custom
CPU count logic needed — but verify the library call used is affinity-aware.

## Audit Recommendation
In `base-rs/src/server/database/callback.rs`: if there is a function that
determines the default callback thread count, confirm it calls
`std::thread::available_parallelism()` (Rust ≥ 1.56, affinity-aware on Linux)
rather than a raw `num_cpus::get_physical()` or manual `sysconf` call.
On non-Linux POSIX targets, `available_parallelism()` may still report system
CPUs — document this limitation.

## C Locations
- `modules/libcom/src/osi/os/posix/osdThread.c:epicsThreadGetCPUs` — `sched_getaffinity` + `CPU_COUNT` added before `sysconf` fallback
