## af9be69f — N/A — minor

**Reason**: Log header cleanup (remove unused include). No functional change.

## 4b5b134e — N/A — Value traverse to Struct parent

**Reason**: Feature enhancement to Value traversal API. Adds struct_index/traverse methods for nested field access. Not a bug fix.

## a34fe899 — N/A — minor

**Reason**: Whitespace/formatting in data encoding. No functional change.

## de47ee4ac — APPLIES — fixup offset calculations

**Reason**: Critical encoding bug in nested struct serialization. Offset range calculations were not adjusted for struct members, leading to incorrect field iteration indices.

**pva-rs target**: `/crates/epics-pva-rs/src/pvdata/encode.rs` (encode_pv_field, decode_pv_field for Structure type)

**Fix**: When serializing/deserializing nested structures, field indices must be relative to the parent struct's base offset, not absolute. The pvxs fix subtracts `top.desc->offset` from loop bounds. pva-rs uses bit_offset-relative iteration (lines ~960, 1046) which avoids this bug by design.

## b7feb98a — N/A — minor

**Reason**: Header export/visibility cleanup. No functional change.

## fecdd52e — N/A — update logging

**Reason**: Logging framework refactor (log.cpp modernization). No protocol/data handling change.

## 9e0598fb — N/A — minor

**Reason**: Documentation/comment updates. No functional change.

## 07ad40586 — N/A — doc

**Reason**: Documentation build configuration. Not a code fix.

## 68cb777702 — N/A — use std::make_shared where possible

**Reason**: C++ optimization (allocator choice). Not relevant to wire protocol.

## 71905b314 — N/A — TypeDef appending

**Reason**: Type introspection API enhancement. Not a bug fix.

## bc048cea — N/A — TypeDef helpers

**Reason**: Type descriptor utility additions. Not a bug fix.

## 4a38184 — N/A — BitMask::onlySet() w/ restricted range

**Reason**: BitSet utility enhancement. Edge case handling for range-limited bit operations.

## 787ad4841 — N/A — better GUID

**Reason**: GUID generation improvement (entropy, format). Not a bug fix affecting protocol.

## 35dd24f4 — N/A — Value::tryAs() tryFrom()

**Reason**: API convenience methods. Feature enhancement, not a bug fix.

## d2c4457 — N/A — server pvlist handling

**Reason**: Server introspection feature (LIST operation). Not core protocol bug.

## cdeecdcc — N/A — update data.h

**Reason**: Type introspection API clarifications. Not a functional bug fix.

## 583ee684 — N/A — generalize Get/Put/RPC handling

**Reason**: Refactoring of operation dispatch code. Code consolidation, not a bug fix.

## 851182c — N/A — ServerChannel_shutdown

**Reason**: IOC-specific server-channel cleanup. Affects serverconn.cpp and channel state; pva-rs server (tcp.rs:1115-1147) handles DESTROY_CHANNEL by direct HashMap removal with automatic Drop-based cleanup. Not applicable to pva-rs which uses tokio task abort patterns.

## c0c7348 — N/A — fixup FixedBuf usage

**Reason**: Test utility fix for buffer size handling (off-by-one in string literal encoding). Not wire protocol bug.

## c7803b5c — N/A — minor

**Reason**: Whitespace/cleanup. No functional change.

## 69c3549 — N/A — oops

**Reason**: Typo fix (chan->opByIOID to opByIOID). IOC-specific server operation tracking; not applicable to pva-rs client/server.

## 35d8e8ace — N/A — minor

**Reason**: Constant/whitespace. No functional change.

## aa973c9a — N/A — server boilerplate reduction

**Reason**: Server-side code refactoring. Consolidation of response-building patterns; pva-rs uses different patterns (async/tokio).

## 7c47975 — N/A — Got GET

**Reason**: Feature implementation (GET operation support). Not a bug fix.

## 9af9f028 — N/A — from_wire_type_value

**Reason**: Wire decoding helper for type+value pairs. Feature implementation.

## 7791205 — N/A — changed tracking via enclosing field

**Reason**: Change-tracking mechanism refactor (valid bitset per field vs per-struct). Architectural change, not a bug fix.

## 810b7d3b — N/A — use freeze/cast members

**Reason**: Array API refactoring. Method rename/consolidation.

## 7dd33a0c — N/A — shared_array redo freeze/cast as members

**Reason**: Array view API refactor. Code consolidation.

## dd7a3d2 — N/A — start xcode data

**Reason**: Data encoding implementation (XCode format). Feature implementation.

## 21ac70db — N/A — redo TypeDef

**Reason**: Type introspection refactoring. Consolidation of TypeDef mechanisms.

