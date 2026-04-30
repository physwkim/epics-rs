---
sha: 4720b61c1f3c9db769f0e416250d43aaf3a4120e
short_sha: 4720b61
date: 2024-02-11
author: Freddie Akeroyd
category: race
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread, Windows, DLL, race, shutdown]
---

# Windows setThreadName race during short-lived process shutdown

## Root Cause
On Windows, `setThreadName()` was called from inside the newly created thread (`epicsWin32ThreadEntry`). During very short-lived process execution (e.g., test runners, MSI builds), the process can begin unloading DLLs while the new thread is still starting. If the DLL that `setThreadName()` calls into gets unloaded between the moment the thread gets a DLL handle and the moment it makes the call, a crash occurs. This is a classic DLL unload-order race during process teardown.

## Symptoms
Occasional crash during process termination, seen exclusively with statically linked EPICS executables and only for threads created near process exit. Running IOCs are not affected.

## Fix
Moved `setThreadName()` to be called from the *creating* thread (in `epicsThreadCreateOpt`) rather than from the new thread itself. A similar fix was applied to `osdTime.cpp`'s PLL thread. The creating thread runs in a context where DLL teardown is not yet in progress.

## Rust Applicability
Eliminated. Rust's `std::thread::Builder::name()` sets the thread name before the thread starts, from the spawning context. Tokio also uses a pre-spawn naming API. There is no equivalent DLL-unload race in Rust's threading model.

## Audit Recommendation
No action needed. If any `ca-rs` Windows-specific code uses unsafe Win32 thread naming APIs (`SetThreadDescription` etc.) from inside a spawned thread, verify they are called from the spawner instead.

## C Locations
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsWin32ThreadEntry` — removed `setThreadName()` call
- `modules/libcom/src/osi/os/WIN32/osdThread.c:epicsThreadCreateOpt` — added `setThreadName()` here
- `modules/libcom/src/osi/os/WIN32/osdTime.cpp:_pllThreadEntry` — removed, moved to `startPLL()`
