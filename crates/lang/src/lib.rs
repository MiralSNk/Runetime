//! Spell language front end: lexer, parser, AST, name and type checking and
//! the bytecode compiler. The entry point is [`compile`].

pub mod ast;
pub mod bytecode;
pub mod check;
mod compile;
pub mod diag;
pub mod lexer;
pub mod parser;

pub use compile::compile;
