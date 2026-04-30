# 6fd01c7bec2e — improve error handling

**Date**: 2021-01-07  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
src/serverget.cpp        |  8 +++++-
src/serverintrospect.cpp | 11 ++++++--
src/sharedpv.cpp         | 66 +++++++++++++++++++++++++++++++-----------------

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs (GET handler)
- crates/epics-pva-rs/src/server_native/tcp.rs
- crates/epics-pva-rs/src/server_native/shared_pv.rs

## Commit message
improve error handling

## Key diff (first 100 lines)
```diff
diff --git a/src/serverget.cpp b/src/serverget.cpp
index 66a01ec..ad6679b 100644
--- a/src/serverget.cpp
+++ b/src/serverget.cpp
@@ -415,7 +415,13 @@ void ServerConn::handle_GPR(pva_app_msg_t cmd)
             ctrl->connect(Value());
 
         } else if(chan->onOp) { // GET, PUT
-            chan->onOp(std::move(ctrl));
+            try {
+                chan->onOp(std::move(ctrl));
+            }catch(std::exception& e){
+                // a remote error will be signaled from ~ServerGPRConnect
+                log_err_printf(connsetup, "Client %s op%2x \"%s\" onOp() error: %s\n",
+                               peerName.c_str(), cmd, chan->name.c_str(), e.what());
+            }
 
         } else {
             ctrl->error("Get/Put/RPC not implemented for this PV");
diff --git a/src/serverintrospect.cpp b/src/serverintrospect.cpp
index bc8e2c7..ec3b506 100644
--- a/src/serverintrospect.cpp
+++ b/src/serverintrospect.cpp
@@ -168,8 +168,15 @@ void ServerConn::handle_GET_FIELD()
     opByIOID[ioid] = op;
     chan->opByIOID[ioid] = op;
 
-    if(chan->onOp)
-        chan->onOp(std::move(ctrl));
+    if(chan->onOp) {
+        try {
+            chan->onOp(std::move(ctrl));
+        }catch(std::exception& e){
+            // a remote error will be signaled from ~ServerIntrospectControl
+            log_err_printf(connsetup, "Client %s Info \"%s\" onOp() error: %s\n",
+                           peerName.c_str(), chan->name.c_str(), e.what());
+        }
+    }
 }
 
 }} // namespace pvxs::impl
diff --git a/src/sharedpv.cpp b/src/sharedpv.cpp
index b22a24f..754d02b 100644
--- a/src/sharedpv.cpp
+++ b/src/sharedpv.cpp
@@ -49,6 +49,45 @@ struct SharedPV::Impl : public std::enable_shared_from_this<Impl>
     Value current;
 
     INST_COUNTER(SharedPVImpl);
+
+    static
+    void connectOp(const std::shared_ptr<Impl>& self, const std::shared_ptr<ConnectOp>& conn)
+    {
+        try{
+            conn->connect(self->current);
+        }catch(std::exception& e){
+            log_warn_printf(logshared, "%s Client %s: Can't attach() get: %s\n",
+                            conn->name().c_str(), conn->peerName().c_str(), e.what());
+            // not re-throwing for consistency
+            // we couldn't deliver an error after pending
+            conn->error(e.what());
+        }
+    }
+
+    static
+    void connectSub(const std::shared_ptr<Impl>& self,
+                                                 const std::shared_ptr<MonitorSetupOp>& conn)
+    {
+        try {
+            std::shared_ptr<MonitorControlOp> sub(conn->connect(self->current));
+
+            conn->onClose([self, sub](const std::string& msg) {
+                log_debug_printf(logshared, "%s on %s Monitor onClose\n", sub->peerName().c_str(), sub->name().c_str());
+                Guard G(self->lock);
+                self->subscribers.erase(sub);
+            });
+
+            sub->post(self->current.clone());
+            self->subscribers.emplace(std::move(sub));
+
+        }catch(std::exception& e){
+            log_warn_printf(logshared, "%s Client %s: Can't attach() monitor: %s\n",
+                            conn->name().c_str(), conn->peerName().c_str(), e.what());
+            // not re-throwing for consistency
+            // we couldn't deliver an error after pending
+            conn->error(e.what());
+        }
+    }
 };
 
 SharedPV SharedPV::buildMailbox()
@@ -192,7 +231,7 @@ void SharedPV::attach(std::unique_ptr<ChannelControl>&& ctrlop)
 
         } else {
             UnGuard U(G);
-            conn->connect(self->current);
+            Impl::connectOp(self, conn);
         }
     });
 
```
