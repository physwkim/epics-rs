---
sha: bc545242708372fd2fea15e7e1a8ae014c537ae5
short_sha: bc54524
date: 2023-01-24
author: Andrew Johnson
category: other
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [dbStatic, lexer, json, char-class, flex]
---
# dbLex.l: shared `stringchar` class admits wrong quote in each string type

## Root Cause
The flex lexer for `.db` / `.dbd` files defined a single character class `stringchar [^"\n\\]` used in both double-quoted and single-quoted string patterns. The class excluded `"` (double-quote) but not `'` (single-quote), so a single-quoted string pattern `{singlequote}({stringchar}|{escape})*{singlequote}` could match a `'` character inside the string body — potentially swallowing the closing single quote and mis-tokenizing. Similarly, the JSON error pattern for single-quoted strings used the wrong class.

The fix introduces two distinct classes: `dqschar [^"\n\\]` (for double-quoted strings, excludes `"`) and `sqschar [^'\n\\]` (for single-quoted strings, excludes `'`).

## Symptoms
Single-quoted strings in `.db` / `.dbd` JSON link syntax could be parsed incorrectly if they contained certain characters, potentially silently accepting malformed input or producing wrong token boundaries.

## Fix
Split `stringchar` into `dqschar` and `sqschar` and update all four patterns to use the appropriate class. Commit `bc54524`.

## Rust Applicability
Rust-based DB/DBD parsers would not use flex; they would use hand-written or nom/pest parsers where this class overlap cannot occur. The pattern is eliminated.

## Audit Recommendation
No direct audit needed. If base-rs has a `.db` / `.dbd` parser, ensure single- and double-quoted string rules use separate exclusion sets.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLex.l` — `stringchar` flex macro and all consuming patterns
