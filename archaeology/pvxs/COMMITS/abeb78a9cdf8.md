# abeb78a9cdf8 — fix TypeDef(const Value& val) for Union/UnionA/StructA

**Date**: 2023-03-20  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: uncertain  

## Changed files
src/type.cpp      | 20 ++++++++++++++------

## pva-rs mapping
- crates/epics-pva-rs/src/types/

## Commit message
fix TypeDef(const Value& val) for Union/UnionA/StructA

## Key diff (first 100 lines)
```diff
diff --git a/src/type.cpp b/src/type.cpp
index 783228f..cdbf060 100644
--- a/src/type.cpp
+++ b/src/type.cpp
@@ -297,12 +297,20 @@ void Member::Helper::copy_tree(const FieldDesc* desc, Member& node)
 {
     node.code = desc->code;
     node.id = desc->id;
-    node.children.reserve(desc->miter.size());
-    for(auto& pair : desc->miter) {
-        auto cdesc = desc+pair.second;
-        node.children.emplace_back(cdesc->code, pair.first);
-        node.children.back().id = cdesc->id;
-        copy_tree(cdesc, node.children.back());
+    if(desc->code==TypeCode::Struct || desc->code==TypeCode::Union) {
+        auto cbase = desc->code==TypeCode::Struct ? desc : desc->members.data();
+        node.children.reserve(desc->miter.size());
+        for(auto& pair : desc->miter) {
+            auto cdesc = cbase+pair.second;
+            assert(desc!=cdesc);
+            node.children.emplace_back(cdesc->code, pair.first);
+            node.children.back().id = cdesc->id;
+            copy_tree(cdesc, node.children.back());
+        }
+
+    } else if(desc->code==TypeCode::StructA || desc->code==TypeCode::UnionA) {
+        copy_tree(&desc->members[0], node);
+        node.code = node.code.arrayOf();
     }
 }
 
diff --git a/test/testtype.cpp b/test/testtype.cpp
index cdc2921..47b9e12 100644
--- a/test/testtype.cpp
+++ b/test/testtype.cpp
@@ -588,11 +588,19 @@ void testFormat()
     );
 }
 
+void testAppendBig()
+{
+    auto orig(neckBolt());
+    TypeDef append(orig);
+    auto copy(append.create());
+    testTrue(orig.equalType(copy));
+}
+
 } // namespace
 
 MAIN(testtype)
 {
-    testPlan(68);
+    testPlan(69);
     testSetup();
     showSize();
     testCode();
@@ -603,6 +611,7 @@ MAIN(testtype)
     testTypeDefAppendIncremental();
     testOp();
     testFormat();
+    testAppendBig();
     cleanup_for_valgrind();
     return testDone();
 }

```
