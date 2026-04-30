---
sha: 87acb98d1e3d7efe0bb788d1dfe24a107dfa3200
short_sha: 87acb98
date: 2022-08-20
author: Michael Davidsaver
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/addr_list.rs
    function: add_addr_to_ca_address_list
tags: [hostname, overflow, fixed-buffer, EPICS_CA_ADDR_LIST, bounds]
---

# CA hostname length limit overflow when parsing EPICS_CA_ADDR_LIST

## Root Cause
`addAddrToChannelAccessAddressList()` used a custom `getToken()` function that copied tokens into a fixed-size `char buf[256]`. If an entry in `EPICS_CA_ADDR_LIST` (a hostname or IP address) exceeded 255 characters, `getToken()` would silently truncate it — the loop wrote up to `bufSIze` chars and force-NUL-terminated at `pBuf[bufSIze-1]`, meaning long hostnames were silently mangled. This would cause `aToIPAddr()` to fail on a truncated name, dropping the address entirely.

## Symptoms
Long hostnames (>255 characters, possible with FQDNs or encoded labels) in `EPICS_CA_ADDR_LIST` would be silently truncated, causing CA clients to fail to connect to servers at those addresses with no error message beyond "Bad internet address or host name".

## Fix
Removed the fixed-buffer `getToken()` function entirely. The new code copies the full config string into a `std::vector<char>` (unbounded) and uses `epicsStrtok_r()` to tokenize it in place. Each token pointer is passed directly to `aToIPAddr()` without any length limit.

## Rust Applicability
Applies. The Rust CA client (`ca-rs`) parses `EPICS_CA_ADDR_LIST` from the environment. If it tokenizes this string into a fixed-length buffer (e.g., a `[u8; N]` array or `String::with_capacity(N)` that truncates), long hostnames would be dropped. Rust `String` is heap-allocated so truncation is unlikely, but the parsing logic should be verified to not apply artificial length caps.

## Audit Recommendation
Audit `ca-rs/src/client/addr_list.rs` (or equivalent env-parsing code). Verify:
1. No fixed-length buffer is used for hostname tokens.
2. Each token from the space-split `EPICS_CA_ADDR_LIST` is passed in full to DNS resolution.
3. Very long tokens (>253 chars) are rejected with a proper error, not silently truncated.

## C Locations
- `modules/ca/src/client/iocinf.cpp:getToken` — removed (was the buggy fixed-buffer tokenizer)
- `modules/ca/src/client/iocinf.cpp:addAddrToChannelAccessAddressList` — rewritten with vector + strtok_r
