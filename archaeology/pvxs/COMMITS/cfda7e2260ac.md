# cfda7e2260ac — client: trap error in close()

**Date**: 2020-04-07  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
src/client.cpp | 11 +++++++++--

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
client: trap error in close()

## Key diff (first 100 lines)
```diff
diff --git a/src/client.cpp b/src/client.cpp
index 624e065..c1c79bd 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -234,8 +234,15 @@ Context::Context(const Config& conf)
 
     // external
     pvt.reset(internal.get(), [internal](Pvt*) mutable {
-        internal->close();
-        internal.reset();
+        auto temp(std::move(internal));
+        try {
+            temp->close();
+        }catch(std::exception& e){
+            // called through ~shared_ptr and can't propagate exceptions.
+            // log and continue...
+            log_exc_printf(setup, "Error while closing Context (%s) : %s\n",
+                           typeid(e).name(), e.what());
+        }
         cnt_ClientPvtLive.fetch_sub(1u);
     });
     // we don't keep a weak_ptr to the external reference.

```
