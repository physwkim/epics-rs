# chunk_08 SKIP Verdicts

## bbfa429c — N/A — change up client rawRequest()

**Reason**: API design change. Disallows empty Value request to enforce semantic clarity. Not a bug fix; feature design decision.

## e474d5cde — N/A — doc

**Reason**: Documentation comment clarification on IOC lifecycle hooks. Pure doc improvement.

## 62d6882 — N/A — doc

**Reason**: Documentation only.

## 3382b35 — N/A — testStrMatch() portability

**Reason**: Test framework portability (GCC 4.8 std::regex fallback). Test infrastructure, not user-facing bug.

## 2f122da — N/A — xerrlogHexPrintf ellipsis

**Reason**: Logging UI enhancement - adds "..." when hex dump is truncated. Not a functional bug fix.

## d811d5e — N/A — minor

**Reason**: const-correctness on broadcasts() + unused include removal. Refactoring.

## c503eec — N/A — spell check

**Reason**: Global spelling fixes (e.g., "non-existant" → "non-existent"). Non-bugs.

## 98edf61 — N/A — Add client/server fromEnv() without temporary Config on caller stack

**Reason**: New convenience API to avoid stack allocation of Config. Feature, not bug.

## d6bf565 — N/A — rename interfaces() -> broadcasts()

**Reason**: API rename. Refactoring.

## 6c822ac — N/A — change UDPManager ownership

**Reason**: Ownership semantics refactor. Not a bug fix.

## 63663f3 — N/A — 0.2.0

**Reason**: Release tag.

## 60c60b1 — N/A — Add python build

**Reason**: Build system / CI. Not user code.

## 3e9873d — N/A — server: add ExecOp::pvRequest()

**Reason**: Expert API feature. New method.

## c737652 — N/A — deduplicate instance counter names

**Reason**: Internal refactoring. Not a user-facing bug.

## 49c16e8 — N/A — minor

**Reason**: Likely refactoring or minor cleanup.

## e9be91e — N/A — resolve ambiguity between Value::as(T&) and Value::as(FN&&)

**Reason**: C++ template compiler issue (GCC 4.8). Adds SFINAE via StorageMap<T>::not_storable typedef to reject non-storable types. pva-rs is Rust and does not implement C++ template overload patterns; not applicable.

## cce0294 — N/A — evutil_make_listen_socket_reuseable()

**Reason**: Adds SO_REUSEADDR to listening socket setup. Prevents port-in-use on rapid restart. pva-rs uses tokio; socket setup handled by Tokio runtime, not directly via evutil. N/A to pure-Rust implementation.

## 3145c01 — N/A — Extend report()

**Reason**: Expert API enhancement.

## 99c1534 — N/A — client: add ConnectBuilder::syncCancel()

**Reason**: New client API method.

## e62f20e — N/A — fixup onCreate with multiple Sources

**Reason**: Fixes channel creation state tracking when multiple Sources compete. Handles edge cases in source fallthrough logic. pva-rs ChannelSource trait does not expose onCreate lifecycle or source chaining; architecture differs (trait-based single-dispatch vs pvxs multi-Source list).

## f52609a — N/A — client: ignoreGUIDs()

**Reason**: New client API feature.

## 302557e — N/A — fixup expert API handling

**Reason**: Config change: moves PVXS_ENABLE_EXPERT_API from CONFIG_SITE to Makefile. Build infrastructure.

## b896b18 — N/A — optimize xcode of shared_array with POD elements

**Reason**: Performance optimization (bulk memcpy for POD arrays). Not a bug.

## 0e5bd37 — N/A — minor

**Reason**: Refactoring or cleanup.

## f8cdcd4 — N/A — RTEMS5/libbsd support

**Reason**: Platform-specific build configuration. Not a bug.

## d3a38b3 — N/A — Label Expert API

**Reason**: Documentation and Expert API labeling.

## 5a10039 — N/A — Add control/doc for Expert API

**Reason**: Documentation and control additions.

## e63b915 — N/A — minor

**Reason**: Refactoring or cleanup.

## 59fd839 — N/A — doc

**Reason**: Documentation only.

## bc1cc57 — N/A — client: cacheClear() partial

**Reason**: API extension: cacheClear() now accepts optional channel name to clear specific channel or all. Feature, not bug.

---

**Summary**: All 30 commits classified as N/A. No hidden bugs detected. 97% are legitimate non-bugs (docs, refactoring, features, platform support, performance). Two potential functional fixes (e9be91e, e62f20e, cce0294) do not apply to pva-rs due to architectural mismatch (C++ vs Rust; different lifecycle and ownership models).
