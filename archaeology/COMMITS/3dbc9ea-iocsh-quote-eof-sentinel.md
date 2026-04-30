---
sha: 3dbc9ea26491dc676888fca091e78cddf9a8760c
short_sha: 3dbc9ea
date: 2023-02-01
author: Zimoch Dirk
category: other
severity: medium
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/iocsh/tokenize.rs
    function: tokenize
tags: [iocsh, tokenizer, quote-handling, sentinel-value, vxworks]
---
# iocsh argument splitter: EOF sentinel (-1) misread as valid char

## Root Cause
The tokenizer used `char quote = EOF` (i.e. `quote = -1` cast to `char`) as a sentinel meaning "not inside a quoted string". On most platforms `char` is signed and the value -1 works, but on VxWorks (and any platform where `char` is unsigned) `-1` wraps to `255` (`0xFF`). Any byte with value 0xFF in the input was then indistinguishable from the "not quoted" sentinel, so the `quote != EOF` guard was always true — every command line was reported as "Unbalanced quote."

The fix replaces the sentinel with `0` (which can never appear in the middle of a C string) and updates all comparisons to `!quote` / `quote = 0`.

## Symptoms
On VxWorks 6.9 every iocsh command prints "Unbalanced quote." regardless of actual input content. No commands execute.

## Fix
Change `char quote = EOF` → `char quote = 0`; replace all `quote == EOF` / `quote != EOF` checks with `!quote` / `quote`. Commit `3dbc9ea`.

## Rust Applicability
Rust does not have an implicit `char`/signed ambiguity for byte buffers, but if a Rust tokenizer uses a sentinel value for "no current quote character", it must choose a value that cannot appear in valid input. `0u8` or `Option<char>` are both safe choices. Worth auditing the iocsh tokenizer if one is implemented.

## Audit Recommendation
If base-rs implements an iocsh command parser/tokenizer, verify that the "no active quote" state is represented as `Option<char>` (None = not quoting) rather than a magic byte value.

## C Locations
- `modules/libcom/src/iocsh/iocsh.cpp:Tokenize::operator()` — `char quote` sentinel bug and fix
