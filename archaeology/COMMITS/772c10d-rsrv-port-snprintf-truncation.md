---
sha: 772c10d904c2c149ce2154996091858514f27265
short_sha: 772c10d
date: 2024-06-14
author: Tynan Ford
category: network-routing
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/server/rsrv.rs
    function: rsrv_init
tags: [snprintf, port-number, truncation, RSRV_SERVER_PORT, environment]
---

# RSRV_SERVER_PORT Truncated for Port Numbers Above 9999

## Root Cause
`caservertask.c:rsrv_init` used:
```c
epicsSnprintf(buf, sizeof(buf)-1u, "%u", ca_server_port);
buf[sizeof(buf)-1u] = '\0';
```
The `sizeof(buf)-1u` argument reserves one byte for null termination but passes
a reduced size to `epicsSnprintf`, which already guarantees null termination
within the given size. For a port number like `10000` (5 digits), if `buf` was
exactly 5 bytes, `epicsSnprintf(buf, 4, "%u", 10000)` would write `"1000"` —
a truncated port number. The extra manual null-write at `sizeof(buf)-1u` then
either overwrites valid data or writes to the byte after the truncated string.
Result: `RSRV_SERVER_PORT` env var is set to a truncated string like `"1000"`
instead of `"10000"`.

## Symptoms
- `RSRV_SERVER_PORT` environment variable set to truncated string for ports >= 10000.
- CA clients reading `RSRV_SERVER_PORT` to determine the server port would
  connect to the wrong port (e.g., port 1000 instead of 10000).
- Affected the beacon and search-reply advertisement mechanism.

## Fix
Changed to `epicsSnprintf(buf, sizeof(buf), "%u", ca_server_port)` — let
`epicsSnprintf` use the full buffer including the null terminator slot,
removing the redundant manual null-write.

## Rust Applicability
Applies. In ca-rs, the server port number is formatted into a string for
`RSRV_SERVER_PORT` env var advertisement. The Rust `format!("{}", port)` call
is safe, but the equivalent of setting the env var should be audited to ensure
the full port string is written without truncation, especially for ports
in the 10000-65535 range.

## Audit Recommendation
In `ca-rs/src/server/rsrv.rs` (or equivalent init function): verify that the
port number formatted for `RSRV_SERVER_PORT` uses the full port value without
any manual buffer size reduction. Search for `env::set_var("RSRV_SERVER_PORT"`.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:rsrv_init` — `epicsSnprintf` with `sizeof(buf)-1u` truncated 5-digit ports
