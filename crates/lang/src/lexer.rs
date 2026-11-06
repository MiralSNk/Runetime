//! Lexer for the spell language, following the grammar and the lexical rules
//! in `docs/SPEC.md`.

use std::iter::Peekable;
use std::str::Chars;

/// A position in the source: 1-based line and column (in `char`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    /// Line number, starting at 1.
    pub line: u32,
    /// Column number in `char`s, starting at 1.
    pub col: u32,
}

/// Kind of a lexical token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// `spell`
    Spell,
    /// `let`
    Let,
    /// `if`
    If,
    /// `else`
    Else,
    /// `while`
    While,
    /// `end`
    End,
    /// `emit`
    Emit,
    /// `and`
    And,
    /// `or`
    Or,
    /// `not`
    Not,
    /// `true`
    True,
    /// `false`
    False,
    /// Identifier that is not a reserved word.
    Ident(String),
    /// Non-negative integer literal.
    Number(i64),
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `<-`
    Arrow,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `==`
    EqEq,
    /// `!=`
    NotEq,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
}

/// A token with the position of its first character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// What the token is.
    pub kind: TokenKind,
    /// Where the token starts.
    pub pos: Pos,
}

/// Why the source could not be tokenized. All kinds map to `C001`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexErrorKind {
    /// A character that cannot start any token (including a lone `=` or `!`).
    UnexpectedChar(char),
    /// A number literal that does not fit in `i64`.
    NumberOverflow,
    /// A number literal directly followed by a letter or `_`.
    InvalidNumber,
}

/// A lexical error. Carries a code and a position; the player-facing text
/// is looked up by `loc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    /// What went wrong.
    pub kind: LexErrorKind,
    /// Where the offending token starts.
    pub pos: Pos,
}

impl LexError {
    /// Diagnostic code from `docs/SPEC.md`. Always `"C001"`.
    pub fn code(&self) -> &'static str {
        "C001"
    }
}

/// Splits `src` into tokens, stopping at the first error.
///
/// # Errors
///
/// Returns [`LexError`] (code `C001`) on an unexpected character, a number
/// literal that overflows `i64`, or a number directly followed by a letter
/// or `_`.
///
/// # Examples
///
/// ```
/// use lang::lexer::{tokenize, TokenKind};
///
/// let kinds: Vec<_> = tokenize("let x <- 12").unwrap().into_iter().map(|t| t.kind).collect();
/// assert_eq!(
///     kinds,
///     [TokenKind::Let, TokenKind::Ident("x".into()), TokenKind::Arrow, TokenKind::Number(12)]
/// );
///
/// let err = tokenize("let x = 1").unwrap_err();
/// assert_eq!((err.code(), err.pos.line, err.pos.col), ("C001", 1, 7));
/// ```
pub fn tokenize(src: &str) -> Result<Vec<Token>, LexError> {
    let mut lx = Lexer {
        chars: src.chars().peekable(),
        pos: Pos { line: 1, col: 1 },
    };
    let mut tokens = Vec::new();
    while let Some(c) = lx.chars.peek().copied() {
        let pos = lx.pos;
        let kind = match c {
            ' ' | '\t' | '\r' | '\n' => {
                lx.bump();
                continue;
            }
            '#' => {
                while lx.chars.peek().is_some_and(|&c| c != '\n') {
                    lx.bump();
                }
                continue;
            }
            'a'..='z' | 'A'..='Z' => lx.word(),
            '0'..='9' => lx.number(pos)?,
            _ => {
                lx.bump();
                match (c, lx.chars.peek()) {
                    ('<', Some('-')) => lx.then(TokenKind::Arrow),
                    ('=', Some('=')) => lx.then(TokenKind::EqEq),
                    ('!', Some('=')) => lx.then(TokenKind::NotEq),
                    ('<', _) => TokenKind::Lt,
                    ('>', _) => TokenKind::Gt,
                    ('(', _) => TokenKind::LParen,
                    (')', _) => TokenKind::RParen,
                    (',', _) => TokenKind::Comma,
                    (':', _) => TokenKind::Colon,
                    ('+', _) => TokenKind::Plus,
                    ('-', _) => TokenKind::Minus,
                    ('*', _) => TokenKind::Star,
                    ('/', _) => TokenKind::Slash,
                    _ => {
                        return Err(LexError {
                            kind: LexErrorKind::UnexpectedChar(c),
                            pos,
                        });
                    }
                }
            }
        };
        tokens.push(Token { kind, pos });
    }
    Ok(tokens)
}

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    pos: Pos,
}

impl Lexer<'_> {
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;
        if c == '\n' {
            self.pos.line += 1;
            self.pos.col = 1;
        } else {
            self.pos.col += 1;
        }
        Some(c)
    }

    /// Consumes the second char of a two-char operator.
    fn then(&mut self, kind: TokenKind) -> TokenKind {
        self.bump();
        kind
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let mut s = String::new();
        while let Some(&c) = self.chars.peek().filter(|&&c| f(c)) {
            s.push(c);
            self.bump();
        }
        s
    }

    fn word(&mut self) -> TokenKind {
        let s = self.take_while(|c| c.is_ascii_alphanumeric() || c == '_');
        match s.as_str() {
            "spell" => TokenKind::Spell,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "end" => TokenKind::End,
            "emit" => TokenKind::Emit,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(s),
        }
    }

    fn number(&mut self, pos: Pos) -> Result<TokenKind, LexError> {
        let digits = self.take_while(|c| c.is_ascii_digit());
        let err = |kind| Err(LexError { kind, pos });
        if self
            .chars
            .peek()
            .is_some_and(|&c| c.is_ascii_alphabetic() || c == '_')
        {
            return err(LexErrorKind::InvalidNumber);
        }
        match digits.parse() {
            Ok(n) => Ok(TokenKind::Number(n)),
            Err(_) => err(LexErrorKind::NumberOverflow),
        }
    }
}
