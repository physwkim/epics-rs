# R079 — e9ecf7e8dd13 [MEDIUM][uncertain]

**Subject**: missing copyright boilerplate  
**Date**: 2023-09-05  
**pvxs SHA**: e9ecf7e8dd13  

## pva-rs mapping
- (no direct mapping found)

## Verdict
**uncertain** — MEDIUM

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit e9ecf7e8dd130b2f6e0ec7b435440f6052e69222
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Tue Sep 5 11:31:41 2023 +0200

    missing copyright boilerplate
---
 src/pvxs/netcommon.h | 5 +++++
 src/pvxs/srvcommon.h | 5 +++++
 2 files changed, 10 insertions(+)

diff --git a/src/pvxs/netcommon.h b/src/pvxs/netcommon.h
index 4492af2..d9e86b2 100644
--- a/src/pvxs/netcommon.h
+++ b/src/pvxs/netcommon.h
@@ -1,4 +1,9 @@
+/**
+ * Copyright - See the COPYRIGHT that is included with this distribution.
+ * pvxs is distributed subject to a Software License Agreement found
+ * in file LICENSE that is included with this distribution.
+ */
 #ifndef PVXS_NETCOMMON_H
 #define PVXS_NETCOMMON_H
 
 #if !defined(PVXS_CLIENT_H) && !defined(PVXS_SERVER_H)
diff --git a/src/pvxs/srvcommon.h b/src/pvxs/srvcommon.h
index 0458828..5ed810a 100644
--- a/src/pvxs/srvcommon.h
+++ b/src/pvxs/srvcommon.h
@@ -1,4 +1,9 @@
+/**
+ * Copyright - See the COPYRIGHT that is included with this distribution.
+ * pvxs is distributed subject to a Software License Agreement found
+ * in file LICENSE that is included with this distribution.
+ */
 #ifndef PVXS_SRVCOMMON_H
 #define PVXS_SRVCOMMON_H
 
 #if !defined(PVXS_SHAREDPV_H) && !defined(PVXS_SOURCE_H)

```
