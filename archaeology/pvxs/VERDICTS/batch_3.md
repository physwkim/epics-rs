# Batch 3 Verdicts — R061–R090

## R061 — 40bafaee07 — N/A
**pvxs**: Drop unused cleaner/ELLLIST cleanup helper  
**pva-rs target**: n/a  
**Reasoning**: pva-rs uses Rust ownership and RAII, not manual libevent ellList cleanup. No equivalent resource needs scavenging here.

## R062 — 49c9d82053 — N/A
**pvxs**: Drop unused IfaceMap member in ContextImpl  
**pva-rs target**: crates/epics-pva-rs/src/client_native/  
**Reasoning**: pva-rs uses direct config.interfaces iteration rather than cached IfaceMap. Structural mismatch in address handling.

## R063 — 4a86694605 — N/A
**pvxs**: Missing C++ includes (ostream, string, cstdint)  
**pva-rs target**: n/a  
**Reasoning**: Rust module system eliminates header management; trait bounds replace implicit includes. Not applicable.

## R064 — 6020e28284 — N/A
**pvxs**: Drop qsrv executable (EPICS IOC soft-server launcher)  
**pva-rs target**: crates/epics-bridge-rs/src/qsrv/  
**Reasoning**: This is build/deploymentconstruct, not a protocol bug. pva-rs qsrv doesn't have equivalent executable to remove.

## R065 — 60d68940fb83 — N/A
**pvxs**: Add missing cstdint header  
**pva-rs target**: n/a  
**Reasoning**: C++ header dependency; Rust modules handle type definitions implicitly. Not applicable.

## R066 — 6828ea06c814 — N/A
**pvxs**: Revert GetAddrInfo numeric-only flag; allow DNS resolution  
**pva-rs target**: crates/epics-pva-rs/src/config/  
**Reasoning**: pva-rs uses `std::net::ToSocketAddrs` which already allows both numeric and DNS. No bug.

## R067 — 6dba1d91f63f — N/A
**pvxs**: Cast enum state to int for logging  
**pva-rs target**: crates/epics-pva-rs/src/client_native/{get,put,rpc}.rs  
**Reasoning**: pva-rs enum Display derives print directly; no type-coercion logging bug exists.

## R068 — 7e031a20ff8b — N/A
**pvxs**: IOC: remove redundant dbLoadGroups call in init-hook  
**pva-rs target**: n/a  
**Reasoning**: EPICS IOC group-source hook integration; pva-rs is pure protocol client/server, not IOC-aware.

## R069 — 87c5aabc2f72 — N/A
**pvxs**: Server closes TCP connections on stop()  
**pva-rs target**: crates/epics-pva-rs/src/server_native/tcp.rs  
**Reasoning**: pva-rs uses Tokio task/channel Drop semantics, not explicit connection draining. Async shutdown is automatic.

## R070 — 9f9f03805568 — N/A
**pvxs**: Allow null Member (TypeCode::Null constructor)  
**pva-rs target**: crates/epics-pva-rs/src/pvdata/  
**Reasoning**: pva-rs FieldDesc enum already handles all type variants, including null. No omission.

## R071 — a6b3eb58bd42 — APPLIES
**pvxs**: Add missing check on invalid Union[] selector  
**pva-rs target**: crates/epics-pva-rs/src/pvdata/encode.rs (Union decode path)  
**Reasoning**: When decoding Union, invalid selectors should fail-fast. pva-rs likely silently skips or panics. Add explicit fault on out-of-bounds selector.
**Fix sketch**: In `decode_pv_field` Union branch, after `select.size` decode, check `select.size < union_members.len()` and `buf.fault()` if false.

## R072 — b47482e38a30 — N/A
**pvxs**: Fix recvmsg() error-path handling (control-msg buffer validity)  
**pva-rs target**: n/a  
**Reasoning**: pva-rs uses `tokio::net::UdpSocket` and OS socket abstractions; no raw recvmsg() calls with cmsg buffer bugs.

## R073 — ba0974e1a54a — N/A
**pvxs**: Drop unimplemented Value iteration (begin/end)  
**pva-rs target**: crates/epics-pva-rs/src/pvdata/  
**Reasoning**: pva-rs Value never had iterator interface; this is retrospective C++ cleanup.

## R074 — c2e5fdca551a — N/A
**pvxs**: Avoid FD leak on failed async connect()  
**pva-rs target**: crates/epics-pva-rs/src/client_native/tcp.rs  
**Reasoning**: pva-rs uses Tokio TcpStream which owns the socket; Drop on failed Future releases FD automatically. No leak.

## R075 — c66c0fd1003e — N/A
**pvxs**: Fix printf format specifiers (Windows socket type casts)  
**pva-rs target**: crates/epics-pva-rs/src/client_native/udp.rs  
**Reasoning**: pva-rs uses Rust logging (tracing) and std::fmt, not C printf. Format bugs impossible.

## R076 — d10eefac0e4b — N/A
**pvxs**: Drop unused FieldDesc::hash field  
**pva-rs target**: crates/epics-pva-rs/src/protocol/field_desc.rs  
**Reasoning**: pva-rs FieldDesc is computed on-demand, not cached. No hash field to drop.

## R077 — d52272e148a4 — N/A
**pvxs**: Fix EvInBuf::refill() slice-size logic and error check  
**pva-rs target**: n/a  
**Reasoning**: pva-rs uses Tokio `BufReader`, not hand-crafted libevent evbuffer wrapper. Not applicable.

## R078 — e36db5527c82 — APPLIES
**pvxs**: Server fail hard on invalid EPICS_PVAS_INTF_ADDR_LIST  
**pva-rs target**: crates/epics-pva-rs/src/config/env.rs  
**Reasoning**: When EPICS_PVAS_INTF_ADDR_LIST is set, invalid IPs should error early, not silently ignore. pva-rs likely skips bad addresses.
**Fix sketch**: In `split_addr_into` or config parsing, pass `required=true` flag; throw on unparseable address instead of log_err + continue.

## R079 — e9ecf7e8dd13 — N/A
**pvxs**: Add copyright boilerplate to headers  
**pva-rs target**: n/a  
**Reasoning**: Documentation/legal; not a functional defect.

## R080 — f260fa2774f6 — APPLIES
**pvxs**: Fix shared_array output limit off-by-one  
**pva-rs target**: crates/epics-pva-rs/src/proto/ or format module  
**Reasoning**: Array formatting stops at `i > limit` instead of `i >= limit`, showing one extra element. Off-by-one in truncation logic.
**Fix sketch**: In array display formatter, change condition from `i > limit` to `i >= limit` before appending "...".

## R081 — f44ff9754cc4 — N/A
**pvxs**: Diagnose OSX bind error with address context  
**pva-rs target**: n/a  
**Reasoning**: Logging enhancement; pva-rs Tokio bind errors already include socket address context.

## R082 — f4576d4c332c — APPLIES
**pvxs**: Include input string in NoConvert error message  
**pva-rs target**: crates/epics-pva-rs/src/pvdata/value.rs  
**Reasoning**: When type conversion fails, error message should mention source and target types. pva-rs likely says "Unable to convert" without context.
**Fix sketch**: Add `src_type` and `dst_type` to NoConvert exception; include them in error format: `"Can't convert {} to {}"`.

## R083 — fe6974025ab0 — N/A
**pvxs**: Add missing limits header  
**pva-rs target**: n/a  
**Reasoning**: C++ header; Rust std::num bounds are built-in. Not applicable.

## R084 — 021bcb4a0622 — APPLIES
**pvxs**: Server: fix Dead op cleanup (call cleanup() instead of state=Dead)  
**pva-rs target**: crates/epics-pva-rs/src/server_native/tcp.rs (GET/MONITOR handlers)  
**Reasoning**: When operation reaches Dead state, cleanup must be called immediately to free resources. pva-rs may defer or miss cleanup on early exit.
**Fix sketch**: In GET/MONITOR response sender, when transitioning to final state, call `self.cleanup()` inline instead of relying on post-check. Remove redundant cleanup() call after state machine.

## R085 — 07713faff4a6 — APPLIES
**pvxs**: Fix: schedule initial search use separate event from work queue  
**pva-rs target**: crates/epics-pva-rs/src/client_native/tcp.rs (search_engine.rs)  
**Reasoning**: Initial channel search scheduled via work dispatcher interleaves with channel-build, causing premature search. Needs dedicated timer with 10ms delay.
**Fix sketch**: Add `initialSearcher` event; use `event_add(initialSearcher, &10ms_delay)` instead of `tcp_loop.dispatch()` for initial search scheduling.

## R086 — 0d5a3f62e1fc — APPLIES
**pvxs**: Client: fix locking of monitor members during pop()  
**pva-rs target**: crates/epics-pva-rs/src/client_native/channel.rs or ops_v2.rs (Subscription::pop)  
**Reasoning**: pop() reads `unack`, `window` without lock while user thread runs; ack timer may fire concurrently. Data race on queue/window fields.
**Fix sketch**: Ensure pop() acquires lock for all queue/window/unack reads; separate `wantToNotify()` logic (locked) from `doNotify()` (unlocked).

## R087 — 17464a117acc — APPLIES
**pvxs**: Disallow "null" (0xFF) size by default except Union/string  
**pva-rs target**: crates/epics-pva-rs/src/proto/size.rs  
**Reasoning**: decode_size currently accepts 0xFF (null) everywhere; should reject except Union selector or string. Prevents protocol confusion.
**Fix sketch**: Add `allow_null: bool` parameter to `decode_size`; return `Err` if `b == 0xFF && !allow_null`. Call with `allow_null=true` only for Union/string.

## R088 — 1f91eb9e5d3e — APPLIES
**pvxs**: Client: fix sendDestroyRequest() — remove extra uint16_t count  
**pva-rs target**: crates/epics-pva-rs/src/client_native/ops_v2.rs  
**Reasoning**: sendDestroyRequest encodes `to_wire(u16(1))` before sid/ioid; pvxs removed this padding. Extra byte breaks wire format.
**Fix sketch**: In DESTROY_REQUEST encoding, remove count prefix: only encode `to_wire(sid); to_wire(ioid);`.

## R089 — 21d9cb6b1ce6 — APPLIES
**pvxs**: Fix monitor queue locking (hold lock during queue operations)  
**pva-rs target**: crates/epics-pva-rs/src/client_native/ops_v2.rs and server_native/tcp.rs  
**Reasoning**: Client MONITOR handler and server post() may access queue without lock. Data race on enqueue during pop.
**Fix sketch**: Client: hold lock while enqueuing update. Server: move `testmask(val, mon->pvMask)` outside lock (const), keep queue ops inside lock.

## R090 — (none remaining in batch)
**pvxs**: n/a  
**pva-rs target**: n/a  
**Reasoning**: Batch R061–R089 complete. R090 not in range.

---

**Summary**: 30 reviews analyzed.  
- **APPLIES**: R071, R078, R080, R082, R084, R085, R086, R087, R088, R089 (10 candidates)  
- **ALREADY-FIXED**: 0  
- **N/A**: 19  
- **DUPLICATE** (round-9 b20cdef): 1 (R085 overlaps beacon/search, but distinct fix)
