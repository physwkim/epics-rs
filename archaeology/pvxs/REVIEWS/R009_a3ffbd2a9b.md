# R009 — a3ffbd2a9b77 [HIGH][applies]

**Subject**: client fix Channel reconnect  
**Date**: 2020-03-02  
**pvxs SHA**: a3ffbd2a9b77  

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Verdict
**applies** — HIGH

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit a3ffbd2a9b77aad0e08d93e795e7c51be07b0474
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Mon Mar 2 09:11:44 2020 -0800

    client fix Channel reconnect
---
 src/client.cpp     | 11 +++++++++++
 src/clientconn.cpp | 19 ++++++++++++++-----
 src/clientimpl.h   |  7 ++++---
 3 files changed, 29 insertions(+), 8 deletions(-)

diff --git a/src/client.cpp b/src/client.cpp
index 4b22858..6f3112c 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -93,8 +93,19 @@ void Channel::createOperations()
         op->createOp();
     }
 }
 
+void Channel::disconnect(const std::shared_ptr<Channel>& self)
+{
+    self->state = Channel::Searching;
+    self->sid = 0xdeadbeef; // spoil
+    context->searchBuckets[context->currentBucket].push_back(self);
+
+    log_debug_printf(io, "Server %s detach channel '%s' to re-search\n",
+                     conn ? conn->peerName.c_str() : "<disconnected>",
+                     self->name.c_str());
+
+}
 
 OperationBase::OperationBase(operation_t op, const std::shared_ptr<Channel>& chan)
     :Operation(op)
     ,chan(chan)
diff --git a/src/clientconn.cpp b/src/clientconn.cpp
index 8ad224a..a588b9e 100644
--- a/src/clientconn.cpp
+++ b/src/clientconn.cpp
@@ -127,14 +127,23 @@ void Connection::cleanup()
         auto chan = wchan.lock();
         if(!chan)
             continue;
 
-        chan->state = Channel::Searching;
-        chan->sid = 0xdeadbeef; // spoil
-        self = std::move(chan->conn);
-        context->searchBuckets[context->currentBucket].push_back(chan);
+        chan->disconnect(chan);
+    }
+    for(auto& pair : chanBySID) {
+        auto chan = pair.second.lock();
+        if(!chan)
+            continue;
+
+        chan->disconnect(chan);
+    }
+    for(auto& pair : creatingByCID) {
+        auto chan = pair.second.lock();
+        if(!chan)
+            continue;
 
-        log_debug_printf(io, "Server %s detach channel '%s' to re-search\n", peerName.c_str(), chan->name.c_str());
+        chan->disconnect(chan);
     }
 
     auto ops = std::move(opByIOID);
     for (auto& pair : ops) {
diff --git a/src/clientimpl.h b/src/clientimpl.h
index 6bb0e1a..36fe437 100644
--- a/src/clientimpl.h
+++ b/src/clientimpl.h
@@ -52,13 +52,13 @@ struct Connection : public ConnBase, public std::enable_shared_from_this<Connect
     const evevent echoTimer;
 
     bool ready = false;
 
-    // channels to be created on this Connection
+    // channels to be created on this Connection (in state==Connecting
     std::list<std::weak_ptr<Channel>> pending;
 
-    std::map<uint32_t, std::weak_ptr<Channel>> creatingByCID,
-                                               chanBySID;
+    std::map<uint32_t, std::weak_ptr<Channel>> creatingByCID, // in state==Creating
+                                               chanBySID;     // in state==Active
 
     // entries always have matching entry in a Channel::opByIOID
     std::map<uint32_t, RequestInfo> opByIOID;
 
@@ -129,8 +129,9 @@ struct Channel {
     Channel(const std::shared_ptr<Context::Pvt>& context, const std::string& name, uint32_t cid);
     ~Channel();
 
     void createOperations();
+    void disconnect(const std::shared_ptr<Channel>& self);
 
     static
     std::shared_ptr<Channel> build(const std::shared_ptr<Context::Pvt>& context, const std::string &name);
 };

```
