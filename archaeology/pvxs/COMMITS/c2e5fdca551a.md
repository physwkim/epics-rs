# c2e5fdca551a — client: avoid FD leak on failed connect()

**Date**: 2024-02-22  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: uncertain  

## Changed files
src/clientconn.cpp | 14 ++++++++------
src/conn.cpp       | 18 ++++++++++--------
src/conn.h         |  2 +-

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
client: avoid FD leak on failed connect()

## Key diff (first 100 lines)
```diff
diff --git a/src/clientconn.cpp b/src/clientconn.cpp
index 21882fe..69f98b0 100644
--- a/src/clientconn.cpp
+++ b/src/clientconn.cpp
@@ -62,17 +62,19 @@ void Connection::startConnecting()
 {
     assert(!this->bev);
 
-    auto bev(bufferevent_socket_new(context->tcp_loop.base, -1, BEV_OPT_CLOSE_ON_FREE|BEV_OPT_DEFER_CALLBACKS));
+    decltype(this->bev) bev(__FILE__, __LINE__,
+                bufferevent_socket_new(context->tcp_loop.base, -1,
+                                       BEV_OPT_CLOSE_ON_FREE|BEV_OPT_DEFER_CALLBACKS));
 
-    bufferevent_setcb(bev, &bevReadS, nullptr, &bevEventS, this);
+    bufferevent_setcb(bev.get(), &bevReadS, nullptr, &bevEventS, this);
 
     timeval tmo(totv(context->effective.tcpTimeout));
-    bufferevent_set_timeouts(bev, &tmo, &tmo);
+    bufferevent_set_timeouts(bev.get(), &tmo, &tmo);
 
-    if(bufferevent_socket_connect(bev, const_cast<sockaddr*>(&peerAddr->sa), peerAddr.size()))
+    if(bufferevent_socket_connect(bev.get(), const_cast<sockaddr*>(&peerAddr->sa), peerAddr.size()))
         throw std::runtime_error("Unable to begin connecting");
     {
-        auto fd(bufferevent_getfd(bev));
+        auto fd(bufferevent_getfd(bev.get()));
         int opt = 1;
         if(setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, (char*)&opt, sizeof(opt))<0) {
             auto err(SOCKERRNO);
@@ -80,7 +82,7 @@ void Connection::startConnecting()
         }
     }
 
-    connect(bev);
+    connect(std::move(bev));
 
     log_debug_printf(io, "Connecting to %s, RX readahead %zu\n", peerName.c_str(), readahead);
 }
diff --git a/src/conn.cpp b/src/conn.cpp
index 80a0c66..7b17afc 100644
--- a/src/conn.cpp
+++ b/src/conn.cpp
@@ -39,8 +39,10 @@ ConnBase::ConnBase(bool isClient, bool sendBE, bufferevent* bev, const SockAddr&
     ,txBody(__FILE__, __LINE__, evbuffer_new())
     ,state(Holdoff)
 {
-    if(bev) // true for server connection.  client will call connect() shortly
-        connect(bev);
+    if(bev) { // true for server connection.  client will call connect() shortly
+        decltype(this->bev) temp(__FILE__, __LINE__, bev);
+        connect(std::move(temp));
+    }
 }
 
 ConnBase::~ConnBase() {}
@@ -50,30 +52,30 @@ const char* ConnBase::peerLabel() const
     return isClient ? "Server" : "Client";
 }
 
-void ConnBase::connect(bufferevent* bev)
+void ConnBase::connect(ev_owned_ptr<bufferevent> &&bev)
 {
     if(!bev)
         throw BAD_ALLOC();
     assert(!this->bev && state==Holdoff);
 
-    this->bev.reset(bev);
-
-    readahead = evsocket::get_buffer_size(bufferevent_getfd(bev), false);
+    readahead = evsocket::get_buffer_size(bufferevent_getfd(bev.get()), false);
 
 #if LIBEVENT_VERSION_NUMBER >= 0x02010000
     // allow to drain OS socket buffer in a single read
-    (void)bufferevent_set_max_single_read(bev, readahead);
+    (void)bufferevent_set_max_single_read(bev.get(), readahead);
 #endif
 
     readahead *= tcp_readahead_mult;
 
 #if LIBEVENT_VERSION_NUMBER >= 0x02010000
     // allow attempt to write as much as is available
-    (void)bufferevent_set_max_single_write(bev, EV_SSIZE_MAX);
+    (void)bufferevent_set_max_single_write(bev.get(), EV_SSIZE_MAX);
 #endif
 
     state = isClient ? Connecting : Connected;
 
+    this->bev = std::move(bev);
+
     // initially wait for at least a header
     bufferevent_setwatermark(this->bev.get(), EV_READ, 8, readahead);
 }
diff --git a/src/conn.h b/src/conn.h
index b4ba715..53491b9 100644
--- a/src/conn.h
+++ b/src/conn.h
@@ -61,7 +61,7 @@ public:
 
     bufferevent* connection() { return bev.get(); }
 
```
