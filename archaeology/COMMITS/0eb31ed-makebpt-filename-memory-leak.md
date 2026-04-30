---
sha: 0eb31ed70b4f6de71e812f079159553ab18c965a
short_sha: 0eb31ed
date: 2024-06-13
author: Grzegorz Kowalski
category: leak
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [makeBpt, memory-leak, free, filename, build-tool]
---
# makeBpt build tool leaks outFilename and pname allocations

## Root Cause
In `makeBpt.c::main`, `outFilename` and `pname` are allocated with
`malloc` or `strdup` to hold file path strings.  On the normal exit path
(after successfully writing the output breakpoint table file) neither
pointer was freed before returning.

## Symptoms
Minor memory leak in the `makeBpt` build-time tool.  Not observable at
runtime (process exits immediately), but detected by Valgrind/LSAN.

## Fix
Added `free(outFilename)` and `free(pname)` immediately before the
`return(0)` at the end of the successful path.

## Rust Applicability
Eliminated.  Rust's `String` and `PathBuf` types manage their allocations
through RAII; no manual free is needed.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/bpt/makeBpt.c:main` — added free(outFilename) and free(pname) before return
