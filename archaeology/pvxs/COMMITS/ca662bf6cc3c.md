# ca662bf6cc3c — fixup data decode

**Date**: 2019-12-14  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: applies  

## Changed files
src/dataencode.cpp |  16 +++--

## pva-rs mapping
- crates/epics-pva-rs/src/protocol/encode.rs

## Commit message
fixup data decode

## Key diff (first 100 lines)
```diff
diff --git a/src/dataencode.cpp b/src/dataencode.cpp
index 3589528..bfa0d31 100644
--- a/src/dataencode.cpp
+++ b/src/dataencode.cpp
@@ -242,7 +242,7 @@ void to_wire_field(Buffer& buf, const FieldDesc* desc, const std::shared_ptr<con
             // serialize entire sub-structure
             for(auto off : range(desc->offset+1u, desc->next_offset)) {
                 auto cdesc = desc + top.member_indicies[off];
-                std::shared_ptr<const FieldStorage> cstore(store, store.get()+off);
+                std::shared_ptr<const FieldStorage> cstore(store, store.get()+off); // TODO avoid shared_ptr/aliasing here
                 if(cdesc->code!=TypeCode::Struct)
                     to_wire_field(buf, cdesc, cstore);
             }
@@ -454,7 +454,7 @@ void from_wire_field(Buffer& buf, TypeStore& ctxt,  const FieldDesc* desc, const
             // serialize entire sub-structure
             for(auto off : range(desc->offset+1u, desc->next_offset)) {
                 auto cdesc = desc + top.member_indicies[off];
-                std::shared_ptr<FieldStorage> cstore(store, store.get()+off);
+                std::shared_ptr<FieldStorage> cstore(store, store.get()+off); // TODO avoid shared_ptr/aliasing here
                 if(cdesc->code!=TypeCode::Struct)
                     from_wire_field(buf, ctxt, cdesc, cstore);
             }
@@ -529,12 +529,16 @@ void from_wire_field(Buffer& buf, TypeStore& ctxt,  const FieldDesc* desc, const
             TypeDeserContext dc{*descs, ctxt};
 
             from_wire(buf, dc);
+            if(!buf.good())
+                return;
 
             if(descs->empty()) {
                 fld = Value();
                 return;
 
             } else {
+                FieldDesc_calculate_offset(descs->data());
+
                 std::shared_ptr<const FieldDesc> stype(descs, descs->data()); // alias
                 fld = Value::Helper::build(stype);
 
@@ -613,12 +617,11 @@ void from_wire_field(Buffer& buf, TypeStore& ctxt,  const FieldDesc* desc, const
                         elem = Value::Helper::build(stype, store, desc);
 
                         from_wire_full(buf, ctxt, elem);
-                        return;
 
                     } else {
                         // invalid selector
                         buf.fault();
-                        break;
+                        return;
                     }
                 }
             }
@@ -637,7 +640,12 @@ void from_wire_field(Buffer& buf, TypeStore& ctxt,  const FieldDesc* desc, const
                     TypeDeserContext dc{*descs, ctxt};
 
                     from_wire(buf, dc);
+                    if(!buf.good())
+                        return;
+
                     if(!descs->empty()) {
+                        FieldDesc_calculate_offset(descs->data());
+
                         std::shared_ptr<const FieldDesc> stype(descs, descs->data()); // alias
                         elem = Value::Helper::build(stype, store, desc);
 
diff --git a/test/testdata.cpp b/test/testdata.cpp
index 8879a45..699f2b7 100644
--- a/test/testdata.cpp
+++ b/test/testdata.cpp
@@ -42,6 +42,15 @@ void testToBytes(bool be, Fn&& fn, const char(&expect)[N])
     testBytes(buf, expect);
 }
 
+template<typename Fn, size_t N>
+void testFromBytes(bool be, const char(&input)[N], Fn&& fn)
+{
+    std::vector<uint8_t> buf(input, input+N-1);
+    FixedBuf S(be, buf);
+    fn(S);
+    testCase(S.good() && S.empty())<<"Deserialize \""<<escape(std::string((const char*)input, N-1))<<"\" leaves "<<S.good()<<" "<<S.size();
+}
+
 void testSerialize1()
 {
     testDiag("%s", __func__);
@@ -76,35 +85,87 @@ void testSerialize1()
     }, "\x02 \x01\x0bhello world\x00\x00\x00\xab");
 }
 
-void testSerialize2()
+void testDeserialize1()
 {
     testDiag("%s", __func__);
 
-    TypeDef def(TypeCode::Struct, "simple_t", {
-                    Member(TypeCode::Float64A, "value"),
-                    Member(TypeCode::Struct, "timeStamp", "time_t", {
-                        Member(TypeCode::UInt64, "secondsPastEpoch"),
-                        Member(TypeCode::UInt32, "nanoseconds"),
```
