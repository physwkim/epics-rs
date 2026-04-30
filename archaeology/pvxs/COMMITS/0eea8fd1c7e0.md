# 0eea8fd1c7e0 — fix CMD_MESSAGE handling

**Date**: 2022-10-11  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: applies  

## Changed files
src/clientconn.cpp | 36 ++++++++++++++++++++++++++++++++++++
src/clientimpl.h   |  2 ++
src/serverconn.cpp | 14 ++++++--------

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs
- crates/epics-pva-rs/src/server_native/tcp.rs

## Commit message
fix CMD_MESSAGE handling

## Key diff (first 100 lines)
```diff
diff --git a/src/clientconn.cpp b/src/clientconn.cpp
index eadd7c3..35c6547 100644
--- a/src/clientconn.cpp
+++ b/src/clientconn.cpp
@@ -13,6 +13,8 @@ namespace pvxs {
 namespace client {
 
 DEFINE_LOGGER(io, "pvxs.client.io");
+DEFINE_LOGGER(connsetup, "pvxs.tcp.setup");
+DEFINE_LOGGER(remote, "pvxs.remote.log");
 
 Connection::Connection(const std::shared_ptr<ContextImpl>& context,
                        const SockAddr& peerAddr,
@@ -416,6 +418,40 @@ void Connection::handle_DESTROY_CHANNEL()
                      peerName.c_str(), chan->name.c_str(), unsigned(cid), unsigned(sid));
 }
 
+void Connection::handle_MESSAGE()
+{
+    EvInBuf M(peerBE, segBuf.get(), 16);
+
+    uint32_t ioid = 0;
+    uint8_t mtype = 0;
+    std::string msg;
+
+    from_wire(M, ioid);
+    from_wire(M, mtype);
+    from_wire(M, msg);
+
+    if(!M.good())
+        throw std::runtime_error(SB()<<M.file()<<':'<<M.line()<<" Decode error for Message");
+
+    auto it = opByIOID.find(ioid);
+    if(it==opByIOID.end()) {
+        log_debug_printf(connsetup, "Server %s Message on non-existent ioid\n", peerName.c_str());
+        return;
+    }
+    auto op = it->second.handle.lock();
+
+    Level lvl;
+    switch(mtype) {
+    case 0:  lvl = Level::Info; break;
+    case 1:  lvl = Level::Warn; break;
+    case 2:  lvl = Level::Err; break;
+    default: lvl = Level::Crit; break;
+    }
+
+    log_printf(remote, lvl, "%s : %s\n",
+               op && op->chan ? op->chan->name.c_str() : "<dead>", msg.c_str());
+}
+
 void Connection::tickEcho()
 {
     if(state==Holdoff) {
diff --git a/src/clientimpl.h b/src/clientimpl.h
index a99e4b9..9839288 100644
--- a/src/clientimpl.h
+++ b/src/clientimpl.h
@@ -132,6 +132,8 @@ public:
     CASE(MONITOR);
     CASE(RPC);
     CASE(GET_FIELD);
+
+    CASE(MESSAGE);
 #undef CASE
 
     void handle_GPR(pva_app_msg_t cmd);
diff --git a/src/serverconn.cpp b/src/serverconn.cpp
index 07ca401..fc4de32 100644
--- a/src/serverconn.cpp
+++ b/src/serverconn.cpp
@@ -308,16 +308,14 @@ void ServerConn::handle_MESSAGE()
 
     Level lvl;
     switch(mtype) {
-    case 0: lvl = Level::Info;
-    case 1: lvl = Level::Warn;
-    case 2: lvl = Level::Err;
-    default:lvl = Level::Crit;
+    case 0:  lvl = Level::Info; break;
+    case 1:  lvl = Level::Warn; break;
+    case 2:  lvl = Level::Err; break;
+    default: lvl = Level::Crit; break;
     }
 
-    if(remote.test(lvl))
-        errlogPrintf("Client %s Channel %s Remote message: %s\n",
-                     peerName.c_str(), chan ? "<dead>" : chan->name.c_str(),
-                     msg.c_str());
+    log_printf(remote, lvl, "%s : %s\n",
+               chan ? chan->name.c_str() : "<dead>", msg.c_str());
 }
 
 std::shared_ptr<ConnBase> ServerConn::self_from_this()

```
