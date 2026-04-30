---
sha: 3a2d225682d79f79c84852dbcb98d54cb250d62f
short_sha: 3a2d225
date: 2023-07-22
author: Sebastian Marsching
category: other
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [fprintf, error-propagation, io-error, accumulate, epicsString]
---
# epicsStrPrintEscaped Silently Loses fprintf Error Return Values

## Root Cause
`epicsString.c:epicsStrPrintEscaped()` accumulates the output of multiple
`fprintf()` calls using `nout += fprintf(...)`. The C standard specifies
that `fprintf()` returns a negative value on error. Adding a negative return
to `nout` produces a smaller (or negative) `nout` — not -1 — so the caller
cannot detect that an error occurred by simply checking `nout < 0`.

Specifically:
1. If only some `fprintf` calls fail, the negative values partially cancel
   positive ones, making the total nout look like a small successful write.
2. If all calls fail, the (wrong) sum is returned rather than -1, still
   not reliably signaling failure.
3. There was no `NULL` check for the `fp` argument.
4. No `NULL`/empty check for `s`.

## Symptoms
- `write()` errors to the log file or network socket are silently swallowed;
  callers that rely on the negative-return error convention to detect broken
  I/O pipelines would miss errors.
- Returned byte count is incorrect when any individual `fprintf` fails.

## Fix
- Check `fp == NULL` and `s == NULL` upfront.
- Per iteration, capture `fprintf` return in `rc`; if `rc < 0`, immediately
  return `rc` to propagate the error.
- Only accumulate `nout += rc` when `rc >= 0`.

## Rust Applicability
`eliminated` — Rust's `Write` trait propagates `io::Error` via `Result`;
`write!` and `writeln!` return `Result<(), io::Error>` and the `?` operator
ensures errors are not silently dropped. `epicsStrPrintEscaped` is a utility
for the `errlog` subsystem; epics-rs uses standard Rust I/O.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/misc/epicsString.c:epicsStrPrintEscaped` — `nout += fprintf(...)` swallows error returns
