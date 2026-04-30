# R076 — d10eefac0e4b [MEDIUM][uncertain]

**Subject**: drop unused FieldDesc::hash  
**Date**: 2020-03-09  
**pvxs SHA**: d10eefac0e4b  

## pva-rs mapping
- crates/epics-pva-rs/src/protocol/encode.rs
- crates/epics-pva-rs/src/types/

## Verdict
**uncertain** — MEDIUM

> TODO: manual review — does this bug exist in pva-rs?

## Diff
```diff
commit d10eefac0e4bb4fb7f0ca320a4855efcbf05c113
Author: Michael Davidsaver <mdavidsaver@gmail.com>
Date:   Mon Mar 9 14:48:39 2020 -0700

    drop unused FieldDesc::hash
---
 src/dataencode.cpp | 6 ------
 src/dataimpl.h     | 4 ----
 src/type.cpp       | 4 ----
 3 files changed, 14 deletions(-)

diff --git a/src/dataencode.cpp b/src/dataencode.cpp
index 68516ab..d2013f0 100644
--- a/src/dataencode.cpp
+++ b/src/dataencode.cpp
@@ -132,9 +132,8 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
         {
             auto& fld = descs.back();
 
             fld.code = code;
-            fld.hash = code.code;
         }
 
         switch(code.code) {
         case TypeCode::StructA:
@@ -156,9 +155,8 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
             {
                 auto& fld = descs.back();
 
                 fld.miter.reserve(nfld.size);
-                fld.hash ^= std::hash<std::string>{}(fld.id);
             }
 
             auto& cdescs = code.code==TypeCode::Struct ? descs : descs.back().members;
             auto cref = code.code==TypeCode::Struct ? index : 0u;
@@ -179,12 +177,8 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
                 auto& cfld = cdescs[cindex];
                 if(code.code==TypeCode::Struct)
                     cfld.parent_index = cindex-cref;
 
-                // update hash
-                // TODO investigate better ways to combine hashes
-                fld.hash ^= std::hash<std::string>{}(name) ^ cfld.hash;
-
                 // update field refs.
                 fld.miter.emplace_back(name, cindex-cref);
                 fld.mlookup[name] = cindex-cref;
                 name+='.';
diff --git a/src/dataimpl.h b/src/dataimpl.h
index a52874c..5144983 100644
--- a/src/dataimpl.h
+++ b/src/dataimpl.h
@@ -64,12 +64,8 @@ struct FieldDesc {
 
     // child iteration.  child# -> ("sub", rel index in enclosing vector<FieldDesc>)
     std::vector<std::pair<std::string, size_t>> miter;
 
-    // hash of this type (aggragating from children)
-    // created using the code ^ id ^ (child_name ^ child_hash)*N
-    size_t hash;
-
     // number of FieldDesc nodes between this node and it's a parent Struct (or 0 if no parent).
     // This value also appears in the parent's miter and mlookup mappings.
     // Only usable when a StructTop is accessible and this!=StructTop::desc
     size_t parent_index=0;
diff --git a/src/type.cpp b/src/type.cpp
index 5a07a86..7bfb71d 100644
--- a/src/type.cpp
+++ b/src/type.cpp
@@ -147,9 +147,8 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
         desc.emplace_back();
         auto& fld = desc.back();
         fld.code = node.code;
         // struct/union array have no ID
-        fld.hash = node.code.code;
 
         Member next{code.scalarOf(), node.name};
         next.id = node.id;
         next.children = node.children; // TODO ick copy
@@ -164,9 +163,8 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
     {
         auto& fld = desc.back();
         fld.code = code;
         fld.id = node.id;
-        fld.hash = code.code ^ std::hash<std::string>{}(fld.id);
     }
 
     auto& cdescs = code.code==TypeCode::Struct ? desc : desc.back().members;
     auto cref = code.code==TypeCode::Struct ? index : 0u;
@@ -180,10 +178,8 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
         auto& child = cdescs[cindex];
         if(code.code==TypeCode::Struct)
             child.parent_index = cindex-cref;
 
-        fld.hash ^= std::hash<std::string>{}(cnode.name) ^ child.hash;
-
         fld.mlookup[cnode.name] = cindex-cref;
         fld.miter.emplace_back(cnode.name, cindex-cref);
 
         std::string cname = cnode.name+".";

```
