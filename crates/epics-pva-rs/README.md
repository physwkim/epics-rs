# epics-pva-rs

pvAccess protocol for [epics-rs](https://github.com/epics-rs/epics-rs).

**Status: Experimental**

- **Client** — PvaClient (get, put, monitor, info)
- **Codec** — pvAccess header and pvData serialization
- **CLI tools** — pvaget-rs, pvaput-rs, pvamonitor-rs, pvainfo-rs

## Usage

```toml
[dependencies]
epics-rs = { git = "https://github.com/epics-rs/epics-rs", features = ["pva"] }
```

## License

MIT
