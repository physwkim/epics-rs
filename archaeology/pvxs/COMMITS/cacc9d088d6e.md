# cacc9d088d6e — fix de-serialize of sub-sub-struct

**Date**: 2020-01-31  
**Author**: Michael Davidsaver  
**Severity**: LOW  
**Verdict**: uncertain  

## Changed files
src/dataencode.cpp |  2 +-

## pva-rs mapping
- crates/epics-pva-rs/src/protocol/encode.rs

## Commit message
fix de-serialize of sub-sub-struct

## Key diff (first 100 lines)
```diff
diff --git a/src/dataencode.cpp b/src/dataencode.cpp
index 34212cd..6737100 100644
--- a/src/dataencode.cpp
+++ b/src/dataencode.cpp
@@ -192,7 +192,7 @@ void from_wire(Buffer& buf, std::vector<FieldDesc>& descs, TypeStore& cache, uns
                 if(code.code==TypeCode::Struct && code==cfld.code) {
                     // copy decendent indicies for sub-struct
                     for(auto& pair : cfld.mlookup) {
-                        fld.mlookup[name+pair.first] = cindex + pair.second;
+                        fld.mlookup[name+pair.first] = cindex - cref + pair.second;
                     }
                 }
             }
diff --git a/test/testdata.cpp b/test/testdata.cpp
index f842b13..e1961cc 100644
--- a/test/testdata.cpp
+++ b/test/testdata.cpp
@@ -405,6 +405,27 @@ void testDeserialize2()
     }
 }
 
+void testDeserialize3()
+{
+    testDiag("%s", __func__);
+
+    {
+        TypeStore ctxt;
+        Value val;
+        testFromBytes(false, "\xfd\x02\x00\x80\x00\x01\x06\x72\x65\x63\x6f\x72\x64\xfd\x03\x00\x80\x00"
+                             "\x01\x08\x5f\x6f\x70\x74\x69\x6f\x6e\x73\xfd\x04\x00\x80\x00\x02\x09\x71"
+                             "\x75\x65\x75\x65\x53\x69\x7a\x65\x60\x08\x70\x69\x70\x65\x6c\x69\x6e\x65"
+                             "\x60\x01\x34\x04\x74\x72\x75\x65"
+,
+                      [&val, &ctxt](Buffer& buf) {
+            from_wire_type_value(buf, ctxt, val);
+        });
+        testShow()<<val;
+        testEq(val["record._options.pipeline"].as<std::string>(), "true");
+        testEq(val["record._options.queueSize"].as<std::string>(), "4");
+    }
+}
+
 void testTraverse()
 {
     testDiag("%s", __func__);
@@ -562,12 +583,13 @@ void testPvRequest()
 
 MAIN(testdata)
 {
-    testPlan(79);
+    testPlan(82);
     testSerialize1();
     testDeserialize1();
     testSimpleDef();
     testSerialize2();
     testDeserialize2();
+    testDeserialize3();
     testTraverse();
     testAssign();
     testName();

```
