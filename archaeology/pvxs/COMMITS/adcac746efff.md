# adcac746efff — server avoid verbose Beacon tx errors

**Date**: 2020-02-18  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: applies  

## Changed files
src/server.cpp | 5 ++++-

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs

## Commit message
server avoid verbose Beacon tx errors

## Key diff (first 100 lines)
```diff
diff --git a/src/server.cpp b/src/server.cpp
index aaeb158..c8a16cb 100644
--- a/src/server.cpp
+++ b/src/server.cpp
@@ -525,7 +525,10 @@ void Server::Pvt::doBeacons(short evt)
 
         if(ntx<0) {
             int err = evutil_socket_geterror(beaconSender.sock);
-            log_warn_printf(serverio, "Beacon tx error (%d) %s\n",
+            auto lvl = Level::Warn;
+            if(err==EINTR || err==EPERM)
+                lvl = Level::Debug;
+            log_printf(serverio, lvl, "Beacon tx error (%d) %s\n",
                        err, evutil_socket_error_to_string(err));
 
         } else if(unsigned(ntx)<pktlen) {

```
