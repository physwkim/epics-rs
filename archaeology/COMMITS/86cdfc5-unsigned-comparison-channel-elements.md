---
sha: 86cdfc596f402657401b07c3a38493159022d023
short_sha: 86cdfc5
date: 2024-08-12
author: Dirk Zimoch
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [unsigned-comparison, signed-promotion, dbChannelElements, element-count]
---

# Wrong unsigned comparison in dbChannelIO::nativeElementCount

## Root Cause
`dbChannelElements()` returns a `long` (signed). The original comparison
`elements >= 0u` promoted the signed `long` to `unsigned long` due to the `u`
suffix on the literal, making a value of `-1` (error sentinel) compare as a
very large positive number — so negative error returns were treated as valid
element counts and cast to `unsigned long`, yielding a garbage huge value.

## Symptoms
A channel returning a negative element count (error) would be misreported as
having `ULONG_MAX` (or similar large) elements, potentially leading to
over-allocation or out-of-bounds access in callers of `nativeElementCount`.

## Fix
Changed `elements >= 0u` to `elements >= 0` (signed literal), ensuring the
comparison is a signed comparison that correctly identifies negative error
returns.

## Rust Applicability
Eliminated. Rust's type system does not allow mixing signed/unsigned in
comparisons without explicit casts; this class of implicit-promotion bug
cannot occur. Element count APIs in ca-rs return `Result<usize, Error>` or
`Option<usize>`, forcing callers to handle error paths explicitly.

## Audit Recommendation
None required — Rust eliminates this category of bug at compile time.

## C Locations
- `modules/database/src/ioc/db/dbChannelIO.cpp:dbChannelIO::nativeElementCount` — wrong unsigned literal in comparison
