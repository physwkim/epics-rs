---
sha: 2e4113b63bb60e4952eb0fcc97adfb8349b2be3e
short_sha: 2e4113b
date: 2023-08-25
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [dbEvent, typed-opaque, USE_TYPED_DBEVENT, dbEventCtx, dbEventSubscription]
---
# dbEvent.h opaque void* types lack type safety without USE_TYPED_DBEVENT

## Root Cause
`dbEventCtx` and `dbEventSubscription` were both `typedef void *`, making
it trivially possible to pass a subscription where a context is expected
and vice versa — C offers no compile-time error.  The internal
implementation (`dbEvent.c`) uses concrete structs `dbEventContext` and
`evSubscrip` but hides them behind the opaque typedefs in the public header,
so API callers get no safety.

## Symptoms
No runtime bug in normal use.  A caller that accidentally passes a
`dbEventSubscription` where a `dbEventCtx` is expected compiles and runs
but accesses the wrong fields, leading to undefined behavior at runtime.

## Fix
Under the compile-time flag `USE_TYPED_DBEVENT`, forward-declare
`struct dbEventContext` and alias `dbEventCtx = struct dbEventContext*`,
and alias `dbEventSubscription = struct evSubscrip*`.  The internal
implementation defines `USE_TYPED_DBEVENT` itself so it sees the typed
versions.  External callers can opt in.  Without the flag, the existing
`void *` typedefs remain for backward compatibility.

## Rust Applicability
Eliminated.  Rust's type system provides zero-cost opaque handles via
newtype wrappers or `NonNull<T>` with private visibility; misuse is a
compile-time error.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbEvent.h` — added typed aliases under USE_TYPED_DBEVENT guard
- `modules/database/src/ioc/db/dbEvent.c` — defined USE_TYPED_DBEVENT to use typed handles internally
