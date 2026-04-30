---
sha: 2e44ca586f4aac27ad7e9ae487ea2b1698f9741e
short_sha: 2e44ca5
date: 2022-07-26
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbConvertJSON, errlog, stderr, logging, json-parse-error]
---
# dbConvertJSON: JSON parse errors written to stderr instead of errlog

## Root Cause
`dbPutConvertJSON()` and `dbLSConvertJSON()` used `fprintf(stderr, ...)` to report yajl JSON parse errors. The EPICS errlog subsystem (`errlogPrintf`) is the standard channel for IOC runtime diagnostics: it handles log buffering, remote logging to iocLogServer, and listener callbacks. Writing directly to `stderr` bypasses all of these, so JSON parse errors during IOC startup or runtime were invisible to log listeners and not forwarded to the log server.

## Symptoms
JSON parse errors in database link values (e.g., malformed JSON in constant links) were printed only to the process's stderr, not captured by errlog listeners or forwarded to the central log server. Silent to CA log clients.

## Fix
Replace `fprintf(stderr, ...)` with `errlogPrintf(...)` in both `dbPutConvertJSON` and `dbLSConvertJSON` error paths. Commit `2e44ca5`.

## Rust Applicability
In Rust, JSON parse errors in link initialization should use the `log` crate (or equivalent) rather than `eprintln!`. This is a logging-channel concern, not a logic bug; Rust code should already use structured logging.

## Audit Recommendation
No production logic audit needed. Verify base-rs JSON link parse errors use `log::error!` / `tracing::error!` not `eprintln!`.

## C Locations
- `modules/database/src/ioc/db/dbConvertJSON.c:dbPutConvertJSON` — `fprintf(stderr)` → `errlogPrintf`
- `modules/database/src/ioc/db/dbConvertJSON.c:dbLSConvertJSON` — `fprintf(stderr)` → `errlogPrintf`
