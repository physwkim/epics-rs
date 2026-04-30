---
sha: 0a3427c835e367792dd3419bc2a5bc1aea02386a
short_sha: 0a3427c
date: 2019-08-28
author: Dirk Zimoch
category: flow-control
severity: medium
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/log/log_client.rs
    function: log_client_close
tags: [log-client, reconnect, message-loss, flow-control, buffer]
---

# logClient: Don't Discard Unsent Buffer on Disconnect

## Root Cause
In `logClientClose()`, when the TCP connection to the log server dropped, the
code reset `nextMsgIndex = 0` and zeroed `msgBuf` — discarding any bytes that
had been buffered but not yet transmitted. This meant that on a transient
network glitch, the log client silently swallowed all in-flight log messages.

## Symptoms
Log messages produced just before a log server disconnect (or server restart)
were silently lost. IOCs would reconnect successfully, but the messages
buffered during the outage were gone with no indication.

## Fix
Remove the two lines that reset `nextMsgIndex` and clear `msgBuf` from
`logClientClose()`. The buffer is preserved across a disconnect/reconnect
cycle, so pending messages are retransmitted after the new connection is
established.

## Rust Applicability
A Rust log client will likely use an async write loop with a VecDeque or
channel-backed buffer. On connection drop, the write task should **not** drain
or discard the pending queue; it should hold the buffer and flush it after the
reconnect completes. Any `clear()` or channel drain inside the disconnect
handler is the Rust analog of this bug.

## Audit Recommendation
In `base-rs/src/log/log_client.rs`, audit the disconnect/reconnect path:
confirm that the write buffer (channel, VecDeque, or BytesMut staging area) is
NOT cleared when the TCP stream closes. The reconnect loop should resume
draining from the same buffer position.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientClose` — removed `nextMsgIndex = 0` and `memset(msgBuf)`
