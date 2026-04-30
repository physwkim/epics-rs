---
sha: 69035629154c1ea20e8dacf5d122802d3d3277e5
short_sha: 6903562
date: 2019-10-15
author: Michael Ritzert
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [realloc, use-after-free, ca-server, buffer, memory]
---

# RSRV casExpandBuffer realloc return value not stored

## Root Cause
`casExpandBuffer` in `caservertask.c` called `realloc(buf->buf, size)` but did
not update `buf->buf` with the returned pointer. `realloc` may return a
*different* address when it cannot resize in place; the old pointer is freed and
the returned pointer points to the new allocation. By discarding the return value
`buf->buf` continued to reference freed memory — a classic use-after-free /
dangling-pointer bug.

```c
// BEFORE (broken):
newbuf = realloc(buf->buf, size);
// buf->buf is now dangling if realloc moved the allocation

// AFTER (fixed):
newbuf = realloc(buf->buf, size);
if(newbuf)
    buf->buf = newbuf;
```

This only affected the `mbtLargeTCP` path (large TCP frame buffers); the `else`
branch used `malloc` into a separate `newbuf` and then assigned later, so it was
unaffected.

## Symptoms
Memory corruption on any CA server connection that triggered a large-buffer
resize (message > 16 KB). Subsequent writes would scribble on already-freed
memory, causing heap corruption, crashes, or silent data corruption of other
heap objects.

## Fix
Store `realloc`'s return value back into `buf->buf` immediately, guarded by a
null check (to handle allocation failure separately from pointer update).

## Rust Applicability
Rust's ownership model makes this class of bug impossible. A `Vec::resize` or
`Vec::reserve` call moves ownership; there is no raw pointer to go stale.
Eliminated.

## Audit Recommendation
No action needed in Rust code. For completeness, verify `ca-rs` server
`src/server/` does not manually manage raw buffer pointers.

## C Locations
- `modules/database/src/ioc/rsrv/caservertask.c:casExpandBuffer` — realloc return value discarded
