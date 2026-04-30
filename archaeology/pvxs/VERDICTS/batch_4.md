# Batch 4 Verdicts — R091–R120

## R091 — 2ab08c115b — N/A
**pvxs**: sharedpv: fix race w/ subscribers list
**pva-rs target**: src/server_native/shared_pv.rs
**Reasoning**: pva-rs uses `Vec<mpsc::Sender>` under `parking_lot::Mutex` with no callback cycles. Unlocking during connect() is unnecessary because tokio channels are send-safe and don't block the Mutex holder. The pvxs pattern of dropping an UnGuard inside a nested scope to manage callback reentrancy doesn't apply.
**Verdict**: N/A — async + ownership model eliminates the callback-reentrancy race.

## R092 — 344a96207f — N/A
**pvxs**: Fix pvxsl: serv->getSource needs order argument
**pva-rs target**: n/a
**Reasoning**: pva-rs has no IOC integration or pvxsl-style admin tool. This is a pvxs-specific helper API for the EPICS IOC.

## R093 — 3dd4dd6a66 — ALREADY-FIXED
**pvxs**: client: different onInit() for monitor
**pva-rs target**: src/client_native/ops_v2.rs (~lines 280–320)
**Reasoning**: pva-rs ops_v2 never exposed onInit to monitor builders. GET/PUT keep onInit, but Monitor users have no access to it — semantic change already enforced by the API.

## R094 — 48ca7b34c7 — N/A
**pvxs**: fix VERSION_INT() order
**pva-rs target**: n/a
**Reasoning**: pva-rs is pure Rust; no C-preprocessor version macros. Compile-time EPICS version checks don't apply.

## R095 — 4ee7ce210841 — ALREADY-FIXED
**pvxs**: ignore beacons with proto!='tcp'
**pva-rs target**: src/server_native/udp.rs (~lines 110–120)
**Reasoning**: run_udp_responder_with_config already silently drops non-TCP-source beacons during decode; the BitSet iteration stops if proto is non-TCP (implicit in the frame structure).
**Note**: Explicit proto check was not needed because the decode loop naturally handles malformed/non-TCP frames.

## R096 — 4fac0672872b — APPLIES
**pvxs**: client: monitor avoid extra wakeups
**pva-rs target**: src/client_native/ops_v2.rs (MonitorState) / monitor decoder
**Reasoning**: pva-rs monitor loop wakes the consumer callback on every DATA frame, even empty ones. No guard to avoid spurious wakeups when the queue is empty. Requires tracking `needNotify` flag (set on pop-empty, cleared on notify).
**Fix sketch**: Add `needNotify: bool` to MonitorState; in run_monitor_loop, only call the callback if `needNotify` is true, then clear it. Set `needNotify = true` when the queue becomes empty after pop().

## R097 — 525c711ee56 — APPLIES
**pvxs**: server: reduce log spam from beacon tx
**pva-rs target**: src/server_native/udp.rs (run_udp_responder_with_config, beacon loop)
**Reasoning**: pva-rs logs beacon TX failures at WARN on every retry with no demoting logic. Should track per-destination success state and downgrade repeated failures to DEBUG, upgrade first success after failure to INFO.
**Fix sketch**: Wrap beacon destinations in `(SockAddr, bool)` pairs; on sendto failure, set flag=false and use DEBUG level if already false. On success, if flag was false, use INFO level; set flag=true.

## R098 — 57f9468c86 — DUPLICATE
**pvxs**: udp: clarify orig/reply addressing, fix mcast handling
**pva-rs target**: Already fixed in b20cdef round-9 commit
**Reasoning**: Covered by A5 fix in round-9 — reply-source-NIC routing + mcast-source validation.

## R099 — 5897fe273e — N/A
**pvxs**: fix intermittent of testsock
**pva-rs target**: n/a
**Reasoning**: pvxs test harness socket utility; pva-rs has no equivalent. Tokio UDP is non-blocking by design.

## R100 — 5ddc2beb47 — APPLIES
**pvxs**: server monitor throttle using send queue size
**pva-rs target**: src/server_native/tcp.rs (handle_MONITOR, connection TX backpressure)
**Reasoning**: pva-rs monitor handler posts to mpsc without checking TX buffer fill. Should suspend reading if the connection's mpsc is backed up, resume when drained. Requires a backlog queue (like pvxs).
**Fix sketch**: Add `backlog: VecDeque<Box<dyn Fn() + Send>>` to ServerConn; in bevWrite (on writable), drain backlog while mpsc depth < threshold; if over-full, defer monitor replies to backlog instead of posting directly.

## R101 — 722759416b — APPLIES
**pvxs**: server: change monitor watermark meaning
**pva-rs target**: src/server_native/tcp.rs (op_monitor, low/high watermark handling)
**Reasoning**: pva-rs uses edge-triggered watermark callbacks (if crossing, fire once). Should be level-triggered (fire when condition holds, not just on transition). Requires `lowMarkPending`/`highMarkPending` flags.
**Fix sketch**: Add `low_mark_fired` and `high_mark_fired` bools to MonitorOp; check `window <= low` and only fire if `!low_mark_fired`, then set it. Clear flag when `window > low` again.

## R102 — 78273124f0 — APPLIES
**pvxs**: more server beacon
**pva-rs target**: src/server_native/udp.rs (beacon init / SendCtx)
**Reasoning**: pva-rs beacon message doesn't include sequence number, flags, or change count. Should populate these fields to match pvxs; update beacon counter to "burst" 10 times fast then fall back to 180s interval.
**Fix sketch**: Add `beacon_seq`, `beacon_cnt`, and `atomic beacon_change_count` to SendCtx; include in beacon msg; reset timer interval logic based on beacon_cnt < 10 check.

## R103 — 78f54455e6 — N/A
**pvxs**: Value fix delta output format
**pva-rs target**: n/a
**Reasoning**: pva-rs has no equivalence of Value format/delta printing. Display/formatting is outside the PVA protocol scope.

## R104 — 82adcb938d — APPLIES
**pvxs**: server monitor pvRequest
**pva-rs target**: src/server_native/tcp.rs (handle_MONITOR) + src/pvdata/encode.rs (to_wire_valid)
**Reasoning**: pva-rs monitor handler doesn't track the pvRequest field mask (BitSet). Sends all fields every time instead of honoring the client's field-selection mask. Needs to extract BitSet from pvRequest and pass it to encode.
**Fix sketch**: Parse pvRequest BitSet in handle_MONITOR; store in MonitorOp. In to_wire_valid, use the mask to prune the serialized fields.

## R105 — 839fc01bfd — APPLIES
**pvxs**: fix Source::Search::source() IPv6 representation
**pva-rs target**: src/server_native/source.rs (Search struct) / src/server_native/tcp.rs (onSearch)
**Reasoning**: pva-rs Search::source field may use IPv4 dotted-IP format even for IPv6 addresses (using ipAddrToDottedIP). Should use evutil_inet_ntop to produce proper IPv6 notation (with colons).
**Fix sketch**: In onSearch handler, check msg.server.family() and call inet_ntop(AF_INET6, ...) for IPv6, else inet_ntop(AF_INET, ...). Store in a 80-byte buffer (>= INET6_ADDRSTRLEN+1).

## R106 — 8c55bf7de7 — APPLIES
**pvxs**: SharedPV monitor discard empty updates
**pva-rs target**: src/server_native/tcp.rs (op_monitor)
**Reasoning**: pva-rs posts all values (even empty ones) to monitors without checking the pvRequest mask. Should filter updates: only post if at least one field in the update matches the requested mask.
**Fix sketch**: Add testmask helper (checks if any marked field in update overlaps mask). In op_monitor, call testmask before enqueueing; skip post if returns false.

## R107 — 92d519702f — APPLIES
**pvxs**: client: search retry step reset on channel reconnection
**pva-rs target**: src/client_native/channel.rs (Channel struct) / search_engine.rs
**Reasoning**: On reconnect, the channel's search-retry counter isn't reset, so it stays at a high backoff step. Should reset to step 0 when the connection succeeds.
**Fix sketch**: In on_connect (Channel), reset `search_step = 0` (or equivalent) so the next search re-enters the BACKOFF_SECS sequence from the start.

## R108 — 94b60d0ac0 — APPLIES
**pvxs**: client monitor cleanup and logging
**pva-rs target**: src/client_native/ops_v2.rs (run_monitor_loop)
**Reasoning**: pva-rs monitor loop has minimal logging and no per-queue pop diagnostics. Should add channelName capture and per-pop logging (data/exception/empty).
**Fix sketch**: Store channel name in MonitorState at construction. Log at info! each pop, showing whether the event was data, exception, or null. Log errors if empty updates arrive.

## R109 — 94f0065a4d — APPLIES
**pvxs**: fix beaconSenders locking
**pva-rs target**: src/client_native/beacon_throttle.rs / search_engine.rs
**Reasoning**: pva-rs beacon tracking (BeaconTracker) may not protect `beaconSenders` map with a consistent lock when called from both UDP (beacon RX) and TCP (search reply) workers. Race on update/read.
**Fix sketch**: Wrap beaconSenders in Mutex; hold lock across the entire find+update or find+insert sequence in onBeacon and tickBeaconClean.

## R110 — 9fefa95df1f — APPLIES
**pvxs**: fix client PUT
**pva-rs target**: src/client_native/ops_v2.rs (op_put, handle_GET_PUT_RPC response)
**Reasoning**: pva-rs op_put doesn't distinguish between GetOPut state (waiting for type) and Exec state (sending value). Should only serialize valid-mask for Exec, not GetOPut.
**Fix sketch**: In the Exec response handler, check `if state == Exec { to_wire_valid(...) }` but not for GetOPut. PUT needs the type from server before sending the user's value.

## R111 — a2b424cba2 — APPLIES
**pvxs**: increase max UDP packet batch size
**pva-rs target**: src/client_native/search_engine.rs (UDP RX handler loop)
**Reasoning**: pva-rs processes up to 4 UDP packets per reactor cycle before returning to async. Should increase to 16 to batch more responses and reduce reactor wake-ups.
**Fix sketch**: In search_engine packet RX loop, increase the iteration count from 4 to 16 before breaking to the reactor.

## R112 — a36dd2a9cca — APPLIES
**pvxs**: fix monitor pipeline and finish()
**pva-rs target**: src/client_native/ops_v2.rs (run_monitor_loop) + src/server_native/source.rs (MonitorControlOp::finish)
**Reasoning**: pva-rs finish() doesn't respect the pipelined-monitor ACK semantics; client-side ACK scheduling is off. Should post finish with force=true flag; client should ACK before closing.
**Fix sketch**: In finish(), pass force=true to doPost. In run_monitor_loop, send an immediate ACK even if window is full when a finish event arrives.

## R113 — b0eecb949f — APPLIES
**pvxs**: fixup client operation object lifetime
**pva-rs target**: src/client_native/ops_v2.rs (op_get/put/rpc destructors)
**Reasoning**: pva-rs GET/PUT/RPC operations may be dropped by the user before the TCP worker has finished cleanup. The operation's drop should schedule an implicit cancel on the TCP loop to avoid orphaned state.
**Fix sketch**: Wrap the returned Operation in a custom drop guard that calls cancel(true) on the TCP loop if the operation isn't already Done.

## R114 — b8d204e35c — ALREADY-FIXED
**pvxs**: proto bug: client search requests incorrectly set Server direction
**pva-rs target**: src/client_native/search_engine.rs (search header construction)
**Reasoning**: pva-rs already sends SEARCH with flags=0 (not pva_flags::Server). The wire format is correct.

## R115 — c32d1ae0e2 — APPLIES
**pvxs**: fix pipeline w/ queueSize=1
**pva-rs target**: src/server_native/tcp.rs (handle_MONITOR, limit validation)
**Reasoning**: If queueSize=1 and pipeline=true, the window can be 0, causing the handler to never send replies. Should ensure limit >= window.
**Fix sketch**: In handle_MONITOR INIT, after setting limit = qSize, add `if op.limit < op.window { op.limit = op.window }`.

## R116 — c373da671b — APPLIES
**pvxs**: server: fix default monitor queueSize to 4
**pva-rs target**: src/server_native/tcp.rs (handle_MONITOR, MonitorOp default)
**Reasoning**: pva-rs MonitorOp defaults to window=0, limit=1. Should default limit to 4 per spec.
**Fix sketch**: Change default from `limit: 1` to `limit: 4` in MonitorOp struct initialization.

## R117 — cc5d382930 — DUPLICATE
**pvxs**: client: monitor yield "complete" updates
**pva-rs target**: Already fixed in b20cdef round-9 commit
**Reasoning**: Covered by A6 fix in round-9 — fill_unmarked_from_prior helper and prior-value tracking in run_monitor_loop.

## R118 — cfde0e26d8 — APPLIES
**pvxs**: avoid assert for mostly absurdly long PV names
**pva-rs target**: src/client_native/search_engine.rs (search packet packing)
**Reasoning**: pva-rs search packet builder may panic (assert/unwrap) if a single PV name exceeds MTU but fits in UDP. Should gracefully attempt send instead of failing.
**Fix sketch**: Remove panic/assert when a single PV exceeds maxSearchPayload; allow best-effort send (fragmentation handled by kernel).

## R119 — d15430fb17 — APPLIES
**pvxs**: fix poke race
**pva-rs target**: src/client_native/search_engine.rs (poke/wake logic)
**Reasoning**: pva-rs poke() flag is accessed from both UDP (beacon RX) and TCP (builder exec) workers without synchronization. Race on poked flag reads/writes.
**Fix sketch**: Wrap poke logic in a Mutex; check and set `poked` atomically inside the lock before scheduling the search timer.

## R120 — dd2f076b4a — APPLIES
**pvxs**: client: only advance search bucket during normal search
**pva-rs target**: src/client_native/search_engine.rs (tick_search / SearchKind logic)
**Reasoning**: pva-rs advances the search bucket on every tick, even forced wakeups. Should only advance during normal periodic search, not on poke/force.
**Fix sketch**: In tick_search, check the SearchKind; only increment nSearch if kind==Normal/Check, not for Force/Discover paths.

