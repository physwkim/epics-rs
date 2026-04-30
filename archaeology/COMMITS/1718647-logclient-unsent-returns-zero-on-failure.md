---
sha: 1718647121799f44d0a2c9a624635d2d8f2077fa
short_sha: 1718647
date: 2019-09-19
author: Dirk Zimoch
category: flow-control
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [log-client, backlog, unsent-bytes, error-sentinel, flow-control]
---

# logClient unsent-bytes query returns 0 on failure masking reconnect trigger

## Root Cause
`epicsSocketCountUnsentBytes` returned `0` when the OS query was unavailable
(unsupported platform, or the ioctl/getsockopt call failed). The caller in
`logClientFlush` then computed:

```c
pClient->backlog = epicsSocketCountUnsentBytes(pClient->sock);
nSent -= pClient->backlog;  // subtracts 0 — no harm
```

However the logic was also used to decide whether to probe for broken
connections (the 0-byte send path in commit `c9b6709` checks `backlog > 0`).
By returning `0` on failure, the "unsupported" case was indistinguishable from
"zero bytes in flight" — so the probe never ran on unsupported platforms even
when the socket was stale.

More critically: before the null-check guard was added (commit `1718647`), the
caller unconditionally subtracted `backlog` from `nSent`. If `backlog` somehow
exceeded `nSent` (possible if the OS reported stale data), `nSent` would
underflow (unsigned subtraction) → huge positive → the subsequent `memmove`
would reference far past the buffer end (heap corruption / crash).

## Symptoms
- On unsupported platforms: backlog probe never fires; stale TCP connections
  not detected until OS-level timeout.
- If backlog > nSent: unsigned underflow causing catastrophic memmove overread.

## Fix
Return `-1` from `epicsSocketCountUnsentBytes` on failure (all branches) and
guard the caller with `if (backlog >= 0)` before using the value:

```c
int backlog = epicsSocketCountUnsentBytes(pClient->sock);
if (backlog >= 0) {
    pClient->backlog = backlog;
    nSent -= backlog;
}
```

## Rust Applicability
In Rust, the equivalent would use `Option<usize>` for the unsent count, making
the "unavailable" case structurally distinct from 0. Unsigned underflow is a
compile-time error in safe Rust (checked subtraction or `.saturating_sub()`).
Eliminated.

## Audit Recommendation
No action needed in Rust. The type system prevents both confusions.

## C Locations
- `modules/libcom/src/log/logClient.c:epicsSocketCountUnsentBytes` — returns 0 instead of -1
- `modules/libcom/src/log/logClient.c:logClientFlush` — missing guard for backlog >= 0
