---
sha: 8dc131dc4cc04eef3df513a978576442af9405b1
short_sha: 8dc131d
date: 2020-01-12
author: Michael Davidsaver
category: network-routing
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, UDP, datagram-fanout, socket, osiSock]
---

# Add osiSockTest coverage for epicsSocketEnableAddressUseForDatagramFanout

## Root Cause
There was no automated test verifying that `epicsSocketEnableAddressUseForDatagramFanout`
actually allows two sockets to bind the same UDP address. The underlying
function had subtle platform-specific bugs (prefer SO_REUSEPORT on BSD,
set both options on Linux) that were only discovered in practice, not by any
CI test.

## Symptoms
- Platform regressions in datagram fanout were undetected until runtime.

## Fix
Add `udpSockFanoutTest()` to `osiSockTest.c`:
1. Create three sockets A, B, C.
2. Enable fanout on B and C (not A).
3. Confirm that B fails to bind the same port as A (sanity: fanout with a
   non-fanout socket should fail).
4. After closing A: bind B and C to the same port successfully.
This tests the positive case (two fanout sockets share a port).

## Rust Applicability
This is a C test file for the EPICS libcom layer. Rust socket tests are
written using `#[test]` and `tokio::test`. No Rust analog needed. Eliminated.

## Audit Recommendation
None — test-only commit; no production code changed.

## C Locations
- `modules/libcom/test/osiSockTest.c:udpSockFanoutTest` — new test function
