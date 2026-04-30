---
short_sha: 3dbc9ea
title: iocsh argument splitter — EOF sentinel (-1) misread as valid char
crate: base-rs
status: not-applicable
---

# Review

## Verdict
**not-applicable** — Rust tokenizer does not use a magic-byte sentinel for
"not in a quote".

## Evidence
- The audited target `src/iocsh/tokenize.rs` does not exist; the iocsh
  tokenizer lives at `crates/epics-base-rs/src/server/iocsh/registry.rs` in
  `tokenize()` (line 178), with helpers `find_closing_paren` (line 231),
  `split_comma_args` (line 261), and `split_space_args` (line 340).
- All quote-state bookkeeping is via a plain `bool in_quotes` (or `in_quote`)
  toggled at each `'"'`. Examples:
  - `registry.rs:232` `let mut in_quotes = false;` in `find_closing_paren`,
  - `registry.rs:265` `let mut in_quotes = false;` in `split_comma_args`,
  - `registry.rs:343` `let mut in_quotes = false;` in `split_space_args`,
  - `iocsh/mod.rs:188` `let mut in_quote = false;` in `parse_redirect`.
- Rust `bool` cannot collide with any input byte value, so the C-3dbc9ea bug
  (`char quote = EOF` aliasing to `0xFF` on platforms where `char` is
  unsigned) has no structural analogue. No `Option<char>` substitute is
  needed because there is no "current quote character" state at all — only
  in/out of quotes.

## Decision
No fix to apply. If a future tokenizer evolves to track the specific quote
character (single vs double), `Option<char>` (None == not quoting) is the
right shape, matching the audit recommendation.
