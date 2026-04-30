---
sha: a83a85af7c730164e8e3b1c1260bd41187e81f99
short_sha: a83a85a
date: 2019-06-04
author: Michael Davidsaver
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [off-by-one, buffer-overflow, access-security, alloc, bounds]
---
# asLib: fix one-byte under-allocation for unresolved host prefix

## Root Cause
In `asHagAddHost()`, when `aToIPAddr()` fails (DNS resolution fails), the code
prepends the literal string `"unresolved:"` to the hostname:
```c
phagname = asCalloc(1, sizeof(HAGNAME) + sizeof(unresolved)-1 + strlen(host));
strcpy(phagname->host, unresolved);
strcat(phagname->host, host);
```
The `sizeof(unresolved)-1` subtracts the NUL terminator of the prefix — correct
so far. But `strlen(host)` does not include the NUL terminator of the hostname,
so the concatenated result `"unresolved:<host>"` needs `strlen("unresolved:")
+ strlen(host) + 1` bytes. The `+1` for the final NUL was missing, causing a
one-byte write past the end of the allocation.

## Symptoms
- One-byte heap write overflow in `strcat(phagname->host, host)` when a
  hostname in an ACF file cannot be resolved by DNS.
- Silent corruption; may manifest as crashes or security-bypass in the
  access-security subsystem under ASAN.

## Fix
Change `sizeof(unresolved)-1` → `sizeof(unresolved)` (which includes the NUL
terminator of `"unresolved:"`, effectively adding the missing byte for the
final NUL of the concatenated string).

## Rust Applicability
Eliminated. `String::push_str` / `format!` handle sizing automatically.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/as/asLibRoutines.c:asHagAddHost` — corrected `sizeof(unresolved)-1` → `sizeof(unresolved)`
