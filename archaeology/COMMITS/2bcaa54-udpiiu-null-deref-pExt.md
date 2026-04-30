---
sha: 2bcaa5448c7e583a5e4f063c54f930e857cebb06
short_sha: 2bcaa54
date: 2020-02-12
author: Karl Vestin
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: ca-rs
    file: src/client/udp.rs
    function: push_datagram_msg
tags: [null-deref, UDP, CA, datagram, memcpy]
---

# CA UDP: memcpy with non-null extsize but null pExt pointer — null dereference

## Root Cause
In `udpiiu::pushDatagramMsg`, a CA UDP datagram is assembled by writing a
`caHdr` header followed by optional extension data. The guard was
`if (extsize)` — it checked that the extension byte-count was non-zero but
did not verify that the `pExt` pointer was non-null. A caller could pass
`extsize > 0` with `pExt == nullptr`, causing `memcpy(pbufmsg + 1, pExt,
extsize)` to dereference a null pointer and crash.

## Symptoms
- Crash (segfault / access violation) in the CA client UDP send path when
  a message with a non-zero extension size is built with a null extension
  data pointer.

## Fix
Change the guard to `if (extsize && pExt)` — require both conditions before
performing the `memcpy`.

## Rust Applicability
In Rust, raw pointers are not used in safe code; the equivalent would be
`Option<&[u8]>` for the extension data. The borrow checker prevents dereferencing
an `Option::None` without an explicit check. However, if ca-rs has a
`push_datagram_msg` equivalent that assembles UDP frames with a separate
length field and optional byte slice, the pattern should be audited to ensure
the length and data are always consistent (e.g., `extsize == 0` iff `ext.is_none()`).
An inconsistency would be a logic bug even in safe Rust (panic rather than segfault).

## Audit Recommendation
Audit `ca-rs/src/client/udp.rs::push_datagram_msg` (or equivalent UDP frame
builder):
1. Confirm extension data is typed as `Option<&[u8]>` or `&[u8]` (never a
   separate length + raw pointer).
2. Verify that the ext-length field in the wire header always matches the
   actual slice length.
3. Check for any `unsafe` memcpy / pointer arithmetic in the UDP send path.

## C Locations
- `modules/ca/src/client/udpiiu.cpp:pushDatagramMsg` — guard changed from `if (extsize)` to `if (extsize && pExt)`
