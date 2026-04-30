---
sha: 333be085c0fc7326273312807971caba3dbd25ca
short_sha: 333be08
date: 2024-12-22
author: Jeremy Lorelli
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [template, compile-error, iterator, comparison, C++]
---
# tsSLList iterator operator== compares wrong member field

## Root Cause
`tsSLIterConst<T>::operator==` and `operator!=` compared `this->pEntry`
against `rhs.pConstEntry` instead of `rhs.pEntry`.  Both `pEntry` and
`pConstEntry` are members of the same template class; using `pConstEntry`
means the comparison reads the wrong pointer (or a non-existent field in
older revisions), producing a compile error or silent mis-comparison.

## Symptoms
Compile error when `tsSLIterConst` equality operators are instantiated
with certain compilers.  If it compiled, two iterators pointing to the
same node would not compare equal.

## Fix
Change both `operator==` and `operator!=` to compare `this->pEntry` with
`rhs.pEntry` (the non-const member), which is the canonical storage slot
for the current position.

## Rust Applicability
Eliminated.  In Rust, iterators are implemented via the `Iterator` trait
with a single `next()` method; the language enforces correct comparison
through type-checked `PartialEq` derivation.  No hand-rolled linked-list
iterator comparison exists in the EPICS-rs crates.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/cxxTemplates/tsSLList.h:tsSLIterConst::operator==` — compared rhs.pConstEntry instead of rhs.pEntry
- `modules/libcom/src/cxxTemplates/tsSLList.h:tsSLIterConst::operator!=` — same wrong field
