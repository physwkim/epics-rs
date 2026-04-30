---
sha: f430389ee72f08fc1f24fd2960f69c1a14bbf9eb
short_sha: f430389
date: 2022-07-30
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/ioc_init.rs
    function: ioc_shutdown
tags: [shutdown, thread-join, scan-threads, callback-threads, lifecycle]
---
# iocShutdown: always stop worker threads, not only in isolated mode

## Root Cause
In `iocInit.c:iocShutdown()`, the calls to `scanStop()` and `callbackStop()`
(which signal and join scan and callback worker threads) were guarded by:
```c
if (iocBuildMode == buildIsolated) {
    scanStop();
    callbackStop();
} else {
    dbStopServers();
}
```
This meant that in normal IOC mode (`buildFull`), `scanStop()` and
`callbackStop()` were **never called** during shutdown. Scan and callback
threads were left running (or at least not joined) even after `iocShutdown()`
returned, leading to:
- Threads accessing freed/destroyed resources (use-after-free).
- Non-deterministic shutdown ordering.
- Resource leaks on process exit if the process tried to free IOC state while
  threads still ran.

## Symptoms
- Crashes or hangs during IOC shutdown in non-isolated (normal) mode.
- Use-after-free access from scan or callback threads that outlive the IOC
  database shutdown.
- `valgrind` / AddressSanitizer reports of post-shutdown accesses.

## Fix
Moved `scanStop()` / `callbackStop()` (and their `initHookAnnounce()` calls)
outside the `if` block so they always run. The remaining `if` check now only
gates `dbStopServers()` (network-related shutdown, not needed in isolated
mode):
```c
scanStop();
callbackStop();
if (iocBuildMode != buildIsolated) {
    dbStopServers();
}
```

## Rust Applicability
In a Rust IOC runtime, scan and callback tasks are `tokio::task::JoinHandle`
or `std::thread::JoinHandle` objects. Clean shutdown requires aborting/joining
all handles. If shutdown logic conditionally skips the join, the same dangling-
task problem occurs. Rust does not prevent this pattern — a `JoinHandle` that
is dropped without `.abort()` or `.join()` simply detaches.

The key audit concern is: does the Rust IOC shutdown path join/abort ALL
background tasks unconditionally, regardless of build mode or configuration?

## Audit Recommendation
In `base-rs` IOC lifecycle (shutdown path): verify that all scan and callback
tasks are joined/aborted unconditionally on shutdown, not only in test/isolated
mode. Check for conditional `abort()` or `join()` calls that might be skipped
in the standard runtime configuration.

## C Locations
- `modules/database/src/ioc/misc/iocInit.c:iocShutdown` — scanStop/callbackStop moved outside buildIsolated guard
