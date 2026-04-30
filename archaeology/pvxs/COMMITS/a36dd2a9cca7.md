# a36dd2a9cca7 — fix monitor pipeline and finish()

**Date**: 2023-05-10  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
src/clientmon.cpp    |  36 ++++++------
src/pvxs/source.h    |   2 +-
src/servermon.cpp    |  91 ++++++++++++++++++++++------

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/monitor.rs
- crates/epics-pva-rs/src/server_native/source.rs
- crates/epics-pva-rs/src/server_native/tcp.rs (MONITOR handler)

## Commit message
fix monitor pipeline and finish()

## Key diff (first 100 lines)
```diff
diff --git a/src/clientmon.cpp b/src/clientmon.cpp
index 6d6d4ec..dd0f782 100644
--- a/src/clientmon.cpp
+++ b/src/clientmon.cpp
@@ -170,10 +170,7 @@ struct SubscriptionImpl final : public OperationBase, public Subscription
                 if(pipeline) {
                     timeval tick{}; // immediate ACK
 
-                    // schedule delayed ack while below threshold.
-                    // avoid overhead of re-scheduling when unack in range [1, ackAt)
-                    if(unack==0u && ackAt!=1u)
-                        tick = timeval{1,0};
+                    unack++;
 
                     if(!ackPending && unack>=ackAt) {
                         if(event_add(ackTick.get(), &tick)) {
@@ -184,13 +181,12 @@ struct SubscriptionImpl final : public OperationBase, public Subscription
                             ackPending = true;
                         }
                     }
-
-                    unack++;
                 }
-                log_info_printf(monevt, "channel '%s' monitor pop() %s %u,%u\n",
-                                channelName.c_str(),
-                                ent.exc ? "exception" : ent.val ? "data" : "null!",
-                                unsigned(window), unsigned(unack));
+                log_printf(monevt, ent.exc || ent.val ? Level::Info : Level::Err,
+                           "channel '%s' monitor pop() %s %u,%u\n",
+                           channelName.c_str(),
+                           ent.exc ? "exception" : ent.val ? "data" : "null!",
+                           unsigned(window), unsigned(unack));
 
                 if(ent.exc)
                     std::rethrow_exception(ent.exc);
@@ -722,20 +718,22 @@ void Connection::handle_MONITOR()
 
             notify = mon->queue.empty();
 
-            if(update.exc || (mon->queue.size() < mon->queueSize) || mon->queue.back().exc) {
+            assert(mon->queueSize >= 1u);
+            if(update.val && mon->queue.size() >= mon->queueSize && mon->queue.back().val && !mon->pipeline) {
+                log_debug_printf(io, "Server %s channel %s monitor Squash\n",
+                                 peerName.c_str(),
+                                 mon->chan->name.c_str());
+
+                mon->queue.back().val.assign(update.val);
+                mon->nCliSquash++;
+
+            } else if(update.exc || update.val) {
                 log_debug_printf(io, "Server %s channel %s monitor PUSH\n",
                                 peerName.c_str(),
                                 mon->chan->name.c_str());
 
                 mon->queue.emplace_back(std::move(update));
 
-            } else if(update.val) {
-                log_debug_printf(io, "Server %s channel %s monitor Squash\n",
-                                peerName.c_str(),
-                                mon->chan->name.c_str());
-
-                mon->queue.back().val.assign(update.val);
-                mon->nCliSquash++;
             }
 
             if(final && !update.exc) {
@@ -812,7 +810,7 @@ std::shared_ptr<Subscription> MonitorBuilder::exec()
         auto sval = ackAny.as<std::string>();
         if(sval.size()>1 && sval.back()=='%') {
             try {
-                auto percent = parseTo<double>(sval);
+                auto percent = parseTo<double>(sval.substr(0, sval.size()-1u));
                 if(percent>0.0 && percent<=100.0) {
                     op->ackAt = uint32_t(percent * op->queueSize);
                 } else {
diff --git a/src/pvxs/source.h b/src/pvxs/source.h
index 197214e..06f09e2 100644
--- a/src/pvxs/source.h
+++ b/src/pvxs/source.h
@@ -106,7 +106,7 @@ public:
     //! Signal to subscriber that this subscription will not yield any further events.
     //! This is not an error.  Client should not retry.
     void finish() {
-        doPost(Value(), false, false);
+        doPost(Value(), false, true);
     }
 
     //! Poll information and statistics for this subscription.
diff --git a/src/servermon.cpp b/src/servermon.cpp
index ed1eae4..7a7c522 100644
--- a/src/servermon.cpp
+++ b/src/servermon.cpp
@@ -61,9 +61,11 @@ struct MonitorOp : public ServerOp,
     // is doReply() scheduled to run
     bool scheduled=false;
     bool pipeline=false;
+    // finish() called
     bool finished=false;
     size_t window=0u, limit=4u;
     size_t low=0u, high=0u;
```
