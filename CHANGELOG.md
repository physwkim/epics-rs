# Changelog

## v0.7.3

### New Crates
- **std-rs**: Port of EPICS std module — epid (PID/MaxMin feedback), throttle (rate-limited output), timestamp (formatted time strings) records, plus device support (Soft/Async/Fast Epid, Time of Day, Sec Past Epoch) and SNL programs (femto gain control, delayDo state machine)
- **scaler-rs**: Port of EPICS scaler module — 64-channel 32-bit counter record with preset-based counting, OneShot/AutoCount modes, DLY/DLY1 delayed start, RATE periodic display update, asyn device support, and software scaler driver

### Framework: ProcessOutcome / ProcessAction
- **Breaking**: `Record::process()` now returns `CaResult<ProcessOutcome>` instead of `CaResult<RecordProcessResult>`
- `ProcessOutcome` contains `result` (Complete/AsyncPending) + `actions` (side-effect requests)
- `ProcessAction::WriteDbLink` — record requests a DB link write without direct DB access
- `ProcessAction::ReadDbLink` — record requests a DB link read (pre-process execution)
- `ProcessAction::ReprocessAfter(Duration)` — delayed self re-process (replaces C `callbackRequestDelayed` + `scanOnce`)
- `ProcessAction::DeviceCommand` — record sends named commands to device support via `handle_command()`
- Processing layer executes actions at the correct point in the cycle (ReadDbLink before process, WriteDbLink/DeviceCommand after, ReprocessAfter via tokio::spawn)

### Framework: DeviceReadOutcome
- **Breaking**: `DeviceSupport::read()` now returns `CaResult<DeviceReadOutcome>` instead of `CaResult<()>`
- `DeviceReadOutcome` carries `did_compute` flag and `actions` list
- `did_compute`: signals that device support already performed the record's compute step (e.g., PID), passed to record via `set_device_did_compute()` before `process()`
- Device support actions are merged into the record's ProcessOutcome by the framework

### Framework: Other Improvements
- `Record::pre_process_actions()` — return ReadDbLink actions executed BEFORE process() (matches C `dbGetLink` immediate semantics)
- `Record::put_field_internal()` — bypasses read-only checks for framework-internal writes
- `Record::set_device_did_compute()` — framework signals device support compute status
- `DeviceSupport::handle_command()` — handle named commands from ProcessAction::DeviceCommand
- `field_io.rs`: `put_pv()` and `put_record_field_from_ca()` now call `on_put()` + `special()` for record-owned fields (was previously only for common fields)
- ReprocessAfter timer cancellation via generation counter in RecordInstance (prevents stale timer accumulation)

### Workspace Integration
- Add `std-rs` and `scaler-rs` to workspace members and default-members
- Add `std` and `scaler` feature flags to epics-rs umbrella crate
- Bundle 70+ database templates (.db) and autosave request files (.req)

### Testing
- Add 390+ new tests across all crates:
  - std-rs: 94 tests (epid PID algorithm, throttle rate limiting, timestamp formats, SNL state machines, framework integration, e2e autosave)
  - scaler-rs: 40 tests (64-channel field access, state machine, TP↔PR1 conversion, soft driver, DLY delayed start, COUT/COUTP link firing)
  - asyn-rs: 20 integration tests (port driver parameters, octet echo, error handling, interrupt callbacks, enum, blocking API)
  - ad-core-rs: 47 tests (NDArray types/dimensions, pool allocation/reuse/memory limits, attributes, concurrent access)
  - epics-macros-rs: 27 tests (derive macro field generation, type mapping, read-only, snake_case conversion)
  - epics-ca-rs: 30 tests (protocol header encoding, server builder, get/put API, field access, multiple record types)
  - epics-pva-rs: 49 tests (scalar types, PvStructure, serialization roundtrip, protocol header, codec)
  - epics-seq-rs: 30 tests (event flags, channel store, program builder, variable traits)
  - snc-core-rs: 42 tests (lexer tokenization, parser AST, codegen output, end-to-end pipeline)
  - snc-rs: 11 tests (CLI help, compilation, error handling, debug flags)

## v0.7.2

- Fix asyn-rs epics feature compilation (get_port export, AsynRecord import)
- Migrate record factory registration from global registry to IocApplication injection
- Replace global port registry with shared PortRegistry instance
- Add feature matrix to CI (asyn-rs/epics, ad-core-rs/ioc, ad-plugins-rs/ioc)
- Add IocApplication::register_record_type() method
- Add motor_record_factory() and asyn_record_factory() returning injectable tuples

## v0.7.1

### Architecture
- Extract `IocBuilder` from `CaServerBuilder` into epics-base-rs (protocol-agnostic IOC bootstrap)
- Move `IocApplication` to epics-base-rs with pluggable protocol runner closure
- Split `database.rs` into modules: field_io, processing, links, scan_index
- Split `record.rs` into modules: alarm, scan, link, common_fields, record_trait, record_instance
- Split `types.rs` into modules: value, dbr, codec
- Split `db_loader.rs` into parser + include expander modules
- Split `asyn_record.rs` registry into separate module
- Extract motor field dispatch to `field_access.rs`
- Remove thin wrapper crates (autosave-rs, busy-rs, epics-calc-rs) — now re-exported from epics-base-rs
- Remove legacy autosave API, migrate to SaveSetConfig/AutosaveManager
- Remove unused calc feature flags
- Crate directory names now match crate names (crates/motor → crates/motor-rs, etc.)

### API
- Reduce public API surface: 7 internal modules → pub(crate) (recgbl, scan_event, exception, interpose, protocol, transport, channel)
- Motor lib.rs: fields, coordinate → pub(crate); remove pub use fields::*, flags::*
- Add `create_record_with_factories()` for dependency injection (avoids global registry)
- `IocApplication::run()` now accepts a protocol runner: `.run(run_ca_ioc).await`

### Testing
- Move large inline test blocks to tests/ directory (3,337 lines)
- Add autosave integration test with mini-beamline (save + restore on restart)

### Fixes
- Fix ad-core path references after directory rename
- Fix remaining old crate directory references in README and examples
- Clean all clippy warnings

## v0.7.0

- **Breaking**: Separate Channel Access into `epics-ca-rs` crate
- **Breaking**: Separate pvAccess into `epics-pva-rs` crate
- **Breaking**: Rename crates for consistent `-rs` suffix (ad-core-rs, ad-plugins-rs, epics-macros-rs, epics-seq-rs, snc-core-rs, snc-rs)
- Add `epics-rs` umbrella crate with feature flags (ca, pva, motor, ad, calc, full, etc.)
- Remove msi from workspace (moved to separate repo)
- Add 113 C EPICS parity tests (ai/bi/bo record, deadband, alarm, calc engine, FLNK chains, CA wire protocol, .db parsing, autosave)
- Add SAFETY comments for production unwrap sites
- Clippy lint cleanup across all crates

## v0.6.1

- Fix monitor deadband for records without MDEL field
- Reset beacon interval on TCP connect/disconnect (C EPICS parity)
- Fix caput-rs to use fire-and-forget write like C caput, add `-c` flag for callback mode
- Show Old/New values in caput-rs output
- Support multiple PV names in CA/PVA CLI tools (caget, camonitor, cainfo, pvaget, etc.)
- Add per-field change detection for monitor notifications
- Add DMOV same-position transition tests
- Poll motor immediately on StartPolling for faster DMOV response
- Add motor tests ported from ophyd (sequential moves, calibration, RBV updates, homing)
- Update minimum Rust version to 1.85+ for edition 2024

## v0.6.0

- Deferred write_notify via callback for motor records
- Motor display/ctrl metadata support
- SET mode RBV updates

## v0.5.2

- Fix monitor notify, DMOV transition, timestamp, and IPv4 resolution

## v0.5.1

- Add DMOV 1->0->1 monitor transition for motor moves

## v0.5.0

- Fix motor record process chain, client error handling, and connection speed
- Add ophyd-test-ioc example

## v0.4.6

- Add client-side DBR_TIME/CTRL decode and get_with_metadata() API

## v0.4.5

- Upgrade Rust edition 2021 -> 2024

## v0.4.4

- Bug fixes

## v0.4.3

- Add generalTime framework for priority-based time providers
- Add random-signals example
- Add GitHub Actions CI workflow

## v0.4.2

- Implement C-compatible autosave iocsh commands and request file infrastructure

## v0.4.1

- Implement full YUV color mode support and refactor color convert plugin

## v0.4.0

- Initial crates.io publish
- Move to epics-rs GitHub organization

## v0.3.0

- Unify workspace version management
