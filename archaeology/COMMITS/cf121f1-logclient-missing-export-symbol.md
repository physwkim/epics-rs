---
sha: cf121f1c14a96b9a3171e5fd46d991cbffa7386b
short_sha: cf121f1
date: 2019-09-23
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [shared-library, symbol-export, windows, dll, build]
---

# logClient missing epicsExportSharedSymbols breaks DLL symbol export

## Root Cause
A prior commit (`f85454a`) removed `#define epicsExportSharedSymbols` from the
top of `logClient.c` while refactoring the debug flag. On Windows (and other
platforms using EPICS' symbol-visibility macros), `epicsShareFunc` / `epicsShareAPI`
expand to `__declspec(dllexport)` only when `epicsExportSharedSymbols` is
defined. Without it, all public functions in `logClient.c` (including
`logClientSend`, `logClientFlush`, `logClientCreate`, etc.) became unexported
from the shared library, causing link failures or silent runtime resolution
errors when IOC code called them through the DLL import table.

## Symptoms
On Windows builds, IOC applications could fail to link or crash at runtime with
"undefined symbol" / "entry point not found" errors for `logClientSend` and
related functions after this commit.

## Fix
Re-add `#define epicsExportSharedSymbols` before the EPICS headers in
`logClient.c`.

## Rust Applicability
Rust does not use a C-style visibility macro system. `pub` functions in a
`cdylib` are exported by default (controlled via `#[no_mangle]` and
`#[export_name]` attributes). No equivalent of this bug class exists. Eliminated.

## Audit Recommendation
No action needed in Rust code.

## C Locations
- `modules/libcom/src/log/logClient.c` — missing `#define epicsExportSharedSymbols`
