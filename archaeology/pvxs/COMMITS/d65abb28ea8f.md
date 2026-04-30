# d65abb28ea8f — shared_array fix print of char[]

**Date**: 2020-04-17  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: uncertain  

## Changed files
src/sharedarray.cpp | 10 +++++++++-

## pva-rs mapping
- crates/epics-pva-rs/src/protocol/field_desc.rs

## Commit message
shared_array fix print of char[]

## Key diff (first 100 lines)
```diff
diff --git a/src/sharedarray.cpp b/src/sharedarray.cpp
index 7bc693b..7837a21 100644
--- a/src/sharedarray.cpp
+++ b/src/sharedarray.cpp
@@ -65,6 +65,14 @@ size_t elementSize(ArrayType type)
 namespace detail {
 
 namespace {
+
+template<typename E>
+struct Print { static inline const E& as(const E& val) { return val; } };
+template<>
+struct Print<int8_t> { static inline int as(int8_t val) { return val; } };
+template<>
+struct Print<uint8_t> { static inline unsigned as(uint8_t val) { return val; } };
+
 template<typename E>
 void showArr(std::ostream& strm, const void* raw, size_t count, size_t limit)
 {
@@ -81,7 +89,7 @@ void showArr(std::ostream& strm, const void* raw, size_t count, size_t limit)
             strm<<"...";
             break;
         }
-        strm<<base[i];
+        strm<<Print<E>::as(base[i]);
     }
     strm<<']';
 }

```
