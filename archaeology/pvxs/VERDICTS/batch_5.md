## R121 — eb11d9e1bc — N/A
**pvxs**: Fix index_sequence C++14 compat for IOC function registration
**pva-rs target**: n/a
**Reasoning**: IOC iochooks integration. pva-rs does not register functions with EPICS IOC shell; this is host-program integration only.

## R122 — f063bd26f5 — DUPLICATE
**pvxs**: Implement TCP search handler on server
**pva-rs target**: server_native/tcp.rs
**Reasoning**: TCP search implementation. Covered by round-9 (8db40be); pva-rs has fully implemented search handlers.

## R123 — fa25bf2aec — DUPLICATE
**pvxs**: Fix TCP search reply variable scope (M→R buffer)
**pva-rs target**: server_native/tcp.rs
**Reasoning**: Continuation of R122; both covered in round-9 working search reply logic.

## R124 — 1ed51c597c — N/A
**pvxs**: Avoid redundant atomic load on compare_exchange_strong
**pva-rs target**: n/a
**Reasoning**: Rust atomics don't require explicit reload; the pattern is language-specific.

## R125 — 6a46e44da9 — ALREADY-FIXED
**pvxs**: Fix SharedPV onLastDisconnect when not open()
**pva-rs target**: server_native/shared_pv.rs:182,414
**Reasoning**: pva-rs fires onLastDisconnect only on subscriber drain transition (prune_subscribers), never on close() when already empty.

## R126 — 785e180f9b — N/A
**pvxs**: Ensure work functor destroyed before completion notification
**pva-rs target**: n/a
**Reasoning**: pva-rs uses Tokio futures, not libevent work queues; Drop cleanup is automatic before await points.

## R127 — 8ed998a896 — ALREADY-FIXED
**pvxs**: Fix race when current Value queued without lock
**pva-rs target**: server_native/shared_pv.rs:263
**Reasoning**: pva-rs clones value before sending; ownership transferred fully, no dangling reference.

## R128 — aea4a4f804 — N/A
**pvxs**: Convert C99 epicsAtomic to C++11 std::atomic
**pva-rs target**: n/a
**Reasoning**: pva-rs uses tokio::sync + parking_lot; C++ idiom differences inapplicable.

## R129 — af973bea66 — N/A
**pvxs**: Harmonize signal handling (unified SigInt wrapper)
**pva-rs target**: n/a
**Reasoning**: pva-rs is a library; signal handling is the application's responsibility.

## R130 — 01745aad72 — N/A
**pvxs**: Fix shared_array::back() member reference typo
**pva-rs target**: n/a
**Reasoning**: pva-rs owns bytes as Vec<u8>; no shared_array API.

## R131 — 056fb2c27b — N/A
**pvxs**: Fix sub-struct iteration offset calculation
**pva-rs target**: crates/epics-pva-rs/src/pvdata/encode.rs
**Reasoning**: C++ field indexing pattern; Rust's enum/struct model doesn't replicate.

## R132 — 05f2b1864e — N/A
**pvxs**: Fix EPICS version check for prepare cleanup hooks
**pva-rs target**: n/a
**Reasoning**: EPICS IOC qsrv integration; pva-rs is not an IOC module.

## R133 — 0ac8092f13 — N/A
**pvxs**: Revert unneeded socket family initializations
**pva-rs target**: n/a
**Reasoning**: Minor evsockaddr cleanup; pva-rs uses system socket calls.

## R134 — 0f90531615 — N/A
**pvxs**: Add broadcast address listeners for EPICS_PVAS_INTF_ADDR_LIST
**pva-rs target**: server_native/udp.rs
**Reasoning**: Already handled; UDP responder listens on all configured interfaces and broadcast.

## R135 — 1220dc7d3e — N/A
**pvxs**: Fix Value::as(T&) and add Value::as(fn&&) variant
**pva-rs target**: n/a
**Reasoning**: pva-rs Value API differs; as_mut() returns Result, no callback variant.

## R136 — 30b040465a — N/A
**pvxs**: Fix Value::unmark() parent traversal pointer order
**pva-rs target**: n/a
**Reasoning**: Rust ownership model doesn't have parent pointer dereference patterns.

## R137 — 383f332d20 — N/A
**pvxs**: Add #include <limits> for std::numeric_limits
**pva-rs target**: n/a
**Reasoning**: C++ include; Rust std always available via traits.

## R138 — 38c15e655f — N/A
**pvxs**: Mark allocArray PVXS_API and add tests
**pva-rs target**: n/a
**Reasoning**: shared_array allocation; pva-rs uses Vec<u8> + ByteOrder reinterpretation.

## R139 — 3e12931f68 — N/A
**pvxs**: Fix tree format escape sequence placement
**pva-rs target**: n/a
**Reasoning**: Formatting bug in string escape; pva-rs format.rs is correct.

## R140 — 46ee1a6917 — N/A
**pvxs**: ACF: use std::any_of instead of std::all_of
**pva-rs target**: n/a
**Reasoning**: IOC security/ACF integration; pva-rs uses on_put/on_rpc handlers.

## R141 — 4d3683d75e — N/A
**pvxs**: Fix RPCBuilder CRTP template parameter
**pva-rs target**: n/a
**Reasoning**: pva-rs client API uses method chaining, not CRTP inheritance.

## R142 — 51bd6a3d6c — N/A
**pvxs**: Fix LocalFieldLog "fast path" condition (pre vs post chain)
**pva-rs target**: n/a
**Reasoning**: IOC field logging; pva-rs is not an IOC module.

## R143 — 5210b7041d — N/A
**pvxs**: Fix TypeDef amend: preserve id when cloning
**pva-rs target**: n/a
**Reasoning**: Type system differs; pva-rs FieldDesc is immutable, no amend workflow.

## R144 — 55d1b7292a — N/A
**pvxs**: Fix hostname extraction to handle IPv6
**pva-rs target**: n/a
**Reasoning**: IOC credentials parsing; pva-rs server auth is generic via handlers.

## R145 — 5f8006fbf3 — N/A
**pvxs**: Fix MCastMembership::operator< return type
**pva-rs target**: n/a
**Reasoning**: Network utility struct; pva-rs uses different socket abstractions.

## R146 — 69ed03e508 — DUPLICATE
**pvxs**: Fix client broadcast address identification (SockAddr comparison)
**pva-rs target**: crates/epics-pva-rs/src/client_native/search_engine.rs
**Reasoning**: Client search address classification; fixed in round-9 (8db40be).

## R147 — 6d9a77d03b — N/A
**pvxs**: Fix SigInt disarm condition (== vs !=)
**pva-rs target**: n/a
**Reasoning**: Signal handler race in util.cpp; pva-rs is a library without global signal machinery.

## R148 — 6fdd4989bd — N/A
**pvxs**: Fix typo: dbChannelFinalFieldSize → dbChannelFinalFieldType
**pva-rs target**: n/a
**Reasoning**: IOC field access API; pva-rs is not an IOC module.

## R149 — 7d16ab3a62 — N/A
**pvxs**: Fix unsigned type sign extension in decode/encode
**pva-rs target**: crates/epics-pva-rs/src/pvdata/encode.rs, client_native/decode.rs
**Reasoning**: pvxs used signed intermediates for unsigned types (sign-extend bug). pva-rs uses unsigned methods (get_u32, put_u32, etc.) throughout; Rust's type system prevents the bug.

## R150 — 7e6a08def7 — N/A
**pvxs**: Fix Delta format: add missing break after union field
**pva-rs target**: n/a
**Reasoning**: Formatting bug in datafmt.cpp; pva-rs format.rs has no equivalent Delta print mode.
