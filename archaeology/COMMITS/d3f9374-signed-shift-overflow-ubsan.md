---
sha: d3f93746a84d4bd49c87a3f01e90143b7cc3e896
short_sha: d3f9374
date: 2023-02-26
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [undefined-behavior, signed-overflow, bit-shift, ubsan, integer]
---
# Fix signed integer UB: 1<<31 (and similar) must use unsigned literal

## Root Cause
In C, shifting `1` (a signed `int`) left by 31 positions produces undefined
behavior on 32-bit `int` platforms because `1 << 31` sets the sign bit of a
signed type, which is UB per the C standard. The same applies to `1 << k`
when `k` can reach 31 and to `1 << i` in bit-field iteration loops.

Affected files:
- `mbboDirectRecord.c`: `1 << (pBn - &prec->b0)` where the offset can be 31.
- `mbbioDirectTest.c`: `~(1 << 31)` in test code.
- `closure.c`: `1 << k` in a closure iteration where `k` iterates over
  `BITS_PER_WORD` (32) positions.
- `warshall.c`: `1 << i` and `1 << i` in transitive-closure algorithm over
  32-bit words.

## Symptoms
- UBSan (`-fsanitize=undefined`) reports signed integer overflow at runtime.
- In optimizing compilers, signed overflow UB may cause the compiler to
  eliminate or misoptimize branches that depend on the shifted value.
- On most platforms with two's-complement ints the behavior is "as expected"
  but is formally undefined and can break under LTO or aggressive opts.

## Fix
Changed `1 << N` to `1u << N` (unsigned literal) in all four files:
- `mbboDirectRecord.c`: `epicsUInt32 bit = 1u << (pBn - &prec->b0);`
- `mbbioDirectTest.c`: `value &= ~(1u << 31u);`
- `closure.c`: `cword & (1u << k)` and `word & (1u << i)`
- `warshall.c`: `*ccol & (1u << i)` and `*rp |= (1u << i)`

## Rust Applicability
Rust's type system makes this impossible: shifting `1u32 << 31` is well-
defined (the `u` suffix ensures unsigned), and shifting a signed `i32` value
by `>= 32` is a panic in debug and wrapping in release. Rust's integer
literals require explicit typing. This class of UB is eliminated.

## Audit Recommendation
None — eliminated by Rust's type system and shift semantics.

## C Locations
- `modules/database/src/std/rec/mbboDirectRecord.c:special` — 1u << N for bit position
- `modules/database/test/std/rec/mbbioDirectTest.c` — 1u<<31u in test mask
- `modules/libcom/src/yacc/closure.c:set_first_derives` — 1u<<k
- `modules/libcom/src/yacc/closure.c:closure` — 1u<<i
- `modules/libcom/src/yacc/warshall.c:transitive_closure` — 1u<<i
- `modules/libcom/src/yacc/warshall.c:reflexive_transitive_closure` — 1u<<i
