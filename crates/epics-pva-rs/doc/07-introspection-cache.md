# 07 — Introspection cache (0xFD / 0xFE)

PVA lets either side reuse a previously-seen `FieldDesc` by emitting a
short cache reference instead of re-encoding the full type tree.
This is a per-direction-per-connection cache; it is NOT a wire-version
flag and either side may opt out at any time.

## Wire markers

| Tag | Meaning |
|-----|---------|
| `0xFD <slot:u16> <body>` | Define cache slot `<slot>` with the inline `<body>` and use it for this descriptor. |
| `0xFE <slot:u16>` | Lookup cache slot `<slot>` (must have been defined earlier on this connection). |
| `0xFF` | Null marker — used by callers (Variant nullable, etc.). Not a cache marker. |

`<body>` is a normal type descriptor — `Scalar`, `Structure`,
`Union`, `StructureArray`, ... — recursively. Define markers can
nest: a structure body can itself contain `0xFD` for sub-fields.

## Decode side

`pva-rs` always accepts both markers. `decode_type_desc_cached`
takes a mutable `TypeCache` and inserts on `0xFD`, looks up on `0xFE`,
recurses on regular tags. Cache misses are fatal —
`DecodeError("typecache miss for slot N")`. Code: `pvdata/encode.rs:312`.

## Encode side

By default, `pva-rs` emits **inline** (no `0xFD` / `0xFE`). The
emit-side helper is `encode_type_desc(desc, order, out)`. The cached
variant `encode_type_desc_cached(desc, order, &mut cache, out)`
exists but is gated behind `PvaServerConfig::emit_type_cache = true`.

### Why default-inline?

EPICS Base 7.x's `pvAccessCPP` (the reference C++ client implementation
that ships with EPICS Base) does NOT understand the `0xFD` / `0xFE`
markers. When a frame containing one arrives, pvAccessCPP reads the
marker as a regular type tag, mis-parses, then reads beyond the
payload boundary into the next frame, surfacing as:

```
Protocol Violation: Not-a-first segmented message expected from the client
  at codec.cpp:362
```

pvxs and pvAccessJava emit the markers; pvAccessCPP does not. Since
the `pva-rs` server may serve any of these clients in a mixed
environment, we default to inline.

### When to enable?

Set `PvaServerConfig::emit_type_cache = true` when:

- The deployment is pvxs-only (or only Rust+Java clients).
- The repeated INIT response cost (e.g. NTScalar / NTTable on
  `pvmonitor_handle`) is a measurable bandwidth issue.

The bandwidth win is significant for NTTable: the inline encoding of
a 12-column table descriptor is ~250 bytes; the cached reference is 3
bytes.

### Reference

This decision is captured as a kodex `decision` memory titled
"pvAccessCPP does not parse 0xFD/0xFE type-cache markers". The bug
was hit in the field on 2026-04, after switching the server emit-side
on by default for a release; reverted in v0.10.x.

## Server emit-side behaviour

```rust
if config.emit_type_cache {
    encode_type_desc_cached(&intro, order, &mut encode_cache, &mut payload);
} else {
    encode_type_desc(&intro, order, &mut payload);
}
```

`encode_cache` is per-connection: `EncodeTypeCache::new()` in
`server_native/tcp.rs::handle_connection_io`. Lives until the
client disconnects.

## Client side

`ServerConn::type_cache()` returns `Arc<Mutex<TypeCache>>` shared with
every op decoder. The cache is per-connection too; reconnects reset
it. Code: `client_native/server_conn.rs::ServerConn`.

## Format gotchas

- The slot ID is a `u16` in the connection's negotiated byte order
  (set during `SetByteOrder`, NOT necessarily LE).
- Slots are arbitrary — peers don't have to use a specific numbering.
  pvxs uses sequential slots starting at 1; we follow the same.
- Slot 0 is reserved by pvxs (used internally for `Status`); we
  avoid it on emit.
