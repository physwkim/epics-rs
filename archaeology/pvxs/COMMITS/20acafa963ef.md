# 20acafa963ef — fix Connection ownership

**Date**: 2020-02-29  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: eliminated  

## Changed files
src/clientconn.cpp | 5 +++++
src/clientimpl.h   | 3 ++-
src/conn.cpp       | 6 +++---
src/conn.h         | 1 +
src/serverconn.cpp | 4 ++++
src/serverconn.h   | 1 +

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs
- crates/epics-pva-rs/src/server_native/tcp.rs

## Commit message
fix Connection ownership

## Key diff (first 100 lines)
```diff
diff --git a/src/clientconn.cpp b/src/clientconn.cpp
index eeb8552..8ff92cf 100644
--- a/src/clientconn.cpp
+++ b/src/clientconn.cpp
@@ -105,6 +105,11 @@ void Connection::bevEvent(short events)
     }
 }
 
+std::shared_ptr<ConnBase> Connection::self_from_this()
+{
+    return shared_from_this();
+}
+
 void Connection::cleanup()
 {
     // (maybe) keep myself alive
diff --git a/src/clientimpl.h b/src/clientimpl.h
index 9df3501..1331ab5 100644
--- a/src/clientimpl.h
+++ b/src/clientimpl.h
@@ -46,7 +46,7 @@ struct RequestInfo {
     RequestInfo(uint32_t sid, uint32_t ioid, std::shared_ptr<OperationBase>& handle);
 };
 
-struct Connection : public ConnBase {
+struct Connection : public ConnBase, public std::enable_shared_from_this<Connection> {
     const std::shared_ptr<Context::Pvt> context;
 
     const evevent echoTimer;
@@ -72,6 +72,7 @@ struct Connection : public ConnBase {
 
     virtual void bevEvent(short events) override final;
 
+    virtual std::shared_ptr<ConnBase> self_from_this() override final;
     virtual void cleanup() override final;
 
 #define CASE(Op) virtual void handle_##Op() override final;
diff --git a/src/conn.cpp b/src/conn.cpp
index 5d29d12..0441e7c 100644
--- a/src/conn.cpp
+++ b/src/conn.cpp
@@ -215,7 +215,7 @@ void ConnBase::bevWrite() {}
 
 void ConnBase::bevEventS(struct bufferevent *bev, short events, void *ptr)
 {
-    auto conn = static_cast<ConnBase*>(ptr);
+    auto conn = static_cast<ConnBase*>(ptr)->self_from_this();
     try {
         conn->bevEvent(events);
     }catch(std::exception& e){
@@ -226,7 +226,7 @@ void ConnBase::bevEventS(struct bufferevent *bev, short events, void *ptr)
 
 void ConnBase::bevReadS(struct bufferevent *bev, void *ptr)
 {
-    auto conn = static_cast<ConnBase*>(ptr);
+    auto conn = static_cast<ConnBase*>(ptr)->self_from_this();
     try {
         conn->bevRead();
     }catch(std::exception& e){
@@ -237,7 +237,7 @@ void ConnBase::bevReadS(struct bufferevent *bev, void *ptr)
 
 void ConnBase::bevWriteS(struct bufferevent *bev, void *ptr)
 {
-    auto conn = static_cast<ConnBase*>(ptr);
+    auto conn = static_cast<ConnBase*>(ptr)->self_from_this();
     try {
         conn->bevWrite();
     }catch(std::exception& e){
diff --git a/src/conn.h b/src/conn.h
index 8b6ade9..e01fda7 100644
--- a/src/conn.h
+++ b/src/conn.h
@@ -64,6 +64,7 @@ protected:
     CASE(MESSAGE);
 #undef CASE
 
+    virtual std::shared_ptr<ConnBase> self_from_this() =0;
     virtual void cleanup() =0;
     virtual void bevEvent(short events);
     virtual void bevRead();
diff --git a/src/serverconn.cpp b/src/serverconn.cpp
index fa43aa9..9bfafeb 100644
--- a/src/serverconn.cpp
+++ b/src/serverconn.cpp
@@ -271,6 +271,10 @@ void ServerConn::handle_MESSAGE()
                      msg.c_str());
 }
 
+std::shared_ptr<ConnBase> ServerConn::self_from_this()
+{
+    return shared_from_this();
+}
 
 void ServerConn::cleanup()
 {
diff --git a/src/serverconn.h b/src/serverconn.h
index 9bc8808..3cde70e 100644
--- a/src/serverconn.h
+++ b/src/serverconn.h
@@ -138,6 +138,7 @@ private:
```
