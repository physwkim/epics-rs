# 5ddc2beb475f — server monitor throttle using send queue size

**Date**: 2020-01-26  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
src/serverconn.cpp | 21 +++++++++++++++++++--
src/serverconn.h   |  2 ++
src/servermon.cpp  | 17 ++++++++++++++++-

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs
- crates/epics-pva-rs/src/server_native/tcp.rs (MONITOR handler)

## Commit message
server monitor throttle using send queue size

## Key diff (first 100 lines)
```diff
diff --git a/src/serverconn.cpp b/src/serverconn.cpp
index 96aa6d2..7d65ecd 100644
--- a/src/serverconn.cpp
+++ b/src/serverconn.cpp
@@ -448,14 +448,31 @@ void ServerConn::bevRead()
             // TODO configure
             (void)bufferevent_disable(bev.get(), EV_READ);
             bufferevent_setwatermark(bev.get(), EV_WRITE, 0x100000/2, 0);
+            log_printf(connio, Debug, "%s suspend READ\n", peerName.c_str());
         }
     }
 }
 
 void ServerConn::bevWrite()
 {
-    (void)bufferevent_enable(bev.get(), EV_READ);
-    bufferevent_setwatermark(bev.get(), EV_WRITE, 0, 0);
+    log_printf(connio, Debug, "%s process backlog\n", peerName.c_str());
+
+    auto tx = bufferevent_get_output(bev.get());
+    // handle pending monitors
+
+    while(!backlog.empty() && evbuffer_get_length(tx)<0x100000) {
+        auto fn = std::move(backlog.front());
+        backlog.pop_front();
+
+        fn();
+    }
+
+    // TODO configure
+    if(evbuffer_get_length(tx)<0x100000) {
+        (void)bufferevent_enable(bev.get(), EV_READ);
+        bufferevent_setwatermark(bev.get(), EV_WRITE, 0, 0);
+        log_printf(connio, Debug, "%s resume READ\n", peerName.c_str());
+    }
 }
 
 void ServerConn::bevEventS(struct bufferevent *bev, short events, void *ptr)
diff --git a/src/serverconn.h b/src/serverconn.h
index a1b1b61..ba38b0a 100644
--- a/src/serverconn.h
+++ b/src/serverconn.h
@@ -113,6 +113,8 @@ struct ServerConn : public std::enable_shared_from_this<ServerConn>
     std::map<uint32_t, std::shared_ptr<ServerChan> > chanByCID;
     std::map<uint32_t, std::shared_ptr<ServerOp> > opByIOID;
 
+    std::list<std::function<void()>> backlog;
+
     ServerConn(ServIface* iface, evutil_socket_t sock, struct sockaddr *peer, int socklen);
     ServerConn(const ServerConn&) = delete;
     ServerConn& operator=(const ServerConn&) = delete;
diff --git a/src/servermon.cpp b/src/servermon.cpp
index 90e53a8..4a56a07 100644
--- a/src/servermon.cpp
+++ b/src/servermon.cpp
@@ -59,11 +59,26 @@ struct MonitorOp : public ServerOp,
     static
     void maybeReply(server::Server::Pvt* server, const std::shared_ptr<MonitorOp>& op)
     {
+        // can we send a reply?
         if(!op->scheduled && op->state==Executing && !op->queue.empty() && (!op->pipeline || op->window))
         {
+            // based on operation state, yes
             server->acceptor_loop.dispatch([op](){
-                op->doReply();
+                auto ch(op->chan.lock());
+                if(!ch)
+                    return;
+                auto conn(ch->conn.lock());
+                if(!conn)
+                    return;
+
+                if(conn->bev && (bufferevent_get_enabled(conn->bev.get())&EV_READ)) {
+                    op->doReply();
+                } else {
+                    // connection TX queue is too full
+                    conn->backlog.push_back(std::bind(&MonitorOp::doReply, op));
+                }
             });
+
             op->scheduled = true;
         }
     }

```
