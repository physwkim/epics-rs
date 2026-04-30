---
sha: 08b7dd12083dec4e8640cb2e873812ffb1b4814c
short_sha: 08b7dd1
date: 2021-01-15
author: Heinz Junkes
category: lifecycle
severity: low
rust_verdict: eliminated
audit_targets: []
tags: [RTEMS, DHCP, callback, global-variable, static-linkage]
---
# RTEMS: epicsEventId finished not static in callback tests; DHCP cmdline option 129 addition

## Root Cause
Two distinct issues in this commit:

1. In `callbackTest.c` and `callbackParallelTest.c`, the `epicsEventId
   finished` global variable was declared without `static`, making it a
   visible external symbol. This created a potential link-time symbol
   collision if both test objects were linked together or if another
   translation unit had a same-named global.

2. RTEMS `rtems_init.c` DHCP configuration for `dhcpcd` was missing the
   DHCP option 129 (`define 129 string rtems_cmdline`) needed to retrieve the
   boot command line on qoriq/e500 hardware. Without this, RTEMS would boot
   without a startup script path, preventing IOC initialization.

Additional miscellaneous RTEMS changes: duplicate `rtems/telnetd.h` include
removed, `__attribute__((unused))` placement corrected for C standard
compliance, RTEMS legacy stack filesystem initialization guarded.

## Symptoms
1. Possible link error or symbol shadowing when both callback test binaries
   are linked in the same test executable.
2. RTEMS IOC on qoriq hardware would fail to retrieve boot script path via
   DHCP, leaving IOC stuck at startup.

## Fix
1. Added `static` to `epicsEventId finished` in both test files.
2. Added `"define 129 string rtems_cmdline\n"` to the `dhcpcd` config string.
3. Various minor RTEMS cleanup (duplicate include, attribute placement, etc.).

## Rust Applicability
RTEMS-specific. Rust on bare-metal RTEMS is not a supported epics-rs target.
Eliminated.

## Audit Recommendation
No audit needed. RTEMS BSP configuration and test linkage issues with no
Rust analog.

## C Locations
- `modules/database/test/ioc/db/callbackTest.c:finished` — missing static on global epicsEventId
- `modules/database/test/ioc/db/callbackParallelTest.c:finished` — missing static on global epicsEventId
- `modules/libcom/RTEMS/posix/rtems_init.c:default_network_dhcpcd` — adds DHCP option 129
