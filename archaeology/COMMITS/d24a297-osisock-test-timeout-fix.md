---
sha: d24a297304bc22bd45916e5ed71fabd888016967
short_sha: d24a297
date: 2020-11-19
author: Michael Davidsaver
category: timeout
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, timeout, socket, time-check]
---
# osiSockTest: fix receiver timeout and loop guard

## Root Cause
The UDP fanout test receiver (`udpSockFanoutTestRx`) had two related timeout
bugs:
1. The `SO_RCVTIMEO` socket option was set to 10 seconds but the test loop only
   ran for 5 seconds — the socket timeout was twice the loop duration and could
   mask slow systems hanging past the loop guard.
2. The `while` loop condition checked `epicsTimeDiffInSeconds(&now, &start)` but
   never refreshed `now` via `epicsTimeGetCurrent`. On every iteration `now`
   held its initial (zero-initialized) value, making the loop either run
   forever or exit immediately depending on platform epoch.

## Symptoms
Test could hang or produce spurious failures on slow machines. The unreffreshed
`now` meant the 5-second guard was never actually enforced.

## Fix
- Reduce `SO_RCVTIMEO` to 5 seconds to match the loop duration.
- Add `!epicsTimeGetCurrent(&now) &&` to the while condition so `now` is
  refreshed on each iteration and the elapsed-time guard works correctly.

## Rust Applicability
This is a test-only fix in C. The epics-rs test suite uses tokio timeouts
(`tokio::time::timeout`) rather than raw socket options, so the pattern of
"socket timeout longer than loop guard" cannot arise. No audit needed.

## Audit Recommendation
None — eliminated by Rust's async timeout model.

## C Locations
- `modules/libcom/test/osiSockTest.c:udpSockFanoutTestRx` — fix socket timeout value and refresh `now` in loop guard
