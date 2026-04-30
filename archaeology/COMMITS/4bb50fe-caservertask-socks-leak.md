---
sha: 4bb50fe66468111a6c7ff4c086bdc56f2937bd1e
short_sha: 4bb50fe
date: 2024-03-14
author: Simon Rose
category: leak
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [memory-leak, caservertask, rsrv_init, socks-array, free]
---

# Memory Leak: socks Array Not Freed in rsrv_init After TCP Bind Loop

## Root Cause
`caservertask.c:rsrv_init` allocated a dynamic array `socks` to hold socket
file descriptors during the TCP server bind loop. After the loop completed
(successfully binding at least one TCP listener), the code continued to use
the socket list for setting up the server, but never called `free(socks)`.
The `socks` pointer was a local variable that went out of scope at function
exit, leaking the allocated memory.

## Symptoms
- Small memory leak (proportional to number of network interfaces) at IOC
  startup. Not a runtime-recurring leak — only happens once per `rsrv_init`.
- In practice, `rsrv_init` is called once at IOC startup, so the leak is
  bounded and non-critical. Purely a hygiene issue.

## Fix
Added `free(socks)` after the TCP server is fully set up and before the
"servers list is read-only" comment.

## Rust Applicability
Eliminated. In ca-rs, socket arrays are managed by `Vec<TcpListener>` which
is automatically freed when it goes out of scope. Rust's ownership model
prevents this class of leak.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:rsrv_init` — `socks` array allocated but never freed
