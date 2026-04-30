# ff3c0e4da4c0 — drop use of std::regex in pvRequest parsing

**Date**: 2020-03-09  
**Author**: Michael Davidsaver  
**Severity**: MEDIUM  
**Verdict**: applies  

## Changed files
src/clientreq.cpp | 59 ++++++++++++++++++++++++++++++-------------------------

## pva-rs mapping
- crates/epics-pva-rs/src/client_native/mod.rs

## Commit message
drop use of std::regex in pvRequest parsing

## Key diff (first 100 lines)
```diff
diff --git a/src/clientreq.cpp b/src/clientreq.cpp
index 4a94288..a3f6b54 100644
--- a/src/clientreq.cpp
+++ b/src/clientreq.cpp
@@ -7,7 +7,6 @@
 #include <stdexcept>
 #include <map>
 #include <string>
-#include <regex>
 
 #include <pvxs/version.h>
 #include <pvxs/client.h>
@@ -143,7 +142,6 @@ struct PVRParser
         tEOF = -1,
     };
 
-    std::regex lexer;
     token_t lextok = tEOF;
     std::string lexval;
 
@@ -152,10 +150,7 @@ struct PVRParser
     CommonBase& target;
 
     PVRParser(CommonBase& target, const char* input)
-        :lexer(R"re((?:([\[\],\(\)=])|([a-zA-Z0-9_.]+))(.*))re")
-        //   (?: literal | name ) remaining
-        //          \1       \2      \3
-        ,input(input)
+        :input(input)
         ,target(target)
     {}
 
@@ -172,34 +167,44 @@ struct PVRParser
         while(' '==*input)
             input++;
 
-        std::cmatch M;
-        std::regex_match(input, M, lexer);
-        if(M.empty())
-            throw std::runtime_error("invalid charactor near: "+std::string(input));
+        switch(*input) {
+        case '[':
+        case ']':
+        case '(':
+        case ')':
+        case ',':
+        case '=':
+            lextok = token_t(*input++);
+            return;
+        default:
+            break;
+        }
 
-        if(M[1].matched) {
-            lextok = token_t(input[M.position(1)]);
+        auto isname = [](char c) {
+            return ((c>='a' && c<='z'))
+                    || ((c>='A' && c<='Z'))
+                    || ((c>='0' && c<='9'))
+                    || c=='.' || c=='_';
+        };
 
-        } else if(M[2].matched) {
-            lexval = M[2].str();
-            if(lexval=="field") {
-                lextok = field;
+        auto start = input;
+        while(isname(*input))
+            input++;
 
-            } else if(lexval=="record") {
-                lextok = record;
+        if(start==input)
+            throw std::runtime_error("invalid charactor near: "+std::string(start));
 
-            } else {
-                lextok = name;
-            }
+        lexval = std::string(start, input-start);
 
-        } else {
-            throw std::logic_error("pvRequest lexer logic error invalid state");
-        }
+        if(lexval=="field") {
+            lextok = field;
 
-        if(!M[3].matched)
-            throw std::logic_error("pvRequest lexer logic error no continuation");
+        } else if(lexval=="record") {
+            lextok = record;
 
-        input += M.position(3);
+        } else {
+            lextok = name;
+        }
     }
 
     void parse()

```
