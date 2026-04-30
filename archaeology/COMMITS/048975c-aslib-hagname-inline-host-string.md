---
sha: 048975ccc708987afdda43712485ff9dc24b0b29
short_sha: 048975c
date: 2019-06-05
author: Michael Davidsaver
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [buffer-size, off-by-one, access-security, string, bounds]
---
# asLib HAGNAME: inline host string, fix allocation size for unresolved prefix

## Root Cause
`HAGNAME` stored `char *host` as a separate pointer that was manually set to
`(char *)(phagname + 1)` after allocation. This two-step pattern is fragile
(the pointer set could be missed) and made size calculations error-prone.

Additionally, when `aToIPAddr()` fails (unresolved host), the allocation was:
```c
asCalloc(1, sizeof(HAGNAME) + sizeof(unresolved)-1 + strlen(host))
```
but `sizeof(unresolved)` includes the NUL terminator of the string literal, and
`-1` was needed to avoid double-counting. This commit makes the flex-array
member change: `char host[1]` instead of `char *host`, so the pointer-set step
is eliminated. The `sizeof(unresolved)` size for the unresolved-prefix path is
also corrected (`sizeof(unresolved)-1` → `sizeof(unresolved)` because the
flex-array `[1]` already counts one byte).

Note: commit a83a85a did the `-1` fix first; this commit then changes the
struct to flex-array and re-adjusts.

## Symptoms
- Potential 1-byte write past the end of the `host` buffer in the
  `unresolved:` prefix concatenation path.

## Fix
Change `HAGNAME.host` from `char *` to `char host[1]` (C flexible-array-member
style). Remove all `phagname->host = (char *)(phagname + 1)` assignments.
Correct the `sizeof(unresolved)` accounting for each allocation path.

## Rust Applicability
Eliminated. Rust `String` / `Vec<u8>` handle allocation and sizing
automatically; no manual flexible-array-member pattern needed.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/as/asLib.h:HAGNAME` — `char *host` → `char host[1]`
- `modules/libcom/src/as/asLibRoutines.c:asHagAddHost` — removed pointer-set, corrected sizes
