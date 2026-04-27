# epics-ca-rs Performance Baseline

Baseline numbers from `cargo bench`, captured on the hardware /
toolchain noted below. Use these as **regression checks** when
touching hot paths — wide deltas (>10%) deserve scrutiny before merge.

## How to run

```bash
# protocol-level micro-benchmarks (header encode/decode etc.)
cargo bench -p epics-ca-rs --bench protocol_bench

# end-to-end (in-process softioc + client)
cargo bench -p epics-ca-rs --bench end_to_end_bench
```

HTML reports land in `target/criterion/<bench>/report/index.html`
with violin plots and historical trend.

## End-to-end baseline

Captured 2026-04-28 on Apple Silicon (Darwin 25.4) with rustc stable,
release profile, single-process loopback.

| Bench | Time per op | Notes |
|------|-------------|-------|
| `e2e_caget_warm_8pvs` | ~1.02 ms | 8 sequential reads on a warm channel; ~127 µs/PV |
| `e2e_caput_warm` | ~114 µs | one fire-and-forget put on a warm channel |

Numbers reflect the full TCP round-trip including kernel scheduling,
not just the protocol code path. The hot work is split roughly:

- **Protocol encode/decode**: <1 µs each (see `protocol_bench`)
- **Tokio scheduler hop + socket syscall**: ~50–80 µs round-trip on
  loopback
- **Server-side handler dispatch + DBR encode**: ~20–30 µs

For multi-thousand-channel workloads, `e2e_caget_warm` dominates
total time — consider monitor-based subscription patterns for
reads >100 Hz instead of repeated caget. Monitor delivery is amortized
to the kernel `sendmsg()` cost (single-digit µs) per update once the
subscription is established.

## Protocol-level baseline

| Bench | Time per op |
|------|-------------|
| `ca_header_to_bytes` | ~7 ns |
| `ca_header_to_bytes_extended` | ~12 ns |
| `ca_header_from_bytes` | ~5 ns |
| `ca_header_from_bytes_extended` | ~10 ns |

(Update with each release. The protocol path is well below the
network/scheduler floor, so micro-optimizations here rarely move the
end-to-end numbers — but a 100× regression would still be visible.)

## What we're NOT measuring (yet)

- **Monitor throughput at 1k/10k channels** — needs a multi-client
  fixture; the current bench is a single-client loopback. Track as
  a follow-up.
- **libca comparison** — head-to-head vs `caget` against a real
  `softIoc` would close the credibility gap. Requires building
  epics-base in CI; out of scope for the in-tree bench.
- **Memory / FD usage under sustained load** — the soak harness
  (`ca-soak-observed`) covers this with metrics, not criterion.

## Interpreting deltas

A bench result is one number, but criterion always reports a 95% CI.
A "regression" is a CI band that doesn't overlap the prior baseline.
Quick rules:

| Delta | Action |
|------|--------|
| within ±5% | noise — proceed |
| ±5–15% | investigate — usually a missed allocation or a new lock |
| >+15% | block merge until explained |
| >-15% | celebrate, then verify the bench still measures the right thing |

When refactoring a hot path (transport buffer, header encode, monitor
queue), run bench *before* and *after*, paste both numbers in the PR
description.
