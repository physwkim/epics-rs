# 01 — Overview

`epics-pva-rs` is a pure-Rust implementation of the EPICS pvAccess
(PVA) protocol — both the client and the server side — with a public
API designed to mirror the C++ reference implementation
[pvxs](https://github.com/mdavidsaver/pvxs).

There is **no FFI** to libpvAccess / libpvData / pvxs. Every wire
byte is produced and consumed by Rust code in this crate. The
upstream code is consulted as a specification, never linked.

## Module map

```text
crates/epics-pva-rs/src/
├── lib.rs                 ── public re-exports
├── error.rs               ── PvaError / PvaResult
├── proto/                 ── wire-protocol primitives
│   ├── header.rs          ── PvaHeader (magic, flags, command, payload size)
│   ├── command.rs         ── Command / ControlCommand enums
│   ├── buffer.rs          ── put_u8 / get_u8 / put_u32 / ... ByteOrder
│   ├── size.rs            ── pvxs Size encoding (1 / 5 byte / null)
│   ├── string.rs          ── nullable string codec
│   ├── status.rs          ── Status (OK / WARN / ERROR / FATAL) + message
│   ├── bitset.rs          ── BitSet (LSB-first, trailing-zero trimmed)
│   ├── ip.rs              ── 16-byte IPv4-in-IPv6 wire form
│   └── selector.rs        ── union selector helpers
├── pvdata/                ── pvData type system + value codec
│   ├── field.rs           ── FieldDesc tree (Scalar / Array / Struct / Union / Variant)
│   ├── value.rs           ── Value (mutable, marked-fields aware)
│   ├── structure.rs       ── PvField / PvStructure / Variant
│   ├── scalar.rs          ── ScalarType / ScalarValue
│   └── encode.rs          ── encode/decode FieldDesc + PvField, type cache 0xFD/0xFE
├── pv_request.rs          ── pvRequest -> BitSet mask
├── codec.rs               ── high-level frame builders for each Command
├── client_native/         ── pure-Rust client
│   ├── server_conn.rs     ── persistent TCP virtual circuit (reader/writer/heartbeat)
│   ├── search_engine.rs   ── UDP search + beacon listener + reconnect
│   ├── channel.rs         ── per-PV state machine + connection pool
│   ├── ops_v2.rs          ── GET / PUT / MONITOR / RPC with auto-reconnect
│   ├── beacon_throttle.rs ── beacon dedup / GUID tracker
│   ├── decode.rs          ── server frame parser
│   └── context.rs         ── public PvaClient + PvaClientBuilder
├── server_native/         ── pure-Rust server
│   ├── runtime.rs         ── PvaServer / PvaServerConfig + start / run / wait
│   ├── tcp.rs             ── per-connection state machine, op dispatch
│   ├── udp.rs             ── search responder + beacon emitter
│   ├── shared_pv.rs       ── SharedPV / SharedSource (mailbox PVs)
│   ├── source.rs          ── ChannelSource / DynSource trait
│   └── composite.rs       ── multi-source registry with priority
├── server/                ── high-level server with PvDatabase wiring
│   ├── pva_server.rs      ── PvaServer + PvaServerBuilder (record-aware)
│   └── native_source.rs   ── PvDatabaseSource — bridges record DB to ChannelSource
├── nt/                    ── Normative Type helpers (NTScalar, NTTable, NTURI, ...)
├── auth/                  ── Connection validation + ca-auth + x509 mTLS
├── format/                ── pretty-print PvField for tools
├── log/                   ── logger handle + reload bridge (pvxs `logger_*` parity)
├── config/                ── env-var parser
└── bin/                   ── pvget-rs / pvput-rs / pvinfo-rs / pvmonitor-rs / pvcall / pvlist /
                              pvxvct / mshim
```

## Layered model

```text
   public API: PvaClient / PvaServer / SharedPV
        │
        ▼
   client_native / server_native — async runtimes
        │
        ▼
   codec — frame builders / parsers per Command
        │
        ▼
   pvdata — FieldDesc + PvField + BitSet + type cache
        │
        ▼
   proto — bytes on the wire (header, Size, String, Status, ...)
```

## Upstream parity model

`pva-rs` follows pvxs at the API level. Every public method on
`PvaClient` / `PvaServer` / `SharedPV` corresponds to a method on the
matching pvxs class. See [`09-pvxs-parity.md`](09-pvxs-parity.md) for
the per-method matrix.

The upstream legacy gateway `pva2pva/p2pApp` is mirrored by
`epics-bridge-rs/src/pva_gateway` — see [`11-gateway.md`](11-gateway.md).

## Where to look first

- **Wire question** ("what bytes does the server emit?") →
  `proto/header.rs` + `proto/buffer.rs` + the relevant `Command` arm
  in `server_native/tcp.rs` `handle_op`.
- **Type question** ("how is NTScalar encoded?") → `pvdata/encode.rs`
  + `nt/scalar.rs`.
- **Lifecycle question** ("when does the channel reconnect?") →
  `client_native/channel.rs` `ChannelState` + `06-state-machines.md`.
- **Performance question** → `06-state-machines.md` flow control
  section + `server_native/shared_pv.rs` watermarks.

## Stability of the surface

The crate is `0.x` — the API can move. Breaking changes are
documented in `CHANGELOG.md`. The wire format is the EPICS pvAccess
v1 (TLS variant pvas v0) protocol; we will not change wire bytes.
