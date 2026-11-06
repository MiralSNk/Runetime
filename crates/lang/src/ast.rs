//! Abstract syntax tree produced by [`crate::parser`].

use crate::diag::Pos;

/// A whole source file: zero or more spells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Spells in source order.
    pub spells: Vec<Spell>,
}

/// An identifier with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// The name as written.
    pub name: String,
    /// Where the name starts.
    pub pos: Pos,
}

/// `spell name(params): body end`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spell {
    /// Spell name.
    pub name: Ident,
    /// Parameter names; all parameters are `Entity`.
    pub params: Vec<Ident>,
    /// Statements of the body.
    pub body: Vec<Stmt>,
}

/// A statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// `let name <- value`
    Let {
        /// Declared name.
        name: Ident,
        /// Initial value.
        value: Expr,
    },
    /// `name <- value`
    Assign {
        /// Assigned name.
        name: Ident,
        /// New value.
        value: Expr,
    },
    /// `if cond: then [else: otherwise] end`
    If {
        /// Condition.
        cond: Expr,
        /// Taken when `cond` is `true`.
        then: Vec<Stmt>,
        /// Taken when `cond` is `false`; `None` without `else`.
        otherwise: Option<Vec<Stmt>>,
    },
    /// `while cond: body end`
    While {
        /// Condition.
        cond: Expr,
        /// Loop body.
        body: Vec<Stmt>,
    },
    /// `emit Effect(args)`
    Emit {
        /// Effect name.
        effect: Ident,
        /// Arguments.
        args: Vec<Arg>,
    },
    /// A builtin call used as a statement.
    Call(Call),
}

/// An expression with the position of its first token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// What the expression is.
    pub kind: ExprKind,
    /// Where the expression starts.
    pub pos: Pos,
}

/// Kind of an [`Expr`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// Integer literal.
    Int(i64),
    /// `true` or `false`.
    Bool(bool),
    /// Variable reference.
    Var(String),
    /// Builtin call.
    Call(Call),
    /// Unary operation.
    Unary(UnaryOp, Box<Expr>),
    /// Binary operation.
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
}

/// `name(args)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Callee name.
    pub name: Ident,
    /// Arguments.
    pub args: Vec<Arg>,
}

/// A call argument, optionally named: `[name:] value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arg {
    /// Argument name, if given.
    pub name: Option<Ident>,
    /// Argument value.
    pub value: Expr,
}

/// Unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `-`
    Neg,
    /// `not`
    Not,
}

/// Binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `or`
    Or,
    /// `and`
    And,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
}
