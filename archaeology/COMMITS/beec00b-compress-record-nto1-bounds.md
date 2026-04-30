---
sha: beec00b403ac5d9e395c9fce37987ff0d7f51b37
short_sha: beec00b
date: 2024-03-14
author: Simon Rose
category: bounds
severity: high
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/records/compress.rs
    function: compress_array
tags: [bounds, compress-record, N-to-M, partial-buffer, array-compression]
---

# Compress Record N-to-M Array Compression Bounds Error with Partial Buffer

## Root Cause
`compressRecord.c:compress_array` handled N-to-1 averaging (and other
algorithms) with an outer loop over `nnew = no_elements / n` groups. When
the partial buffer option (`PBUF=YES`) was added, it allowed processing arrays
smaller than `nsam * n`. However, the existing bounds check
`if (no_elements < prec->n && prec->pbuf != menuYesNoYES) return 1;`
checked `no_elements < prec->n`, but `nnew` was still computed as
`no_elements / n`, so for inputs that aren't an exact multiple of `n`, the
last partial group was included in `nnew` but the source pointer advanced past
the available data.

Additionally, with `NSAM=2, N=2` and 4 input elements, `nnew` was computed
as `min(no_elements, nsam*n) = 4`, but the algorithm iterated `nnew` times
advancing by `n=2` each time — giving 2 correct groups. The N-to-1 Average
median case had a separate bug: `psource += nnew` instead of `psource += n`.

## Symptoms
- For N-to-M compressions (NSAM != no_elements/N), produced wrong or missing
  output values in the circular/FIFO buffer.
- With partial buffer enabled (`PBUF=YES`), the last partial group was
  processed with incorrect source pointer arithmetic, reading garbage data.
- The median algorithm advanced the source pointer by `nnew` instead of `n`.

## Fix
Rewrote `compress_array` with a `while (nnew > 0)` loop that:
1. Breaks early if remaining elements `< n` and `pbuf != YES`.
2. Uses `min(n, nnew)` to handle the last partial group.
3. Always advances `psource` by the actual `n` elements consumed.
4. Returns `(samples_written == 0)` instead of hardcoded `0`.

## Rust Applicability
Partial. If base-rs implements a compress record type, the same N-to-M loop
logic must be carefully reviewed. The Rust iterator approach (`.chunks(n)`)
naturally handles partial final groups, but the PBUF option and the
"skip partial" behavior need explicit handling.

## Audit Recommendation
In `base-rs/src/server/database/records/compress.rs:compress_array`:
- Verify that N-to-1 algorithm iterates over input in chunks of exactly `n`,
  using `chunks(n)` or equivalent, not computing total groups upfront.
- Verify `PBUF=NO` correctly discards incomplete final chunks.
- Verify the median variant advances by the sorted chunk size, not outer count.

## C Locations
- `modules/database/src/std/rec/compressRecord.c:compress_array` — nnew computation, psource advancement, median psource bug
