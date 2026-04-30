---
sha: 3eeebb74cd83757756acb2024560e602ee8f9118
short_sha: 3eeebb7
date: 2021-03-11
author: Michael Davidsaver
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [env-set, putenv, memory-leak, platform-compat, startup]
---

# epicsEnvSet leaks memory on every call due to putenv() ownership semantics

## Root Cause

POSIX `putenv()` takes ownership of the `"NAME=VALUE"` string passed to it —
the caller must not free it and must ensure it remains valid for the lifetime
of the process. The C implementation in `default/osdEnv.c` and `WIN32/osdEnv.c`
allocated a heap buffer with `mallocMustSucceed`, called `putenv()`, and then
either never freed it (intended: "Leaks memory, but...") or freed it on error.

On every call to `epicsEnvSet()` a new buffer was leaked. In long-running IOCs
that repeatedly set environment variables (e.g., via iocsh scripts or PV hooks
that update config), this accumulates unboundedly.

The fix replaced `putenv()` with `setenv()`/`unsetenv()` (POSIX) which make
internal copies, or `_putenv_s()` (WIN32) which also copies. The explicit heap
allocation was removed entirely.

## Symptoms

- Heap growth proportional to the number of `epicsEnvSet()` calls over the IOC
  lifetime.
- Valgrind / ASAN reports for every `epicsEnvSet` call site at IOC startup or
  reconfiguration.

## Fix

- `default/osdEnv.c`: replaced `mallocMustSucceed` + `putenv` with
  `setenv(name, value, 1)`, returning error via `errlogPrintf` (never fatal).
- `WIN32/osdEnv.c`: replaced heap-alloc + `putenv` with `_putenv_s(name, value)`.
- `vxWorks/osdEnv.c`: vxWorks `putenv()` documented to copy — use on-stack or
  small-alloc buffer for the combined string, free after `putenv`.
- Darwin/RTEMS/iOS/Solaris `osdEnv.c` files: deleted (folded into `default`
  which already used `setenv`).

## Rust Applicability

Rust's `std::env::set_var()` / `remove_var()` own the value internally and do
not leak. No equivalent issue in the Rust implementation. This bug is eliminated
by the language's memory model.

## C Locations
- `modules/libcom/src/osi/os/default/osdEnv.c:epicsEnvSet` — was leaking the "NAME=VALUE" buffer
- `modules/libcom/src/osi/os/WIN32/osdEnv.c:epicsEnvSet` — was leaking via putenv()
- `modules/libcom/src/osi/os/vxWorks/osdEnv.c:epicsEnvSet` — fixed to free after putenv copy
