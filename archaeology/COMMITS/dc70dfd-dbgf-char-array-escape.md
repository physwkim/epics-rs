---
sha: dc70dfd62553390e39b94e6d11142f679ff49050
short_sha: dc70dfd
date: 2022-07-28
author: Dirk Zimoch
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbgf, DBF_CHAR, printBuffer, escape, non-printable]
---
# dbgf: CHAR array printBuffer overflows MAXLINE with unescaped binary data

## Root Cause
`printBuffer()` in `dbTest.c` used `sprintf(pmsg, "\"%.*s\"", chunk, ...)` to format DBF_CHAR array chunks. It computed `chunk` as `min(len, MAXLINE-5)` bytes of raw buffer data. If the raw bytes were printable ASCII, this fit within `MAXLINE`. However, non-printable bytes (control characters, binary data) require escape sequences when printed (e.g., `\x1b` for ESC), which can be up to 4 bytes per input byte — so `chunk` bytes of input can expand to up to `4*chunk` bytes of output, overflowing `pmsg[MAXLINE]`.

## Symptoms
Buffer overflow in `dbgf` output when a DBF_CHAR waveform record contains non-printable bytes (binary data, embedded NULs, high-byte values). Can corrupt stack memory or crash the IOC shell.

## Fix
Replace `sprintf(pmsg, "\"%.*s\"", chunk, ...)` with:
1. Call `epicsStrnEscapedFromRawSize()` to determine the escaped size of `chunk` bytes.
2. Reduce `chunk` until the escaped output fits in `MAXLINE-5`.
3. Use `epicsStrnEscapedFromRaw()` to write the escaped string.
Commit `dc70dfd`.

## Rust Applicability
Rust's `format!` / `write!` use heap-allocated `String`; there is no fixed output buffer to overflow. A Rust `dbgf` equivalent would just call a `.escape_default()` or similar iterator. Eliminated by design.

## Audit Recommendation
No audit needed — Rust string formatting is heap-allocated. No fixed-size output buffer risk.

## C Locations
- `modules/database/src/ioc/db/dbTest.c:printBuffer` — `sprintf` with raw char array data, no escape
