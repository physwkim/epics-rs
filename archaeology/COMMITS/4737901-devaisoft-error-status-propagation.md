---
sha: 473790124bbf1a5f0cd377082ae71a399caf3a30
short_sha: 4737901
date: 2020-02-13
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/dev_ai_soft.rs
    function: read_ai
tags: [error-status, device-support, ai, SoftDevice, link-alarm]
---

# devAiSoft read_ai returns error status on device read failure

## Root Cause
In `devAiSoft`'s `read_ai()`, the function unconditionally returned `2` (meaning "success, do not convert") regardless of whether `dbGetLink()` succeeded. The `return 2` was placed after the `if(status == 0)` success block, so even when `dbGetLink` returned an error (e.g., the input link was disconnected), the record was marked with `udf=FALSE` and `dpvt` set, then returned `2` as if everything was fine.

## Symptoms
An `ai` record with a `SoftDevice` and a disconnected or erroring input link would not raise `LINK_ALARM / INVALID_ALARM`. The record appeared healthy (no UDF, no alarm) while returning a stale or zero value. Test regression: `testdbPutFieldOk` was incorrect — the correct expectation is `testdbPutFieldFail(-1, ...)`.

## Fix
Moved `return 2` inside the success branch (`if(status == 0) { ... return 2; }`). The `else` branch sets `dpvt = NULL` and falls through to `return status` (which is the error code). This correctly propagates the read failure as a non-zero status, causing the record to raise `LINK_ALARM`.

## Rust Applicability
In base-rs device support for soft AI (or any read device), the success return value (equivalent to `2` = "raw value already converted") must not be returned on a failed link read. The error status must propagate so the record processing layer can set the appropriate alarm severity. Check any `read_ai` / `read_*` soft-device implementations.

## Audit Recommendation
In `base-rs/src/server/database/dev_ai_soft.rs::read_ai` (or equivalent), verify that the "no-convert" success indicator is returned only within the `Ok` branch, and errors are propagated via `Err` or a non-OK status code to trigger alarm setting.

## C Locations
- `modules/database/src/std/dev/devAiSoft.c:read_ai` — `return 2` moved inside success block; error path returns `status`
