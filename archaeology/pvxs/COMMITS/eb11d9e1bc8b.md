# eb11d9e1bc8b — Fix registering functions with EPICS

**Date**: 2020-05-01  
**Author**: Vodopivec, Klemen  
**Severity**: LOW  
**Verdict**: applies  

## Changed files
ioc/iochooks.cpp | 50 ++++++++++++++++++++++++++------------------------

## pva-rs mapping
- (no direct mapping found)

## Commit message
Fix registering functions with EPICS

## Key diff (first 100 lines)
```diff
diff --git a/ioc/iochooks.cpp b/ioc/iochooks.cpp
index ae1a4e6..35dc5cd 100644
--- a/ioc/iochooks.cpp
+++ b/ioc/iochooks.cpp
@@ -69,32 +69,33 @@ void pvxsr(int detail)
     }
 }
 
-template <size_t... Ns>
-struct index_sequence {};
+// index_sequence from:
+//http://stackoverflow.com/questions/17424477/implementation-c14-make-integer-sequence
+
+template< std::size_t ... I >
+struct index_sequence {
+    using type = index_sequence;
+    using value_type = std::size_t;
+    static constexpr std::size_t size() {
+        return sizeof ... (I);
+    }
+};
 
-template<typename Tag>
-struct next_index_sequence {};
+template< typename Seq1, typename Seq2 >
+struct concat_sequence;
 
-template<size_t... Ns>
-struct next_index_sequence<index_sequence<Ns...>>
-{
-    typedef index_sequence<Ns..., sizeof...(Ns)> type;
-};
+template< std::size_t ... I1, std::size_t ... I2 >
+struct concat_sequence< index_sequence< I1 ... >, index_sequence< I2 ... > > : public index_sequence< I1 ..., (sizeof ... (I1)+I2) ... > {};
 
-template<size_t I, size_t Cnt, size_t... Idxs>
-struct build_index_sequence
-{
-    typedef typename build_index_sequence<I+1, Cnt, Idxs..., I+1>::type type;
-};
+template< std::size_t I >
+struct make_index_sequence : public concat_sequence< typename make_index_sequence< I/2 >::type,
+                                                     typename make_index_sequence< I-I/2 >::type > {};
 
-template<size_t Cnt, size_t... Idxs>
-struct build_index_sequence<Cnt, Cnt, Idxs...>
-{
-    typedef index_sequence<Idxs...> type;
-};
+template<>
+struct make_index_sequence< 0 > : public index_sequence<> {};
 
-template<typename ...Args>
-using make_index_sequence = typename build_index_sequence<0, sizeof...(Args)>::type;
+template<>
+struct make_index_sequence< 1 > : public index_sequence< 0 > {};
 
 template<typename E>
 struct Arg;
@@ -140,8 +141,9 @@ struct Reg {
     template<void (*fn)(Args...), size_t... Idxs>
     void doit(index_sequence<Idxs...>)
     {
-        static const iocshArg args[sizeof...(Args)] = {{argnames[Idxs], Arg<Args>::code}...};
-        static const iocshFuncDef def = {name, sizeof...(Args), (const iocshArg* const*)&args};
+        static const iocshArg argstack[sizeof...(Args)] = {{argnames[Idxs], Arg<Args>::code}...};
+        static const iocshArg * const args[] = {&argstack[Idxs]...};
+        static const iocshFuncDef def = {name, sizeof...(Args), args};
 
         iocshRegister(&def, &call<fn, Idxs...>);
     }
@@ -149,7 +151,7 @@ struct Reg {
     template<void (*fn)(Args...)>
     void ister()
     {
-        doit<fn>(make_index_sequence<Args...>{});
+        doit<fn>(make_index_sequence<sizeof...(Args)>{});
     }
 };
 

```
