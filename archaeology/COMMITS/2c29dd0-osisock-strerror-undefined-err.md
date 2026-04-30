---
sha: 2c29dd0c7ef54910cad421f3155dd4a3e859015c
short_sha: 2c29dd0
date: 2021-02-22
author: Brendan Chandler
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [test, strerror, merge-error, diagnostic, socket]
---
# osiSockTest: strerror(err) uses undeclared variable `err`

## Root Cause
A merge error in `osiSockTest.c` left a `strerror(err)` call referencing a
variable `err` that was not declared in the current scope. The variable used
elsewhere in the function for `getsockopt`/`getsockname` return codes was not
in scope at this `sendto` error diagnostic. This caused a compile error or UB
depending on compiler.

## Symptoms
Compile-time error or silent use of an indeterminate `err` value in the
`testDiag` output for the UDP fanout test's send failure path. The error
message would either fail to compile or print garbage for the errno string.

## Fix
Removed the `strerror(err)` argument from the `testDiag` format string,
changing the format from `"... (%d): %s"` to `"... (%d)"`. The format no
longer attempts to print the errno string; `SOCKERRNO` (the integer) is still
shown.

## Rust Applicability
Test infrastructure only. Rust test harness does not use `strerror` — error
types implement `Display`. No analog exists.

## Audit Recommendation
No audit needed. Test-only change with no production code path.

## C Locations
- `modules/libcom/test/osiSockTest.c:udpSockFanoutTestIface` — strerror(err) references undeclared `err`
