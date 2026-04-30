---
sha: 172597e0e6348308ae8c24c7a1c200bdd0e0226c
short_sha: 172597e
date: 2023-03-07
author: Dirk Zimoch
category: bounds
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [out-of-bounds, array-access, dbr-type, ca-client, guard]
---
# CA client: guard dbr_text[] array access against out-of-range type

## Root Cause
In `modules/ca/src/client/test_event.cpp`, the function `ca_dump_dbr()`
first checked `INVALID_DB_REQ(type)` and printed a "bad DBR type" message,
but was **missing a `return`** after the error print. Execution fell through
to `printf("%s\t", dbr_text[type])`, which accesses `dbr_text` at an
out-of-range index. `dbr_text` is a fixed-size array indexed by valid DBR
type values; an invalid `type` value causes an out-of-bounds array access
(undefined behavior, potential segfault or garbage read).

## Symptoms
- Crash or garbage output in `ca_dump_dbr()` when called with an invalid DBR
  type value (e.g., from a corrupt CA message or future type extension).
- The error message "bad DBR type N" is printed but the function continues
  and dereferences `dbr_text[N]` out of bounds.

## Fix
Added `return;` immediately after the `printf("bad DBR type %ld\n", type)`
inside the `if (INVALID_DB_REQ(type))` block. The function now exits early
on invalid type before touching `dbr_text`.

## Rust Applicability
In Rust, array indexing with an out-of-bounds value panics in debug and
release (bounds-checked by default). Accessing a slice by an unvalidated index
is caught at runtime. Furthermore, a Rust CA client would model DBR types as
an enum, making invalid-type access statically impossible. Fully eliminated.

## Audit Recommendation
None — Rust's array bounds checks and enum typing eliminate this pattern.

## C Locations
- `modules/ca/src/client/test_event.cpp:ca_dump_dbr` — missing return after INVALID_DB_REQ check
