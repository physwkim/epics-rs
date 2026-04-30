---
sha: c23012d0817b7724eb5a00eb69f29ada8293f231
short_sha: c23012d
date: 2018-01-30
author: Michael Davidsaver
category: network-routing
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/server/online_notify.rs
    function: rsrv_online_notify_task
tags: [udp, beacon, error-log, spam, rsrv]
---

# CA server (rsrv) suppresses repeated beacon UDP send error messages

## Root Cause
`rsrv_online_notify_task()` sent beacons to all addresses in `beaconAddrList` and logged an error via `errlogPrintf` on every failed `sendto()`. Beacons are sent every ~15s under normal conditions (faster during startup). A persistent failure to reach a beacon address (e.g., misconfigured interface) would log the same error repeatedly, spamming the IOC log.

## Symptoms
IOC log filled with "CAS: CA beacon send to X.X.X.X error: ..." on every beacon interval when a beacon destination is persistently unreachable. No recovery message when the send succeeds again.

## Fix
Allocated a `lastError[]` array (one entry per beacon address). On send failure: only log if `err != lastError[i]`; store `lastError[i] = err`. On send success after a prior error: log "CAS: CA beacon send to X.X.X.X ok" and reset `lastError[i] = 0`. Freed `lastError` at task exit.

## Rust Applicability
In ca-rs server `src/server/online_notify.rs` (or beacon task), the per-destination error-dedup pattern is needed. Without it, a single persistently-unreachable beacon address floods the log at beacon rate. Implement with `last_errors: Vec<Option<io::ErrorKind>>` indexed by destination.

## Audit Recommendation
Audit the ca-rs beacon send loop. Confirm per-destination error deduplication is present. The recovery log ("ok after error") is optional but good practice.

## C Locations
- `modules/database/src/ioc/rsrv/online_notify.c:rsrv_online_notify_task` — added per-address `lastError[]` array; dedup on error; recovery log on success
