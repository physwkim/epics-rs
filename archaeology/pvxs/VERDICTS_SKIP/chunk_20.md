## a93ab3b — APPLIES — type of num_index should match other indices

**Reason**: Type-safety fix changing `uint16_t num_index` to `size_t num_index` in FieldDesc. Prevents potential truncation/overflow when field descriptor indices exceed 64K.

**pva-rs target**: `src/pvdata/field.rs` (FieldDesc enum)

**Fix**: Ensure pva-rs FieldDesc bit-numbering uses consistent integer width (currently implicit in Rust enum representation; no explicit uint16 truncation risk, but worth validating for nested structures >64K fields).

---

## 9aaaccaf — APPLIES — track StructTop::member_indicies

**Reason**: Structural enhancement adding `member_indicies: Vec<size_t>` to map FieldStorage offsets back to FieldDesc offsets. Required for efficient iteration by storage offset (inverse lookup).

**pva-rs target**: `src/pvdata/value.rs` or structure initialization path (field offset caching)

**Fix**: Verify pva-rs value decoding caches or reconstructs field-offset mappings when needed; if not present, may require similar offset-tracking during Value construction for performance-critical paths.

---

## f2715d264e65c234bb5e308edc1a8ab38b0c2135 — N/A — doc

Documentation comment cleanup in dataimpl.h.

---

## 18d13e81e80ed3d34d9ef41bdef1de5db0c55821 — N/A — squeeze BitMask

Test refactor optimizing BitMask layout.

---

## 841ef0c048b68cb90cdd54bbfb91911711f89fc7 — N/A — BitMask xcode

BitMask wire codec implementation (to_wire/from_wire). pva-rs has equivalent BitSet codec (proto/bitset.rs).

---

## a207a54ec1e80f647fd1156047204b2c2eab09ab — N/A — start NT

Named Type (NT) utility framework. Orthogonal feature, not a bug fix.

---

## 50963f0a3e56327ab4f231b0a430ff54ffae64bb — N/A — reorg to separate type handling

Refactor: moved type encoding logic from data.cpp to type.cpp. Cleanup only.

---

## 92c513d6dd7b9b6e07663d6335fbd11cfb7b05c7 — N/A — de-templateize xcode

Code generation refactor (template→function). Not a bug.

---

## 2332ed8e519d396ad27988a4fe8d6bf3973674b0 — N/A — cleanup

Server header cleanup (removed stubs).

---

## 582b1b9a79c2f51f4d6c2bde6b263e072df4120b — N/A — add skeleton for server Get/Put/RPC

Server skeleton. Feature, not fix.

---

## 439be977c20509d10c857607bfa6538bd80962a6 — N/A — apply bitmask

Integrate BitMask into data model. Feature.

---

## 2ba0150bd9975429c187572586ce3efc5bb6994e — N/A — disambiguate

Namespace cleanup (pvxs::detail → pvxs::impl::detail). Refactor.

---

## d20ac8ee6dafd64183980db7c9c10f773b2aea98 — N/A — server GET_FIELD reply

Server introspection feature.

---

## a6ff69eb425a5eb31284a149cf494d2959e026ec — N/A — gcc 4.8 compat

Compiler compatibility shim. C++-specific.

---

## 8fca41b68310becd7df489eac0668e92188eda85 — N/A — add BitMask

BitMask implementation (new feature).

---

## 801d295c1f53da906c5a79da1bc2d379833a798e — N/A — start PVD

Major feature: PVData type system foundation. Not a bug.

---

## 1f6502aef75f83d049fb3a8657266dfc6cf36db1 — N/A — more util

Utility function update (non-bug).

---

## ceedd6bacceb52c71a7972ac97ae66d6b04157ab — N/A — unittest multi-line messages

Test infrastructure enhancement.

---

## dd4da5d6d50109bd741e214b586bbbe2fa2a63fb — N/A — more test utils

Test helper expansion.

---

## 4c60d72f9cbb0d3b520368fb4609eff3907000f2 — N/A — add shared_array

New feature: reference-counted typed arrays.

---

## 629277662970de896310d8e12b719580c2b8eb71 — N/A — normalize message names to CMD_*

Protocol constant renaming. Refactor.

---

## 083bad3e286e419a42d5cd48e6454b6816e7f057 — N/A — redo namespaces

Namespace reorganization. Refactor.

---

## 9c68205428db5af61e726c9fb1d15399fc1d220c — N/A — more server

Server feature expansion.

---

## 495eeaa4acfec4e1a7348ac0a4ca314f57576fb0 — N/A — rework rwlock

Internal locking mechanism update. Not a bug fix.

---

## 12598c86797513cb5655dc25fa37956a648aaa2b — N/A — server logging

Logging enhancement.

---

## 946e557960449864c4a52b2c1b0f321def2b7eba — N/A — respond to CreateChan

Server channel lifecycle feature.

---

## e1b8923f33d98f5318a7a90d952580153bfab9a8 — N/A — server basics

Server foundation feature.

---

## 073edee87bd926e0002f42b04b25cf872ffd6426 — N/A — minor

Internal utility refactor.

---

## 06e780872b0b2cb956b28e04e50331762f413056 — N/A — progress

Major feature commit (server + protocol work).

---

## 985687a54d6ba0190058cee2d5677a4cc88574e1 — N/A — test UDP handling

Test infrastructure.

---

**Summary**: 28 N/A (features, refactors, tests, docs), 2 APPLIES (type safety + offset tracking).
