---
sha: 144f9756eac4e800f18d3d9a35f0d42a712614fd
short_sha: 144f975
date: 2024-06-13
author: JJL772
category: other
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/ioc_register.rs
    function: null
tags: [iocsh, error-propagation, iocshSetError, startup-script]
---
# iocsh: propagate error codes from db/libcom commands via iocshSetError

## Root Cause
The iocsh command wrappers in `dbIocRegister.c` and `libComRegister.c` called
their underlying functions but discarded return values — they were `void`
wrappers that silently swallowed errors. When a command like `dbLoadRecords`,
`dbgf`, `scanppl`, or `iocLogInit` failed, the iocsh script continued without
error indication. This made it impossible to detect failures in startup scripts
or automated tests.

Additionally, several functions were `void` returning when they should return
`int` (e.g., `dlload`, `zoneset`), and null-pointer guards for missing required
arguments did not set an error code.

## Symptoms
Iocsh scripts that encountered errors (missing DB file, bad PV name, etc.)
continued executing silently. Automated startup verification could not detect
partial-initialization failures.

## Fix
Wrap all iocsh command function calls with `iocshSetError(func(...))`. Change
`dlload` and `zoneset` from `void` to `int` returning. Add `iocshSetError(-1)`
at null-pointer guard sites. Add `iocshSetError(-1)` at `dbStateCreate/Set/
Clear/Show` null-sid guard sites.

## Rust Applicability
In `base-rs`, iocsh commands are registered via a trait or function registry.
If the Rust iocsh implementation ignores `Result::Err` from command handlers
(e.g., calls `.ok()` silently), startup errors will be swallowed. The Rust
equivalent of `iocshSetError` is propagating `Err` up the iocsh execution
chain and setting a non-zero exit code for the script.

## Audit Recommendation
In `base-rs/src/server/database/ioc_register.rs` (or equivalent iocsh
registration), verify that iocsh command wrappers check their return values and
set the iocsh error state. Ensure the iocsh runner exits with a non-zero code
if any command returns an error.

## C Locations
- `modules/database/src/ioc/db/dbIocRegister.c` — wrap 25+ commands with `iocshSetError`
- `modules/database/src/ioc/dbStatic/dbStaticIocRegister.c:dbPvdTableSizeCallFunc` — wrap with `iocshSetError`
- `modules/database/src/ioc/misc/dlload.c:dlload` — change to `int`, return -1 on failure
- `modules/libcom/src/iocsh/libComRegister.c` — wrap `iocLogInit`, `generalTimeReport`, etc.
- `modules/libcom/RTEMS/posix/rtems_init.c` — wrap `nfsMount`, `zoneset`, add null guards
