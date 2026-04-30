---
sha: b890d584bce92d8374ca76d09a68c49f9a8fad05
short_sha: b890d58
date: 2020-11-13
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [iocInit, double-init, softIoc, lifecycle, startup]
---
# softIoc: prevent double iocInit when startup script runs iocInit

## Root Cause
`softMain.cpp` tracked whether a `.db` file had been loaded via the `loadedDb`
flag and called `iocInit()` automatically if set. When a startup script was
passed (via `-s` / positional argument), `softMain` ran the script through the
IOC shell but still set `loadedDb = true` with the comment "give it the benefit
of the doubt." If the script already called `iocInit()`, the main body would
call it a second time, which either crashed or silently double-initialized
subsystems.

## Symptoms
Double `iocInit()` when a startup script contained an `iocInit` command.
Depending on EPICS version: crash, assertion failure, or silent corruption of
scan/callback state.

## Fix
Introduce a separate `ranScript` boolean that is set when a startup script is
executed (replacing the old `loadedDb = true` shortcut). The `loadedDb` flag
now only tracks actual `dbLoadRecords` calls. The `epicsThreadExitMain()` path
checks `loadedDb || ranScript`, but the auto-`iocInit()` path only fires for
`loadedDb` (not `ranScript`), preventing the double-call.

## Rust Applicability
`softIoc` is a C binary entry point. The epics-rs equivalent is a Rust binary
(`softIoc.rs` or `iocsh` crate) that calls `ioc_init()` exactly once. The
Rust lifecycle enforces single-init via type-state or `Once` — the double-init
pattern cannot arise structurally. No audit needed.

## Audit Recommendation
None — eliminated by Rust's single-init lifecycle model.

## C Locations
- `modules/database/src/std/softIoc/softMain.cpp:main` — add `ranScript` flag, separate script-ran path from loadedDb auto-iocInit path
