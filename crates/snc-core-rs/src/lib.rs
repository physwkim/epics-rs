#![allow(
    clippy::approx_constant,
    clippy::doc_lazy_continuation,
    clippy::empty_line_after_doc_comments,
    clippy::let_and_return,
    clippy::manual_pattern_char_comparison,
    clippy::manual_strip,
    clippy::redundant_locals,
    clippy::unnecessary_map_or,
    clippy::useless_format
)]

pub mod analysis;
pub mod ast;
pub mod codegen;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod preprocess;

#[cfg(test)]
mod codegen_test;
