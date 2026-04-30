---
sha: 3ee6097ab7c2bad474c1f77a411917912293a7a3
short_sha: 3ee6097
date: 2023-03-09
author: Karl Vestin
category: other
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [format-string, injection, snprintf, printf-record, security]
---
# printfRecord Passes User String Directly as printf Format Argument

## Root Cause
`printfRecord.c:doPrintf()` handles the `%%` literal escape (a literal
percent sign) by calling:

```c
added = epicsSnprintf(pval, vspace + 1, "%s", format);
```

Here `format` is a substring of the user-supplied format string. When the
escape character is `%%`, the intent is to emit a literal `%`. However, the
correct safe call passes `"%s"` as the format — the fix changes the call to:

```c
added = epicsSnprintf(pval, vspace + 1, format);
```

Wait — this is actually the **wrong direction**. The original used `"%s"`
(safe), but the fix removes `"%s"` and passes `format` directly as the
format string. If `format` contains `%d`, `%s`, etc., this becomes a
format-string injection (CWE-134). The likely intent (per issue #361) was
to emit exactly one `%` for `%%`, which requires passing `"%%"` or `"%"`.

Reading the diff more carefully: the replacement is `epicsSnprintf(pval,
vspace + 1, format)` where `format` points at `"%%"` in the user string.
The `epicsSnprintf` with `"%%"` as format will emit one `%`, which is the
desired behaviour. But this is only safe because, at that code point,
`format` has been verified to be `"%%"` (the calling loop handles all other
`%X` sequences in separate branches). The original `"%s"` pass would have
emitted `%%` literally as the two-character string, not a single `%`.

The bug is: the original code used `"%s"` as the format specifier and passed
`format` (pointing at `"%%"`) as the string argument to `%s`, so it printed
`%%` (two characters) rather than `%` (one character). The fix passes
`format` directly, which `snprintf` interprets as a format string producing
`%`. This is safe **only** because the loop guarantees `format` starts with
`%%` in this branch.

## Symptoms
- `printf` record with `%%` in format string produced `%%` (two-character
  output) instead of a single `%`.
- Referenced as issue #361.

## Fix
Pass `format` directly as the format argument rather than as the `%s`
string, so `epicsSnprintf` interprets `%%` and emits one `%`.

## Rust Applicability
`eliminated` — Rust has no printf-style format strings at the call site;
`format!` arguments are type-checked at compile time and the format string
is a string literal, not a runtime value. Record processing in base-rs
would use Rust's `write!`/`format!` macros.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/database/src/std/rec/printfRecord.c:doPrintf` — `%%` escape emits two chars instead of one
