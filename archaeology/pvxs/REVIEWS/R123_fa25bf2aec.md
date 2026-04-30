# R123 — fa25bf2aecac [LOW][applies]

**Subject**: server: fix TCP search reply  
**Date**: 2021-04-16  
**pvxs SHA**: fa25bf2aecac  

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs (channel state)

## Verdict
**applies** — LOW

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit fa25bf2aecac11fa36df63aa634b4a7f99a426c0
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Fri Apr 16 22:47:34 2021 -0700

    server: fix TCP search reply
---
 src/serverchan.cpp | 15 ++++++++-------
 1 file changed, 8 insertions(+), 7 deletions(-)

diff --git a/src/serverchan.cpp b/src/serverchan.cpp
index b05ad60..65d16fd 100644
--- a/src/serverchan.cpp
+++ b/src/serverchan.cpp
@@ -236,19 +236,20 @@ void ServerConn::handle_SEARCH()
         (void)evbuffer_drain(txBody.get(), evbuffer_get_length(txBody.get()));
 
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
 
     enqueueTxBody(CMD_SEARCH_RESPONSE);

```
