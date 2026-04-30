---
sha: a42197f0d670066c63f46b0af2eb3d01e8bb14f5
short_sha: a42197f
date: 2020-06-08
author: Dirk Zimoch
category: wire-protocol
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/channel.rs
    function: write
tags: [empty-array, ca-put, caput, bounds-check, wire-protocol]
---

# CA client: Allow writing zero-element arrays via caput

## Root Cause
`nciu::write()` validated `countIn > this->count || countIn == 0` and threw
`cacChannel::outOfBounds` for any zero-count write. This prevented legitimate
`caput` operations on waveform PVs with zero elements: even if the server's
array field was already empty (nelm=0 or nord=0), the client-side guard
rejected the write before any network message was sent.

The intent of `countIn == 0` rejection was presumably "don't send empty
payloads", but an empty array is a valid EPICS value (waveform with nord=0)
and the CA protocol supports zero-element payloads.

## Symptoms
- `caput` to a waveform PV with zero elements fails on the client side with
  `ECA_BADCOUNT` / out-of-bounds exception.
- No CA_PROTO_WRITE message sent; the server never sees the update.

## Fix
Removed `|| countIn == 0` from both `write()` overloads in `nciu.cpp`,
keeping only `countIn > this->count` as the upper-bound guard. Zero-count
writes now reach the server.

## Rust Applicability
Applies. In ca-rs, `channel::write()` must not reject `count == 0`. The CA
wire format for a zero-element array is a valid CA_PROTO_WRITE with payload
size 0 (or dbr_size[TYPE] for the fixed header element, count=0). Verify that
the write path does not short-circuit on empty slices before encoding.

## Audit Recommendation
In `ca-rs/src/client/channel.rs:write`, check for a `count == 0` early-return
or error guard that would block empty-array puts. Also verify that the CA
message encoder correctly encodes `count=0` (payload = just the DBR fixed
header, no value bytes).

## C Locations
- `modules/ca/src/client/nciu.cpp:nciu::write` — removed `|| countIn == 0` from bounds check (both overloads)
