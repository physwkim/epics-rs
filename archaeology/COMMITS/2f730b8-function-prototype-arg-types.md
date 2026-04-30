---
sha: 2f730b8e9f64b677603f7ebf026fef848a805602
short_sha: 2f730b8
date: 2023-12-24
author: Andrew Johnson
category: type-system
severity: medium
rust_verdict: eliminated
audit_targets: []
tags: [function-prototype, empty-prototype, UB, dbAccess, dbEvent, dbScan]
---

# Function Pointer Prototypes Missing Argument Types Allow UB Calls

## Root Cause
Several function pointer typedefs and local variables used the empty `()`
prototype form in C, which means "takes unspecified arguments" — not "takes
no arguments". This allows calling the function with incorrect argument types
without a compile-time error. Specific cases:
- `dbAccess.c`: `long int (*pspecial)()` — should be `(struct dbAddr *, int)`.
- `dbEvent.c`: `void (*init_func)()` — should be `(void *)`, and
  `init_func_arg` typed as `epicsThreadId` instead of `void *`.
- `dbScan.c`: `DEVSUPFUN get_ioint_info` — should be the full typed prototype
  `long (*)(int, struct dbCommon *, IOSCANPVT*)`.
- `dbYacc.y`: `static int yyerror()` — should be `(char *str)`.

## Symptoms
Calling through an unprototyped function pointer with the wrong argument types
is undefined behavior. On architectures where integer and pointer arguments are
passed differently (e.g., some 64-bit ABIs), this can corrupt arguments,
causing wrong scan lists to be used or IO interrupt registration failures.

## Fix
Added full argument types to all affected function pointer declarations and
struct members. Changed `init_func_arg` from `epicsThreadId` to `void *`.

## Rust Applicability
Eliminated. Rust function pointers always carry full type signatures; there is
no "unspecified arguments" equivalent. The Rust type system enforces correct
call signatures at compile time.

## Audit Recommendation
None required.

## C Locations
- `modules/database/src/ioc/db/dbAccess.c:dbPutSpecial` — unprototyped `pspecial` function pointer
- `modules/database/src/ioc/db/dbEvent.c:event_user` — unprototyped `init_func`, wrong `init_func_arg` type
- `modules/database/src/ioc/db/dbScan.c:scanAdd` — unprototyped `get_ioint_info`
- `modules/database/src/ioc/db/dbScan.c:scanDelete` — unprototyped `get_ioint_info`
- `modules/database/src/ioc/dbStatic/dbYacc.y:yyerror` — unprototyped declaration
