# adab53e5c5fd — client: error on empty PV name

**Date**: 2025-10-01  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
src/clientget.cpp        | 3 +++
src/clientintrospect.cpp | 2 ++
src/clientmon.cpp        | 2 ++

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/{get,put,rpc}.rs
- crates/epics-pva-rs/src/client_native/introspect.rs
- crates/epics-pva-rs/src/client_native/monitor.rs

## Commit message
client: error on empty PV name

## Key diff (first 100 lines)
```diff
diff --git a/src/clientget.cpp b/src/clientget.cpp
index 858f636..7922427 100644
--- a/src/clientget.cpp
+++ b/src/clientget.cpp
@@ -582,6 +582,9 @@ std::shared_ptr<Operation> gpr_setup(const std::shared_ptr<ContextImpl>& context
                                      std::shared_ptr<GPROp>&& op,
                                      bool syncCancel)
 {
+    if(name.empty())
+        throw std::logic_error("Empty channel name");
+
     auto internal(std::move(op));
     internal->internal_self = internal;
 
diff --git a/src/clientintrospect.cpp b/src/clientintrospect.cpp
index 5f1f376..9ce651d 100644
--- a/src/clientintrospect.cpp
+++ b/src/clientintrospect.cpp
@@ -184,6 +184,8 @@ std::shared_ptr<Operation> GetBuilder::_exec_info()
         throw std::logic_error("NULL Builder");
     if(!_autoexec)
         throw std::logic_error("autoExec(false) not possible for info()");
+    if(_name.empty())
+        throw std::logic_error("Empty channel name");
 
     auto context(ctx->impl->shared_from_this());
 
diff --git a/src/clientmon.cpp b/src/clientmon.cpp
index 00810ba..5601371 100644
--- a/src/clientmon.cpp
+++ b/src/clientmon.cpp
@@ -739,6 +739,8 @@ std::shared_ptr<Subscription> MonitorBuilder::exec()
 {
     if(!ctx)
         throw std::logic_error("NULL Builder");
+    if(_name.empty())
+        throw std::logic_error("Empty channel name");
 
     auto context(ctx->impl->shared_from_this());
 

```
