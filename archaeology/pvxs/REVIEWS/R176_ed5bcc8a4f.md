# R176 — ed5bcc8a4fb1 [LOW][uncertain]

**Subject**: fix handling of segmented messages  
**Date**: 2020-04-17  
**pvxs SHA**: ed5bcc8a4fb1  

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Verdict
**uncertain** — LOW

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit ed5bcc8a4fb125fdfcf82760c1a6db156418356c
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Fri Apr 17 13:24:39 2020 -0700

    fix handling of segmented messages
---
 src/conn.cpp | 5 +++--
 1 file changed, 3 insertions(+), 2 deletions(-)

diff --git a/src/conn.cpp b/src/conn.cpp
index 4d350e3..4d4fb3b 100644
--- a/src/conn.cpp
+++ b/src/conn.cpp
@@ -200,11 +200,12 @@ void ConnBase::bevRead()
             // silently drain any unprocessed body (forward compatibility)
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
         cleanup();

```
