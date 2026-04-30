# bebd7a91aaab — SockAddr fallback to sync. dns lookup

**Date**: 2022-04-06  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: eliminated  

## Changed files
src/osiSockExt.h | 43 ++++++++++++++++++++++++++++++++++++++++++-
src/util.cpp     | 43 ++++++++++++++++++++++++++++++++++++++-----

## pva-rs mapping
- crates/epics-pva-rs/src/

## Commit message
SockAddr fallback to sync. dns lookup

## Key diff (first 100 lines)
```diff
diff --git a/src/osiSockExt.h b/src/osiSockExt.h
index a9ed34a..39ade62 100644
--- a/src/osiSockExt.h
+++ b/src/osiSockExt.h
@@ -50,7 +50,7 @@ public:
 
     explicit SockAddr(int af = AF_UNSPEC);
     explicit SockAddr(const char *address, unsigned short port=0);
-    explicit SockAddr(const sockaddr *addr);
+    explicit SockAddr(const sockaddr *addr, socklen_t alen=0);
     inline explicit SockAddr(const std::string& address, unsigned short port=0) :SockAddr(address.c_str(), port) {}
 
     size_t size() const noexcept;
@@ -158,6 +158,47 @@ bool operator==(const SockEndpoint& lhs, const SockEndpoint& rhs);
 inline
 bool operator!=(const SockEndpoint& lhs, const SockEndpoint& rhs) { return !(lhs==rhs); }
 
+struct GetAddrInfo {
+    explicit GetAddrInfo(const char *name);
+    inline explicit GetAddrInfo(const std::string& name) :GetAddrInfo(name.c_str()) {}
+    GetAddrInfo(const GetAddrInfo&) = delete;
+    inline
+    GetAddrInfo(GetAddrInfo&& o) :info(o.info) {
+        o.info = nullptr;
+    }
+    ~GetAddrInfo();
+
+    struct iterator {
+        evutil_addrinfo *pos = nullptr;
+        inline iterator() = default;
+        inline iterator(evutil_addrinfo *pos) :pos(pos) {}
+        inline SockAddr operator*() const {
+            return SockAddr(pos->ai_addr, pos->ai_addrlen);
+        }
+        inline iterator& operator++() {
+            pos = pos->ai_next;
+            return *this;
+        }
+        inline iterator operator++(int) {
+            auto ret(*this);
+            pos = pos->ai_next;
+            return ret;
+        }
+        inline bool operator==(const iterator& o) const {
+            return pos==o.pos;
+        }
+        inline bool operator!=(const iterator& o) const {
+            return pos!=o.pos;
+        }
+    };
+
+    inline iterator begin() const { return iterator{info}; }
+    inline iterator end() const { return iterator{}; }
+
+private:
+    evutil_addrinfo *info;
+};
+
 struct recvfromx {
     evutil_socket_t sock;
     void *buf;
diff --git a/src/util.cpp b/src/util.cpp
index 9c12afc..96f1608 100644
--- a/src/util.cpp
+++ b/src/util.cpp
@@ -275,13 +275,16 @@ SockAddr::SockAddr(const char *address, unsigned short port)
     setAddress(address, port);
 }
 
-SockAddr::SockAddr(const sockaddr *addr)
+SockAddr::SockAddr(const sockaddr *addr, socklen_t alen)
     :SockAddr(addr ? addr->sa_family : AF_UNSPEC)
 {
     if(!addr)
         return; // treat NULL as AF_UNSPEC
 
-    if(family()!=AF_UNSPEC && family()!=AF_INET && family()!=AF_INET6)
+    if(family()==AF_UNSPEC) {}
+    else if(family()==AF_INET && (!alen || alen>=sizeof(sockaddr_in))) {}
+    else if(family()==AF_INET6 && (!alen || alen>=sizeof(sockaddr_in6))) {}
+    else
         throw std::invalid_argument("Unsupported address family");
 
     if(family()!=AF_UNSPEC)
@@ -402,8 +405,26 @@ void SockAddr::setAddress(const char *name, unsigned short defport)
         throw std::runtime_error(SB()<<"Invalid IP address form \""<<escape(name)<<"\"");
     }
 
-    if(evutil_inet_pton(temp->sa.sa_family, addr, sockaddr)<=0)
-        throw std::runtime_error(SB()<<"Not a valid IP address \""<<escape(name)<<"\"");
+    if(evutil_inet_pton(temp->sa.sa_family, addr, sockaddr)<=0) {
+        // not a plain IP4/6 address.
+        // Fall back to synchronous DNS lookup (could be sloooow)
+
+        GetAddrInfo info(addr);
+
+        // We may get a mixture of IP v4 and/or v6 addresses.
+        // For maximum compatibility, we always prefer IPv4
+
+        for(const auto addr : info) {
```
