//! Player-facing text built from `loc` strings: diagnostics, fuel cost and
//! the cast journal. No drawing here, so every key is checked by tests.

use lang::diag::{Diagnostic, DiagnosticKind, Expected};
use lang::lexer::TokenKind;
use std::fmt::Display;
use vm::{CellInfo, Corruption, ErrorKind, Estimate, Outcome, RuntimeError};

/// The `loc` string for `key` with `args` filled in.
///
/// # Panics
///
/// If `loc` has no such key: a missing string is a bug, not a player error.
pub fn tr(key: &str, args: &[&dyn Display]) -> String {
    loc::format(key, args).unwrap_or_else(|| panic!("loc has no string for {key}"))
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The text.
    pub text: String,
    /// Whether the line reports an error and should stand out.
    pub error: bool,
}

impl Line {
    fn plain(text: String) -> Self {
        Line { text, error: false }
    }

    fn error(text: String) -> Self {
        Line { text, error: true }
    }
}

/// A compile error as the grimoire shows it: code, line, column, reason.
pub fn diagnostic(d: &Diagnostic) -> String {
    use DiagnosticKind::*;
    let reason = match &d.kind {
        UnexpectedChar(c) => tr("error.C001.unexpected_char", &[c]),
        NumberOverflow => tr("error.C001.number_overflow", &[]),
        InvalidNumber => tr("error.C001.invalid_number", &[]),
        UnexpectedToken { expected, found } => tr(
            "error.C001.unexpected_token",
            &[&expect(expected), &token(found)],
        ),
        UnknownName(n) => tr("error.C002", &[n]),
        UnknownBuiltin(n) => tr("error.C003.builtin", &[n]),
        UnknownEffect(n) => tr("error.C003.effect", &[n]),
        TypeMismatch { expected, found } => tr(
            "error.C004.type_mismatch",
            &[&format!("{expected:?}"), &format!("{found:?}")],
        ),
        NoValue(n) => tr("error.C004.no_value", &[n]),
        UnusedValue(n) => tr("error.C004.unused_value", &[n]),
        ArgCount { expected, found } => tr("error.C005.arg_count", &[expected, found]),
        UnknownArg(n) => tr("error.C005.unknown_arg", &[n]),
        DuplicateArg(n) => tr("error.C005.duplicate_arg", &[n]),
        MixedArgs => tr("error.C005.mixed_args", &[]),
        PositionalEmitArg => tr("error.C005.positional_emit_arg", &[]),
        Redeclared(n) => tr("error.C006", &[n]),
        OpaqueArithmetic(ty) => tr("error.C007", &[&format!("{ty:?}")]),
        TooManyLocals => tr("error.C008", &[]),
        TooDeep => tr("error.C009", &[]),
    };
    tr(
        "grimoire.error",
        &[&d.code(), &d.pos.line, &d.pos.col, &reason],
    )
}

fn expect(e: &Expected) -> String {
    match e {
        Expected::Token(t) => token(t),
        Expected::Ident => tr("token.ident", &[]),
        Expected::Expression => tr("expected.expression", &[]),
        Expected::Statement => tr("expected.statement", &[]),
        Expected::Spell => tr("expected.spell", &[]),
    }
}

/// A token as the player wrote it; spell language tokens are never
/// translated, only the names of token classes are.
fn token(t: &TokenKind) -> String {
    use TokenKind::*;
    let s = match t {
        Ident(name) => return format!("{} {name}", tr("token.ident", &[])),
        Number(n) => return format!("{} {n}", tr("token.number", &[])),
        Eof => return tr("token.eof", &[]),
        Spell => "spell",
        Let => "let",
        If => "if",
        Else => "else",
        While => "while",
        End => "end",
        Emit => "emit",
        And => "and",
        Or => "or",
        Not => "not",
        True => "true",
        False => "false",
        LParen => "(",
        RParen => ")",
        Comma => ",",
        Colon => ":",
        Arrow => "<-",
        Lt => "<",
        Gt => ">",
        EqEq => "==",
        NotEq => "!=",
        Plus => "+",
        Minus => "-",
        Star => "*",
        Slash => "/",
    };
    tr("token.literal", &[&s])
}

/// The fuel cost line shown after a successful build.
pub fn cost(e: Estimate) -> String {
    match e {
        Estimate::Exact(n) => tr("grimoire.cost.exact", &[&n]),
        Estimate::AtMost(n) => tr("grimoire.cost.at_most", &[&n]),
        Estimate::Loop => tr("grimoire.cost.loop", &[]),
    }
}

/// A runtime error with its code, instruction and reason.
pub fn runtime_error(e: &RuntimeError) -> String {
    let cell = |key: &str, c: &CellInfo| {
        let freed = c.freed_at.map_or("-".to_owned(), |f| f.to_string());
        tr(key, &[&c.cell, &c.allocated_at, &freed])
    };
    let reason = match &e.kind {
        ErrorKind::NoSuchCell(c) => tr("error.R002", &[c]),
        ErrorKind::UseAfterFree(c) => cell("error.R003", c),
        ErrorKind::DoubleFree(c) => cell("error.R004", c),
        ErrorKind::Corrupt(Corruption::ArgCount) => tr("error.R010.arg_count", &[]),
        kind => tr(&format!("error.{}", kind.code()), &[]),
    };
    tr("journal.error", &[&e.kind.code(), &e.at, &reason])
}

/// The journal of one cast: summary, errors, then every executed instruction.
pub fn journal(o: &Outcome, fuel: u64) -> Vec<Line> {
    let mut lines = vec![Line::plain(tr(
        "journal.summary",
        &[&o.fuel_used, &fuel, &o.executed()],
    ))];
    let errors: Vec<_> = o.fault.iter().chain(&o.error).collect();
    if errors.is_empty() {
        lines.push(Line::plain(tr("journal.ok", &[])));
    }
    lines.extend(errors.iter().map(|e| Line::error(runtime_error(e))));
    let failed: Vec<u32> = errors.iter().map(|e| e.at).collect();
    for t in &o.trace {
        let mut text = tr(
            "journal.entry",
            &[&format!("{:>3}", t.index), &t.instr, &t.fuel_left],
        );
        if let Some(c) = &t.memory {
            let key = if c.freed_at.is_some() {
                "journal.cell.freed"
            } else {
                "journal.cell"
            };
            let freed = c.freed_at.unwrap_or_default();
            text = format!("{text} · {}", tr(key, &[&c.cell, &c.allocated_at, &freed]));
        }
        let error = failed.contains(&t.index);
        lines.push(Line { text, error });
    }
    lines
}

/// Splits `text` into rows of at most `width` chars, breaking at spaces.
/// A single word longer than `width` stays whole.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut rows = vec![String::new()];
    for piece in text.split_inclusive(' ') {
        let row = rows.last_mut().expect("rows is never empty");
        if !row.is_empty() && row.chars().count() + piece.trim_end().chars().count() > width {
            rows.push(String::new());
        }
        rows.last_mut()
            .expect("rows is never empty")
            .push_str(piece);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_breaks_at_spaces() {
        assert_eq!(wrap("  9  ab cd", 7), ["  9  ab ", "cd"]);
        assert_eq!(wrap("abcdefgh x", 4), ["abcdefgh ", "x"]);
        assert_eq!(wrap("", 4), [""]);
    }
    use lang::check::Ty;
    use lang::diag::Pos;

    #[test]
    fn every_diagnostic_has_text() {
        use DiagnosticKind::*;
        let n = || "x".to_owned();
        let kinds = [
            UnexpectedChar('$'),
            NumberOverflow,
            InvalidNumber,
            UnexpectedToken {
                expected: Expected::Ident,
                found: TokenKind::Eof,
            },
            UnexpectedToken {
                expected: Expected::Expression,
                found: TokenKind::Number(1),
            },
            UnexpectedToken {
                expected: Expected::Statement,
                found: TokenKind::Ident(n()),
            },
            UnexpectedToken {
                expected: Expected::Spell,
                found: TokenKind::Arrow,
            },
            UnexpectedToken {
                expected: Expected::Token(TokenKind::Colon),
                found: TokenKind::End,
            },
            UnknownName(n()),
            UnknownBuiltin(n()),
            UnknownEffect(n()),
            TypeMismatch {
                expected: Ty::Int,
                found: Ty::Cell,
            },
            NoValue(n()),
            UnusedValue(n()),
            ArgCount {
                expected: 1,
                found: 2,
            },
            UnknownArg(n()),
            DuplicateArg(n()),
            MixedArgs,
            PositionalEmitArg,
            Redeclared(n()),
            OpaqueArithmetic(Ty::Entity),
            TooManyLocals,
            TooDeep,
        ];
        for kind in kinds {
            let d = Diagnostic {
                kind,
                pos: Pos { line: 3, col: 7 },
            };
            let text = diagnostic(&d);
            assert!(text.starts_with(d.code()) && !text.contains('{'), "{text}");
        }
    }

    #[test]
    fn every_runtime_error_has_text() {
        let c = CellInfo {
            cell: 0,
            allocated_at: 3,
            freed_at: Some(6),
        };
        let kinds = [
            ErrorKind::FrameOverflow,
            ErrorKind::NoSuchCell(4),
            ErrorKind::UseAfterFree(c),
            ErrorKind::DoubleFree(c),
            ErrorKind::DivisionByZero,
            ErrorKind::IntegerOverflow,
            ErrorKind::TooManyCells,
            ErrorKind::OutOfFuel,
            ErrorKind::OperandOverflow,
            ErrorKind::Corrupt(Corruption::ArgCount),
            ErrorKind::Corrupt(Corruption::NoReturn),
        ];
        for kind in kinds {
            let text = runtime_error(&RuntimeError { kind, at: 9 });
            assert!(
                text.starts_with(kind.code()) && !text.contains('{'),
                "{text}"
            );
        }
        for e in [Estimate::Exact(1), Estimate::AtMost(1), Estimate::Loop] {
            assert!(!cost(e).contains('{'));
        }
    }
}
