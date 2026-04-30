---
sha: 918a188285f554f99f02bb8feccc36604fc41e51
short_sha: 918a188
date: 2023-12-24
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [typed-pointer, struct-tag, drvet, driver-support, miscast]
---

# Typed Driver Pointer: struct drvet vs drvet Prevents Miscast

## Root Cause
In C, `struct drvet` and a `typedef drvet` can diverge if the typedef is not
consistently applied. Without `USE_TYPED_DRVET`, code used `struct drvet *`
(the struct-tag form) in some places and a future typedef in others. The
inconsistency could allow passing a `void *` or wrong struct pointer to
driver support functions without a compile-time error, since the compiler
may accept implicit `struct drvet *` ↔ `void *` casts in C.

## Symptoms
Potential for passing the wrong pointer type to `registryDriverSupportAdd`
or `registryDriverSupportFind`, leading to incorrect driver dispatch at
runtime if the struct layout differs from what callers expected.

## Fix
Defined `USE_TYPED_DRVET` and unified all declarations to use `drvet *` (the
typedef) instead of `struct drvet *`, enabling stronger type checking by the
compiler. Also added the `drvSup.h` include where missing.

## Rust Applicability
Eliminated. Rust's trait objects and strong typing make it impossible to
confuse driver support pointers with other types. The base-rs driver registry
uses a concrete `Arc<dyn DriverSupport>` trait object.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/dbStatic/dbBase.h:drvSup` — `pdrvet` field was `struct drvet *`
- `modules/database/src/ioc/registry/registryDriverSupport.h` — prototypes used `struct drvet *`
- `modules/database/src/ioc/registry/registryDriverSupport.c` — implementation used `struct drvet *`
- `modules/database/src/ioc/registry/registryCommon.c:registerDrivers` — parameter was `struct drvet * const *`
