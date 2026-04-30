---
sha: 7c991f3f2a804e98df7ab89d83577694c8503a48
short_sha: 7c991f3
date: 2021-07-21
author: JJL772
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [null-pointer, segfault, dbtpn, bounds-check, dbNotify]
---

# dbtpn: Null Pointer Dereference When value Parameter is nullptr

## Root Cause
`dbtpn(char *pname, char *pvalue)` in `dbNotify.c` unconditionally called
`strncpy(ptpnInfo->buffer, pvalue, 80)` without checking whether `pvalue` is
`NULL`. When invoked from iocsh as `dbtpn Record` (without a value argument),
`pvalue` is `nullptr`, causing an immediate segfault in `strncpy`.

Additionally the original code used the magic number `80` for both the copy
length and the NUL-termination index instead of `sizeof(ptpnInfo->buffer)`,
creating a latent buffer sizing mismatch.

## Symptoms
Running `dbtpn Record` (no second argument) in iocsh crashes the IOC with
SIGSEGV.

## Fix
Guard `strncpy` with `if (pvalue)`. Replace magic number `80` with
`sizeof(ptpnInfo->buffer)` and `sizeof(ptpnInfo->buffer)-1` for the NUL
terminator index.

## Rust Applicability
Eliminated. Rust's type system represents optional string arguments as
`Option<&str>`. A function accepting `Option<&str>` cannot be called with a
null pointer; the null-check is enforced at the call site by the type system.
There is no equivalent of C's unchecked `char*` parameter.

## Audit Recommendation
No audit needed. The Rust equivalent of `dbtpn` would take `Option<&str>` and
use `if let Some(val) = pvalue { buf.copy_from_slice(...) }`.

## C Locations
- `modules/database/src/ioc/db/dbNotify.c:dbtpn` — added `if (pvalue)` guard around `strncpy`
