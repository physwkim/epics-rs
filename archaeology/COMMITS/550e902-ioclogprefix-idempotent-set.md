---
sha: 550e902bf3c88422257a42af5503271849db80b1
short_sha: 550e902
date: 2023-01-19
author: Andrew Johnson
category: lifecycle
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/log_client.rs
    function: ioc_log_prefix
tags: [iocLog, prefix, idempotency, errlog, startup]
---
# iocLogPrefix warns on identical re-set instead of accepting silently

## Root Cause
`iocLogPrefix()` in `logClient.c` stored the prefix once and rejected any subsequent call with a warning message, even if the caller passed the exact same prefix string. This caused spurious noise during IOC startup sequences that call `iocLogPrefix` more than once with the same value (e.g., from multiple startup scripts that each call the same setup function).

## Symptoms
Duplicate IOC log initialization (e.g., two scripts both calling `iocLogPrefix("SYS:")`) produced a confusing "prefix was already set" warning in the console log even though no actual conflict existed.

## Fix
Add a `strcmp` check before printing the warning: if the new prefix is identical to the existing one, silently return. Commit `550e902`.

## Rust Applicability
If base-rs has a log-prefix API (`ioc_log_prefix`), it should likewise accept idempotent re-sets silently and warn only on genuine conflicts (different prefix value).

## Audit Recommendation
Check `base-rs/src/log/log_client.rs::ioc_log_prefix` — verify the "already set" guard compares prefix values, not just checks for `Some(_)`.

## C Locations
- `modules/libcom/src/log/logClient.c:iocLogPrefix` — missing strcmp before warning
