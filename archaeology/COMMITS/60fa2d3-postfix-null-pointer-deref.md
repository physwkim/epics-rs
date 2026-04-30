---
sha: 60fa2d31da2deaaa89636b4a1f5b852efe0b2699
short_sha: 60fa2d3
date: 2023-07-18
author: Jeremy Lorelli
category: bounds
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/calc/postfix.rs
    function: postfix
tags: [null-pointer, postfix, calculator, bounds, stack-underflow]
---

# Null pointer dereference in postfix() on empty operator stack

## Root Cause
In `postfix.c`, the `SEPERATOR` (comma) and `CLOSE_PAREN` cases in the expression parser both entered a `while (pstacktop->name[0] != '(')` loop to pop operators until an opening parenthesis was found. But if the operator stack was empty (i.e., `pstacktop == stack`, pointing to the sentinel element), the loop would immediately dereference `pstacktop->name[0]` without checking if it was already at the bottom of the stack. With malformed input like `"0,"` or `"0)"`, `pstacktop` was at the base sentinel with uninitialized or invalid `name`, causing a null/invalid pointer dereference.

## Symptoms
Crash (segfault or access violation) when `postfix()` was called with expressions containing a comma or close-paren at a position where no corresponding opening paren exists on the operator stack: e.g., `"0,"`, `"0)"`, `",1"`. These are invalid expressions that should return a parse error, not crash.

## Fix
Added bounds checks before each loop: `if (pstacktop == stack) { *perror = CALC_ERR_BAD_SEPERATOR; goto bad; }` for the SEPERATOR case and `CALC_ERR_PAREN_NOT_OPEN` for the CLOSE_PAREN case. Two test cases added to `epicsCalcTest.cpp`.

## Rust Applicability
Applies. `base-rs` implements the EPICS calc expression parser for IOC record CALC/CALCOUT fields. If `base-rs/src/calc/postfix.rs` implements the shunting-yard algorithm, it must check for operator stack underflow before accessing the top element in the SEPARATOR/CLOSE_PAREN cases. A Rust `Vec::last()` returns `Option` and will not crash, but failing to handle `None` with `?` or an explicit check would produce a wrong error code rather than a crash. The explicit error code assignment (`CALC_ERR_BAD_SEPERATOR`, `CALC_ERR_PAREN_NOT_OPEN`) must also be correct.

## Audit Recommendation
In `base-rs/src/calc/postfix.rs::postfix()`, verify:
1. The SEPARATOR (comma) handling checks for empty stack before the inner loop.
2. The CLOSE_PAREN handling checks for empty stack before the inner loop.
3. The appropriate error enum variants (`BadSeparator`, `ParenNotOpen`) are returned.
4. Test inputs `"0,"` and `"0)"` are covered by unit tests.

## C Locations
- `modules/libcom/src/calc/postfix.c:postfix` — added `pstacktop == stack` guard in SEPERATOR and CLOSE_PAREN cases
