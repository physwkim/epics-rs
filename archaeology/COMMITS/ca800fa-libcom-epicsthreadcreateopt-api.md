---
sha: ca800fa57dcfa76d03666571ee6cdaa75ec55e19
short_sha: ca800fa
date: 2018-04-04
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [thread-api, epicsThreadCreateOpt, lifecycle, options, abi]
---

# Add epicsThreadCreateOpt() extensible thread creation API

## Root Cause
`epicsThreadCreate()` had a fixed signature with positional `priority` and `stackSize` arguments. Adding new thread creation options (e.g., joinability) required either breaking ABI or adding new functions for each option combination. The fixed signature made it impossible to add `joinable` without a new API.

## Symptoms
No runtime bug — this is an API extensibility fix that enables the joinable threads work in subsequent commits (`d989c8f`, `c9dcab9`). Without it, the joinable flag could not be passed to the OS-level thread creation.

## Fix
Added `epicsThreadOpts` struct with `priority`, `stackSize` (and later `joinable`) fields, `epicsThreadOptsDefaults()`, and `epicsThreadCreateOpt()`. All platform `epicsThreadCreate()` implementations were refactored to call `epicsThreadCreateOpt()` internally. ABI-compatible: `epicsThreadCreate()` is preserved as a wrapper.

## Rust Applicability
Tokio's `tokio::task::spawn()` and `std::thread::Builder` already support all relevant options (stack size, name) via builder pattern. No analog needed in Rust.

## Audit Recommendation
None.

## C Locations
- `modules/libcom/src/osi/epicsThread.h` — added `epicsThreadOpts`, `epicsThreadOptsDefaults`, `epicsThreadCreateOpt`
- `modules/libcom/src/osi/epicsThread.cpp` — `epicsThreadCreate` refactored as wrapper around `epicsThreadCreateOpt`
- Platform-specific `osdThread.c` files — all refactored to implement `epicsThreadCreateOpt`
