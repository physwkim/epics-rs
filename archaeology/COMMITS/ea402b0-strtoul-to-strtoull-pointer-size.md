---
sha: ea402b0f73d50cf4125eff264338ba59ff56849a
short_sha: ea402b0
date: 2025-02-25
author: Dirk Zimoch
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [pointer-size, windows-x64, strtoul, truncation, iocsh]
---

# Fix thread-id truncation on 64-bit Windows (strtoul → strtoull)

## Root Cause
`libComRegister.c` implemented the iocsh `threadCall` and `epicsThreadResume`
commands by parsing a thread ID string with `strtoul()` into an `unsigned long`.
On Windows x64, `sizeof(unsigned long) == 4` while `sizeof(void*) == 8`, so
thread IDs (which are pointers) parsed from user input were silently truncated
to 32 bits. The resulting `epicsThreadId` pointed into unmapped memory,
causing a crash or no-op when dereferenced.

## Symptoms
- `threadCall <id>` or `epicsThreadResume <id>` crash on 64-bit Windows when
  the thread ID is > 0xFFFFFFFF (the upper 32 bits are stripped).
- Incorrect thread looked up silently if lower 32 bits happen to match another
  thread.

## Fix
Changed `strtoul(cp, &endp, 0)` to
`(epicsThreadId)(uintptr_t)strtoull(cp, &endp, 0)`.
Using `strtoull` reads the full 64-bit value; the `uintptr_t` cast ensures
correct round-trip through pointer-width integer before the final
`(epicsThreadId)` cast. Also removed the now-unnecessary `unsigned long ltmp`
local variable.

## Rust Applicability
Eliminated. Rust's type system prevents implicit narrowing casts; pointer-sized
integers are represented as `usize`/`isize` and converting a `u64` to a
pointer requires an explicit `as *const _` cast that is auditable at each site.
No `strtoul`/iocsh parsing analog exists in the Rust codebase.

## Audit Recommendation
No action required. If any Rust code parses user-supplied pointer/thread-id
strings, verify the parse uses `u64::from_str_radix` and cast through `usize`.

## C Locations
- `modules/libcom/src/iocsh/libComRegister.c:threadCallFunc` — strtoul → strtoull with uintptr_t cast
- `modules/libcom/src/iocsh/libComRegister.c:epicsThreadResumeCallFunc` — same fix
