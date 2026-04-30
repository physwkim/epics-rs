---
sha: 8c9dd71f6e790dbe5d5963b0b569d84125a18447
short_sha: 8c9dd71
date: 2025-11-07
author: Dirk Zimoch
category: bounds
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [strncpy, truncation, null-terminator, buffer-safety, gcc-warning]
---
# Fix strncpy without explicit null terminator and off-by-one truncation

## Root Cause
Multiple sites used `strncpy(dst, src, n)` where the copy length `n` equalled
the full buffer size (not `n-1`), relying on a subsequent `dst[n-1] = '\0'`
that was either missing or placed after a branch. GCC's `-Wstringop-truncation`
warning fires when `strncpy` is called with `n == sizeof(dst)` because the
standard allows `strncpy` to omit the null terminator when the source is
exactly `n` bytes long.

Specific sites:
- `Cap5.xs`: `strncpy(data.dbr_string, ..., MAX_STRING_SIZE)` without null
  terminator — fixed by adding `data.dbr_string[MAX_STRING_SIZE-1] = 0`.
- `dbAccess.c:getOptions`: `strncpy(pbuffer, amsg, sizeof(amsg)-1)` then
  `pbuffer[sizeof(amsg)-1] = '\0'` separately — changed to use `DB_AMSG_SIZE`
  constant consistently and place null terminator correctly.
- `dbAccess.c:getLinkValue`: `strncpy(pbuf, rtnString, maxlen-1)` — changed to
  `strncpy(pbuf, rtnString, maxlen)` + `pbuf[maxlen-1] = 0` (full range then clamp).
- `dbStaticLib.c`: `strncpy(directory, path, length)` → `memcpy` (exact known length).
- `iocsh.cpp`: `strncpy(argBuf->sval, arg, slen)` → `memcpy(argBuf->sval, arg, slen+1)`.
- `epicsTime.cpp`, `osiSock.c`: `strncpy` of known-length strings → `memcpy`.

## Symptoms
- No runtime crashes reported; the bugs are latent: if source string is exactly
  buffer-size bytes, `strncpy` leaves no null terminator, causing subsequent
  string operations to read past the buffer.
- GCC `-Wstringop-truncation` warnings during build.

## Fix
Replace `strncpy` with `memcpy` where the source length is known, or ensure an
explicit `dst[n-1] = '\0'` follows every `strncpy(dst, src, n)` where `n ==
sizeof(dst)`.

## Rust Applicability
Rust's `str`/`String`/`&[u8]` API does not have `strncpy` semantics; all string
operations are bounds-checked and null-termination is not part of the type.
When writing FFI code that fills C string buffers, use
`std::ffi::CString::new()` + `copy_nonoverlapping` + explicit null termination.
This entire class of bug is eliminated by Rust's ownership model. No audit
needed for pure Rust code.

## C Locations
- `modules/ca/src/perl/Cap5.xs:CA_put,CA_put_callback` — missing null after `strncpy(dbr_string, ..., MAX_STRING_SIZE)`
- `modules/database/src/ioc/db/dbAccess.c:getOptions` — amsg strncpy inconsistency
- `modules/database/src/ioc/db/dbAccess.c:getLinkValue` — off-by-one in strncpy length
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbAddOnePath` — strncpy → memcpy for exact-length path copy
- `modules/libcom/src/iocsh/iocsh.cpp:cvtArg` — strncpy → memcpy for null-terminated arg copy
- `modules/libcom/src/osi/epicsTime.cpp:epicsTimeToStrftime` — strncpy → memcpy for known-length substrings
- `modules/libcom/src/osi/osiSock.c:sockAddrToA` — strncpy of string literal → memcpy
