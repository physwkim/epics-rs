---
sha: 804577075120218c2a38d457906e6a3182659e82
short_sha: 8045770
date: 2024-08-29
author: Érico Nogueira
category: other
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [std-unexpected, abort, C++17, epicsThread, exception]
---
# epicsThread uses deprecated std::unexpected removed in C++17

## Root Cause
`epicsThread::printLastChanceExceptionMessage` called `std::unexpected()`
to abort the process after an unhandled exception in a thread entry
point.  `std::unexpected()` is deprecated in C++11 and removed in C++17;
it is also semantically wrong here — `std::unexpected` is called by the
C++ runtime for dynamic-exception-spec violations, not for uncaught user
exceptions.  Compiling with `-std=c++17` or later produced a compile
error.

## Symptoms
Compile error with C++17 or later compilers.  With C++11/14 the behavior
was correct (process aborted) but the semantics were misleading.

## Fix
Replaced `std::unexpected()` with `abort()` (from `<stdlib.h>`).
`abort()` has the same process-termination effect with correct semantics.

## Rust Applicability
Eliminated.  Rust's thread model propagates panics through the thread's
`JoinHandle::join()` result; there is no equivalent of C++ dynamic
exception specifications.

## Audit Recommendation
None required.

## C Locations
- `modules/libcom/src/osi/epicsThread.cpp:epicsThread::printLastChanceExceptionMessage` — std::unexpected() → abort()
