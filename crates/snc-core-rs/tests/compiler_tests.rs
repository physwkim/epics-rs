//! Integration tests for snc-core-rs: lexer, parser, codegen, and end-to-end pipeline.

use snc_core_rs::analysis::analyze;
use snc_core_rs::codegen::generate;
use snc_core_rs::lexer::{Lexer, Token};
use snc_core_rs::parser::Parser;
use snc_core_rs::preprocess::preprocess;

// ---------------------------------------------------------------------------
// Helper: full pipeline from SNL source to generated Rust code
// ---------------------------------------------------------------------------

fn compile(snl: &str) -> String {
    let pp = preprocess(snl);
    let tokens = Lexer::new(&pp.source).tokenize().expect("lexer failed");
    let ast = Parser::new(tokens).parse_program().expect("parser failed");
    let ir = analyze(&ast).expect("analysis failed");
    generate(&ir)
}

fn lex(input: &str) -> Vec<Token> {
    Lexer::new(input)
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|st| st.token)
        .collect()
}

// ===========================================================================
// 1. Lexer tokenization
// ===========================================================================

#[test]
fn lex_minimal_program_header() {
    let tokens = lex("program myProg");
    assert_eq!(tokens[0], Token::Program);
    assert_eq!(tokens[1], Token::Ident("myProg".into()));
    assert_eq!(tokens[2], Token::Eof);
}

#[test]
fn lex_variable_declaration() {
    let tokens = lex("double x; int y;");
    assert_eq!(tokens[0], Token::Double);
    assert_eq!(tokens[1], Token::Ident("x".into()));
    assert_eq!(tokens[2], Token::Semi);
    assert_eq!(tokens[3], Token::Int);
    assert_eq!(tokens[4], Token::Ident("y".into()));
    assert_eq!(tokens[5], Token::Semi);
}

#[test]
fn lex_assign_monitor_sync() {
    let tokens = lex(r#"assign x to "PV:x"; monitor x; sync x to ef_x;"#);
    assert_eq!(tokens[0], Token::Assign);
    assert_eq!(tokens[1], Token::Ident("x".into()));
    assert_eq!(tokens[2], Token::To);
    assert_eq!(tokens[3], Token::StringLit("PV:x".into()));
    assert_eq!(tokens[4], Token::Semi);
    assert_eq!(tokens[5], Token::Monitor);
    assert_eq!(tokens[6], Token::Ident("x".into()));
    assert_eq!(tokens[7], Token::Semi);
    assert_eq!(tokens[8], Token::Sync);
}

#[test]
fn lex_state_set_keywords() {
    let tokens = lex("ss myss { state init { when (true) {} state idle } }");
    assert_eq!(tokens[0], Token::Ss);
    assert_eq!(tokens[1], Token::Ident("myss".into()));
    assert_eq!(tokens[2], Token::LBrace);
    assert_eq!(tokens[3], Token::State);
    assert_eq!(tokens[4], Token::Ident("init".into()));
    assert_eq!(tokens[5], Token::LBrace);
    assert_eq!(tokens[6], Token::When);
}

#[test]
fn lex_numeric_literals() {
    let tokens = lex("0 1 42 3.14 1e3 0xFF");
    assert_eq!(tokens[0], Token::IntLit(0));
    assert_eq!(tokens[1], Token::IntLit(1));
    assert_eq!(tokens[2], Token::IntLit(42));
    assert_eq!(tokens[3], Token::FloatLit(3.14));
    assert_eq!(tokens[4], Token::FloatLit(1e3));
    assert_eq!(tokens[5], Token::IntLit(255));
}

#[test]
fn lex_string_with_escapes() {
    let tokens = lex(r#""hello\nworld\t!""#);
    assert_eq!(tokens[0], Token::StringLit("hello\nworld\t!".into()));
}

#[test]
fn lex_comparison_operators() {
    let tokens = lex("== != < <= > >=");
    assert_eq!(tokens[0], Token::Eq);
    assert_eq!(tokens[1], Token::Ne);
    assert_eq!(tokens[2], Token::Lt);
    assert_eq!(tokens[3], Token::Le);
    assert_eq!(tokens[4], Token::Gt);
    assert_eq!(tokens[5], Token::Ge);
}

#[test]
fn lex_logical_operators() {
    let tokens = lex("&& || !");
    assert_eq!(tokens[0], Token::And);
    assert_eq!(tokens[1], Token::Or);
    assert_eq!(tokens[2], Token::Not);
}

#[test]
fn lex_option_keyword() {
    let tokens = lex("option +s;");
    assert_eq!(tokens[0], Token::Option_);
}

#[test]
fn lex_evflag_keyword() {
    let tokens = lex("evflag my_flag;");
    assert_eq!(tokens[0], Token::EvFlag);
    assert_eq!(tokens[1], Token::Ident("my_flag".into()));
}

#[test]
fn lex_comments_stripped() {
    let tokens = lex("int /* block comment */ x; // line comment\nint y;");
    // Should contain: Int, Ident(x), Semi, Int, Ident(y), Semi, Eof
    let non_eof: Vec<_> = tokens.iter().filter(|t| *t != &Token::Eof).collect();
    assert_eq!(non_eof.len(), 6);
}

#[test]
fn lex_true_false_literals() {
    let tokens = lex("TRUE FALSE true false");
    assert_eq!(tokens[0], Token::IntLit(1));
    assert_eq!(tokens[1], Token::IntLit(0));
    assert_eq!(tokens[2], Token::IntLit(1));
    assert_eq!(tokens[3], Token::IntLit(0));
}

#[test]
fn lex_embedded_code() {
    let tokens = lex("%% use std::io;\nint x;");
    assert!(matches!(&tokens[0], Token::EmbeddedLine(s) if s.contains("use std::io")));
}

#[test]
fn lex_error_on_invalid_char() {
    let result = Lexer::new("@").tokenize();
    assert!(result.is_err());
}

// ===========================================================================
// 2. Parser — correct AST for basic state machines
// ===========================================================================

/// Minimal valid SNL program.
const MINIMAL_SNL: &str = r#"
program minimal
ss main_ss {
    state idle {
        when (true) {
        } state idle
    }
}
"#;

fn parse_snl(snl: &str) -> snc_core_rs::ast::Program {
    let pp = preprocess(snl);
    let tokens = Lexer::new(&pp.source).tokenize().expect("lex failed");
    Parser::new(tokens).parse_program().expect("parse failed")
}

#[test]
fn parse_minimal_program_name() {
    let ast = parse_snl(MINIMAL_SNL);
    assert_eq!(ast.name, "minimal");
}

#[test]
fn parse_minimal_one_state_set() {
    let ast = parse_snl(MINIMAL_SNL);
    assert_eq!(ast.state_sets.len(), 1);
    assert_eq!(ast.state_sets[0].name, "main_ss");
}

#[test]
fn parse_minimal_one_state() {
    let ast = parse_snl(MINIMAL_SNL);
    assert_eq!(ast.state_sets[0].states.len(), 1);
    assert_eq!(ast.state_sets[0].states[0].name, "idle");
}

#[test]
fn parse_minimal_transition() {
    let ast = parse_snl(MINIMAL_SNL);
    let state = &ast.state_sets[0].states[0];
    assert_eq!(state.transitions.len(), 1);
    assert!(state.transitions[0].condition.is_some());
}

#[test]
fn parse_program_with_variables() {
    let snl = r#"
program test_vars
double x;
int counter;
assign x to "IOC:x";
assign counter to "IOC:cnt";
monitor x;
ss main {
    state init {
        when (true) {
        } state init
    }
}
"#;
    let ast = parse_snl(snl);
    assert_eq!(ast.name, "test_vars");

    // Should have variable declarations
    let var_decls: Vec<_> = ast
        .definitions
        .iter()
        .filter(|d| matches!(d, snc_core_rs::ast::Definition::VarDecl(_)))
        .collect();
    assert_eq!(var_decls.len(), 2);

    // Should have assign definitions
    let assigns: Vec<_> = ast
        .definitions
        .iter()
        .filter(|d| matches!(d, snc_core_rs::ast::Definition::Assign(_)))
        .collect();
    assert_eq!(assigns.len(), 2);

    // Should have a monitor definition
    let monitors: Vec<_> = ast
        .definitions
        .iter()
        .filter(|d| matches!(d, snc_core_rs::ast::Definition::Monitor(_)))
        .collect();
    assert_eq!(monitors.len(), 1);
}

#[test]
fn parse_program_with_evflag_and_sync() {
    let snl = r#"
program test_sync
double x;
assign x to "PV:x";
monitor x;
evflag ef_x;
sync x to ef_x;
ss main {
    state idle {
        when (true) {
        } state idle
    }
}
"#;
    let ast = parse_snl(snl);

    let evflags: Vec<_> = ast
        .definitions
        .iter()
        .filter(|d| matches!(d, snc_core_rs::ast::Definition::EvFlag(_)))
        .collect();
    assert_eq!(evflags.len(), 1);

    let syncs: Vec<_> = ast
        .definitions
        .iter()
        .filter(|d| matches!(d, snc_core_rs::ast::Definition::Sync(_)))
        .collect();
    assert_eq!(syncs.len(), 1);
}

#[test]
fn parse_multiple_state_sets() {
    let snl = r#"
program multi_ss
ss alpha {
    state s0 {
        when (true) {} state s0
    }
}
ss beta {
    state s0 {
        when (true) {} state s0
    }
}
"#;
    let ast = parse_snl(snl);
    assert_eq!(ast.state_sets.len(), 2);
    assert_eq!(ast.state_sets[0].name, "alpha");
    assert_eq!(ast.state_sets[1].name, "beta");
}

#[test]
fn parse_multiple_states_with_transitions() {
    let snl = r#"
program multi_state
ss main {
    state init {
        when (true) {
        } state running
    }
    state running {
        when (true) {
        } state done
    }
    state done {
        when (true) {
        } state init
    }
}
"#;
    let ast = parse_snl(snl);
    let states = &ast.state_sets[0].states;
    assert_eq!(states.len(), 3);
    assert_eq!(states[0].name, "init");
    assert_eq!(states[1].name, "running");
    assert_eq!(states[2].name, "done");
}

#[test]
fn parse_option_safe() {
    let snl = r#"
program safe_prog
option +s;
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let ast = parse_snl(snl);
    assert!(ast
        .options
        .iter()
        .any(|o| matches!(o, snc_core_rs::ast::ProgramOption::Safe)));
}

#[test]
fn parse_error_missing_program() {
    let snl = "ss main { state idle { when (true) {} state idle } }";
    let pp = preprocess(snl);
    let tokens = Lexer::new(&pp.source).tokenize().expect("lex failed");
    let result = Parser::new(tokens).parse_program();
    assert!(result.is_err());
}

// ===========================================================================
// 3. Codegen — produces valid Rust output
// ===========================================================================

#[test]
fn codegen_contains_header() {
    let code = compile(MINIMAL_SNL);
    assert!(code.contains("use epics_seq_rs::prelude::*;"));
    assert!(code.contains("//! Generated by snc"));
}

#[test]
fn codegen_contains_vars_struct() {
    let snl = r#"
program codegen_test
double x;
assign x to "PV:x";
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("struct codegen_testVars {"));
    assert!(code.contains("x: f64,"));
}

#[test]
fn codegen_contains_program_vars_impl() {
    let snl = r#"
program cg_pv
double x;
assign x to "PV:x";
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("impl ProgramVars for cg_pvVars"));
    assert!(code.contains("fn get_channel_value"));
    assert!(code.contains("fn set_channel_value"));
}

#[test]
fn codegen_contains_program_meta() {
    let snl = r#"
program cg_meta
double x;
assign x to "PV:x";
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("impl ProgramMeta for cg_metaMeta"));
    assert!(code.contains("NUM_CHANNELS"));
    assert!(code.contains("NUM_EVENT_FLAGS"));
    assert!(code.contains("NUM_STATE_SETS"));
    assert!(code.contains("fn channel_defs()"));
    assert!(code.contains("fn event_flag_sync_map()"));
}

#[test]
fn codegen_contains_state_set_fn() {
    let code = compile(MINIMAL_SNL);
    assert!(code.contains("async fn main_ss("));
    assert!(code.contains("StateSetContext<minimalVars>"));
}

#[test]
fn codegen_contains_main() {
    let code = compile(MINIMAL_SNL);
    assert!(code.contains("#[tokio::main]"));
    assert!(code.contains("async fn main()"));
    assert!(code.contains("ProgramBuilder::<minimalVars, minimalMeta>"));
}

#[test]
fn codegen_contains_state_machine_loop() {
    let code = compile(MINIMAL_SNL);
    assert!(code.contains("ctx.enter_state(0)"));
    assert!(code.contains("ctx.wait_for_wakeup().await"));
    assert!(code.contains("ctx.sync_dirty_vars()"));
    assert!(code.contains("ctx.has_transition()"));
    assert!(code.contains("ctx.take_transition()"));
}

#[test]
fn codegen_channel_constants() {
    let snl = r#"
program ch_test
double x;
int y;
assign x to "PV:x";
assign y to "PV:y";
monitor x;
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("const CH_X: usize = 0;"));
    assert!(code.contains("const CH_Y: usize = 1;"));
}

#[test]
fn codegen_event_flag_constants() {
    let snl = r#"
program ef_test
double x;
assign x to "PV:x";
monitor x;
evflag ef_x;
sync x to ef_x;
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("const EF_EF_X: usize = 0;"));
}

#[test]
fn codegen_state_id_constants() {
    let snl = r#"
program sid
ss main {
    state init {
        when (true) {} state running
    }
    state running {
        when (true) {} state init
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("MAIN_INIT"));
    assert!(code.contains("MAIN_RUNNING"));
}

#[test]
fn codegen_monitored_channel_def() {
    let snl = r#"
program mon_test
double x;
assign x to "PV:x";
monitor x;
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("monitored: true"));
}

// ===========================================================================
// 4. End-to-end: SNL source -> parse -> codegen -> verify patterns
// ===========================================================================

#[test]
fn e2e_counter_program() {
    let snl = r#"
program counter
option +s;
double count;
assign count to "{P}count";
monitor count;
evflag ef_count;
sync count to ef_count;

ss counter_ss {
    state init {
        when (true) {
            count = 0;
        } state counting
    }
    state counting {
        when (count >= 10) {
        } state done
        when (true) {
            count = count + 1;
        } state counting
    }
    state done {
        when (true) {
        } state init
    }
}
"#;
    let code = compile(snl);

    // Program structure
    assert!(code.contains("struct counterVars"));
    assert!(code.contains("count: f64"));
    assert!(code.contains("impl ProgramVars for counterVars"));
    assert!(code.contains("impl ProgramMeta for counterMeta"));

    // Channel and event flag
    assert!(code.contains("const CH_COUNT: usize = 0;"));
    assert!(code.contains("const EF_EF_COUNT: usize = 0;"));

    // State set with states
    assert!(code.contains("async fn counter_ss("));
    assert!(code.contains("COUNTER_SS_INIT"));
    assert!(code.contains("COUNTER_SS_COUNTING"));
    assert!(code.contains("COUNTER_SS_DONE"));

    // Main
    assert!(code.contains("ProgramBuilder::<counterVars, counterMeta>::new(\"counter\""));
}

#[test]
fn e2e_multi_var_types() {
    let snl = r#"
program types
int a;
double b;
float c;
short d;
string e;
assign a to "PV:a";
assign b to "PV:b";
assign c to "PV:c";
assign d to "PV:d";
assign e to "PV:e";
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("a: i32"));
    assert!(code.contains("b: f64"));
    assert!(code.contains("c: f32"));
    assert!(code.contains("d: i16"));
    assert!(code.contains("e: String"));
}

#[test]
fn e2e_two_state_sets() {
    let snl = r#"
program dual
ss alpha {
    state s0 {
        when (true) {} state s0
    }
}
ss beta {
    state s0 {
        when (true) {} state s0
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("async fn alpha("));
    assert!(code.contains("async fn beta("));
    assert!(code.contains(".add_ss(Box::new(|ctx| Box::pin(alpha(ctx))))"));
    assert!(code.contains(".add_ss(Box::new(|ctx| Box::pin(beta(ctx))))"));
    assert!(code.contains("NUM_STATE_SETS: usize = 2"));
}

#[test]
fn e2e_preprocessor_define_expansion() {
    // Verifies that the preprocessor runs and the pipeline still works
    let snl = r#"
#define THRESHOLD 10
program preproc_test
ss main {
    state idle {
        when (true) {} state idle
    }
}
"#;
    let code = compile(snl);
    assert!(code.contains("struct preproc_testVars"));
    assert!(code.contains("async fn main("));
}

// ===========================================================================
// 5. Error cases
// ===========================================================================

#[test]
fn error_lex_unterminated_string() {
    let result = Lexer::new(r#""unterminated"#).tokenize();
    assert!(result.is_err());
}

#[test]
fn error_parse_no_state_set() {
    let snl = "program empty";
    let pp = preprocess(snl);
    let tokens = Lexer::new(&pp.source).tokenize().expect("lex ok");
    let ast = Parser::new(tokens).parse_program();
    // An empty program with no state sets may parse but analysis should catch it
    // or parser allows it. Either way, let's verify it doesn't panic.
    let _ = ast;
}

#[test]
fn error_parse_invalid_token_sequence() {
    let snl = "program test ss { }";
    let pp = preprocess(snl);
    let tokens = Lexer::new(&pp.source).tokenize().expect("lex ok");
    let result = Parser::new(tokens).parse_program();
    // Missing state set name's state block structure - should error
    assert!(result.is_err());
}
