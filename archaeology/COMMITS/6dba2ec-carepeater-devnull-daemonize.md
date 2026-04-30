---
sha: 6dba2ec1d7f600d66fddb6c30d0c3e5b6b3a8618
short_sha: 6dba2ec
date: 2020-02-13
author: Michael Davidsaver
category: lifecycle
severity: medium
rust_verdict: partial
audit_targets:
  - crate: ca-rs
    file: src/bin/carepeater.rs
    function: main
tags: [caRepeater, daemonize, stdin-stdout, lifecycle, process]
---

# caRepeater inherits parent stdin/out/err — causes problems when spawned by caget

## Root Cause
When a CA client such as `caget` spawns `caRepeater` as a background process,
the child inherits the parent's open file descriptors for stdin, stdout, and
stderr. If the parent is itself a shell script, those file descriptors may be
pipes or special files. This has caused documented issues where the inherited
file descriptors interfere with the parent process's I/O, or where the
repeater holds open file descriptors that prevent pipes from closing.

## Symptoms
- Shell scripts that use `caget` hang waiting for EOF on a pipe that is kept
  open by the background caRepeater process.
- Interactive programs inverted on terminal when caRepeater inherits a tty.

## Fix
On POSIX targets (excluding WIN32, RTEMS, vxWorks), redirect stdin to
`/dev/null` (O_RDONLY) and stdout/stderr to `/dev/null` (O_WRONLY) using
`dup2` immediately after argument parsing. Add a `-v` flag to suppress the
redirect for debugging. This is a partial daemonization (stdio redirect only;
process group and working directory remain).

## Rust Applicability
If ca-rs implements a `carepeater` binary that is spawned as a background
process, the same concern applies on POSIX targets. The Rust binary should
redirect its stdio to `/dev/null` at startup (using `nix::unistd::dup2` or
`std::fs::File::open("/dev/null")` + raw fd operations) unless a verbose flag
is provided.

## Audit Recommendation
Audit `ca-rs/src/bin/carepeater.rs::main`:
1. Check whether the binary redirects stdio to `/dev/null` on POSIX.
2. If spawned via `Command::new().spawn()` from another binary, verify
   `.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())` is set
   on the `Command` to avoid inheriting parent FDs.

## C Locations
- `modules/ca/src/client/caRepeater.cpp:main` — /dev/null redirect logic added
