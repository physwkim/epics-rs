---
sha: d1491e0860efc2ea3bd85a9f68cd83f18b575ae0
short_sha: d1491e0
date: 2020-07-17
author: Dirk Zimoch
category: wire-protocol
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/db_test.rs
    function: dbpf
tags: [dbpf, json, array, put, cli]
---

# dbpf switches from whitespace-delimited to JSON array format for array puts

## Root Cause
The `dbpf` shell command parsed array values as whitespace-delimited strings, which cannot represent empty arrays, values with embedded spaces, or typed numeric arrays correctly. The parser was custom handwritten, allocating `MAX_STRING_SIZE` per element and lacking proper element-size awareness (`dbValueSize` was not used).

## Symptoms
- Empty array puts were not representable.
- Non-string array elements were over-allocated (MAX_STRING_SIZE bytes each regardless of type).
- No escaping for values with embedded whitespace.
- The prior `d1491e0` parent commit (`a9731b9`) already added an OOM guard for the old path, which this commit then replaces entirely.

## Fix
Replaced the custom whitespace tokenizer with `dbPutConvertJSON()`, using `dbValueSize(dbrType)` for per-element allocation and `addr.no_elements` as the max count. The JSON format unambiguously supports empty arrays (`[]`), typed numerics, and strings.

## Rust Applicability
If base-rs exposes a `dbpf`-equivalent CLI command or test utility, it should parse array arguments as JSON rather than whitespace-split strings. The exact allocation pattern (pre-allocate `no_elements * element_size`, then fill via JSON parser) should be followed in Rust equivalents. Partial: affects diagnostic/CLI layer, not the wire protocol itself.

## Audit Recommendation
Check base-rs test utilities and any CLI `put` helpers for array argument parsing. Ensure they use a typed JSON path, not a raw `split_whitespace()` approach that cannot represent empty arrays.

## C Locations
- `modules/database/src/ioc/db/dbTest.c:dbpf` — replaced whitespace parser with `dbPutConvertJSON`
