# acfba6469ed3 — start client beacon rx

**Date**: 2020-02-22  
**Author**: Michael Davidsaver  
**Severity**: HIGH  
**Verdict**: applies  

## Changed files
src/client.cpp    | 79 +++++++++++++++++++++++++++++++++++++++++++++++++++----
src/clientimpl.h  | 11 ++++++++
src/config.cpp    |  9 +++++--
src/pvxs/client.h |  8 +++++-
src/server.cpp    |  4 +++

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs
- crates/epics-pva-rs/src/config.rs
- crates/epics-pva-rs/src/server_native/tcp.rs

## Commit message
start client beacon rx

## Key diff (first 100 lines)
```diff
diff --git a/src/client.cpp b/src/client.cpp
index 48d5b7e..861d3eb 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -26,6 +26,8 @@ constexpr size_t nBuckets = 30u;
 
 constexpr size_t maxSearchPayload = 0x4000;
 
+constexpr timeval beaconCleanInterval{2*180, 0};
+
 Disconnect::Disconnect()
     :std::runtime_error("Disconnected")
 {}
@@ -180,14 +182,12 @@ Context::Pvt::Pvt(const Config& conf)
     ,tcp_loop("PVXCTCP", epicsThreadPriorityCAServerLow)
     ,searchRx(event_new(tcp_loop.base, searchTx.sock, EV_READ|EV_PERSIST, &Pvt::onSearchS, this))
     ,searchTimer(event_new(tcp_loop.base, -1, EV_TIMEOUT, &Pvt::tickSearchS, this))
+    ,beaconCleaner(event_new(tcp_loop.base, -1, EV_TIMEOUT, &Pvt::tickBeaconCleanS, this))
 {
     effective.expand();
 
     searchBuckets.resize(nBuckets);
 
-    if(effective.udp_port==0)
-        throw std::runtime_error("Client can't use UDP random port");
-
     std::set<std::string> bcasts;
     {
         ELLLIST list = ELLLIST_INIT;
@@ -242,14 +242,26 @@ Context::Pvt::Pvt(const Config& conf)
         searchDest.emplace_back(saddr, isucast);
     }
 
-    // TODO: receive beacons
-    //auto manager = UDPManager::instance();
+    auto manager = UDPManager::instance();
+
+    for(auto& iface : effective.interfaces) {
+        SockAddr addr(AF_INET, iface.c_str(), effective.udp_port);
+        log_info_printf(io, "Listening for beacons on %s\n", addr.tostring().c_str());
+        beaconRx.push_back(manager.onBeacon(addr, [this](const UDPManager::Beacon& msg) {
+            onBeacon(msg);
+        }));
+    }
 
+    for(auto& listener : beaconRx) {
+        listener->start();
+    }
 
     if(event_add(searchTimer.get(), &bucketInterval))
         log_err_printf(setup, "Error enabling search timer\n%s", "");
     if(event_add(searchRx.get(), nullptr))
         log_err_printf(setup, "Error enabling search RX\n%s", "");
+    if(event_add(searchTimer.get(), &beaconCleanInterval))
+        log_err_printf(setup, "Error enabling beacon clean timer on\n%s", "");
 }
 
 Context::Pvt::~Pvt() {}
@@ -291,6 +303,29 @@ void Context::Pvt::poke()
         throw std::runtime_error("Unable to schedule searchTimer");
 }
 
+void Context::Pvt::onBeacon(const UDPManager::Beacon& msg)
+{
+    const auto& guid = msg.guid;
+
+    epicsTimeStamp now;
+    epicsTimeGetCurrent(&now);
+
+    auto it = beaconSenders.find(msg.src);
+    if(it!=beaconSenders.end() && msg.guid==it->second.guid) {
+        it->second.lastRx = now;
+        return;
+    }
+
+    beaconSenders.emplace(msg.src, BTrack{msg.guid, now});
+
+    log_debug_printf(io, "%s New server %02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x%02x %s\n",
+               msg.src.tostring().c_str(),
+               guid[0], guid[1], guid[2], guid[3], guid[4], guid[5], guid[6], guid[7], guid[8], guid[9], guid[10], guid[11],
+               msg.server.tostring().c_str());
+
+    poke();
+}
+
 bool Context::Pvt::onSearch()
 {
     searchMsg.resize(0x10000);
@@ -560,6 +595,40 @@ void Context::Pvt::tickSearchS(evutil_socket_t fd, short evt, void *raw)
     }
 }
 
+void Context::Pvt::tickBeaconClean()
+{
+    epicsTimeStamp now;
+    epicsTimeGetCurrent(&now);
+
+    auto it = beaconSenders.begin();
+    while(it!=beaconSenders.end()) {
+        auto cur = it++;
```
