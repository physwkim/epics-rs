---
sha: 42d06d6a38fd08d49daa28e782525393b5c8d82c
short_sha: 42d06d6
date: 2021-08-29
author: Dirk Zimoch
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [signed-char, record-name, validation, format-string, UB]
---
# dbRecordNameValidate: signed char comparison and wrong printf format

## Root Cause
Two bugs in `dbRecordNameValidate()` in `dbLexRoutines.c`:

1. **Signed `char` comparison**: The loop variable `c` was declared as `char`
   (signed on most platforms). The check `if (c < ' ')` is intended to catch
   non-printable bytes. With signed `char`, bytes ≥ 0x80 (e.g. UTF-8
   multi-byte sequences) become negative and pass the `< ' '` test, incorrectly
   triggering the "non-printable" warning for valid high-byte characters.

2. **Wrong `printf` format**: The error message used `%02u` (unsigned decimal)
   for the byte value but passed `(unsigned)c` — when `c` is negative,
   the cast to `unsigned` produces a large decimal, not a hex byte. The format
   should be `%02x` (hex) to display the raw byte value as expected.

## Symptoms
- Record names containing high-byte characters (e.g. UTF-8) trigger false
  "non-printable character" warnings.
- The warning message shows a large unsigned decimal number instead of a
  2-digit hex byte value, making it unreadable.

## Fix
Change `char c = *pos` to `unsigned char c = *pos` so the comparison
`c < ' '` is always an unsigned comparison (correct for printable-range
detection). Change `%02u` to `%02x` in the warning format string.

## Rust Applicability
Rust's `char` and `u8` type system makes this class of bug impossible: Rust
does not have a "signed char" concept. String validation iterates `u8` bytes
or Unicode codepoints, both of which are unsigned. The `< ' '` comparison on a
`u8` is unambiguously unsigned. The format string issue is also impossible
since Rust's format macro is type-checked. No audit needed.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbRecordNameValidate` — `char c` → `unsigned char c`; `%02u` → `%02x`
