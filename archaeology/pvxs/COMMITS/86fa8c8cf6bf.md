# 86fa8c8cf6bf — fix usage/example of Subscription::pop()

**Date**: 2020-10-19  
**Author**: Michael Davidsaver  
**Severity**: HIGH  
**Verdict**: applies  

## Changed files
src/pvxs/client.h        | 34 +++++++++++++++++++++++++++-------

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/tcp.rs

## Commit message
fix usage/example of Subscription::pop()

## Key diff (first 100 lines)
```diff
diff --git a/documentation/client.rst b/documentation/client.rst
index a20308b..ef4ed84 100644
--- a/documentation/client.rst
+++ b/documentation/client.rst
@@ -45,6 +45,8 @@ effected named arguments.
 .. doxygenclass:: pvxs::client::Context
     :members:
 
+.. _clientgetapi:
+
 Get/Info
 ^^^^^^^^
 
@@ -56,6 +58,8 @@ which will never have any fields marked.
 .. doxygenclass:: pvxs::client::GetBuilder
     :members:
 
+.. _clientputapi:
+
 Put
 ^^^
 
@@ -74,6 +78,8 @@ to an NTEnum.
 .. doxygenclass:: pvxs::client::PutBuilder
     :members:
 
+.. _clientrpcapi:
+
 RPC
 ^^^
 
@@ -112,6 +118,8 @@ if the operation succeeded, or an exception.
 .. doxygenclass:: pvxs::client::Result
     :members:
 
+.. _clientmonapi:
+
 Monitor
 ^^^^^^^
 
@@ -128,8 +136,13 @@ The `pvxs::client::Subscription::pop` method will remove an entry from the queue
 Data updates are returned as a valid Value.
 Events/errors are thrown as exceptions.
 
+An `pvxs::client::MonitorBuilder::event` callback is only invoked when the
+Subscription queue becomes not-empty.
+It will not be called again until `pvxs::client::Subscription::pop` has returned
+an empty/invliad Value.
+
 The special exceptions `pvxs::client::Connected`, `pvxs::client::Disconnect`, and `pvxs::client::Finished`
-have specific meaning for a Subscription.
+have specific meaning when thrown by `pvxs::client::Subscription::pop`.
 
 Connected
     Depending on `pvxs::client::MonitorBuilder::maskConnected` (default true).
diff --git a/documentation/index.rst b/documentation/index.rst
index 08bad96..b0b1e1b 100644
--- a/documentation/index.rst
+++ b/documentation/index.rst
@@ -33,7 +33,7 @@ Status
 This module is considered feature complete, but is not yet making releases.
 
 .. toctree::
-   :maxdepth: 2
+   :maxdepth: 3
    :caption: Contents:
 
    overview
diff --git a/src/pvxs/client.h b/src/pvxs/client.h
index 2c4b165..59c2c08 100644
--- a/src/pvxs/client.h
+++ b/src/pvxs/client.h
@@ -235,6 +235,7 @@ public:
      *               .exec();
      * // store op until completion
      * @endcode
+     * See <a href="#get-info">Get</a> for details.
      */
     inline
     GetBuilder get(const std::string& pvname);
@@ -262,6 +263,8 @@ public:
      *               .exec();
      * // store op until completion
      * @endcode
+     *
+     * See <a href="#get-info">Info</a> for details.
      */
     inline
     GetBuilder info(const std::string& pvname);
@@ -301,6 +304,8 @@ public:
      *               .exec();
      * // store op until completion
      * @endcode
+     *
+     * See <a href="#put">Put</a> for details.
      */
     inline
     PutBuilder put(const std::string& pvname);
@@ -334,6 +339,8 @@ public:
      *               .exec();
```
