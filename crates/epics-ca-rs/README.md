# epics-ca-rs

Channel Access protocol for [epics-rs](https://github.com/epics-rs/epics-rs).

- **Client** — CaClient, CaChannel (search, get, put, monitor)
- **Server** — TCP handler, UDP search responder, beacon emitter
- **Protocol** — CA header encoding/decoding, DBR type serialization
- **CLI tools** — caget-rs, caput-rs, camonitor-rs, cainfo-rs, softioc-rs

100% wire-compatible with C EPICS clients and servers.

## Usage

```toml
[dependencies]
epics-rs = { git = "https://github.com/epics-rs/epics-rs", features = ["ca"] }
```

`ca` is enabled by default.

## License

MIT
