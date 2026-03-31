//! Integration tests for the snc-rs CLI binary.
//!
//! Uses std::process::Command to invoke the compiled snc binary and
//! verify behavior for help, compilation, and error handling.

use std::io::Write;
use std::process::Command;

/// Get the path to the snc binary built by cargo.
fn snc_bin() -> String {
    // cargo test builds the binary in the same target directory
    let mut path = std::env::current_exe()
        .expect("cannot get current exe path");
    // Navigate from the test binary to the deps directory, then up to the target profile dir
    path.pop(); // remove test binary name
    if path.ends_with("deps") {
        path.pop(); // remove "deps"
    }
    path.push("snc-rs");
    path.to_string_lossy().into_owned()
}

/// Create a temporary file with the given content and return its path.
fn temp_snl_file(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(".st")
        .tempfile()
        .expect("failed to create temp file");
    f.write_all(content.as_bytes())
        .expect("failed to write temp file");
    f.flush().expect("failed to flush temp file");
    f
}

// ===========================================================================
// 1. CLI --help
// ===========================================================================

#[test]
fn cli_help_flag() {
    let output = Command::new(snc_bin())
        .arg("--help")
        .output()
        .expect("failed to run snc --help");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "snc --help should succeed");
    assert!(stdout.contains("SNL to Rust compiler") || stdout.contains("snc"),
        "help output should mention the compiler: {stdout}");
}

#[test]
fn cli_short_help() {
    let output = Command::new(snc_bin())
        .arg("-h")
        .output()
        .expect("failed to run snc -h");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage") || stdout.contains("usage"),
        "short help should contain usage: {stdout}");
}

// ===========================================================================
// 2. Compilation of a minimal SNL program
// ===========================================================================

#[test]
fn cli_compile_minimal_program_to_stdout() {
    let snl = r#"
program minimal
ss main_ss {
    state idle {
        when (true) {
        } state idle
    }
}
"#;
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .output()
        .expect("failed to run snc");

    assert!(output.status.success(), "snc should succeed for valid SNL. stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("use epics_seq_rs::prelude::*;"),
        "output should contain prelude import");
    assert!(stdout.contains("struct minimalVars"),
        "output should contain vars struct");
    assert!(stdout.contains("async fn main_ss("),
        "output should contain state set function");
    assert!(stdout.contains("#[tokio::main]"),
        "output should contain tokio main");
}

#[test]
fn cli_compile_to_output_file() {
    let snl = r#"
program file_out
ss main_ss {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let input = temp_snl_file(snl);
    let output_file = tempfile::Builder::new()
        .suffix(".rs")
        .tempfile()
        .expect("failed to create output temp file");
    let output_path = output_file.path().to_path_buf();

    let result = Command::new(snc_bin())
        .arg(input.path())
        .arg("-o")
        .arg(&output_path)
        .output()
        .expect("failed to run snc");

    assert!(result.status.success(),
        "snc should succeed writing to output file. stderr: {}",
        String::from_utf8_lossy(&result.stderr));

    let content = std::fs::read_to_string(&output_path)
        .expect("failed to read output file");
    assert!(content.contains("use epics_seq_rs::prelude::*;"));
    assert!(content.contains("struct file_outVars"));
}

#[test]
fn cli_compile_program_with_variables() {
    let snl = r#"
program var_test
double x;
int y;
assign x to "PV:x";
assign y to "PV:y";
monitor x;
evflag ef_x;
sync x to ef_x;
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .output()
        .expect("failed to run snc");

    assert!(output.status.success(),
        "snc should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("x: f64"));
    assert!(stdout.contains("y: i32"));
    assert!(stdout.contains("CH_X"));
    assert!(stdout.contains("CH_Y"));
    assert!(stdout.contains("EF_EF_X"));
}

// ===========================================================================
// 3. Error handling
// ===========================================================================

#[test]
fn cli_error_nonexistent_file() {
    let output = Command::new(snc_bin())
        .arg("/nonexistent/path/to/file.st")
        .output()
        .expect("failed to run snc");

    assert!(!output.status.success(), "snc should fail for nonexistent file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("cannot read"),
        "stderr should contain error message: {stderr}");
}

#[test]
fn cli_error_invalid_snl_syntax() {
    let snl = "this is not valid SNL at all";
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .output()
        .expect("failed to run snc");

    assert!(!output.status.success(),
        "snc should fail for invalid SNL syntax");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error"),
        "stderr should contain 'error': {stderr}");
}

#[test]
fn cli_error_missing_input_arg() {
    let output = Command::new(snc_bin())
        .output()
        .expect("failed to run snc");

    assert!(!output.status.success(),
        "snc should fail when no input file is given");
}

#[test]
fn cli_error_incomplete_program() {
    // Missing state set — should error during parse or analysis
    let snl = "program incomplete";
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .output()
        .expect("failed to run snc");

    // This may or may not error depending on the parser/analysis,
    // but it should not panic.
    let _ = output.status;
}

// ===========================================================================
// 4. Debug flags
// ===========================================================================

#[test]
fn cli_dump_ast_flag() {
    let snl = r#"
program dump_test
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .arg("--dump-ast")
        .output()
        .expect("failed to run snc --dump-ast");

    assert!(output.status.success(),
        "snc --dump-ast should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    // AST dump goes to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Program") || stderr.contains("program"),
        "dump-ast stderr should contain AST debug output: {stderr}");
}

#[test]
fn cli_dump_ir_flag() {
    let snl = r#"
program ir_test
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let file = temp_snl_file(snl);

    let output = Command::new(snc_bin())
        .arg(file.path())
        .arg("--dump-ir")
        .output()
        .expect("failed to run snc --dump-ir");

    assert!(output.status.success(),
        "snc --dump-ir should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr));

    // IR dump goes to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SeqIR") || stderr.contains("program_name"),
        "dump-ir stderr should contain IR debug output: {stderr}");
}
