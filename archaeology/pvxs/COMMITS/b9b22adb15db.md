# b9b22adb15db — fix version_str()

**Date**: 2019-12-18  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: uncertain  

## Changed files
src/util.cpp | 10 ++++++++--

## pva-rs mapping
- crates/epics-pva-rs/src/

## Commit message
fix version_str()

## Key diff (first 100 lines)
```diff
diff --git a/src/util.cpp b/src/util.cpp
index 544274f..c9456a9 100644
--- a/src/util.cpp
+++ b/src/util.cpp
@@ -21,11 +21,17 @@
 
 namespace pvxs {
 
-#define stringify(X) #X
+#define stringifyX(X) #X
+#define stringify(X) stringifyX(X)
 
 const char *version_str()
 {
-    return "PVXS " stringify(PVXS_MAJOR_VERSION);
+    return "PVXS "
+            stringify(PVXS_MAJOR_VERSION)
+            "."
+            stringify(PVXS_MINOR_VERSION)
+            "."
+            stringify(PVXS_MAINTENANCE_VERSION);
 }
 
 unsigned long version_int()

```
