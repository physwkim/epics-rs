# 9fefa95df1f7 — fix client PUT

**Date**: 2020-02-25  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
src/clientget.cpp | 7 ++++---

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/{get,put,rpc}.rs

## Commit message
fix client PUT

## Key diff (first 100 lines)
```diff
diff --git a/src/clientget.cpp b/src/clientget.cpp
index f8e71d0..ecced26 100644
--- a/src/clientget.cpp
+++ b/src/clientget.cpp
@@ -302,11 +302,12 @@ void Connection::handle_GPR(pva_app_msg_t cmd)
         to_wire(R, op->chan->sid);
         to_wire(R, ioid);
         if(gpr->state==GPROp::GetOPut) {
-            to_wire(R, 0x40);
+            to_wire(R, uint8_t(0x40));
 
         } else if(gpr->state==GPROp::Exec) {
-            to_wire(R, 0x00);
-            to_wire_valid(R, info->prototype);
+            to_wire(R, uint8_t(0x00));
+            if(cmd!=CMD_GET)
+                to_wire_valid(R, info->prototype);
 
         } else if(gpr->state==GPROp::Done) {
             // we're actually building CMD_DESTROY_REQUEST

```
