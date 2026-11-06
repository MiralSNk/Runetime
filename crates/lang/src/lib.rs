//! Spell language front end: lexer, parser, AST, name and type checking
//! (the bytecode compiler comes in a later step).

pub mod ast;
pub mod check;
pub mod diag;
pub mod lexer;
pub mod parser;
