# R039 — adcac746efff [MEDIUM][applies]

**Subject**: server avoid verbose Beacon tx errors  
**Date**: 2020-02-18  
**pvxs SHA**: adcac746efff  

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs

## Verdict
**applies** — MEDIUM

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit adcac746efff80c0038dead3bee760821f4b2214
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Tue Feb 18 21:07:57 2020 -0800

    server avoid verbose Beacon tx errors
---
 src/server.cpp | 5 ++++-
 1 file changed, 4 insertions(+), 1 deletion(-)

diff --git a/src/server.cpp b/src/server.cpp
index aaeb158..c8a16cb 100644
--- a/src/server.cpp
+++ b/src/server.cpp
@@ -524,9 +524,12 @@ void Server::Pvt::doBeacons(short evt)
         int ntx = sendto(beaconSender.sock, (char*)beaconMsg.data(), pktlen, 0, &dest->sa, dest.size());
 
         if(ntx<0) {
             int err = evutil_socket_geterror(beaconSender.sock);
-            log_warn_printf(serverio, "Beacon tx error (%d) %s\n",
+            auto lvl = Level::Warn;
+            if(err==EINTR || err==EPERM)
+                lvl = Level::Debug;
+            log_printf(serverio, lvl, "Beacon tx error (%d) %s\n",
                        err, evutil_socket_error_to_string(err));
 
         } else if(unsigned(ntx)<pktlen) {
             log_warn_printf(serverio, "Beacon truncated %u < %u",

```
