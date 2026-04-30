---
sha: c9b670977d590fb95455c358ed5597599804d66a
short_sha: c9b6709
date: 2019-09-18
author: Dirk Zimoch
category: network-routing
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/log/log_client.rs
    function: flush
tags: [log-client, broken-connection, tcp, send-probe, platform-specific]
---

# logClient zero-byte send to detect broken TCP connections

## Root Cause
Without an explicit probe, a TCP connection that has been broken by the remote
end (server crash, network partition) will appear "connected" until the kernel's
TCP keep-alive fires or data is sent. `logClientFlush` would continue to call
`send()` successfully (data entering the kernel send buffer) even though no
bytes were reaching the server — giving false confidence the connection was live.

The fix adds a 0-byte `send()` after flushing pending data, when `backlog > 0`:

```c
if (pClient->backlog > 0 && status >= 0) {
    errno = 0;
    status = send(pClient->sock, NULL, 0, 0);
    if (!(errno == SOCK_ECONNRESET || errno == SOCK_EPIPE)) status = 0;
}
```

Note the comment acknowledges platform-specific behavior:
- Linux: 0-byte send can trigger EPIPE detection (useful, but technically UB
  per POSIX for NULL buffer)
- Windows: documented no-op
- vxWorks: fails

## Symptoms
With a broken log server, the log client could continue buffering and
"sending" messages for minutes (until TCP keep-alive fires), while the
application believed logging was active. No reconnect was triggered.

## Fix
After a successful flush with remaining backlog, probe the connection with a
0-byte send and check for `SOCK_ECONNRESET` / `SOCK_EPIPE`. If detected,
trigger `logClientClose` and reconnect.

## Rust Applicability
In a Rust async log client backed by tokio TCP, write-side errors surface via
`AsyncWriteExt::write` returning `Err(io::Error)`. A broken remote end shows
up when the send buffer drains and the ACK is not received — typically via
`ErrorKind::BrokenPipe` or `ConnectionReset` on the next write. However, the
same latency issue applies: if the send buffer has space, tokio writes succeed
until the kernel detects the broken connection. An explicit half-open probe
(or TCP keepalive via `socket2::TcpKeepalive`) may be needed for fast
reconnection in the Rust log client.

## Audit Recommendation
In `base-rs/src/log/log_client.rs`: verify TCP keepalive is configured (via
`socket2` before handing the socket to tokio) so broken connections are
detected within seconds, not minutes. Alternatively check if the flush loop
explicitly tests for broken-pipe after each write.

## C Locations
- `modules/libcom/src/log/logClient.c:logClientFlush` — 0-byte send probe added
