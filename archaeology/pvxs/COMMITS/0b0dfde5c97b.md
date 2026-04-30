# 0b0dfde5c97b — ioc: group put w/o effect is an error.

**Date**: 2023-09-18  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: eliminated  

## Changed files
ioc/groupsource.cpp | 23 ++++++++++++++++-------

## pva-rs mapping
- (no direct mapping found)

## Commit message
ioc: group put w/o effect is an error.

## Key diff (first 100 lines)
```diff
diff --git a/ioc/groupsource.cpp b/ioc/groupsource.cpp
index 438e62d..fa7771b 100644
--- a/ioc/groupsource.cpp
+++ b/ioc/groupsource.cpp
@@ -485,7 +485,7 @@ void GroupSource::get(Group& group, const std::unique_ptr<server::ExecOp>& getOp
  * @param securityClient the security client to use to authorise the operation
  */
 static
-void putGroupField(const Value& value,
+bool putGroupField(const Value& value,
                    const Field& field,
                    const SecurityClient& securityClient,
                    const GroupSecurityCache& groupSecurityCache) {
@@ -501,7 +501,9 @@ void putGroupField(const Value& value,
     if (marked || field.info.type==MappingInfo::Proc) {
         // Do processing if required
         IOCSource::doPostProcessing(field.value, groupSecurityCache.forceProcessing);
+        return true;
     }
+    return false;
 }
 
 /**
@@ -542,6 +544,8 @@ void GroupSource::putGroup(Group& group, std::unique_ptr<server::ExecOp>& putOpe
         // Reset index for subsequent loops
         fieldIndex = 0;
 
+        bool didSomething = false;
+
         // If the group is configured for an atomic put operation,
         // then we need to put all the fields at once, so we lock them all together
         // and do the operation in one go
@@ -551,9 +555,9 @@ void GroupSource::putGroup(Group& group, std::unique_ptr<server::ExecOp>& putOpe
             // Loop through all fields
             for (auto& field: group.fields) {
                 // Put the field
-                putGroupField(value, field,
-                              groupSecurityCache.securityClients[fieldIndex],
-                              groupSecurityCache);
+                didSomething |= putGroupField(value, field,
+                                              groupSecurityCache.securityClients[fieldIndex],
+                                              groupSecurityCache);
                 fieldIndex++;
             }
 
@@ -571,14 +575,19 @@ void GroupSource::putGroup(Group& group, std::unique_ptr<server::ExecOp>& putOpe
                 // Lock this field
                 DBLocker F(pDbChannel->addr.precord);
                 // Put the field
-                putGroupField(value, field,
-                              groupSecurityCache.securityClients[fieldIndex],
-                              groupSecurityCache);
+                didSomething |= putGroupField(value, field,
+                                              groupSecurityCache.securityClients[fieldIndex],
+                                              groupSecurityCache);
                 fieldIndex++;
                 // Unlock this field when locker goes out of scope
             }
         }
 
+        if(!didSomething && value.isMarked(true, true)) {
+            // not fields actually changed, but client intended to change something.
+            throw std::runtime_error("No fields changed");
+        }
+
     } catch (std::exception& e) {
         log_debug_printf(_logname, "%s %s remote error: %s\n",
                          __func__, group.name.c_str(), e.what());
diff --git a/test/testqgroup.cpp b/test/testqgroup.cpp
index 6729fe6..a04ea6a 100644
--- a/test/testqgroup.cpp
+++ b/test/testqgroup.cpp
@@ -220,6 +220,29 @@ void testEnum()
     testStrEq(std::string(SB()<<val.format().delta()),
               "value.index int32_t = 0\n");
 
+    testDiag("attempt to write unwritable choices list");
+    {
+        testThrows<client::RemoteError>([&ctxt]{
+            shared_array<const std::string> choices({"foo"});
+            ctxt.put("enm:ENUM").set("value.choices", choices).exec()->wait(5.0);
+        });
+        const char expect[2][MAX_STRING_SIZE] = {"ZERO", "ONE"};
+        testdbGetArrFieldEqual("enm:ENUM:CHOICES", DBR_STRING, 2, 2, expect);
+    }
+
+    testDiag("attempt to write both index and choices list");
+    {
+        shared_array<const std::string> choices({"foo"});
+        ctxt.put("enm:ENUM")
+                .record("process", false) // no update posted
+                .set("value.index", 1)
+                .set("value.choices", choices)
+                .exec()->wait(5.0);
+        const char expect[2][MAX_STRING_SIZE] = {"ZERO", "ONE"};
+        testdbGetArrFieldEqual("enm:ENUM:CHOICES", DBR_STRING, 2, 2, expect);
+        testdbGetFieldEqual("enm:ENUM:INDEX", DBR_LONG, 1);
+    }
+
     sub.testEmpty();
```
