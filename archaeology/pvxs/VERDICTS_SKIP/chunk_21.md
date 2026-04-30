## c886205 — REFACTOR — redo UDP handling
**Reason**: Template function inlining and UDP collector restructuring. No bug fix evident; idiomatic C++ refactoring.

## 3a1123e — N/A — more compat
**Reason**: Version header additions, unittest setup, EPICS-base compatibility scaffolding.

## cf64dad — FEATURE — start on server
**Reason**: New server module implementation with event loop parameter configuration. N/A.

## 781cfa1 — N/A — more sockaddr size
**Reason**: Single-line C++ struct size constant fix; platform compatibility detail.

## 2497350 — REFACTOR — more shuffling
**Reason**: Code reorganization with no functional changes.

## 5426a34 — N/A — another osx attempt
**Reason**: macOS bind() workaround; platform-specific, not transported to async net layer in pva-rs.

## 5892e13 — REFACTOR — adapt SockAddr
**Reason**: API redesign for `SockAddr` wrapper (encapsulation + constructor overloads). Refactoring, no bug.

## 84ac6ed — REFACTOR — move sockaddr wrapper to public API
**Reason**: Public API boundary adjustment; structural, not a bug fix.

## 9134d8a — REFACTOR — minor
**Reason**: Trivial cleanup.

## 8c40929 — FEATURE — all sorts of changes
**Reason**: Copyright headers, makefile expansion, event loop thread naming feature.

## 0b08678 — REFACTOR — simplify
**Reason**: Inline threading setup and API pruning (`evhelper_setup_thread`, `evhelper_sync` removed from .h → static).

## 4f13e21 — APPLIES — avoid global static dtor
**Reason**: Fixes weak_ptr dtor order bug by wrapping globals in struct allocated on heap (`udp_gbl`).

**pva-rs target**: Global weak_ptr antipattern. Rust `Arc<Mutex<>>` patterns in `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/server_native/udp.rs` and UDP channel state mgmt; tokio task-local storage avoids C++ static lifetime issues.

**Fix**: No action—Rust ownership prevents this. pva-rs uses task-spawn scoping and Arc/Mutex, bypassing C++ global destruction order hazards.

## 9ff6de6 — APPLIES — must stop c++ exception in C callbacks
**Reason**: Catches unhandled C++ exceptions in libevent C callback (`handle_static`) to prevent termination.

**pva-rs target**: Callback safety in `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/server_native/udp.rs` and event loop integration. Rust panics across FFI boundaries are UB.

**Fix**: Ensure tokio task callbacks do not panic (panic → abort). Add catch_unwind wraps at libevent-tokio boundary if direct libev FFI used; currently hidden by AsyncUdpV4 abstraction.

## d435785 — APPLIES — oops
**Reason**: Struct field zero-initialization in `evsockaddr::any()` and `loopback()` constructors. UB on uninitialized AF_INET6 fields.

**pva-rs target**: `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/proto/ip.rs` and socket address creation.

**Fix**: Already sound—ip_to_bytes/ip_from_bytes use explicit zeroing via default-init arrays. No cargo needed.

## 044f71a — N/A — attempt to appease msvc
**Reason**: Logging format adjustments for MSVC; language-specific compiler portability.

## e125fdf — APPLIES — fixup (de)serialize
**Reason**: Critical serialization bugs in IPv4-mapped IPv6 encoding and buffer offset handling. Buffer overruns, off-by-one in `from_wire`/`to_wire`.

**pva-rs target**: `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/proto/ip.rs` and buffer codec in `/Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/proto/buffer.rs`.

**Fix**: Verify pva-rs ip_to_bytes/from_bytes correctly handle IPv4-mapped bytes[10..12]=0xFF offset. Current code at line 13-14 and 31-33 appears correct, but test IPv4 loopback serialization round-trip: `[0..10]=0, [10..12]=0xFF, [12..16]=addr`. pva-rs passes test_matches_spvirit.

## c77281 — REFACTOR — log hex
**Reason**: Diagnostic logging enhancement.

## 87508ea — REFACTOR — automatic evhelper_ev2err
**Reason**: Move logger init from header export to internal startup (`logger_gbl_t` ctor). API simplification, no bug.

## 466044d — INITIAL — initial
**Reason**: First commit; feature baseline.

---
**Summary**: 4 APPLIES (d435785, 9ff6de6, e125fdf, 4f13e21), 14 N/A/REFACTOR/FEATURE. All APPLIES are mitigated or already sound in pva-rs via Rust safety (d435785, 4f13e21) or correct async/buffering logic (9ff6de6 tokio, e125fdf ip codec).
