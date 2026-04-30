# chunk_10 — 1 APPLIES / 30 N/A

## ca594f40 — APPLIES (UNVERIFIED) — server: randomize UUID
**pva-rs target**: server_native/udp.rs:24-34 (`random_guid()`)
**Reason**: pvxs UUID was time+host+pid (deterministic). Fix initializes from secure random + XOR with metadata to prevent info disclosure.
**pva-rs status**: uses time+pid only via `SystemTime::now()`. Could be strengthened with `getrandom`/`rand` crate for cryptographically random GUID.

30 others N/A: docs (4), version bumps (2), test/diagnostic infra (5), minor cleanup (5), logging config (2), feature additions (3 — NTEnum, TimeStamp/Alarm, sharedArray ctor, all already in pva-rs), network config (3), security/socket fixes (3 — SO_REUSEADDR, O_CLOEXEC handled by tokio), misc (2 — iosfwd, Xcode debug). Note: 8db40be present in chunk 10 but is round-9 DUPLICATE.
