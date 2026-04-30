# Batch 6 — Verdicts (R151-R177)

## R151 — 816838bcd5 — N/A
**pvxs**: fix hex dump initialization
**Reasoning**: C++ uninitialized buffer on stack. Rust Vec/String are initialized.

## R152 — 8333ce30ec — ALREADY-FIXED
**pvxs**: re-define user bufferevent limits
**Reasoning**: Tokio handles socket buffers transparently; no libevent watermark API.

## R153 — 90131d0a85 — N/A
**pvxs**: ifaddrs::ifa_addr can be NULL
**Reasoning**: Rust uses Option patterns for OS enumeration.

## R154 — 939391590e — N/A
**pvxs**: client: clear nameServers during close()
**Reasoning**: Rust struct drop semantics auto-clean refs; no manual cycle break needed.

## R155 — 9996abef31 — APPLIES (UNVERIFIED — needs deep recheck)
**pvxs**: fix Value::isMarked parents=true (arithmetic order bug)
**pva-rs target**: pvdata/encode.rs, pvdata/structure.rs
**Reasoning**: Parent traversal pointer arithmetic; pva-rs may have similar indexing.
**Fix sketch**: ensure state updates from child happen before parent dereference.

## R156 — a4e974def9 — APPLIES (UNVERIFIED)
**pvxs**: client: fix batch pop() of exception
**pva-rs target**: client_native/ops_v2.rs (monitor batch ACK)
**Reasoning**: Move queue entry before checking exception; pop_front order matters.
**Fix sketch**: Peek queue front for exception check BEFORE moving entry.

## R157 — abeb78a9cd — APPLIES (UNVERIFIED)
**pvxs**: fix TypeDef(const Value& val) for Union/UnionA/StructA
**pva-rs target**: pvdata/encode.rs
**Reasoning**: Tree copy uses wrong base pointer for StructA/UnionA.
**Fix sketch**: For StructA/UnionA, use `desc.members.as_ref()` as base.

## R158 — b0c36f365e — ALREADY-FIXED
**pvxs**: bevRead fix low water mark
**Reasoning**: Rust async select! framing; no libevent enable/disable.

## R159 — b377858115 — N/A
**pvxs**: fix shared_array operator<<
**Reasoning**: C++ qualification. No analog in Rust.

## R160 — b54b9fb78d — APPLIES (UNVERIFIED)
**pvxs**: server fix bind() when 5075 in use
**pva-rs target**: server_native/tcp.rs
**Reasoning**: Multi-interface bind fallback; only first iface should use port 0.
**Fix sketch**: Track `firstiface` flag; allow fallback only on first.

## R161 — b8be9bd058 — ALREADY-FIXED
**pvxs**: fix Value iteration (temporary lifetime)
**Reasoning**: Rust borrow checker enforces lifetime at compile time.

## R162 — b9170a9885 — APPLIES (UNVERIFIED)
**pvxs**: fix Value::nmembers
**pva-rs target**: pvdata/value.rs
**Reasoning**: StructA/UnionA misidentify member count (use desc.miter not desc.members[0].miter).
**Fix sketch**: For StructA/UnionA, return members[0].miter.len(), not desc's own.

## R163 — b9b22adb15 — ALREADY-FIXED
**pvxs**: fix version_str() macro
**Reasoning**: Rust macros distinct; no double-expansion.

## R164 — c2f1f13bb3 — ALREADY-FIXED
**pvxs**: server: fix beacons TX on Linux
**Reasoning**: pva-rs handles dual-stack at config layer.

## R165 — c7b4650ba1 — APPLIES (UNVERIFIED)
**pvxs**: fix TypeStore maintenance
**pva-rs target**: pvdata/encode.rs
**Reasoning**: Off-by-one in arena slicing.
**Fix sketch**: Use `descs[index..].to_vec()`, not from 0.

## R166 — c870415908 — N/A
**pvxs**: fix formatting of uint8/int8 fields
**Reasoning**: Rust Display traits avoid char ambiguity.

## R167 — cacc9d088d — APPLIES (UNVERIFIED)
**pvxs**: fix de-serialize of sub-sub-struct (mlookup)
**pva-rs target**: pvdata/encode.rs
**Reasoning**: Nested struct index offset; should be `cindex - cref + pair.second`.
**Fix sketch**: Track copy base index, subtract from cindex.

## R168 — d65abb28ea — ALREADY-FIXED
**pvxs**: shared_array fix print of char[]
**Reasoning**: Rust Display avoids char promotion.

## R169 — dc4c4ae870 — N/A
**pvxs**: fix *Builder visibility
**Reasoning**: C++ visibility; no analog in Rust.

## R170 — dfbed0c850 — N/A
**pvxs**: server ExecOp timer
**Reasoning**: New feature, not bug fix. pva-rs uses tokio::time.

## R171 — e09f901e72 — APPLIES (UNVERIFIED)
**pvxs**: client: fix _reExecPut() allowed for .get()
**pva-rs target**: client_native/ops_v2.rs
**Reasoning**: Logic check `if(op!=Get && op!=Put)` should be `if(op!=Put)`.
**Fix sketch**: Change condition to `if op != Op::Put`.

## R172 — e0a8572c2d — APPLIES (UNVERIFIED)
**pvxs**: server: fix stats(reset=true)
**pva-rs target**: server_native/tcp.rs (monitor stats)
**Reasoning**: Resets local var instead of shared `mon->maxQueue`.
**Fix sketch**: Assign `monitor.maxqueue = 0`.

## R173 — e51954529a — N/A
**pvxs**: ioc: avoid *NULL on exit
**Reasoning**: IOC-specific (ioc/group.cpp); no pva-rs equivalent.

## R174 — e93909cf7e — APPLIES (UNVERIFIED)
**pvxs**: fix shared_array::convertTo() (sign/unsign)
**pva-rs target**: pvdata/field_desc.rs
**Reasoning**: Multiple type conversion cases use wrong source type.
**Fix sketch**: Audit convertArr matrix for signedness.

## R175 — ed5b15d38e — ALREADY-FIXED
**pvxs**: fix log macros
**Reasoning**: Rust tracing macros distinct.

## R176 — ed5bcc8a4f — APPLIES (UNVERIFIED)
**pvxs**: fix segmented messages (watermark scope)
**pva-rs target**: client_native/ops_v2.rs
**Reasoning**: Watermark reset should be after segmented message completes.
**Fix sketch**: Move reset outside conditional.

## R177 — f2777e319b — APPLIES (UNVERIFIED)
**pvxs**: fix shared_array::convertTo() (force conversion)
**pva-rs target**: pvdata/field.rs
**Reasoning**: Early-exit castTo() branch masks bugs.
**Fix sketch**: Remove early-exit; always call convertArr.

---

**Summary**: 11 APPLIES (all UNVERIFIED — agent speculated without actually reading pva-rs source for some), 8 ALREADY-FIXED, 8 N/A.

**WARNING**: This batch's APPLIES verdicts must be re-verified by reading pva-rs source — agent did not consistently verify each finding.
