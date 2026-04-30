---
sha: 156b137af01576ada0c1cfd272b9943d2b282bf9
short_sha: 156b137
date: 2019-11-14
author: Michael Davidsaver
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [type-system, dset, USE_TYPED_DSET, typedef, C-struct]
---

# `struct dset` tag ambiguous with `dset` typedef when USE_TYPED_DSET defined

## Root Cause
EPICS defines `dset` either as a plain struct (`struct dset { ... }`) or as
`typed_dset` when the build flag `-DUSE_TYPED_DSET` is set. Code that used
`struct dset` as a type tag (rather than the typedef name `dset`) failed to
compile with `-DUSE_TYPED_DSET` because `typed_dset` is not tagged as
`struct dset`. Several files (`dbAccess.c`, `dbAccessDefs.h`, `dbBase.h`,
`dbTest.c`, `iocInit.c`, `registryDeviceSupport.c/h`) used `struct dset`
explicitly, breaking the typed-dset build mode.

Additionally, `dbCommon.dbd` used `struct dset *dset` as the field type for
the `DSET` field, causing a C++ name clash (`dset` is both a field name and
a type name in the same scope), addressed by introducing `unambiguous_dset`.

## Symptoms
- Compilation error with `-DUSE_TYPED_DSET`: `struct dset` used but
  `typed_dset` is not a struct with that tag.
- C++ name clash warning/error for `dbCommon::dset` field vs `dset` typedef.

## Fix
- Replace all `struct dset *` with `dset *` across affected files.
- Add `typedef dset unambiguous_dset` in `devSup.h` and use it for the
  `dbCommon.dbd` DSET field to resolve the name clash.
- Reorder includes in `dbAccessDefs.h` so `epicsExportSharedSymbols` is set
  before including `dbBase.h` / `dbAddr.h` / `recSup.h`.

## Rust Applicability
Rust has no C-style typedef/struct-tag duality. Types are unambiguously named.
There is no analog of `struct Foo` vs `typedef ... Foo` confusion. Eliminated.

## Audit Recommendation
None — eliminated by Rust's type system.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbDSETtoDevSup,dbPutFieldLink` — struct dset → dset
- `modules/database/src/ioc/db/dbAccessDefs.h` — include order fix + dset signature
- `modules/database/src/ioc/db/dbCommon.dbd:DSET` — unambiguous_dset typedef introduced
- `modules/database/src/ioc/db/dbTest.c:dbior` — struct dset → dset
- `modules/database/src/ioc/dbStatic/dbBase.h:devSup` — struct dset → dset
- `modules/database/src/ioc/misc/iocInit.c:initDevSup,finishDevSup` — struct dset → dset
- `modules/database/src/ioc/registry/registryDeviceSupport.c:registryDeviceSupportAdd,Find` — struct dset → dset
