---
sha: 13d6ca598cca495b2e559e808392ed22265c57e3
short_sha: 13d6ca5
date: 2025-02-05
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/init_hooks.rs
    function: init_hook_register
tags: [initHook, idempotent, double-register, lifecycle, startup]
---
# initHookRegister: make idempotent and use mallocMustSucceed

## Root Cause
`initHookRegister(func)` appended a new `initHookLink` node to the
`functionList` linked list without checking for duplicates. If called twice
with the same function pointer (e.g., from a shared library loaded multiple
times, or a module that registers at both static-init time and `iocsh`
execution), the hook was registered twice and would fire twice for every
`initHookCall`. Additionally, the original code returned -1 on malloc failure
and the caller's check of the return value was inconsistent — most callers
ignored it.

## Symptoms
Double-registration caused hook functions to execute twice per state
transition. For hooks that allocate resources or start threads, this caused
double-initialization, resource leaks, or assertion failures.

## Fix
- Acquire `listLock` before the duplicate check to make it thread-safe.
- Walk the `functionList` and silently return 0 if `func` is already present.
- Replace `malloc` + `printf` on failure with `mallocMustSucceed` (aborts on
  OOM rather than returning -1, since allocation failure here is unrecoverable).
- Return value is now always 0.

## Rust Applicability
In `base-rs`, init hooks are likely registered via a `Vec<fn()>` or
`DashMap<TypeId, fn()>`. If the same function/closure can be registered twice,
verify idempotency. The `mallocMustSucceed` → panic on OOM behavior is natural
in Rust (OOM aborts by default). The lock-before-duplicate-check pattern maps
to a `Mutex<Vec<fn()>>` where the lock covers both the contains-check and the
push.

## Audit Recommendation
In `base-rs/src/server/database/init_hooks.rs::init_hook_register`, verify:
1. Duplicate registration is detected (function pointer or comparable key
   equality check) before insertion.
2. The check and insert are atomic under the same lock (no TOCTOU window).
3. No hook is called twice for the same `InitHookState` transition.

## C Locations
- `modules/libcom/src/iocsh/initHooks.c:initHookRegister` — add duplicate check under `listLock`, use `mallocMustSucceed`
