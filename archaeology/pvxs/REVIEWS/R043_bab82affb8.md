# R043 — bab82affb84d [MEDIUM][applies]

**Subject**: redo packet build/parse  
**Date**: 2019-11-07  
**pvxs SHA**: bab82affb84d  

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs
- crates/epics-pva-rs/src/client_native/udp.rs
- crates/epics-pva-rs/src/

## Verdict
**applies** — MEDIUM

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit bab82affb84da0066dd0e7987dba8b284b46c731
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Thu Nov 7 11:48:58 2019 -0800

    redo packet build/parse
    
    now with evbuffer
---
 src/evhelper.cpp      |  99 +++++++++++--------
 src/evhelper.h        |  36 +++++--
 src/log.cpp           |  16 +++
 src/pvaproto.h        | 269 +++++++++++++++++++++++++++++++-------------------
 src/pvxs/version.h    |  12 +++
 src/server.cpp        |  51 +++++-----
 src/serverconn.cpp    |  85 +++++++++-------
 src/udp_collector.cpp |  67 +++++++------
 src/udp_collector.h   |   1 +
 src/util.cpp          |   7 ++
 src/utilpvt.h         |   3 +
 test/testev.cpp       |  52 +++++++++-
 test/testsock.cpp     |  72 +++++++-------
 test/testudp.cpp      |  39 ++++----
 14 files changed, 514 insertions(+), 295 deletions(-)

diff --git a/src/evhelper.cpp b/src/evhelper.cpp
index 7bd7932..28f4f67 100644
--- a/src/evhelper.cpp
+++ b/src/evhelper.cpp
@@ -182,27 +182,8 @@ bool evbase::inLoop()
 {
     return pvt->worker.isCurrentThread();
 }
 
-void to_wire(sbuf<uint8_t>& buf, const SockAddr &val, bool be)
-{
-    if(buf.err || buf.size()<16) {
-        buf.err = true;
-
-    } else if(val.family()==AF_INET) {
-        for(unsigned i=0; i<10; i++)
-            buf[i]=0;
-        buf[10] = buf[11] = 0xff;
-
-        memcpy(buf.pos+12, &val->in.sin_addr.s_addr, 4);
-
-    } else if(val.family()==AF_INET6) {
-        static_assert (sizeof(val->in6.sin6_addr)==16, "");
-        memcpy(buf.pos, &val->in6.sin6_addr, 16);
-    }
-    buf += 16;
-}
-
 evsocket::evsocket(evutil_socket_t sock)
     :sock(sock)
 {
     if(sock==evutil_socket_t(-1))
@@ -306,31 +287,71 @@ void evsocket::mcast_iface(const SockAddr& iface) const
 
     // IPV6_MULTICAST_IF
 }
 
+bool VectorOutBuf::refill(size_t more) {
+    assert(pos <= limit);
+    assert(pos >= backing.data());
 
-void from_wire(sbuf<const uint8_t>& buf, Size& size, bool be)
+    if(err) return false;
+
+    more = ((more-1)|0xff)+1; // round up to multiple of 256
+    size_t idx = pos - backing.data(); // save current offset
+    try{
+        backing.resize(backing.size()+more);
+    }catch(std::bad_alloc& e) {
+        return false;
+    }
+    pos = backing.data()+idx;
+    limit = backing.data()+backing.size();
+    return true;
+}
+
+bool EvOutBuf::refill(size_t more)
 {
-    if(buf.err || buf.empty()) {
-        buf.err = true;
-        return;
+    if(err) return false;
+
+    evbuffer_iovec vec;
+    vec.iov_base = base;
+    vec.iov_len  = pos-base;
+
+    if(base && evbuffer_commit_space(backing, &vec, 1))
+        throw std::bad_alloc(); // leak?
+
+    limit = base = pos = nullptr;
+
+    if(more) {
+        auto n = evbuffer_reserve_space(backing, more, &vec, 1);
+        if(n!=1) {
+            return false;
+        }
+
+        base = pos = (uint8_t*)vec.iov_base;
+        limit = base+vec.iov_len;
     }
-    uint8_t s=buf[0];
-    buf+=1;
-    if(s<254) {
-        size.size = s;
-
-    } else if(s==255) {
-        // "null" size.  not sure it is used.  Replicate weirdness of pvDataCPP
-        size.size = -1;
-
-    } else if(s==254) {
-        uint32_t ls = 0;
-        from_wire(buf, ls, be);
-        size.size = ls;
-    } else {
-        // unreachable
-        buf.err = true;
+    return true;
+}
+
+bool EvInBuf::refill(size_t more)
+{
+    if(err) return false;
+
+    if(base && evbuffer_drain(backing, pos-base))
+        throw std::bad_alloc();
+
+    limit = base = pos = nullptr;
+
+    if(more) {
+        evbuffer_iovec vec;
+
+        auto n = evbuffer_peek(backing, -1, nullptr, &vec, 1);
+        if(n<=0) { // current (2.1) impl never returns negative
+            return false;
+        }
+
+        base = pos = (uint8_t*)vec.iov_base;
+        limit = base+vec.iov_len;
     }
+    return true;
 }
 
 } // namespace pvxsimpl
diff --git a/src/evhelper.h b/src/evhelper.h
index f3c1cc9..17b2992 100644
... (truncated)
```
