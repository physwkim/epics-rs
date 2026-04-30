---
sha: a8e8d22c31aaa2bfff6511017e883b5e751a3e0a
short_sha: a8e8d22
date: 2022-08-24
author: Torsten Bögershausen
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [CA-client, EPICS_CA_ADDR_LIST, hostname, buffer-overflow, DNS]
---
# iocinf.cpp: 32-byte buffer silently truncates long hostnames in CA addr list

## Root Cause
`addAddrToChannelAccessAddressList()` declared `char buf[32u]` to hold a hostname or IP address string parsed from `EPICS_CA_ADDR_LIST`. The maximum valid DNS hostname length is 255 bytes (RFC 1034), so any hostname longer than 31 characters was silently truncated. The truncated string then failed DNS resolution, the address was silently dropped from the search list, and with `EPICS_CA_AUTO_ADDR_LIST=NO`, the resulting search list was empty — causing all CA searches to fail with "Empty PV search address list".

## Symptoms
- `EPICS_CA_ADDR_LIST=<long-hostname>` + `EPICS_CA_AUTO_ADDR_LIST=NO` → "Empty PV search address list" → all PV searches fail silently.
- If `EPICS_CA_AUTO_ADDR_LIST=YES`, the long hostname is ignored but broadcast addresses are still searched, causing timeouts rather than immediate failure.
- No error or warning is printed about the truncation.

## Fix
Increase `buf[32u]` to `buf[256u]` to hold a full RFC 1034 hostname plus the NUL terminator. Commit `a8e8d22`.

## Rust Applicability
Rust's `String` / `&str` types are heap-allocated and have no fixed size limit; this buffer overflow cannot occur. The CA address-list parser in ca-rs should use `String` for hostname tokens.

## Audit Recommendation
No audit needed — Rust string types eliminate fixed-size hostname buffers. Confirm ca-rs `EPICS_CA_ADDR_LIST` parsing uses `String`, not a fixed-size `[u8; N]` buffer.

## C Locations
- `modules/ca/src/client/iocinf.cpp:addAddrToChannelAccessAddressList` — `char buf[32u]` hostname buffer
