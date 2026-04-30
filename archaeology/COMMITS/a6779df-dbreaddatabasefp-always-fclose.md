---
sha: a6779df21ce838c90c19d4c04aff5dd5972dece4
short_sha: a6779df
date: 2022-03-03
author: Michael Davidsaver
category: leak
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [file-descriptor-leak, fclose, early-return, goto-cleanup, dbStatic]
---

# dbReadDatabaseFP() always fclose() the provided FILE pointer

## Root Cause
`dbReadCOM()` — the internal implementation called by `dbReadDatabaseFP()` —
had an early-return path that bypassed the `cleanup:` label where `fclose()`
would be called. Specifically, the check:

```c
if (getIocState() != iocVoid)
    return -2;
```

returned directly without closing the caller-provided `FILE*`. Additionally,
even in the normal success path, after the file pointer was transferred into
`pinputFile->fp`, the original `fp` local variable was not set to `NULL`, so
the cleanup block could attempt a double-close.

The API contract (documented in the fix) is: `dbReadDatabaseFP()` **always**
closes the provided `fp`, regardless of success or failure.

## Symptoms
- File descriptor leak whenever `dbReadDatabaseFP()` was called while the IOC
  was not in `iocVoid` state (e.g., a second call after `iocInit()`).
- Potential double-close of a `FILE*` in some code paths if the cleanup code
  ran after `fp` was transferred to `pinputFile`.

## Fix
- Changed the early `return -2` to `status = -2; goto cleanup;` so cleanup
  always runs.
- After transferring `fp` to `pinputFile->fp`, set `fp = NULL` so the cleanup
  block's `if(fp) fclose(fp)` does not attempt a double-close.
- Added `if(fp) fclose(fp)` to the cleanup block.
- Added docstring clarifying the always-close contract to `dbStaticLib.h`.

## Rust Applicability
In Rust, `std::fs::File` closes automatically on drop via `Drop`. There is no
equivalent of forgetting to call `fclose()`. A `BufReader<File>` passed into a
parsing function is always closed when it goes out of scope. This pattern is
fully eliminated by Rust's ownership model.

## Audit Recommendation
None — eliminated. But if `base-rs` wraps any C FFI calls to
`dbReadDatabaseFP()`, verify the Rust wrapper does not hold a raw `FILE*` that
could leak.

## C Locations
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbReadCOM` — goto cleanup + NULL assignment
- `modules/database/src/ioc/dbStatic/dbStaticLib.h:dbReadDatabaseFP` — always-close contract doc
