---
sha: 9af7ff3b3f09ca38185c655443c11eae7f161c13
short_sha: 9af7ff3
date: 2025-10-08
author: Andrew Johnson
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbLoadRecords, error-message, dedup, softMain, UX]
---
# Don't duplicate dbLoadRecords error message in softMain

## Root Cause
When `dbLoadRecords()` failed, it printed its own error message via
`fprintf(stderr, ERL_ERROR " failed to load '%s'\n", file)`. The caller
in `softMain.cpp` then also called `errIf(..., std::string("Failed to load: ") + optarg)`,
which threw a `std::runtime_error` whose message was printed again in the
catch block: `std::cerr << ERL_ERROR ": " << e.what() << "\n"`. The result
was the same error printed twice to stderr.

## Symptoms
On `dbLoadRecords` failure, stderr shows the same error message twice,
confusing operators trying to diagnose database loading problems.

## Fix
- Pass an empty string to `errIf` so the thrown exception carries no
  additional message.
- In the catch block, check `e.what()[0] != '\0'` before printing, so empty
  messages are silently discarded.
- Minor: fix the `dbLoadRecords` error string from `"failed"` to `"Failed"`
  for consistency.

## Rust Applicability
`eliminated` — Rust's error handling via `Result` / `?` propagation does not
duplicate error messages. The fix is a UX cleanup with no Rust analog.

## Audit Recommendation
No audit needed. Verify that epics-rs's database loader does not print
duplicate error messages on failure.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbLoadRecords` — error string capitalization fix
- `modules/database/src/std/softIoc/softMain.cpp` — `errIf` now passes empty message to avoid duplication
