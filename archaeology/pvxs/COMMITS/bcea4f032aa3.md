# bcea4f032aa3 — server missing channel onClose

**Date**: 2020-04-10  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
src/serverchan.cpp |  3 +++

## pva-rs mapping
- crates/epics-pva-rs/src/server_native/tcp.rs (channel state)

## Commit message
server missing channel onClose

## Key diff (first 100 lines)
```diff
diff --git a/src/serverchan.cpp b/src/serverchan.cpp
index ab7f1ab..65a3677 100644
--- a/src/serverchan.cpp
+++ b/src/serverchan.cpp
@@ -135,6 +135,9 @@ void ServerChannel_shutdown(const std::shared_ptr<ServerChan>& chan)
     }
 
     chan->opByIOID.clear();
+
+    if(chan->onClose)
+        chan->onClose("");
 }
 
 void ServerChannelControl::close()
diff --git a/test/testget.cpp b/test/testget.cpp
index 089ef62..c22b677 100644
--- a/test/testget.cpp
+++ b/test/testget.cpp
@@ -101,22 +101,25 @@ struct Tester {
         testShow()<<__func__;
 
         std::atomic<bool> onFC{false}, onLD{false};
+        epicsEvent done;
 
         mbox.onFirstConnect([this, &onFC](){
-            testShow()<<__func__;
+            testShow()<<"In onFirstConnect()";
 
             mbox.open(initial);
             onFC.store(true);
         });
-        mbox.onLastDisconnect([this, &onLD](){
-            testShow()<<__func__;
+        mbox.onLastDisconnect([this, &onLD, &done](){
+            testShow()<<"In onLastDisconnect";
             mbox.close();
             onLD.store(true);
+            done.signal();
         });
 
         serv.start();
 
         testWait();
+        testOk1(done.wait(5.0));
 
         serv.stop();
 
@@ -239,7 +242,7 @@ void testError(bool phase)
 
 MAIN(testget)
 {
-    testPlan(14);
+    testPlan(15);
     logger_config_env();
     Tester().testWaiter();
     Tester().loopback();
diff --git a/test/testinfo.cpp b/test/testinfo.cpp
index aa5b9de..d60c249 100644
--- a/test/testinfo.cpp
+++ b/test/testinfo.cpp
@@ -77,22 +77,25 @@ struct Tester {
         testShow()<<__func__;
 
         std::atomic<bool> onFC{false}, onLD{false};
+        epicsEvent done;
 
         mbox.onFirstConnect([this, &onFC](){
-            testShow()<<__func__;
+            testShow()<<"In onFirstConnect()";
 
             mbox.open(initial);
             onFC.store(true);
         });
-        mbox.onLastDisconnect([this, &onLD](){
-            testShow()<<__func__;
+        mbox.onLastDisconnect([this, &onLD, &done](){
+            testShow()<<"In onLastDisconnect";
             mbox.close();
             onLD.store(true);
+            done.signal();
         });
 
         serv.start();
 
         testWait();
+        testOk1(done.wait(5.0));
 
         serv.stop();
 
@@ -200,7 +203,7 @@ void testError()
 
 MAIN(testinfo)
 {
-    testPlan(11);
+    testPlan(12);
     logger_config_env();
     Tester().loopback();
     Tester().lazy();
diff --git a/test/testput.cpp b/test/testput.cpp
index 1971ed3..eaefb32 100644
```
