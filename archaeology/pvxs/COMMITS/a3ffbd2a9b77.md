# a3ffbd2a9b77 — client fix Channel reconnect

**Date**: 2020-03-02  
**Author**: Michael Davidsaver  
**Severity**: HIGH  
**Verdict**: applies  

## Changed files
src/client.cpp     | 11 +++++++++++
src/clientconn.cpp | 19 ++++++++++++++-----
src/clientimpl.h   |  7 ++++---

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
client fix Channel reconnect

## Key diff (first 100 lines)
```diff
diff --git a/src/client.cpp b/src/client.cpp
index 4b22858..6f3112c 100644
--- a/src/client.cpp
+++ b/src/client.cpp
@@ -94,6 +94,17 @@ void Channel::createOperations()
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
diff --git a/src/clientconn.cpp b/src/clientconn.cpp
index 8ad224a..a588b9e 100644
--- a/src/clientconn.cpp
+++ b/src/clientconn.cpp
@@ -128,12 +128,21 @@ void Connection::cleanup()
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
diff --git a/src/clientimpl.h b/src/clientimpl.h
index 6bb0e1a..36fe437 100644
--- a/src/clientimpl.h
+++ b/src/clientimpl.h
@@ -53,11 +53,11 @@ struct Connection : public ConnBase, public std::enable_shared_from_this<Connect
 
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
@@ -130,6 +130,7 @@ struct Channel {
     ~Channel();
 
     void createOperations();
+    void disconnect(const std::shared_ptr<Channel>& self);
 
     static
     std::shared_ptr<Channel> build(const std::shared_ptr<Context::Pvt>& context, const std::string &name);

```
