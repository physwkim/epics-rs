---
sha: 0b01fb20db0102d045a0daba24453ea6f37c8a96
short_sha: 0b01fb2
date: 2023-03-07
author: Dirk Zimoch
category: bounds
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [buffer-overflow, stack-buffer, flex, lexer, static-array]
---
# flex misc.c: static buffer too small for any readable_form argument

## Root Cause
In `modules/libcom/src/flex/misc.c`, the function `readable_form(int c)` uses
a static `char rform[10]` buffer to format the human-readable representation
of a character code. The buffer is filled via `sprintf` with escape sequences
like `"\\%03o"` (which produces `\` + 3 octal digits + NUL = 5 chars) or
`"\\%d"` / `"\\x%02x"`, but the maximum possible output length was not
carefully bounded.

For the general `else` branch (single char + NUL = 2 bytes) and most escape
codes, 10 bytes is sufficient. However, the comment in the commit and the size
increase to 16 indicate that certain argument values (e.g., large `int`
values, or escape sequences with a prefix string) could produce output longer
than 9 printable characters + NUL.

## Symptoms
- Stack buffer overflow in `readable_form()` for certain character code values.
- Can corrupt the stack frame of the calling flex tool code, leading to
  crashes or wrong output in the EPICS flex-based lexer tool.
- Only affects the build-time flex tool, not the runtime IOC.

## Fix
Increased the static buffer from `char rform[10]` to `char rform[16]`,
providing sufficient margin for any argument value including all escape
sequence formats.

## Rust Applicability
Rust does not use fixed static char arrays for formatted output; `format!()`
returns a heap-allocated `String`. Stack overflow from `sprintf` is
eliminated. No audit needed.

## Audit Recommendation
None — Rust build tooling uses `format!` / `String`, not fixed C char arrays.

## C Locations
- `modules/libcom/src/flex/misc.c:readable_form` — buffer `rform` increased from 10 to 16
