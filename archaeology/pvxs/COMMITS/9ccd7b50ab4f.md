# 9ccd7b50ab4f — ioc: fix block=true to DBF_ENUM

**Date**: 2023-09-19  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
ioc/singlesource.cpp |  4 ++--

## pva-rs mapping
- (no direct mapping found)

## Commit message
ioc: fix block=true to DBF_ENUM

## Key diff (first 100 lines)
```diff
diff --git a/ioc/singlesource.cpp b/ioc/singlesource.cpp
index 506be5e..66bc0e1 100644
--- a/ioc/singlesource.cpp
+++ b/ioc/singlesource.cpp
@@ -315,8 +315,8 @@ void onOp(const std::shared_ptr<SingleInfo>& sInfo, const Value& valuePrototype,
                         putOperationCache->valueToSet = value;
                         // TODO prevent concurrent put with callbacks (notifyBusy)
 
-                        putOperationCache->notify.requestType = value["value"].isMarked() ? putProcessRequest
-                                                                                          : processRequest;
+                        putOperationCache->notify.requestType = value["value"].isMarked(true, true)
+                                ? putProcessRequest : processRequest;
                         putOperationCache->putOperation = std::move(putOperation);
                         dbProcessNotify(&putOperationCache->notify);
                         return;
diff --git a/test/testqsingle.cpp b/test/testqsingle.cpp
index b692eb7..03e4a72 100644
--- a/test/testqsingle.cpp
+++ b/test/testqsingle.cpp
@@ -675,8 +675,13 @@ void testPutBlock()
 
     testdbGetFieldEqual("test:slowmo", DBR_DOUBLE, 2.0);
 
+    testdbPutFieldOk("test:bo", DBR_LONG, 0);
+    ctxt.put("test:bo").set("value.index", 1).pvRequest("record[block=true]").exec()->wait(5.0);
+
+    testdbGetFieldEqual("test:bo", DBR_LONG, 1);
+
 #else
-    testSkip(7, "dbNotify testing broken on 3.15");
+    testSkip(9, "dbNotify testing broken on 3.15");
     /* epics-base 3.15 circa a249561677de73e3f174ec8e4478937a7a55a9b2
      * contains ddaa6e4eb6647545db7a43c9b83ca7e2c497f3b8
      * but not a7a87372aab2c086f7ac60db4a5d9e39f08b9f05
@@ -770,6 +775,8 @@ void testMonitorBO(TestClient& ctxt)
 {
     testDiag("%s", __func__);
 
+    testdbPutFieldOk("test:bo", DBR_STRING, "One");
+
     TestSubscription sub(ctxt.monitor("test:bo")
                          .maskConnected(true)
                          .maskDisconnected(true));
@@ -782,7 +789,7 @@ void testMonitorBO(TestClient& ctxt)
               "alarm.severity int32_t = 0\n"
               "alarm.status int32_t = 0\n"
               "alarm.message string = \"\"\n"
-              "timeStamp.secondsPastEpoch int64_t = 643497678\n"
+              "timeStamp.secondsPastEpoch int64_t = 643497681\n"
               "timeStamp.nanoseconds int32_t = 102030\n"
               "display.description string = \"\"\n");
 
@@ -873,7 +880,7 @@ void testMonitorAIFilt(TestClient& ctxt)
 
 MAIN(testqsingle)
 {
-    testPlan(84);
+    testPlan(87);
     testSetup();
     pvxs::logger_config_env();
     {

```
