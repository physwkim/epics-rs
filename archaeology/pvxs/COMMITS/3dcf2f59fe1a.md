# 3dcf2f59fe1a — fix logger_level_set()

**Date**: 2021-05-05  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: eliminated  

## Changed files
src/log.cpp      | 29 +++++++++++--------

## pva-rs mapping
- (no direct mapping found)

## Commit message
fix logger_level_set()

## Key diff (first 100 lines)
```diff
diff --git a/src/log.cpp b/src/log.cpp
index 9c02e44..b43e8bd 100644
--- a/src/log.cpp
+++ b/src/log.cpp
@@ -189,24 +189,31 @@ struct logger_gbl_t {
         if(lvl<=Level(0))
             lvl = Level(1);
 
+        decltype (config)::value_type* conf = nullptr;
+
         for(auto& tup : config) {
             if(tup.first==exp) {
                 // update of existing config
-                if(tup.second!=lvl) {
-                    tup.second = lvl;
-
-                    for(auto& pair : loggers) {
-                        if(epicsStrGlobMatch(pair.first.c_str(), tup.first.c_str())) {
-                            pair.second->lvl.store(lvl, std::memory_order_relaxed);
-                        }
-                    }
-                }
-                return;
+                conf = &tup;
+                break;
             }
         }
         // new config
 
-        config.emplace_back(exp, lvl);
+        if(!conf) {
+            config.emplace_back(exp, Level(-1));
+            conf = &config.back();
+        }
+
+        if(conf->second!=lvl) {
+            conf->second = lvl;
+
+            for(auto& pair : loggers) {
+                if(epicsStrGlobMatch(pair.first.c_str(), conf->first.c_str())) {
+                    pair.second->lvl.store(lvl, std::memory_order_relaxed);
+                }
+            }
+        }
     }
 } *logger_gbl;
 
diff --git a/test/Makefile b/test/Makefile
index 4c4622d..62c32a5 100644
--- a/test/Makefile
+++ b/test/Makefile
@@ -22,6 +22,10 @@ TESTPROD_HOST += testev
 testev_SRCS += testev.cpp
 TESTS += testev
 
+TESTPROD_HOST += testlog
+testlog_SRCS += testlog.cpp
+TESTS += testlog
+
 TESTPROD_HOST += testudp
 testudp_SRCS += testudp.cpp
 TESTS += testudp
diff --git a/test/testlog.cpp b/test/testlog.cpp
new file mode 100644
index 0000000..eaaecfa
--- /dev/null
+++ b/test/testlog.cpp
@@ -0,0 +1,85 @@
+/**
+ * Copyright - See the COPYRIGHT that is included with this distribution.
+ * pvxs is distributed subject to a Software License Agreement found
+ * in file LICENSE that is included with this distribution.
+ */
+
+#include <ostream>
+
+#include <testMain.h>
+
+#include <epicsUnitTest.h>
+
+#include <pvxs/unittest.h>
+#include <pvxs/log.h>
+
+namespace pvxs {
+
+std::ostream& operator<<(std::ostream& strm, Level lvl)
+{
+    switch (lvl) {
+#define CASE(NAME) case Level::NAME: strm<<#NAME; break
+    CASE(Crit);
+    CASE(Err);
+    CASE(Warn);
+    CASE(Info);
+    CASE(Debug);
+#undef CASE
+    }
+    return strm;
+}
+
+} // namespace pvxs
```
