## b71c645769bab77e79ddb99aa15667588aeeb96b — N/A — update server config
**Reason**: Server configuration refactoring (C++ server API, IOC-specific). Not applicable to pva-rs.

## b81ec5257789f64afd01749b28b6efee1e09532c — N/A — more doc
**Reason**: Documentation and comment expansion. No functional change.

## 5815d595c4d9a05eeaddf183e63b285092d42ad9 — N/A — move server Config out
**Reason**: Server struct refactoring (IOC-specific). No functional change.

## 6c9b0b8800f86ed4393ef233122edb4da2feaa4c — N/A — NTScalar w/ string still has description/units
**Reason**: NTScalar struct building for C++ server. Not applicable to pva-rs.

## 6f584f8299248e9267dc681abf70c55223408780 — N/A — more doc
**Reason**: Documentation and comment updates only.

## cd11ac7e4595c371acba177b39fb2d65c98d4bf2 — N/A — minor
**Reason**: Trivial comment change.

## 7734507ce7550c402bf731d2cb809bdeac98e0fb — N/A — minor GUID
**Reason**: GUID formatting in C++ server code.

## 3129c2de7556ce1e244d00492bb899138adeaede — N/A — doc sharedpv
**Reason**: Documentation addition. No functional change.

## cd2d9265819ef16e34ae884a8725319aee815d22 — N/A — add SharedPV
**Reason**: C++ server feature (SharedPV is IOC-specific concept).

## 95ed4b29934c1d788508088836fc5d855b9f1c55 — APPLIES — onClose confusion
**Reason**: Bug fix for server connection cleanup. Operations' onClose callbacks were not being invoked during ServerConn destruction.
**pva-rs target**: /Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/server_native/peers.rs
**Fix**: Iterate all operations in opByIOID and chanBySID, invoking onClose callback on cleanup.

## 24d9eb49f1ddbef7e39cde999dae5b795acb99cb — APPLIES — fit Put reply argument validation
**Reason**: Bug fix for GET/PUT reply validation logic. GET and GET-with-write must return Value; PUT-without-write must not.
**pva-rs target**: /Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/server_native/mod.rs
**Fix**: Distinguish GET/PUT(w/subcmd 0x40) requiring exact type vs. PUT requiring no value.

## 03c41c2ce3a250fd8c76aef565fd568f125ab301 — N/A — minor
**Reason**: Minor refactoring in IOC event helper (evhelper.cpp).

## 80dacb895e9d5e3ea6efe1b309a33f426935daf4 — N/A — log show time
**Reason**: Logging enhancement only.

## 363761ab77d8d7957982439fc5f64cf23be59d0a — APPLIES — server relax handling of op on non-existant IOID
**Reason**: Protocol race condition fix. Demote unacked destroy→late-arriving message errors to debug level.
**pva-rs target**: /Users/stevek/codes/epics-rs/crates/epics-pva-rs/src/server_native/peers.rs
**Fix**: On non-existent IOID, return silently if Dead; only error on type mismatch or Creating state.

## 1a286ede6eb8e6c217f544ea1492483720beba45 — N/A — Value explicit handling of bool
**Reason**: C++ bool marshalling enhancement. pva-rs handles bool natively.

## 3bd86f977796b4e0cd350350dad327dceb2fa139 — N/A — server GET apply pvRequest to mask
**Reason**: Server-side pvRequest masking (not client-side).

## 31412aff2ea371ffa1b7877e2b4a94a664cc99c6 — N/A — add request2mask()
**Reason**: Server utility for pvRequest masking (IOC-specific).

## 52147c5749f44d68cad0630ac10c63627571fff0 — N/A — BitMask equality
**Reason**: Data structure utility for C++ server.

## 584cf5b4507907323b3c0b12f83fc39b8e982e82 — N/A — minor
**Reason**: Code style refactoring only.

## c78ec7718b17262ec4319a6aa9d256857d4d19e3 — N/A — Value iteration
**Reason**: C++ data API enhancement (iterator support). Not applicable to pva-rs.

## d4f4fe970da59d734089c3d60acbd2288d5f61f6 — N/A — add Value::nameOf()
**Reason**: C++ API utility method.

## 586e93b2d0eca4116476528d8568ed8ab266a1ff — N/A — split Source and friends into seperate header
**Reason**: C++ header organization refactoring.

## 9ffb8dab9abe80bc1829d54ac85c6c83b85eef8a — N/A — minor
**Reason**: Code cleanup.

## c284439a811b2a8efd553218d47c9c17a4b21b6c — N/A — more log
**Reason**: Logging adjustment only.

## 1837e3bc4726284ca8580cf80d666ff6a886765c — N/A — doc
**Reason**: Documentation build configuration.

## a1b9a64a06b70f4b4de090f7f5feb4abca39f3da — N/A — misc
**Reason**: Code organization and logging refactoring.

## 4db60a0fe5f6a8254b8c3bdbc8ca44d6b85f5da3 — N/A — TypeDef cache resulting FieldDesc
**Reason**: Data structure caching optimization in C++. Not applicable to pva-rs.

## 8eae991006c6c488f0b3f3b02132edbeffafcfd4 — N/A — g++ >=4.9 required
**Reason**: C++ compiler requirement. Not applicable to Rust.

## bbe5fa26a2146fcee0b62dca605a0be1186da592 — N/A — Redo FieldDesc
**Reason**: Major C++ data structure refactoring. pva-rs uses different recursive enum FieldDesc.

## 9cc742d7f9e5e85326bbdef6f51264a3e146cd4b — N/A — Value marking
**Reason**: C++ field validity marking. Not applicable to pva-rs.
