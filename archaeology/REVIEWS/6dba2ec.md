---
short_sha: 6dba2ec
status: not-applicable
files_changed: []
---
C commit added a `dup2`-to-`/dev/null` block inside `caRepeater.cpp::main` so the daemon would not inherit pipes/ttys from the spawning shell. In ca-rs the equivalent isolation is already enforced **at the spawn site** — `crates/epics-ca-rs/src/repeater.rs:327-336` (`spawn_repeater`) constructs the child with `Command::new(bin).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()`. The OS hands the child fresh `/dev/null` descriptors before `main` runs, which is the same end state the C `dup2` workaround achieves and is the canonical Rust idiom (see `Stdio::null` docs). The CLI binary `crates/epics-ca-rs/src/bin/ca-repeater-rs.rs` itself is therefore correct — it relies on the parent giving it clean stdio, exactly as the C fix relies on the parent leaving them inheritable and then re-opening them. There is also a fallback in-process repeater thread (`std::thread::spawn` at `repeater.rs:342`) which never inherits a child stdio at all. No code change required.
