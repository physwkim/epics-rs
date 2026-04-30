# chunk_16 — 3 APPLIES candidates / 27 N/A

## 51f005d — APPLIES (UNVERIFIED) — client warn when operation implicitly canceled
**pva-rs target**: client_native/ ops Drop handlers
**Fix**: Add tracing warning on Drop of incomplete operations.

## 648d7ae — APPLIES (UNVERIFIED) — from_wire_full() more forgiving (null guard)
**pva-rs target**: pvdata/encode.rs decode paths
**Fix**: Replace assertions with graceful null/missing-descriptor handling.

## e668038 — APPLIES (UNVERIFIED) — client track opByIOID per channel
**pva-rs target**: client_native/server_conn.rs Router or channel.rs
**Fix**: Track operations per-channel; clean both ServerConn ioid map + Channel-level on CMD_DESTROY_CHANNEL.

27 others N/A: test infrastructure (1), feature additions (8 — credentials, NTTable expand, Result peerName, client SetEndian removal, etc), refactors (10), documentation (3), minor style (3), oops follow-up (1), feature pvRequest (1).
