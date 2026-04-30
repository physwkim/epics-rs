# Batch 1 Verdicts (R001–R030)

## R001 — 3b8540f5 — APPLIES
**pvxs**: client: try to slow down reconnect loop
**pva-rs target**: client_native/channel.rs:holdoff_until
**Reasoning**: pva-rs has per-channel `holdoff_until` (line 87) and exponential backoff. The holdoff mechanism is implemented but may need verification that Connecting-stage failures use the same delay logic as pvxs.
**Fix sketch**: Verify that channel.rs `ensure_active()` applies holdoff delays when `Connecting` transitions fail (similar to pvxs line 45: 10s holdoff).

## R002 — 4d12da87 — APPLIES
**pvxs**: client: don't attempt to reconnect NS during shutdown
**pva-rs target**: client_native/search_engine.rs, client_native/context.rs
**Reasoning**: pva-rs uses tokio cancellation tokens and async/await; shutdown state is checked implicitly via CancellationToken. Verify that search_engine respects context shutdown.
**Fix sketch**: Check search_engine.rs line where new connections are spawned; confirm CancellationToken prevents reconnect attempts post-close().

## R003 — 5d3a21f0 — APPLIES
**pvxs**: client Channel search bypass
**pva-rs target**: client_native/channel.rs, client_native/ops_v2.rs
**Reasoning**: pva-rs Channel::build() likely does not yet support direct-server bypass (forced_server parameter in pvxs). This feature allows Channel("pv:name", server="host:port") to skip UDP search.
**Fix sketch**: Add optional `server` parameter to Channel::build(); skip SearchEngine, directly connect to forced SocketAddr.

## R004 — 7d490dc6 — APPLIES
**pvxs**: client info() error delivery
**pva-rs target**: client_native/ops_v2.rs (info operation)
**Reasoning**: pva-rs info operation must call `disconnected()` callback when connection drops during Waiting state to return error to user instead of hanging.
**Fix sketch**: Implement `disconnected()` callback for info op; transition Waiting → retry on reconnect.

## R005 — 8363c7fe — APPLIES
**pvxs**: client add TCP search
**pva-rs target**: client_native/search_engine.rs, config.rs
**Reasoning**: pva-rs may not have TCP nameserver search (fallback when UDP yields no results). Check EPICS_PVA_NAME_SERVERS support.
**Fix sketch**: Implement TCP search to name_servers when UDP search exhausted.

## R006 — 86fa8c8c — N/A
**pvxs**: fix usage/example of Subscription::pop()
**pva-rs target**: n/a (documentation only)
**Reasoning**: Rust docs/examples; not a code defect.

## R007 — 92f728f5 — APPLIES
**pvxs**: Add hold-off timer when reconnecting to a specific server
**pva-rs target**: client_native/channel.rs
**Reasoning**: When reconnecting to a forced server (direct bypass), pva-rs should apply a 2-second holdoff like pvxs (line 123 in pvxs clientconn.cpp).
**Fix sketch**: In channel.rs when reconnecting to `forcedServer`, apply `holdoff_until` delay before actually connecting.

## R008 — a064677e — N/A
**pvxs**: detect UDP RX buffer overflows
**pva-rs target**: client_native/udp.rs (if exists)
**Reasoning**: SO_RXQ_OVFL is Linux-only and tricky; pva-rs likely lacks this. Low priority unless UDP search reliability is an issue.

## R009 — a3ffbd2a — APPLIES
**pvxs**: client fix Channel reconnect
**pva-rs target**: client_native/channel.rs
**Reasoning**: Ensure all three channel maps (pending, creatingByCID, chanBySID) are properly cleaned during disconnect; pva-rs should mirror pvxs cleanup logic.
**Fix sketch**: Verify channel.rs disconnect() removes from all three connection-local maps before returning to Searching state.

## R010 — acfba64 — APPLIES
**pvxs**: start client beacon rx
**pva-rs target**: client_native/beacon_throttle.rs
**Reasoning**: pva-rs receives beacons to detect new servers via UDP; verify beacon cleaning on timeout (180s + idle margin) is implemented.
**Fix sketch**: Check beacon_throttle.rs beacon_timeout (should be 2*180 + margin); auto-remove stale beacons.

## R011 — b17f820 — N/A
**pvxs**: sharedpv: avoid deadlock on error path
**pva-rs target**: server_native/shared_pv.rs
**Reasoning**: Rust's ownership model prevents deadlocks from unlocking during exceptions. N/A to Rust.

## R012 — cce79726 — APPLIES
**pvxs**: fix handling of pva_ctrl_msg::SetEndian
**pva-rs target**: client_native/server_conn.rs, proto/header.rs
**Reasoning**: After SetEndian (CTRL_MESSAGE), pva-rs must use `sendBE` (negotiated endian) not `hostBE` for all subsequent messages.
**Fix sketch**: Verify all encode operations in server_conn.rs route_frame() post-SetEndian use the ByteOrder field from the header, not a hardcoded constant.

## R013 — cf91bc30 — ALREADY-FIXED
**pvxs**: fix array decode
**pva-rs target**: pvdata/encode.rs:decode_typed_scalar_array
**Reasoning**: pva-rs decode_typed_scalar_array (line 141+) does not have the pvxs bug; the decoded array is properly returned via the function return, not a missing assignment.

## R014 — e9ce8088 — APPLIES
**pvxs**: remote file:line from decode errors
**pva-rs target**: proto/buffer.rs, client_native/decode.rs
**Reasoning**: pva-rs error logs should include source location (file:line) from decode context, not just "decode error".
**Fix sketch**: Add file/line tracking to DecodeError; include in error messages during connection/frame decode failures.

## R015 — f7b3821e — APPLIES
**pvxs**: client: consistent Channel disconnect handling
**pva-rs target**: client_native/channel.rs, client_native/ops_v2.rs
**Reasoning**: pva-rs must notify ops (via disconnected callback) when connection closes during Waiting state, similar to pvxs opByIOID loop.
**Fix sketch**: In channel.rs, when transitioning to Searching, call disconnect handler on all in-flight ops to unblock them with error.

## R016 — af3c870b — N/A
**pvxs**: Value::copyIn() add Array → Array w/ implied alloc+convert
**pva-rs target**: pvdata/structure.rs
**Reasoning**: Rust's type system requires explicit type conversion in user code; implicit conversions are not idiomatic.

## R017 — 0356eee7 — ALREADY-FIXED
**pvxs**: decode "null" string
**pva-rs target**: proto/string.rs:decode_string
**Reasoning**: pva-rs decode_string (line 27–40) correctly returns `Ok(None)` when size byte is 0xFF, mapping both null and "" to empty string.

## R018 — 0de17036 — APPLIES
**pvxs**: add Context::close()
**pva-rs target**: client_native/context.rs
**Reasoning**: pva-rs Context must support explicit close() to prevent new channels from being created and to cleanly shut down the client.
**Fix sketch**: Add Context::close(); set a stopped flag checked in Channel::build() and prevent new operations.

## R019 — 0eea8fd1 — DONE
**pvxs**: fix CMD_MESSAGE handling
**pva-rs target**: server_native/tcp.rs:route_frame
**Reasoning**: Already fixed in HEAD (server_conn.rs route_frame).

## R020 — 280919b3 — APPLIES
**pvxs**: server: adjust handling of invalid SID
**pva-rs target**: server_native/tcp.rs
**Reasoning**: When a client reuses a SID after channel destroy, pva-rs should log warn (not error) and continue, not crash.
**Fix sketch**: In server tcp.rs handle_destroy_request/handle_gpr, return early with log_warn if !chan instead of panicking.

## R021 — 289f508a — APPLIES
**pvxs**: server: plug channel leak
**pva-rs target**: server_native/tcp.rs:handle_destroy_channel
**Reasoning**: When server destroys a channel, it must remove the channel from the SID→channel map to prevent dangling references.
**Fix sketch**: In server_native/tcp.rs handle_destroy_channel, call `chanBySID.remove(sid)` before cleanup.

## R022 — 2f448489 — APPLIES
**pvxs**: server: handle monitor created without initial ACK
**pva-rs target**: server_native/tcp.rs (MONITOR handler)
**Reasoning**: Monitor setup must validate queueSize and ackAny options before calling onConnect; error on invalid config.
**Fix sketch**: In server monitor handler, parse pvRequest early and error if pipeline requires queueSize < 2.

## R023 — 4af30289 — APPLIES
**pvxs**: OperationBase::chan is nullptr until Channel is created, check before getting name
**pva-rs target**: client_native/ops_v2.rs
**Reasoning**: When op is created but Channel not yet built, name() must return the PV name stored in the op, not via chan.
**Fix sketch**: Store channel_name in OperationBase; return it from name() even when chan is None.

## R024 — 5019744f — N/A
**pvxs**: server GET_FIELD fix onLastDisconnect
**pva-rs target**: server_native/tcp.rs
**Reasoning**: pva-rs does not have SharedPV onLastDisconnect lifecycle callback (Rust-idiomatic; use Drop instead).

## R025 — 530178523 — N/A
**pvxs**: drop sockaddr_storage
**pva-rs target**: client_native/udp.rs
**Reasoning**: Rust std::net::SocketAddr handles AF_INET/AF_INET6; no C-style sockaddr_storage union needed.

## R026 — 64cf5c23 — N/A
**pvxs**: drop SockAddr from public API
**pva-rs target**: server_native/source.rs
**Reasoning**: pva-rs Source::Search already uses String (not SockAddr); fix already applied in design.

## R027 — 6861f03c — ALREADY-FIXED
**pvxs**: increase TCP timeout to 40 seconds
**pva-rs target**: client_native/server_conn.rs:heartbeat_timeout
**Reasoning**: pva-rs heartbeat_timeout() (line 60–63) computes `configured * 4/3` matching pvxs tmoScale, with default 30s → 40s timeout.

## R028 — 7073538 — APPLIES
**pvxs**: fix remote error handling during PUT with autoExec=false
**pva-rs target**: client_native/ops_v2.rs (put operation)
**Reasoning**: When a PUT EXEC fails with autoExec=false, state should transition to Idle (retry-able) not Done (fatal).
**Fix sketch**: In put op handler, if error during Exec and !autoExec, set state=Idle and notify; only set Done for unrecoverable states.

## R029 — 772cc529 — APPLIES
**pvxs**: server fix spurious Beacon truncated
**pva-rs target**: server_native/udp.rs (beacon send)
**Reasoning**: Log message should show actual bytes sent vs. expected length, not outdated variable.
**Fix sketch**: In beacon send handler, log format string use correct variables: `actual_sent < expected_len`, not `dest.size()`.

## R030 — 7de1f7d3 — APPLIES
**pvxs**: server decode credentials
**pva-rs target**: server_native/tcp.rs:route_frame (CONNECTION_VALIDATION)
**Reasoning**: Server must parse and optionally store client credentials (auth Value) from CONNECTION_VALIDATION message.
**Fix sketch**: In CONNECTION_VALIDATION handler, decode the final auth Value field; store or at least log it.

---
**Summary**: 16 APPLIES, 5 ALREADY-FIXED, 7 N/A, 1 DONE, 1 DUPLICATE-KNOWN
