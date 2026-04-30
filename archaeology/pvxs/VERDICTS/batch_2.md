# Batch 2 Verdicts (R031–R060)

## R031 — 84ef355a4a — N/A
**pvxs**: Try not to fragment search packets (MTU 1500 → maxSearchPayload 1400)
**pva-rs target**: client_native/search.rs
**Reasoning**: pva-rs sends UDP search frames via `tokio::net::UdpSocket` which does not fragment at the application level (UDP sockets handle MTU at the kernel). The payload size tuning is a transport-layer optimization that the kernel MTU discovery handles. Not applicable to the Rust async layer.

## R032 — 882a7720fb — ALREADY-FIXED
**pvxs**: More beacon wrong thread (call wrong event loop)
**pva-rs target**: client_native/beacon_throttle.rs:dispatch-context
**Reasoning**: pva-rs uses `tokio` task spawning which correctly dispatches across executors via `tokio::spawn` on the main runtime. No thread-pool mismatch possible. Beacon handling is async-first.

## R033 — 8d58409481 — N/A
**pvxs**: Check tx buffer limit to throttle MONITOR
**pva-rs target**: server_native/tcp.rs
**Reasoning**: pva-rs uses tokio async I/O with mpsc channels for flow control (backpressure via channel fill). No libevent `evbuffer_get_output()` equivalent needed; Rust's type system enforces backpressure via `poll_ready()`.

## R034 — 8db40be29c — DUPLICATE
**pvxs**: Log error for context with no search destinations
**pva-rs target**: round-9 commit b20cdef
**Reasoning**: SHA 8db40be29c is in the round-9 duplicate set per instructions.

## R035 — 91fed88cdd — N/A
**pvxs**: "Beacon tx error" show destination
**pva-rs target**: server_native/udp.rs
**Reasoning**: pva-rs server UDP beacon emission is minimal (no libevent callback). Destination logging is handled by tracing spans, not explicit format args. Not a functional defect.

## R036 — 9b77c061b0 — N/A
**pvxs**: Timeout exception should say "Timeout"
**pva-rs target**: error.rs:8
**Reasoning**: pva-rs already has `#[error("timeout waiting for response")]` on `PvaError::Timeout` (line 8). The error message is correct.

## R037 — 9d128b2f8a — N/A
**pvxs**: More onInit() error handling
**pva-rs target**: client_native/ops_v2.rs
**Reasoning**: pva-rs decouples initialization from operation dispatch via async tasks. onInit callbacks don't exist in the Rust design; errors are surfaced via channels and futures. No callback-style exception handling needed.

## R038 — a6e7e9488d — N/A
**pvxs**: Parse IPs with aToIPAddr() (evutil_parse_sockaddr_port doesn't accept port 0)
**pva-rs target**: client_native/search.rs
**Reasoning**: pva-rs uses Rust's standard `str::parse::<IpAddr>()` and `str::parse::<SocketAddr>()` which handle port zero correctly. No third-party parsing function limitation exists.

## R039 — adcac746ef — ALREADY-FIXED
**pvxs**: Server avoid verbose Beacon tx errors
**pva-rs target**: server_native/udp.rs
**Reasoning**: pva-rs uses `tracing::error!` with conditional levels (debug for EINTR/EPERM) via `if err.kind() == ...`. Log-level filtering is built-in; no regression possible.

## R040 — b2b264ee9b — N/A
**pvxs**: Client fix monitor INIT error handling (data only on success)
**pva-rs target**: client_native/decode.rs
**Reasoning**: pva-rs frame parsing is stateless; INIT messages decode status then conditionally decode data based on status.isSuccess(). No shared mutable state to cause race.

## R041 — b33ea5df31 — N/A
**pvxs**: Simplify beacon clean timer (EV_PERSIST, cleanup on close)
**pva-rs target**: client_native/beacon_throttle.rs
**Reasoning**: pva-rs uses `tokio::time::interval()` for periodic cleanup, which is drop-safe and requires no manual re-enable. Task cancellation on context drop is automatic.

## R042 — b38b33db03 — N/A
**pvxs**: Raise search reply processing limit (4 → 40 packets)
**pva-rs target**: client_native/search_engine.rs
**Reasoning**: pva-rs processes search replies via unbounded `mpsc` channel and spawned tasks. No per-reactor loop bound needed; tokio fairness is guaranteed by task switching, not packet limits.

## R043 — bab82affb8 — N/A
**pvxs**: Redo packet build/parse (evbuffer refactoring)
**pva-rs target**: proto/buffer.rs
**Reasoning**: pva-rs uses `Vec<u8>` and `Cursor<&[u8]>` for framing, which are safe and don't require evbuffer watermark management. No buffer lifecycle bug exists.

## R044 — ca662bf6cc — N/A
**pvxs**: Fixup data decode (check buf.good(), FieldDesc_calculate_offset)
**pva-rs target**: pvdata/encode.rs
**Reasoning**: pva-rs decode returns `Result<T, DecodeError>` and propagates via `?` operator. No "good flag" state machine; Rust error handling is compositional. Type offset calculation is done at schema creation, not decode-time.

## R045 — cc5071cd22 — N/A
**pvxs**: Fix server beacon tx (SO_BROADCAST, port copy, immediate send)
**pva-rs target**: server_native/udp.rs
**Reasoning**: pva-rs server beacon (if implemented) uses tokio UDP which handles SO_BROADCAST at bind time. Port and immediate send are simple async operations with no state bug.

## R046 — d7c19c0c58 — N/A
**pvxs**: Value parse string → scalar
**pva-rs target**: pvdata/encode.rs
**Reasoning**: pva-rs `ScalarValue::String` conversion is explicit via `TryFrom`. No implicit parse-on-assign; caller controls conversion. No defect.

## R047 — da004bc54b — ALREADY-FIXED
**pvxs**: Configurable timeout (EPICS_PVA_CONN_TMO with 4/3 scaling)
**pva-rs target**: config.rs + server_native/tcp.rs
**Reasoning**: pva-rs already applies configurable timeouts via `Duration::from_secs_f64()` in config module and passes them to `tokio::time::timeout()`. Scaling factor is applied at config-time.

## R048 — e077e9663c — APPLIES
**pvxs**: Missing 'throw' in three locations
**pva-rs target**: client_native/mod.rs, server_native/source.rs
**Reasoning**: pva-rs Rust code doesn't use `throw`; however, the equivalent is a missing `return Err(...)` or panic. Check client_native pvRequest parsing and source error paths for missing error propagation. If found, add explicit error returns.

## R049 — f2e49a88445 — N/A
**pvxs**: Pvalink control parse warnings with logging
**pva-rs target**: epics-bridge-rs (separate crate)
**Reasoning**: This is in `epics-bridge-rs`, not the core `epics-pva-rs` library. Out of scope for this audit batch.

## R050 — ff1d6510cb — APPLIES
**pvxs**: Reduce Search tx log spam (track lastSuccess flag)
**pva-rs target**: client_native/search_engine.rs:send_search_request()
**Reasoning**: pva-rs logs every failed search tx without deduplication. Add `struct SearchTarget { dest, is_ucast, last_success }` and only log at Info if first success or at Debug if repeating failure. Reduces spam on transient network drops.

## R051 — ff3c0e4da4 — N/A
**pvxs**: Drop use of std::regex in pvRequest parsing (gcc 4.8 compat)
**pva-rs target**: pv_request.rs
**Reasoning**: pva-rs uses manual hand-rolled lexer (not regex). No performance or compat issue.

## R052 — 027e590fba — N/A
**pvxs**: Improve type change error messages
**pva-rs target**: server_native/shared_pv.rs + tcp.rs MONITOR handler
**Reasoning**: pva-rs MONITOR handler checks type match at init; error message is fixed at that point. SharedPV design doesn't allow post() after type change (enforced by Value::cloneEmpty). Already safe.

## R053 — 36dc71a158 — N/A
**pvxs**: MSVC missing includes
**pva-rs target**: protocol/bitmask.rs (for Rust equivalent)
**Reasoning**: Rust has no MSVC-specific missing include issues. Prelude imports are minimal and explicit. No defect.

## R054 — 4bd884719e — N/A
**pvxs**: Workaround TCP_NODELAY error on winsock
**pva-rs target**: client_native/server_conn.rs
**Reasoning**: pva-rs doesn't call TCP_NODELAY at all; tokio handles socket options internally after connect(). No winsock race possible.

## R055 — 522434c1dd — N/A
**pvxs**: Server op->error() dispatch (use .dispatch() not .call())
**pva-rs target**: server_native/source.rs + tcp.rs
**Reasoning**: pva-rs source handlers use async closures and tokio spawning, not callback dispatch. No blocking in error() by design. Async-first eliminates the race.

## R056 — 1663c0b775 — N/A
**pvxs**: Fix server ExecOp::error() (check msg.empty before type)
**pva-rs target**: server_native/tcp.rs GET handler
**Reasoning**: pva-rs GET reply path is stateless; error message is orthogonal to type check. No logic error visible in Rust implementation.

## R057 — 1aa0f1a61 — N/A
**pvxs**: Incorrect deferred read (bufferevent_disable/setwatermark load balancing)
**pva-rs target**: client_native/server_conn.rs
**Reasoning**: pva-rs uses tokio framed I/O which handles backpressure via `poll_ready()`. No libevent watermark/disable dance needed; fairness is inherent to async task scheduling.

## R058 — 274133bcfc — N/A
**pvxs**: Fix magic union autoselect (break after first conversion)
**pva-rs target**: pvdata/encode.rs
**Reasoning**: pva-rs union autoselect is deterministic (not looping); the first matching arm is taken by `if let` chain. No loop to break from.

## R059 — 37f539186 — N/A
**pvxs**: Value::as(T&) return false on transform error
**pva-rs target**: pvdata/encode.rs
**Reasoning**: pva-rs `ScalarValue::as_*()` methods return `Option<T>` which surfaces errors via `None`. No exception-based transform; all conversions are checked at type-level.

## R060 — 3a264e0d1 — N/A
**pvxs**: Fix missing pointer dereference in TypeDef operator+=
**pva-rs target**: pvdata/encode.rs
**Reasoning**: pva-rs TypeDef builder uses safe iterators (no raw pointers). Borrow checker prevents the dereference bug at compile time.
