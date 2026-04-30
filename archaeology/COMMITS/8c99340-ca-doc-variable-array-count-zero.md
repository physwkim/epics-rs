---
sha: 8c993405b4b4cf3bb9268ffa1b1e5731e5719810
short_sha: 8c99340
date: 2019-05-09
author: Ralph Lange
category: wire-protocol
severity: low
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/subscription.rs
    function: null
tags: [documentation, variable-array, subscription, count-zero, wire-protocol]
---
# CA: clarify count=0 means variable-size array subscription

## Root Cause
The CA reference documentation for `ca_create_subscription` did not make clear
that `COUNT=0` requests a variable-size subscription (the server sends however
many elements the record currently has, which may change). This caused
implementors to assume count=0 was an error or meant "no elements".

## Symptoms
- Implementors of CA clients set count=1 as a safe default instead of 0, missing
  the variable-array behavior.
- CA client implementations (including ca-rs) may not correctly handle the
  count=0 → variable-size subscription semantic.

## Fix
Documentation update only: `"A count of zero means use the current element count
from the server, effectively resulting in a variable size array subscription."`

## Rust Applicability
Applies. In ca-rs subscription code, verify that when the user requests count=0,
the `EVENT_ADD` message sends `count=0` to the server, and that incoming monitor
updates with varying element counts are handled correctly (not truncated to a
fixed initial count).

## Audit Recommendation
Audit `ca-rs/src/client/subscription.rs`: confirm that count=0 is passed
through to the wire without substitution, and that each monitor update uses the
element count from the response header rather than a stored initial count.

## C Locations
- `modules/ca/src/client/CAref.html` — documentation clarification for `COUNT=0`
