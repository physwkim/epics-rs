---
sha: 5d9ffe15da05f44ccc946d4869778c0e6ff8c004
short_sha: 5d9ffe1
date: 2024-06-18
author: Grzegorz Kowalski
category: leak
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [caget, memory-leak, free, PV-value, CA-client]
---
# caget: PV value buffer not freed after printing

## Root Cause
In `caget.c`'s inner loop over PVs, each `pvs[n].value` pointer is
allocated to hold the CA get result.  After printing the value the
buffer was never freed, leaking one allocation per PV per invocation.
For command-line tools this is immediately reclaimed by process exit,
but it masks the pattern in longer-lived processes that embed caget-style
logic.

## Symptoms
Minor memory leak per PV in `caget`.  Not observable in practice because
the process exits immediately after printing, but detected by memory
sanitisers or leak checkers (e.g., Valgrind).

## Fix
Added `free(pvs[n].value)` at the end of the per-PV print loop in
`caget()`.

## Rust Applicability
Eliminated.  In Rust, CA client buffers are owned by `Vec<u8>` or a
typed response struct that drops automatically when it goes out of scope.

## Audit Recommendation
None required.

## C Locations
- `modules/ca/src/tools/caget.c:caget` — added free(pvs[n].value) after printing each PV value
