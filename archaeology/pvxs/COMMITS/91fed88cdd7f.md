# 91fed88cdd7f — "Beacon tx error" show destination

**Date**: 2025-02-20  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: applies  

## Changed files
src/server.cpp | 3 ++-

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs

## Commit message
"Beacon tx error" show destination

## Key diff (first 100 lines)
```diff
diff --git a/src/server.cpp b/src/server.cpp
index 0a45dc9..277014d 100644
--- a/src/server.cpp
+++ b/src/server.cpp
@@ -788,7 +788,8 @@ void Server::Pvt::doBeacons(short evt)
             auto lvl = Level::Warn;
             if(err==EINTR || err==EPERM)
                 lvl = Level::Debug;
-            log_printf(serverio, lvl, "Beacon tx error (%d) %s\n",
+            log_printf(serverio, lvl, "Beacon tx %s error (%d) %s\n",
+                       (SB()<<dest).str().c_str(),
                        err, evutil_socket_error_to_string(err));
 
         } else if(unsigned(ntx)<pktlen) {

```
