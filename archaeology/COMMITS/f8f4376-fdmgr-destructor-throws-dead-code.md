---
sha: f8f43765945b7e89457dbdb14a1c072449484109
short_sha: f8f4376
date: 2023-06-13
author: Michael Davidsaver
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [destructor, exception, cpp, fdmgr, double-delete]
---
# fdRegForOldFdmgr Destructor Throws Unreachable Exception

## Root Cause
`fdRegForOldFdmgr::~fdRegForOldFdmgr()` contained a guard that called
`throwWithLocation(doubleDelete())` if the callback function pointer was
NULL (indicating a potential double-delete). However, in C++ it is
undefined behavior (and silently ignored by the runtime in practice) to
throw an exception out of a destructor. The `throwWithLocation` call would
at best terminate the process via `std::terminate`, and at worst produce
undefined behavior depending on the exception handling model.

The `doubleDelete` exception class was declared but could never be safely
caught in a destructor context, making the guard dead code with false
safety guarantees.

## Symptoms
- Double-deletion of `fdRegForOldFdmgr` objects would invoke `std::terminate`
  rather than propagating a catchable exception.
- Dead exception classes `doubleDelete` in both `fdRegForOldFdmgr` and
  `timerForOldFdmgr` inflated the API surface without providing any benefit.

## Fix
Removed the `throw`-in-destructor guard entirely; both `doubleDelete`
exception class declarations removed. Destructor is now empty. The old code
was effectively dead code since C++11 made destructors implicitly `noexcept`.

## Rust Applicability
`eliminated` — Rust destructors (`Drop::drop`) cannot panic-propagate (a
panic in `drop` triggers a double-panic abort). The idiom of "detect double
free in destructor and throw" does not exist in Rust. The `fdmgr` subsystem
is replaced by tokio's reactor in epics-rs.

## Audit Recommendation
No Rust audit needed.

## C Locations
- `modules/libcom/src/fdmgr/fdmgr.cpp:fdRegForOldFdmgr::~fdRegForOldFdmgr` — throws in destructor (UB)
- `modules/libcom/src/fdmgr/fdmgr.cpp:timerForOldFdmgr` — dead `doubleDelete` exception class
