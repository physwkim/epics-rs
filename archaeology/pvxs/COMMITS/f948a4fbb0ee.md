# f948a4fbb0ee — client: log invalid monitor queueSize

**Date**: 2025-01-31  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
src/clientmon.cpp | 13 ++++++++++---

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/monitor.rs

## Commit message
client: log invalid monitor queueSize

## Key diff (first 100 lines)
```diff
diff --git a/src/clientmon.cpp b/src/clientmon.cpp
index 4ab1600..a2a62b3 100644
--- a/src/clientmon.cpp
+++ b/src/clientmon.cpp
@@ -757,10 +757,17 @@ std::shared_ptr<Subscription> MonitorBuilder::exec()
 
     auto options = op->pvRequest["record._options"];
 
-    options["queueSize"].as<uint32_t>([&op](uint32_t Q) {
-        if(Q>1)
+    if(auto queueSize = options["queueSize"]) {
+        uint32_t Q = 0;
+        if(queueSize.as(Q) && Q>1) {
             op->queueSize = Q;
-    });
+        } else {
+            log_warn_printf(monevt, "%s requested invalid %s : %s\n",
+                            op->channelName.c_str(),
+                            op->pvRequest.nameOf(queueSize).c_str(),
+                            std::string(SB()<<queueSize).c_str());
+        }
+    }
 
     (void)options["pipeline"].as(op->pipeline);
 

```
