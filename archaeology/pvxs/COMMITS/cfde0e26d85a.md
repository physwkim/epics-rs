# cfde0e26d85a — avoid assert for mostly absurdly long PV names

**Date**: 2022-10-13  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
src/client.cpp | 15 +++++++++++----

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
avoid assert for mostly absurdly long PV names

## Key diff (first 100 lines)
```diff
diff --git a/src/client.cpp b/src/client.cpp
index 189cab8..c6d4771 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -1012,10 +1012,17 @@ void ContextImpl::tickSearch(bool discover)
                 continue;
 
             } else if(size_t(M.save() - searchMsg.data()) > maxSearchPayload) {
-                assert(payload); // must have something
-                // too large, defer
-                M.restore(save);
-                break;
+                if(payload) {
+                    // other names did fit, defer this one to the next packet
+                    M.restore(save);
+                    break;
+
+                } else {
+                    // some slightly less absurdly long PV name.
+                    // Less than the UDP packet limit, but longer
+                    // than typical MTU.  Try to send, probably
+                    // no choice but to fragment.
+                }
             }
 
             count++;

```
