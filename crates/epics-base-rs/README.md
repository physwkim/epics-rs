# epics-base-rs

Core IOC infrastructure for [epics-rs](https://github.com/epics-rs/epics-rs).

- **Record system** — 20 record types with `#[derive(EpicsRecord)]` proc macro
- **PvDatabase** — record storage, processing chains (FLNK, INP/OUT links)
- **db loader** — `.db` file parser with macro substitution
- **iocsh** — interactive IOC shell
- **Calc engine** — numeric/string/array expressions
- **Access security** — ACF file parser
- **Autosave** — PV automatic save/restore

No wire protocol code — see `epics-ca-rs` for Channel Access, `epics-pva-rs` for pvAccess.

## Usage

```toml
[dependencies]
epics-rs = { git = "https://github.com/epics-rs/epics-rs" }
```

## License

MIT
