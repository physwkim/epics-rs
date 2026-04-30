# fa25bf2aecac — server: fix TCP search reply

**Date**: 2021-04-16  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
src/serverchan.cpp | 15 ++++++++-------

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs (channel state)

## Commit message
server: fix TCP search reply

## Key diff (first 100 lines)
```diff
diff --git a/src/serverchan.cpp b/src/serverchan.cpp
index b05ad60..65d16fd 100644
--- a/src/serverchan.cpp
+++ b/src/serverchan.cpp
@@ -237,17 +237,18 @@ void ServerConn::handle_SEARCH()
 
         EvOutBuf R(hostBE, txBody.get());
 
-        to_wire(M, searchID);
-        to_wire(M, iface->bind_addr);
-        to_wire(M, iface->bind_addr.port());
-        to_wire(M, "tcp");
+        _to_wire<12>(M, iface->server->effective.guid.data(), false);
+        to_wire(R, searchID);
+        to_wire(R, SockAddr::any(AF_INET));
+        to_wire(R, iface->bind_addr.port());
+        to_wire(R, "tcp");
         // "found" flag
-        to_wire(M, uint8_t(nreply!=0 ? 1 : 0));
+        to_wire(R, uint8_t(nreply!=0 ? 1 : 0));
 
-        to_wire(M, uint16_t(nreply));
+        to_wire(R, uint16_t(nreply));
         for(auto i : range(op._names.size())) {
             if(op._names[i]._claim)
-                to_wire(M, uint32_t(nameStorage[i].first));
+                to_wire(R, uint32_t(nameStorage[i].first));
         }
     }
 

```
