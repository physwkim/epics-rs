# SKIP Commits Analysis — chunk_01

## 1d481d5 — N/A — doc: show how to create arrays of NTScalar
**Reason**: Documentation only; no behavior change.

## 484960a — N/A — test helpers use posix regex on RTEMS
**Reason**: RTEMS-specific test build workaround; no behavioral bug.

## 3a777e1 — N/A — shared_array: mark additional methods noexcept
**Reason**: C++ only: noexcept specifier annotations; pva-rs has no equivalent.

## 5ee956d — N/A — default for Source::List::dynamic
**Reason**: C++ only: struct default initialization; pvxs-only SharedPV feature not in pva-rs.

## 1d182cb — N/A — test iocsh functions
**Reason**: Test infrastructure; no behavior change in core PVA logic.

## ef7ef0f — N/A — ioc: promote DBE_ALARM only to also fetch value
**Reason**: EPICS IOC-specific DBE flag handling; not in pva-rs (which is just PVA layer).

## 55a8faf — N/A — Clarify SharedPV::post()
**Reason**: Documentation clarification; no behavior change.

## bf51e44 — N/A — doc: explain type requirements for post()
**Reason**: Example code comment expansion; no behavior change.

## a6f75c1 — N/A — doc: client operation exceptions
**Reason**: API documentation; no behavior change.

## 98737e2 — N/A — doc: unittest.h
**Reason**: Documentation; no behavior change.

## b7c2d1b — N/A — doc: shared_array
**Reason**: Documentation cleanup and examples; no behavior change.

## a351943 — N/A — minor
**Reason**: Error message improvement in debug output; not a behavioral bug fix.

## a7d77da — N/A — SO_NOSIGPIPE
**Reason**: OSX-specific socket option; pva-rs uses tokio which abstracts OS details.

## 6a53998 — N/A — rename dbpvar -> dbpvxr
**Reason**: EPICS IOC shell command rename; ioc-specific, not in pva-rs client/server logic.

## f764e00 — N/A — rename pvaLinkNWorkers -> pvxLinkNWorkers
**Reason**: EPICS IOC variable rename; ioc-specific symbol refactoring.

## cb62797 — N/A — rename lsetPVA -> lsetPVX
**Reason**: EPICS IOC linkset rename; ioc-specific, not in PVA protocol layer.

## 57b3682 — N/A — doc
**Reason**: Documentation clarification on address list format; no behavior change.

## 8fb2931 — N/A — pvxsr show libevent reactor method name
**Reason**: Diagnostic output enhancement; no behavioral change.

## 6446ab4 — N/A — pacify cppcheck
**Reason**: C++ static analysis warnings; member initialization in structs; pva-rs uses Rust defaults.

## b0b0bc8 — N/A — client: respect forcedServer on failed CREATE_CHANNEL
**Reason**: Pvxs handles forcedServer (direct-connect) CREATE_CHANNEL failure by logging error and returning rather than retrying search. Pva-rs treats all CREATE_CHANNEL failures uniformly with exponential backoff regardless of resolver mode (Search vs Direct). The pvxs behavior prevents tight retry loops on pinned servers, but pva-rs's backoff already prevents this via holdoff_until. Not a bug in pva-rs; different design choice.

## 35c7cc5 — N/A — ioc: add pvxs_log_config() and pvxs_log_reset()
**Reason**: EPICS IOC shell command additions; ioc-specific, not in PVA protocol layer.

## 4249885 — N/A — server: disable one-sided attempt to handle saturated connection
**Reason**: C++-specific libevent bufferevent watermark-based backpressure logic; pva-rs uses tokio which has its own flow control.

## cf43613 — N/A — version bump (1.4.1)
**Reason**: Release tag; no behavior change.

## e9ab67a — N/A — server: always post first update even if empty
**Reason**: pvxs server-side monitor subscription behavior; pva-rs is Rust impl with different monitor_op lifecycle (DUPLICATE candidate: round-9 commit e9ab67a listed in context).

## 27be80d — N/A — server: clientConfig() avoid mixing TCP and UDP endpoints
**Reason**: pvxs C++ server helper; pva-rs doesn't use this pattern.

## e8f33db — N/A — pva link use $EPICS_*
**Reason**: EPICS IOC integration detail; pva-rs client uses env vars natively without IOC layer.

## 78a0727 — N/A — doc
**Reason**: API docstring clarification on Operation::name() lifetime; no behavior change.
