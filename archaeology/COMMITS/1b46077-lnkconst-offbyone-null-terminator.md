---
sha: 1b46077096df18933168455f7f26dd4777b46b98
short_sha: 1b46077
date: 2024-03-13
author: Simon Rose
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [off-by-one, null-terminator, strncpy, buffer-overflow, lnkConst]
---

# Off-by-One: Null Terminator Written Past End of Buffer in lnkConst

## Root Cause
`lnkConst.c:lnkConst_loadArray` used `strncpy` to copy a scalar string into a
buffer of size `*pnReq`, then wrote a null terminator at `((char *)pbuffer)[*pnReq]`
— one byte past the end of the buffer. For example, with an `aai` record of
`NELM=1, FTVL=CHAR`, the buffer is only 1 byte, but the null terminator was
written at index 1 (the second byte), overflowing by one byte.

Additionally, the code did not check `*pnReq > 0` before calling `strncpy`,
so a zero-length buffer request would call `strncpy(..., ..., 0)` and then
write at index 0 with `= 0`, which is technically valid but confused intent.

## Symptoms
One-byte heap buffer overflow for `aai CHAR` records with `NELM=1` linked via
`{const: "..."}`. Could corrupt adjacent heap metadata, causing eventual crash
or heap corruption, typically non-deterministic.

## Fix
- Added `if (*pnReq > 0)` guard.
- Changed `((char *)pbuffer)[*pnReq] = 0` to `((char *)pbuffer)[*pnReq - 1] = 0`
  so the null terminator stays within the buffer.

## Rust Applicability
Eliminated. Rust string and slice operations enforce bounds at runtime and do
not permit manual null terminator writes. The equivalent constant-link data
copy in base-rs uses `&str` / `Vec<u8>` slices with checked indexing.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/std/link/lnkConst.c:lnkConst_loadArray` — `pbuffer[*pnReq] = 0` writes one past end
