# 11 — CA over TLS (design)

EPICS CA was designed in the late 1980s for trusted LANs. PV names
and values traverse the wire in plaintext; ACF rules trust the
client-supplied hostname/username with no cryptographic verification.
For modern environments — remote operations, cloud bridges, multi-site
collaborations, and sites under medical or nuclear compliance regimes
— that's increasingly untenable.

`epics-ca-rs` adds **CA over TLS** as an opt-in extension: TCP virtual
circuits are wrapped in TLS before the CA handshake; UDP search and
beacons remain plaintext (PV names are not secret, broadcast doesn't
play nicely with TLS, and treating them as public is a deliberate
design choice).

This document describes the design. The runtime API is in
[`tls.rs`](../src/tls.rs) and is gated behind the `tls` cargo feature.

## Goals

1. **Encrypt** TCP-borne CA traffic so neither values nor write
   commands are observable on the wire.
2. **Authenticate** clients via certificates (mTLS) so ACF rules can
   key on a verifiable identity instead of the spoofable
   `CA_PROTO_HOST_NAME` message.
3. **Coexist** with plaintext peers — a TLS-enabled IOC must still
   answer non-TLS clients (and vice versa) on a separate port.
4. **No protocol changes** — once the TLS handshake completes, the
   CA wire format is identical. libca-aware tools that link our TLS
   stack should work unchanged.

## Non-goals

- Encrypting UDP (search, beacons). PV names are not secret; covering
  them with DTLS adds complexity without clear value. Beacons over
  multicast/broadcast plus TLS is impractical.
- Replacing `CA_PROTO_HOST_NAME`. We retain it for legacy clients.
  When mTLS is in effect, the cert-derived identity takes precedence
  for ACF matching.

## Wire-level model

```
┌────────────┐                ┌──────────────────┐
│   Client   │                │      Server      │
└─────┬──────┘                └────────┬─────────┘
      │                                │
      │ TCP connect to TLS port (e.g. 5076) │
      ├───────────────────────────────▶│
      │                                │
      │ TLS handshake (rustls)         │
      ├══════════════════════════════▶│
      │ ◀═════════════════════════════│
      │ session keys established       │
      │                                │
      │ CA_PROTO_VERSION + HOST + ... (encrypted)│
      ├───────────────────────────────▶│
      │ ◀──────────────────────────────│
      │                                │
      │ ... normal CA traffic, all     │
      │     bytes inside TLS record    │
      │ ◀────────────────────────────▶│
```

There is **no** TLS negotiation inside CA — the choice is made by
which port the client connects to.

## Port allocation

| Port | Service |
|------|---------|
| 5064 | CA TCP plaintext (default) |
| 5076 | CA TCP TLS (proposed default; configurable via `EPICS_CA_TLS_PORT`) |

Servers can listen on either or both. Search responses include the
port the listener is bound to, so a client connecting to a SEARCH
reply pointing at port 5076 must use TLS; a reply pointing at 5064
must use plaintext.

(Open question: should SEARCH replies indicate "TLS available on
port X" out-of-band? Current CA wire format has no flag for this.
Sites running mixed plaintext/TLS would either point each PV to one
or the other, or run two IOC instances.)

## Configuration API

```rust
use epics_ca_rs::tls::{TlsConfig, load_certs, load_private_key, load_root_store};

// Server side: TLS only (server-auth, no client cert required)
let cert_chain = load_certs("server.crt")?;
let key       = load_private_key("server.key")?;
let tls       = TlsConfig::server_from_pem(cert_chain, key)?;

let server = epics_ca_rs::server::CaServer::builder()
    .pv("MOTOR:X:VAL", value)
    .with_tls(tls)
    .build().await?;

// Client side: verify the server cert against a custom CA bundle
let roots = load_root_store("site-ca.pem")?;
let tls   = TlsConfig::client_from_roots(roots);
let client = epics_ca_rs::client::CaClient::new_with_config(
    epics_ca_rs::client::CaClientConfig { tls: Some(tls), ..Default::default() }
).await?;
```

## mTLS (mutual auth)

```rust
// Server: require valid client cert + use cert CN/SAN as ACF identity
let client_ca_roots = load_root_store("operator-ca.pem")?;
let tls = TlsConfig::server_mtls_from_pem(cert_chain, key, client_ca_roots)?;

// Client: present a certificate
let tls = TlsConfig::client_mtls(roots, client_cert, client_key)?;
```

When mTLS is in effect, the server's per-client `state.hostname` is
populated from the client cert's Subject Alternative Name (or CN as
fallback), regardless of `EPICS_CAS_USE_HOST_NAMES`. ACF rules can
match on this verified identity:

```
HAG(operators) { CN=alice, CN=bob }
ASG(MOTORS) { RULE(1, WRITE) { HAG(operators) } }
```

## Architecture

### Configuration storage

Both client and server hold the TLS config behind the same enum:

```rust
pub enum TlsConfig {
    Server(Arc<rustls::ServerConfig>),
    Client(Arc<rustls::ClientConfig>),
}
```

Server-side state: `CaServer` gains an `Option<Arc<ServerConfig>>`
field set by `with_tls(...)`. Client-side: `CaClient::new_with_config`
takes a `CaClientConfig { tls: Option<TlsConfig>, ... }`.

### Stream type

The transport manager currently operates over `tokio::net::TcpStream`
directly. To carry TLS, the read/write loops switch to a generic
form:

```rust
async fn read_loop<R: AsyncRead + Unpin + Send + 'static>(reader: R, ...) {}
async fn write_loop<W: AsyncWrite + Unpin + Send + 'static>(writer: W, ...) {}
```

`connect_server` then dispatches:

```rust
let tcp = TcpStream::connect(addr).await?;
let stream = match tls_config {
    Some(c) => StreamKind::Tls(TlsConnector::from(c).connect(server_name, tcp).await?),
    None    => StreamKind::Plain(tcp),
};
```

`StreamKind` is a wrapper enum that implements AsyncRead + AsyncWrite
by dispatching to the active variant. Same pattern on the server side
in `handle_client`.

### Identity propagation (mTLS)

After a successful TLS handshake the server side calls
`stream.peer_certificates()` to extract the client cert, parses the
CN/SAN, and stores it in `ClientState::hostname`. The
`CA_PROTO_HOST_NAME` opcode is then ignored regardless of
`EPICS_CAS_USE_HOST_NAMES` (the cert-derived identity is always more
trustworthy).

## Migration path

For an existing C-based facility:

1. **Phase A** — Run `epics-ca-rs` IOCs alongside C IOCs. Both serve
   plaintext on 5064. No security change.
2. **Phase B** — Enable TLS on Rust IOCs (port 5076). Critical
   clients (operator GUIs, trusted services) connect via TLS;
   everyone else continues on 5064.
3. **Phase C** — Issue client certs to operators via the existing
   PKI. Switch to mTLS. ACF rules updated to match cert identities.
4. **Phase D** — When all important clients are TLS-capable, disable
   plaintext listener on selected IOCs.

## Status in this crate

- `tls.rs` module: ✅ complete (helpers, config types, mTLS builders)
- `CaServerBuilder::with_tls()`: ✅ stores config
- `CaClient::new_with_config(tls=...)`: ✅ accepts config
- Transport-level stream wrapping: 🚧 follow-up (touches
  `client/transport.rs::read_loop`, `client/transport.rs::write_loop`,
  `client/transport.rs::connect_server`, plus the server-side
  `tcp.rs::handle_client`)
- mTLS identity extraction: 🚧 follow-up (after stream plumbing)
- Port-based dispatch: 🚧 follow-up

The current commit lays the foundation: stable API, cert/key loading
helpers, and a documented design. Wiring the actual TLS handshake
through the read/write loops requires changing the concrete
`OwnedReadHalf` / `OwnedWriteHalf` types to be generic, which is a
contained refactor scheduled for a follow-up patch.

## Threat model

This document does not enumerate every threat — that's site policy —
but the key claims our TLS extension makes:

| Threat | Mitigation |
|--------|------------|
| Eavesdropper on the LAN reads PV values | TLS encryption on the TCP circuit |
| Eavesdropper learns which PVs exist | Not mitigated (UDP search remains plaintext) |
| Attacker writes to a PV by spoofing IP | mTLS: server requires signed client cert |
| Attacker hijacks an established TCP circuit | TLS authenticated encryption (no in-band injection) |
| Attacker MITMs the TLS handshake | rustls cert-chain validation against site root CA |
| Compromised IOC presents a cert it was issued | Out of scope (revoke and rotate via PKI) |

## Caveats

- TLS adds ~50-100 µs per request (handshake amortized over the
  connection lifetime). On low-latency control loops this is
  noticeable; reserve TLS for cross-LAN segments.
- Cert lifecycle (issuance, rotation, revocation) is outside this
  crate. Use your site's PKI and rotate certs periodically.
- The client's `EPICS_CA_NAME_SERVERS` (TCP nameserver) stream
  benefits from TLS the same way virtual circuits do — the
  follow-up patch covers both paths.
