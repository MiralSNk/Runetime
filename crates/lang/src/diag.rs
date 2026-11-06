//! Compile-time diagnostics. The core produces a code and structured details;
//! the player-facing text is looked up by `loc`.

use crate::check::Ty;
use crate::lexer::TokenKind;

/// A position in the source: 1-based line and column (in `char`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    /// Line number, starting at 1.
    pub line: u32,
    /// Column number in `char`s, starting at 1.
    pub col: u32,
}

/// A compile error: what went wrong and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What went wrong.
    pub kind: DiagnosticKind,
    /// Where the offending token or expression starts.
    pub pos: Pos,
}

/// What the parser was looking for when it hit an unexpected token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// This exact token.
    Token(TokenKind),
    /// Any identifier.
    Ident,
    /// The start of an expression.
    Expression,
    /// The start of a statement, or the token that closes the block.
    Statement,
    /// `spell` or the end of the source.
    Spell,
}

/// Details of a [`Diagnostic`]. Each kind maps to one code from `docs/SPEC.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// `C001`: a character that cannot start any token.
    UnexpectedChar(char),
    /// `C001`: a number literal that does not fit in `Int`.
    NumberOverflow,
    /// `C001`: a number literal directly followed by a letter or `_`.
    InvalidNumber,
    /// `C001`: a token that does not fit the grammar here.
    UnexpectedToken {
        /// What the grammar allows here.
        expected: Expected,
        /// What was actually found.
        found: TokenKind,
    },
    /// `C002`: a name that is not declared in any visible scope.
    UnknownName(String),
    /// `C003`: a call to a builtin function that does not exist.
    UnknownBuiltin(String),
    /// `C003`: `emit` of an effect that does not exist.
    UnknownEffect(String),
    /// `C004`: a value of the wrong type.
    TypeMismatch {
        /// The type required here.
        expected: Ty,
        /// The type actually given.
        found: Ty,
    },
    /// `C004`: a builtin without a return value used inside an expression.
    NoValue(String),
    /// `C004`: a builtin with a return value used as a statement.
    UnusedValue(String),
    /// `C005`: wrong number of arguments.
    ArgCount {
        /// Number of parameters of the callee.
        expected: usize,
        /// Number of arguments given.
        found: usize,
    },
    /// `C005`: a named argument the callee does not have.
    UnknownArg(String),
    /// `C005`: the same named argument given twice.
    DuplicateArg(String),
    /// `C005`: named and positional arguments mixed in one call.
    MixedArgs,
    /// `C005`: a positional argument given to `emit`.
    PositionalEmitArg,
    /// `C006`: a name declared twice in the same scope.
    Redeclared(String),
    /// `C007`: arithmetic over a `Cell` or an `Entity`.
    OpaqueArithmetic(Ty),
    /// `C008`: more than [`crate::bytecode::MAX_LOCALS`] local variables in a spell.
    TooManyLocals,
}

impl Diagnostic {
    /// Diagnostic code from `docs/SPEC.md`, such as `"C001"`.
    ///
    /// # Examples
    ///
    /// ```
    /// let err = lang::lexer::tokenize("$").unwrap_err();
    /// assert_eq!(err.code(), "C001");
    /// ```
    pub fn code(&self) -> &'static str {
        use DiagnosticKind::*;
        match self.kind {
            UnexpectedChar(_) | NumberOverflow | InvalidNumber | UnexpectedToken { .. } => "C001",
            UnknownName(_) => "C002",
            UnknownBuiltin(_) | UnknownEffect(_) => "C003",
            TypeMismatch { .. } | NoValue(_) | UnusedValue(_) => "C004",
            ArgCount { .. } | UnknownArg(_) | DuplicateArg(_) | MixedArgs | PositionalEmitArg => {
                "C005"
            }
            Redeclared(_) => "C006",
            OpaqueArithmetic(_) => "C007",
            TooManyLocals => "C008",
        }
    }
}
