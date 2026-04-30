---
sha: a352865df9b0da81ef6dab6be48eb21ab98f0846
short_sha: a352865
date: 2023-10-27
author: Michael Davidsaver
category: other
severity: low
rust_verdict: partial
audit_targets:
  - crate: ca-rs
    file: src/client/udpiiu.rs
    function: exception_resp_action
tags: [ANSI, error-format, stderr, udpiiu, logging]
---

# Print ANSI-colored error prefix to stderr in udpiiu and tools

## Root Cause
Multiple `fprintf(stderr, "error ...")` calls used the plain string `"error"` instead of the `ERL_ERROR` macro, which expands to an ANSI-colored `"\033[31mError\033[0m"` (or similar) when the terminal supports it. This made error messages from `udpiiu.cpp`, `caput`, and various IOC tools visually indistinguishable from informational output.

## Symptoms
Error messages printed to stderr lacked the colored `Error:` prefix that the EPICS logging infrastructure provides for other messages. Operators scanning log output could miss these error indications.

## Fix
Replaced literal `"error"` / `"Error"` strings with `ERL_ERROR` in error-printing `fprintf` calls across `udpiiu.cpp`, `caput.c`, `makeBpt.c`, `dbLexRoutines.c`, `dbStaticIocRegister.c`, `dbStaticLib.c`, `dbLoadTemplate.y`, `msi.cpp`, `registerAllRecordDeviceDrivers.cpp`, and `iocsh.cpp`.

## Rust Applicability
Partial. The `ca-rs` client has a UDP IIU equivalent (`udpiiu.rs`) that handles exception responses from the CA server. The `exceptionRespAction` function should use the appropriate colored/structured logging macro (`error!()` from the `tracing` or `log` crate) rather than `eprintln!("error ...")`. This is cosmetic but affects operator UX.

## Audit Recommendation
In `ca-rs/src/client/udpiiu.rs`, verify that exception response error messages use `tracing::error!()` or equivalent rather than `eprintln!()`. Similarly audit `caput` CLI equivalent in `ca-rs` tools if present.

## C Locations
- `modules/ca/src/client/udpiiu.cpp:exceptionRespAction` — `"error condition"` → `ERL_ERROR " condition"`
- `modules/ca/src/client/udpiiu.cpp:caRepeaterRegistrationMessage` — error prefix added
- `modules/ca/src/tools/caput.c:main` — two error paths updated
