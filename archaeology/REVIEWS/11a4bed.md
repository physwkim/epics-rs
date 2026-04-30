---
short_sha: 11a4bed
status: not-applicable
files_changed: []
---
The C bug concerns `compressRecord.c::compress_scalar`'s `PBUF=YES` path: when a partial-buffer flush fires before `inx + 1 >= prec->n`, the running sum is pushed instead of the average, and `prec->inx` is unconditionally reset. The fix swaps the running sum for an incremental mean and conditionalizes the `inx` reset.

In `crates/epics-base-rs/src/server/records/compress.rs`, `CompressRecord` has no `pbuf` field and no partial-flush logic. The N-to-1 Mean (`alg=2`) accumulates into `accum: Vec<f64>` and only emits a value when `accum.len() >= self.n` (full window), at which point `sum() / accum.len()` is exact (modulo final-step rounding) and `accum.clear()` discards exactly N samples. There is no intermediate `put_value` callsite that could observe a partial sum, so the running-sum-vs-mean defect cannot manifest. The `inx`-reset bug also has no analogue: the Rust accumulator is fully drained on each emit, so there is no "position to retain on partial flush." If PBUF support is added later, the incremental-mean `(n*mean + x)/(n+1)` form would be preferable, but that is forward-looking and not a fix to existing code.
