# 9af841cccde5 — server/client search logging

**Date**: 2021-07-04  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: eliminated  

## Changed files
src/client.cpp     |  5 ++++-
src/server.cpp     |  5 ++++-
src/serverchan.cpp | 11 +++++++----
src/sharedpv.cpp   | 10 ++++++++--

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs
- crates/epics-pva-rs/src/server_native/tcp.rs
- crates/epics-pva-rs/src/server_native/tcp.rs (channel state)
- crates/epics-pva-rs/src/server_native/shared_pv.rs

## Commit message
server/client search logging

## Key diff (first 100 lines)
```diff
diff --git a/src/client.cpp b/src/client.cpp
index a42c788..a5bc0c4 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -683,8 +683,11 @@ void procSearchReply(ContextImpl& self, const SockAddr& src, Buffer& M, bool ist
 
     if(M.good()) {
         for(const ServerGUID& ignore : self.ignoreServerGUIDs) {
-            if(guid==ignore)
+            if(guid==ignore) {
+                log_info_printf(io, "Ignore reply from %s with %s\n",
+                                 src.tostring().c_str(), std::string(SB()<<guid).c_str());
                 return;
+            }
         }
     }
 
diff --git a/src/server.cpp b/src/server.cpp
index c6e9fed..b2829d3 100644
--- a/src/server.cpp
+++ b/src/server.cpp
@@ -36,6 +36,7 @@ using namespace impl;
 
 DEFINE_LOGGER(serversetup, "pvxs.server.setup");
 DEFINE_LOGGER(serverio, "pvxs.server.io");
+DEFINE_LOGGER(serversearch, "pvxs.server.search");
 
 Server::Server(const Config& conf)
 {
@@ -638,8 +639,10 @@ void Server::Pvt::onSearch(const UDPManager::Search& msg)
 
     to_wire(M, uint16_t(nreply));
     for(auto i : range(msg.names.size())) {
-        if(searchOp._names[i]._claim)
+        if(searchOp._names[i]._claim) {
             to_wire(M, uint32_t(msg.names[i].id));
+            log_debug_printf(serversearch, "Search claimed '%s'\n", msg.names[i].name);
+        }
     }
     auto pktlen = M.save()-searchReply.data();
 
diff --git a/src/serverchan.cpp b/src/serverchan.cpp
index 138db2e..02d3c93 100644
--- a/src/serverchan.cpp
+++ b/src/serverchan.cpp
@@ -18,6 +18,7 @@ DEFINE_LOGGER(connsetup, "pvxs.tcp.setup");
 DEFINE_LOGGER(connio, "pvxs.tcp.io");
 
 DEFINE_LOGGER(serversetup, "pvxs.server.setup");
+DEFINE_LOGGER(serversearch, "pvxs.server.search");
 
 ServerChan::ServerChan(const std::shared_ptr<ServerConn> &conn,
                        uint32_t sid,
@@ -218,7 +219,7 @@ void ServerConn::handle_SEARCH()
             try {
                 pair.second->onSearch(op);
             }catch(std::exception& e){
-                log_exc_printf(serversetup, "Unhandled error in Source::onSearch for '%s' : %s\n",
+                log_exc_printf(serversearch, "Unhandled error in Source::onSearch for '%s' : %s\n",
                            pair.first.second.c_str(), e.what());
             }
         }
@@ -248,8 +249,10 @@ void ServerConn::handle_SEARCH()
 
         to_wire(R, uint16_t(nreply));
         for(auto i : range(op._names.size())) {
-            if(op._names[i]._claim)
+            if(op._names[i]._claim) {
                 to_wire(R, uint32_t(nameStorage[i].first));
+                log_debug_printf(serversearch, "Search claimed '%s'\n", op._names[i]._name);
+            }
         }
     }
 
@@ -301,12 +304,12 @@ void ServerConn::handle_CREATE_CHANNEL()
                     pair.second->onCreate(std::move(op));
                     if(!op || chan->onOp || chan->onClose || chan->state!=ServerChan::Creating) {
                         claimed = chan->state==ServerChan::Creating;
-                        log_debug_printf(connsetup, "Client %s %s channel to %s through %s\n", peerName.c_str(),
+                        log_debug_printf(serversearch, "Client %s %s channel to %s through %s\n", peerName.c_str(),
                                    claimed?"accepted":"rejected", name.c_str(), pair.first.second.c_str());
                         break;
                     }
                 }catch(std::exception& e){
-                    log_exc_printf(connsetup, "Client %s Unhandled error in onCreate %s,%d %s : %s\n", peerName.c_str(),
+                    log_exc_printf(serversearch, "Client %s Unhandled error in onCreate %s,%d %s : %s\n", peerName.c_str(),
                                pair.first.second.c_str(), pair.first.first,
                                typeid(&e).name(), e.what());
                 }
diff --git a/src/sharedpv.cpp b/src/sharedpv.cpp
index 41372bc..37804ed 100644
--- a/src/sharedpv.cpp
+++ b/src/sharedpv.cpp
@@ -23,6 +23,7 @@ typedef epicsGuard<epicsMutex> Guard;
 typedef epicsGuardRelease<epicsMutex> UnGuard;
 
 DEFINE_LOGGER(logshared, "pvxs.server.sharedpv");
+DEFINE_LOGGER(logsource, "pvxs.server.staticsource");
 DEFINE_LOGGER(logmailbox, "pvxs.mailbox");
 
```
