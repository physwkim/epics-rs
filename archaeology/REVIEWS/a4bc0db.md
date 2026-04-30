---
short_sha: a4bc0db
status: fixed
files_changed: [crates/epics-base-rs/src/server/database/processing.rs]
---
The Rust rewrite has no `dbCa.c` callback layer, but the moral equivalent of dbCaTask's CA_DBPROCESS handler is the CP-link target dispatch in `process_record_with_links` and `complete_async_record`. Both sites previously called `process_record_with_links` on the target without setting `PUTF`, which is the same hole the C fix closed when it removed `scanLinkOnce`/`scanComplete` and routed CP updates through `db_process(prec)`. Set `common.putf = true` before dispatching, and if the target is already processing (Rust `processing` AtomicBool == C `pact`), set `common.rpro = true` instead so the in-flight pass reprocesses on completion. cargo check passes.
