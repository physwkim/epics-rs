---
sha: e329fa3296bf587a1c2159a48da9d826f1310afc
short_sha: e329fa3
date: 2022-04-24
author: Andrew Johnson
category: lifecycle
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [exception-safety, strftime, CA-client, abort, timezone]
---

# CA Client: protect against epicsTime::strftime() throwing on timezone errors

## Root Cause
`ca_client_context::vSignal()` called `epicsTime::strftime()` unconditionally
to format the current time for error diagnostics. On systems with broken or
missing timezone data (e.g. embedded targets, containers without `/etc/localtime`,
or corrupt TZ databases), `strftime()` can throw a `std::exception`. Since
`vSignal()` was not inside any try/catch, an uncaught exception propagated up
through the CA client library into application code, causing `std::terminate()`
(abort) — crashing the entire process.

This is a CA diagnostic/logging path called when the CA client detects an error,
so timezone problems would cause a secondary crash obscuring the original error.

## Symptoms
- EPICS CA client process aborts with `terminate called after throwing an instance
  of 'std::exception'` on machines with bad TZ configuration.
- The crash occurs in the error-reporting path (`vSignal`), making the root CA
  error invisible in logs.

## Fix
Wrapped the `strftime` call in a `try/catch(std::exception&)` block:
- On success: formats and prints the human-readable timestamp.
- On exception: logs the exception message via `errlogPrintf` and falls back
  to printing raw `secPastEpoch.nsec` integers.

## Rust Applicability
Rust does not use exceptions. The equivalent Rust concern is a `format!` or
`chrono` call panicking on invalid time data. In `ca-rs` or `base-rs`,
any time formatting in diagnostic/logging paths should use `?` propagation
or explicit error handling rather than `unwrap()`/`expect()`. In practice,
`std::time::SystemTime` and `chrono` formatting do not panic on bad TZ in Rust.

This specific pattern is eliminated by Rust's lack of exceptions.

## Audit Recommendation
None — eliminated. However, audit any `unwrap()` in `ca-rs` diagnostic/logging
paths for panic safety under bad system state.

## C Locations
- `modules/ca/src/client/ca_client_context.cpp:ca_client_context::vSignal` — try/catch added
