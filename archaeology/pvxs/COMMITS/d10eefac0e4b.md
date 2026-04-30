# d10eefac0e4b — drop unused FieldDesc::hash

**Date**: 2020-03-09  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: uncertain  

## Changed files
src/dataencode.cpp | 6 ------
src/dataimpl.h     | 4 ----
src/type.cpp       | 4 ----

## pva-rs mapping
- crates/epics-pva-rs/src/protocol/encode.rs
- crates/epics-pva-rs/src/types/

## Commit message
drop unused FieldDesc::hash

## Key diff (first 100 lines)
```diff
diff --git a/src/dataencode.cpp b/src/dataencode.cpp
index 68516ab..d2013f0 100644
--- a/src/dataencode.cpp
+++ b/src/dataencode.cpp
@@ -133,7 +133,6 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
             auto& fld = descs.back();
 
             fld.code = code;
-            fld.hash = code.code;
         }
 
         switch(code.code) {
@@ -157,7 +156,6 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
                 auto& fld = descs.back();
 
                 fld.miter.reserve(nfld.size);
-                fld.hash ^= std::hash<std::string>{}(fld.id);
             }
 
             auto& cdescs = code.code==TypeCode::Struct ? descs : descs.back().members;
@@ -180,10 +178,6 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
                 if(code.code==TypeCode::Struct)
                     cfld.parent_index = cindex-cref;
 
-                // update hash
-                // TODO investigate better ways to combine hashes
-                fld.hash ^= std::hash<std::string>{}(name) ^ cfld.hash;
-
                 // update field refs.
                 fld.miter.emplace_back(name, cindex-cref);
                 fld.mlookup[name] = cindex-cref;
diff --git a/src/dataimpl.h b/src/dataimpl.h
index a52874c..5144983 100644
--- a/src/dataimpl.h
+++ b/src/dataimpl.h
@@ -65,10 +65,6 @@ struct FieldDesc {
     // child iteration.  child# -> ("sub", rel index in enclosing vector<FieldDesc>)
     std::vector<std::pair<std::string, size_t>> miter;
 
-    // hash of this type (aggragating from children)
-    // created using the code ^ id ^ (child_name ^ child_hash)*N
-    size_t hash;
-
     // number of FieldDesc nodes between this node and it's a parent Struct (or 0 if no parent).
     // This value also appears in the parent's miter and mlookup mappings.
     // Only usable when a StructTop is accessible and this!=StructTop::desc
diff --git a/src/type.cpp b/src/type.cpp
index 5a07a86..7bfb71d 100644
--- a/src/type.cpp
+++ b/src/type.cpp
@@ -148,7 +148,6 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
         auto& fld = desc.back();
         fld.code = node.code;
         // struct/union array have no ID
-        fld.hash = node.code.code;
 
         Member next{code.scalarOf(), node.name};
         next.id = node.id;
@@ -165,7 +164,6 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
         auto& fld = desc.back();
         fld.code = code;
         fld.id = node.id;
-        fld.hash = code.code ^ std::hash<std::string>{}(fld.id);
     }
 
     auto& cdescs = code.code==TypeCode::Struct ? desc : desc.back().members;
@@ -181,8 +179,6 @@ void build_tree(std::vector<FieldDesc>& desc, const Member& node)
         if(code.code==TypeCode::Struct)
             child.parent_index = cindex-cref;
 
-        fld.hash ^= std::hash<std::string>{}(cnode.name) ^ child.hash;
-
         fld.mlookup[cnode.name] = cindex-cref;
         fld.miter.emplace_back(cnode.name, cindex-cref);
 

```
