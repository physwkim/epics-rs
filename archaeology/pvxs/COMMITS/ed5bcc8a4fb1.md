# ed5bcc8a4fb1 — fix handling of segmented messages

**Date**: 2020-04-17  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: uncertain  

## Changed files
src/conn.cpp | 5 +++--

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
fix handling of segmented messages

## Key diff (first 100 lines)
```diff
diff --git a/src/conn.cpp b/src/conn.cpp
index 4d350e3..4d4fb3b 100644
--- a/src/conn.cpp
+++ b/src/conn.cpp
@@ -201,9 +201,10 @@ void ConnBase::bevRead()
             if(auto n = evbuffer_get_length(segBuf.get()))
                 evbuffer_drain(segBuf.get(), n);
 
-            // wait for next header
-            bufferevent_setwatermark(bev.get(), EV_READ, 8, tcp_readahead);
         }
+
+        // wait for next header
+        bufferevent_setwatermark(bev.get(), EV_READ, 8, tcp_readahead);
     }
 
     if(!bev) {

```
