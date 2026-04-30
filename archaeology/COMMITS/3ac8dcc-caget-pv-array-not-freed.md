---
sha: 3ac8dcc3de34b15211e56093bdccb48e46956d53
short_sha: 3ac8dcc
date: 2024-06-18
author: Grzegorz Kowalski
category: leak
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [caget, memory-leak, free, PV-array, CA-client]
---
# caget: PV array (pvs) not freed before process exit

## Root Cause
In `caget.c::main`, the `pvs` array of `pv` structs is allocated with
`malloc` to hold all PV handles.  After calling `ca_context_destroy()`,
the array was never freed.  This is a trivial process-exit leak, but it
also meant `pvs` was uninitialized on the error paths that jump past the
`malloc` (e.g., when the argument count is wrong), so `free` on an
uninitialized pointer would be undefined behavior if it were ever added
naively.

## Symptoms
Minor memory leak in `caget` main.  Not observable at runtime because
`ca_context_destroy` triggers process-exit cleanup, but caught by
Valgrind and LSAN.

## Fix
Initialized `pvs = NULL` at declaration and added `free(pvs)` before
`ca_context_destroy()`.  The NULL initialisation ensures that `free(NULL)`
on the early-exit paths is a no-op (safe by the C standard).

## Rust Applicability
Eliminated.  Rust's `Vec` drops its allocation automatically when the
owner goes out of scope.

## Audit Recommendation
None required.

## C Locations
- `modules/ca/src/tools/caget.c:main` — initialize pvs=NULL; free(pvs) before ca_context_destroy
