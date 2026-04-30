---
sha: bf533ac195d93dc8e30b87d2e931db12e78cd2ec
short_sha: bf533ac
date: 2020-02-13
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [WIN32, calling-convention, DLL-export, socket, epicsShareAPI]
---

# WIN32: epicsSocketEnableAddressUseForDatagramFanout missing epicsShareAPI — linker failure

## Root Cause
On Win32, all exported shared-library symbols must be decorated with both
`__declspec(dllexport)` on the definition and `__declspec(dllimport)` on the
declaration side (abstracted as `epicsShareAPI`). The definition of
`epicsSocketEnableAddressUseForDatagramFanout` in
`osdSockAddrReuse.cpp` (default platform) was missing `epicsShareAPI`, so the
symbol was not exported from the DLL. Callers in other modules that linked
against the import library found an unresolved external symbol at link time.

## Symptoms
- Link failure (unresolved external symbol) on WIN32 builds when any module
  calls `epicsSocketEnableAddressUseForDatagramFanout`.

## Fix
Add `epicsShareAPI` to the function definition so it matches the declaration.

## Rust Applicability
Rust uses Cargo and the native linker visibility model; there is no equivalent
of Win32 `__declspec(dllexport/dllimport)` decoration required in source.
`pub` functions in a `cdylib` crate are automatically exported. Eliminated.

## Audit Recommendation
None — eliminated by Rust's build model.

## C Locations
- `modules/libcom/src/osi/os/default/osdSockAddrReuse.cpp:epicsSocketEnableAddressUseForDatagramFanout` — missing epicsShareAPI on definition
