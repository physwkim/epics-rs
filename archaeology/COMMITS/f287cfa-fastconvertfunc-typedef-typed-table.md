---
sha: f287cfa2ac619e166ea7dd6c2bbd0e746fb7deaa
short_sha: f287cfa
date: 2023-12-24
author: Andrew Johnson
category: type-system
severity: high
rust_verdict: eliminated
audit_targets: []
tags: [function-pointer-typedef, FASTCONVERTFUNC, UB, type-safety, LINKCVT]
---

# Introduce FASTCONVERTFUNC to Replace Untyped long (*)() Tables

## Root Cause
`dbFastGetConvertRoutine` and `dbFastPutConvertRoutine` were declared as
`long (*)()` (empty-prototype function pointers) rather than having the full
argument types. Each call site that retrieved a function pointer from these
tables and called it was doing so through an incompatible type, which is
undefined behavior per the C standard. Additionally, `LINKCVT` in `link.h`
was defined as `long (*)()` — same untyped form — used for `pv_link.getCvt`.

## Symptoms
Any call through `dbFastGetConvertRoutine[from][to]` was technically UB.
In `lnkCalc.c`, `lnkConst.c`, `lnkState.c`, and `jlinkz.c`, local typedefs
`FASTCONVERT` aliased `long (*)()` — inconsistent with the actual signature
`long (const void *, void *, const dbAddr *)`. Passing typed pointers through
the untyped call could corrupt conversion results.

## Fix
- Defined `FASTCONVERTFUNC` as `long (*)(const void *, void *, const struct dbAddr *)` in `link.h`.
- Replaced all `long (*)()` declarations with `FASTCONVERTFUNC`.
- Removed local `FASTCONVERT` typedefs from lnkCalc, lnkConst, lnkState.
- Fixed `jlinkz.c` to call `pconv(&priv->value, pbuffer, NULL)` with correct argument order.

## Rust Applicability
Eliminated. Rust function pointer types always carry full signatures. The
equivalent Rust code uses `fn(from: &dyn Any, to: &mut dyn Any, addr: &DbAddr) -> Result<()>` or trait objects.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbConvertFast.h` — tables declared as `long (*)()`
- `modules/database/src/ioc/dbStatic/link.h:pv_link.getCvt` — `LINKCVT` was `long (*)()`
- `modules/database/src/std/link/lnkCalc.c` — local `FASTCONVERT` typedef
- `modules/database/src/std/link/lnkConst.c` — local `FASTCONVERT` typedef
- `modules/database/src/std/link/lnkState.c` — local `FASTCONVERT` typedef
- `modules/database/test/ioc/db/jlinkz.c:z_putval` — argument order reversed in call
