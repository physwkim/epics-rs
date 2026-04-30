---
sha: cb1571783b4793cb9c545e5736659ee5ed7d03bb
short_sha: cb15717
date: 2024-04-02
author: Michael Davidsaver
category: type-system
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [wrong-type, struct-layout, link-flags, unsigned-short, size-change]
---

# link.h: flags Field Wrong Type Increases struct link Size

## Root Cause
A previous commit changed `link.flags` from `unsigned short` to `unsigned int`
(incorrectly — the comment "change to unsigned incorrectly increased size"
refers to the previous change in commit e88a186). `unsigned int` is 4 bytes on
most platforms vs `unsigned short` at 2 bytes. This silently increased
`sizeof(struct link)` by 2 bytes (plus alignment padding), which:
1. Changed the in-memory layout of all records containing link fields.
2. Potentially broke binary compatibility with compiled device support modules
   that computed field offsets at compile time.
3. Changed `dbFldDes::offset` values for any field after a link in a record
   structure, causing wrong offsets for database access.

## Symptoms
- `sizeof(struct link)` silently grew, affecting all record struct layouts.
- Potential binary incompatibility with device support modules compiled against
  the old header.
- Wrong `dbAddr::field_size` or field offsets for any record field after a
  link field (INP, OUT, etc.), potentially causing data corruption in
  `dbGetField`/`dbPutField`.

## Fix
Reverted `unsigned flags` back to `unsigned short flags`.

## Rust Applicability
Eliminated. In base-rs, the `Link` struct uses Rust enums with explicit
`#[repr(u16)]` or bitflags crates that enforce the exact size. Rust's
`#[repr(C)]` structs have explicit size guarantees checked by tests.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/dbStatic/link.h:struct link` — `flags` field was `unsigned` (4 bytes) instead of `unsigned short` (2 bytes)
