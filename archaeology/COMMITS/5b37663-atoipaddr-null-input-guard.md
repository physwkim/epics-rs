---
sha: 5b37663bcb99aee7f6d4c511a9c8e854997a33ec
short_sha: 5b37663
date: 2020-08-06
author: Matic Pogacnik
category: lifecycle
severity: medium
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/addr_list.rs
    function: null
  - crate: pva-rs
    file: src/config/env.rs
    function: null
tags: [null-check, aToIPAddr, addr-parse, network-routing, crash]
---

# aToIPAddr crashes on NULL input string

## Root Cause
`aToIPAddr()` dereferenced its `pAddrString` argument without a null
check. Callers that pass the result of `envGetConfigParam()` or
`getenv()` can receive NULL when the environment variable is unset, and
if they pass it directly to `aToIPAddr()`, the function segfaults on the
first character access.

## Symptoms
IOC crash (null dereference / SIGSEGV) when `aToIPAddr` is called with
a NULL address string. This typically occurs during CA address-list
parsing when an environment variable (`EPICS_CA_ADDR_LIST`,
`EPICS_PVA_ADDR_LIST`) is not set and the caller does not pre-check for
NULL.

## Fix
Added an early return of `-1` when `pAddrString == NULL`.

## Rust Applicability
In Rust, the equivalent of `aToIPAddr` is an address-parsing function
that takes a `&str` or `Option<&str>`. If the function takes `&str`
(non-optional), callers that derive the string from environment
variables must handle the `None` case before calling. In epics-ca-rs
`addr_list.rs` and epics-pva-rs `config/env.rs`, verify that
`EPICS_CA_ADDR_LIST` / `EPICS_PVA_ADDR_LIST` absence is handled as
`Option::None` upstream and does not reach the parser as an empty or
default value that triggers a different parse failure.

## Audit Recommendation
In `src/client/addr_list.rs` (ca-rs) and `src/config/env.rs` (pva-rs):
search for environment variable reads for `ADDR_LIST` config. Verify
the result is `Option`-wrapped and that `None` (unset env var) is handled
gracefully (empty list, not panic or unwrap). Check for any `.unwrap()`
or direct `.parse()` on potentially-missing env config.

## C Locations
- `modules/libcom/src/misc/aToIPAddr.c:aToIPAddr` — added null check at top, returns -1 for NULL input
