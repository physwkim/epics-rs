# R157 — abeb78a9cdf8 [LOW][uncertain]

**Subject**: fix TypeDef(const Value& val) for Union/UnionA/StructA  
**Date**: 2023-03-20  
**pvxs SHA**: abeb78a9cdf8  

## pva-rs mapping
- crates/epics-pva-rs/src/types/

## Verdict
**uncertain** — LOW

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit abeb78a9cdf8f316922a35cd03ef30d3ad186ad3
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Mon Mar 20 08:53:10 2023 -0700

    fix TypeDef(const Value& val) for Union/UnionA/StructA
---
 src/type.cpp      | 20 ++++++++++++++------
 test/testtype.cpp | 11 ++++++++++-
 2 files changed, 24 insertions(+), 7 deletions(-)

diff --git a/src/type.cpp b/src/type.cpp
index 783228f..cdbf060 100644
--- a/src/type.cpp
+++ b/src/type.cpp
@@ -296,14 +296,22 @@ TypeDef::TypeDef(std::shared_ptr<const Member>&& temp)
 void Member::Helper::copy_tree(const FieldDesc* desc, Member& node)
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
 
 TypeDef::TypeDef(const Value& val)
diff --git a/test/testtype.cpp b/test/testtype.cpp
index cdc2921..47b9e12 100644
--- a/test/testtype.cpp
+++ b/test/testtype.cpp
@@ -587,13 +587,21 @@ void testFormat()
         "array.choice[2]->two.ahalf int32_t = 2468\n"
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
     testBasic();
@@ -602,7 +610,8 @@ MAIN(testtype)
     testTypeDefAppend();
     testTypeDefAppendIncremental();
     testOp();
     testFormat();
+    testAppendBig();
     cleanup_for_valgrind();
     return testDone();
 }

```
