---
sha: 9df98c18386f1204fdf21a58f0f6dc1aae04126e
short_sha: 9df98c1
date: 2019-08-28
author: Dirk Zimoch
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/log_client.rs
    function: reconnect_loop
tags: [log-client, reconnect, pending-messages, lifecycle, connect]
---

# logClient pending messages not flushed immediately after reconnect

## Root Cause
The `logClientRestart` loop handled the connected/disconnected state as
mutually exclusive branches:

```c
// BEFORE (broken):
if (isConn) {
    logClientFlush(pClient);
} else {
    logClientConnect(pClient);
}
// flush never called on the iteration that transitions connected → true
```

On the iteration where `logClientConnect` succeeds (transitioning from
disconnected to connected), `isConn` was false at the top of the loop, so
`logClientFlush` was NOT called. Pending messages buffered during the
disconnected period would sit in the buffer until the *next* loop iteration
(after `LOG_RESTART_DELAY = 5.0` seconds).

Additionally `logClientFlush` had an early return guard:
```c
if (!pClient || !pClient->connected) return;
```
which was added in the same commit to prevent flushing on a disconnected socket
(correct guard), but means the guard on the *calling* side also needed updating.

## Symptoms
After a log server restart, messages buffered during the outage were delayed
by up to 5 seconds (one `LOG_RESTART_DELAY`) before being sent to the newly
connected server.

## Fix
Always call `logClientFlush` after (potentially) calling `logClientConnect`,
regardless of the prior connection state:

```c
// AFTER (fixed):
if (!isConn) logClientConnect(pClient);
logClientFlush(pClient);  // flush unconditionally; guarded internally by connected check
```

`logClientFlush` internally checks `pClient->connected` and returns early if
not connected, so calling it unconditionally is safe.

## Rust Applicability
In a Rust async log client with a persistent retry loop, the `connect` future
and the `flush` future should be chained (`connect.await?; flush().await?`) so
that a successful reconnect immediately drains the pending buffer. If the retry
loop has a `select!` that only runs flush on the "already-connected" arm, this
same bug can arise.

## Audit Recommendation
In `base-rs/src/log/log_client.rs`: verify that the reconnect loop calls
flush immediately after a successful `TcpStream::connect`, not just on
subsequent loop iterations. Specifically check for a `select!` or `match`
where connect and flush are in different arms with no tail-flush.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientRestart` — flush not called on connect iteration
- `modules/libcom/src/log/logClient.c:logClientFlush` — added `!pClient->connected` early-return guard
