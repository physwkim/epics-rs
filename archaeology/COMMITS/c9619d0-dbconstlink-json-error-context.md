---
sha: c9619d013a871423987724b88ffb8311a012a147
short_sha: c9619d0
date: 2022-07-26
author: Dirk Zimoch
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbConstLink, json-parse, error-context, diagnostics, link-name]
---
# dbConstLink JSON parse errors omit link field name in error message

## Root Cause
`dbConstLoadLS()` and `dbConstLoadArray()` called `dbLSConvertJSON` / `dbPutConvertJSON` and returned the error status without printing any context about which record field contained the bad JSON. The JSON parser itself printed a low-level parse error, but the operator had no way to identify which PV and field triggered it.

## Symptoms
JSON parse failures during IOC startup produced cryptic low-level yajl error messages with no indication of which `.db` record field caused the problem, making troubleshooting difficult.

## Fix
After detecting a non-zero return status from the JSON converter, print `errlogPrintf("... while parsing link %s.%s %s\n", plink->precord->name, dbLinkFieldName(plink), pstr)` to add record/field context. Commit `c9619d0`.

## Rust Applicability
Rust error handling with `?` + `anyhow`/`thiserror` context wrapping (`context("while parsing link {}.{}", rec, field)`) provides the same pattern in idiomatic Rust. Not a logic bug.

## Audit Recommendation
No logic audit needed. Ensure base-rs link initialization errors include PV name and field name in the error context.

## C Locations
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadLS` — missing errlog context on JSON error
- `modules/database/src/ioc/db/dbConstLink.c:dbConstLoadArray` — missing errlog context on JSON error
