---
sha: 8a0fc0373bb61260ad9deefee47003a3ce158914
short_sha: 8a0fc03
date: 2021-11-03
author: Michael Davidsaver
category: lifecycle
severity: high
rust_verdict: applies
audit_targets:
  - crate: base-rs
    file: src/server/database/db_access.rs
    function: db_put_field_link
tags: [error-propagation, dbChannelOpen, status-code, dbAccess, lifecycle]
---

# dbPutFieldLink: propagate dbChannelOpen() error status correctly

## Root Cause
In `dbPutFieldLink()`, the call to `dbChannelOpen(chan)` was made without
capturing its return value into `status`:

```c
if (chan && dbChannelOpen(chan) != 0) {
    errlogPrintf("...");
    goto cleanup;
}
```

The `status` variable was left at its initial value (0 = success) even when
`dbChannelOpen` failed. The `goto cleanup` path would therefore proceed with
`status == 0`, causing the caller of `dbPutFieldLink()` to believe the
operation succeeded when it had actually failed. This could silently configure
a broken link pointing to a non-existent or unopenable channel.

Additionally, the error message did not include the actual error code, making
diagnosis harder.

## Symptoms
- `dbPutFieldLink()` returns `S_db_Ok` (success) after `dbChannelOpen()` failure.
- Callers of `dbPutFieldLink` (e.g., during record link initialization) silently
  proceed with an invalid channel, leading to confusing downstream errors or
  silent misbehavior.
- Error log message did not show the error code.

## Fix
Changed to:
```c
if (chan && (status = dbChannelOpen(chan)) != 0) {
    errlogPrintf(ERL_ERROR ": dbPutFieldLink ... failed w/ 0x%lx\n", ..., status);
    goto cleanup;
}
```

Now `status` is set to the actual `dbChannelOpen` return value, so `cleanup:`
returns the correct non-zero error code to the caller.

## Rust Applicability
In Rust, `?` operator propagates errors naturally — forgetting to capture a
return value is impossible if the function returns `Result`. In `base-rs`,
`db_put_field_link` should return `Result<(), EpicsError>` and use `?` on
`db_channel_open()`. This pattern is fully eliminated by Rust's `?` and
`Result` type.

However: audit any `base-rs` code that calls a function returning `Result` or
`Option` and discards the value (e.g., `let _ = foo()` or calling a function
without binding the result).

## Audit Recommendation
- In `base-rs/src/server/database/db_access.rs`: verify `db_put_field_link`
  returns `Result` and uses `?` on `db_channel_open()`.
- Grep for `let _ =` on fallible calls in `db_access.rs`.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPutFieldLink` — `status = dbChannelOpen(chan)` fix
