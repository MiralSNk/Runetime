//! Recursive-descent parser for the grammar in `docs/SPEC.md`.

use crate::ast::{Arg, BinaryOp, Call, Expr, ExprKind, Ident, Program, Spell, Stmt, UnaryOp};
use crate::diag::{Diagnostic, DiagnosticKind, Expected, Pos};
use crate::lexer::{Token, TokenKind, tokenize};

/// Tokenizes and parses `src` into a [`Program`]. Does not check names or
/// types; see [`crate::check::check`].
///
/// # Errors
///
/// Returns the first `C001` [`Diagnostic`]: a lexical error or a token that
/// does not fit the grammar.
///
/// # Examples
///
/// ```
/// let program = lang::parser::parse("spell Spark(t):\nend").unwrap();
/// assert_eq!(program.spells[0].name.name, "Spark");
///
/// let err = lang::parser::parse("spell Spark(t):").unwrap_err();
/// assert_eq!((err.code(), err.pos.line, err.pos.col), ("C001", 1, 16));
/// ```
pub fn parse(src: &str) -> std::result::Result<Program, Diagnostic> {
    Parser {
        tokens: tokenize(src)?,
        i: 0,
    }
    .program()
}

type Result<T> = std::result::Result<T, Diagnostic>;

struct Parser {
    /// Always ends with [`TokenKind::Eof`].
    tokens: Vec<Token>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &TokenKind {
        &self.tokens[self.i].kind
    }

    fn peek2(&self) -> &TokenKind {
        &self.tokens[(self.i + 1).min(self.tokens.len() - 1)].kind
    }

    fn pos(&self) -> Pos {
        self.tokens[self.i].pos
    }

    fn advance(&mut self) {
        if *self.peek() != TokenKind::Eof {
            self.i += 1;
        }
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        let hit = self.peek() == kind;
        if hit {
            self.advance();
        }
        hit
    }

    fn error(&self, expected: Expected) -> Diagnostic {
        Diagnostic {
            kind: DiagnosticKind::UnexpectedToken {
                expected,
                found: self.peek().clone(),
            },
            pos: self.pos(),
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        if self.eat(&kind) {
            Ok(())
        } else {
            Err(self.error(Expected::Token(kind)))
        }
    }

    fn ident(&mut self) -> Result<Ident> {
        let TokenKind::Ident(name) = self.peek() else {
            return Err(self.error(Expected::Ident));
        };
        let ident = Ident {
            name: name.clone(),
            pos: self.pos(),
        };
        self.advance();
        Ok(ident)
    }

    fn program(&mut self) -> Result<Program> {
        let mut spells = Vec::new();
        while *self.peek() != TokenKind::Eof {
            if !self.eat(&TokenKind::Spell) {
                return Err(self.error(Expected::Spell));
            }
            let name = self.ident()?;
            self.expect(TokenKind::LParen)?;
            let mut params = Vec::new();
            if *self.peek() != TokenKind::RParen {
                params.push(self.ident()?);
                while self.eat(&TokenKind::Comma) {
                    params.push(self.ident()?);
                }
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::Colon)?;
            let body = self.block(&[TokenKind::End])?;
            self.expect(TokenKind::End)?;
            spells.push(Spell { name, params, body });
        }
        Ok(Program { spells })
    }

    /// Parses statements until one of `closers` (left unconsumed).
    fn block(&mut self, closers: &[TokenKind]) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        while !closers.contains(self.peek()) {
            stmts.push(self.statement()?);
        }
        Ok(stmts)
    }

    fn statement(&mut self) -> Result<Stmt> {
        match self.peek() {
            TokenKind::Let => {
                self.advance();
                let name = self.ident()?;
                self.expect(TokenKind::Arrow)?;
                let value = self.expr()?;
                Ok(Stmt::Let { name, value })
            }
            TokenKind::If => {
                self.advance();
                let cond = self.expr()?;
                self.expect(TokenKind::Colon)?;
                let then = self.block(&[TokenKind::Else, TokenKind::End])?;
                let otherwise = if self.eat(&TokenKind::Else) {
                    self.expect(TokenKind::Colon)?;
                    Some(self.block(&[TokenKind::End])?)
                } else {
                    None
                };
                self.expect(TokenKind::End)?;
                Ok(Stmt::If {
                    cond,
                    then,
                    otherwise,
                })
            }
            TokenKind::While => {
                self.advance();
                let cond = self.expr()?;
                self.expect(TokenKind::Colon)?;
                let body = self.block(&[TokenKind::End])?;
                self.expect(TokenKind::End)?;
                Ok(Stmt::While { cond, body })
            }
            TokenKind::Emit => {
                self.advance();
                let effect = self.ident()?;
                let args = self.args()?;
                Ok(Stmt::Emit { effect, args })
            }
            TokenKind::Ident(_) if *self.peek2() == TokenKind::LParen => {
                Ok(Stmt::Call(self.call()?))
            }
            TokenKind::Ident(_) => {
                let name = self.ident()?;
                self.expect(TokenKind::Arrow)?;
                let value = self.expr()?;
                Ok(Stmt::Assign { name, value })
            }
            _ => Err(self.error(Expected::Statement)),
        }
    }

    fn call(&mut self) -> Result<Call> {
        let name = self.ident()?;
        let args = self.args()?;
        Ok(Call { name, args })
    }

    /// `"(" [ arg { "," arg } ] ")"`
    fn args(&mut self) -> Result<Vec<Arg>> {
        self.expect(TokenKind::LParen)?;
        let mut args = Vec::new();
        if *self.peek() != TokenKind::RParen {
            args.push(self.arg()?);
            while self.eat(&TokenKind::Comma) {
                args.push(self.arg()?);
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(args)
    }

    fn arg(&mut self) -> Result<Arg> {
        let name =
            if matches!(self.peek(), TokenKind::Ident(_)) && *self.peek2() == TokenKind::Colon {
                let name = self.ident()?;
                self.advance();
                Some(name)
            } else {
                None
            };
        let value = self.expr()?;
        Ok(Arg { name, value })
    }

    fn expr(&mut self) -> Result<Expr> {
        self.left_assoc(Self::and_expr, |t| {
            (*t == TokenKind::Or).then_some(BinaryOp::Or)
        })
    }

    fn and_expr(&mut self) -> Result<Expr> {
        self.left_assoc(Self::cmp_expr, |t| {
            (*t == TokenKind::And).then_some(BinaryOp::And)
        })
    }

    /// Comparison is non-associative: at most one operator.
    fn cmp_expr(&mut self) -> Result<Expr> {
        let left = self.add_expr()?;
        let op = match self.peek() {
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::NotEq => BinaryOp::Ne,
            _ => return Ok(left),
        };
        self.advance();
        let right = self.add_expr()?;
        Ok(binary(op, left, right))
    }

    fn add_expr(&mut self) -> Result<Expr> {
        self.left_assoc(Self::mul_expr, |t| match t {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Sub),
            _ => None,
        })
    }

    fn mul_expr(&mut self) -> Result<Expr> {
        self.left_assoc(Self::unary, |t| match t {
            TokenKind::Star => Some(BinaryOp::Mul),
            TokenKind::Slash => Some(BinaryOp::Div),
            _ => None,
        })
    }

    fn left_assoc(
        &mut self,
        operand: fn(&mut Self) -> Result<Expr>,
        op: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> Result<Expr> {
        let mut left = operand(self)?;
        while let Some(op) = op(self.peek()) {
            self.advance();
            let right = operand(self)?;
            left = binary(op, left, right);
        }
        Ok(left)
    }

    /// `[ "-" | "not" ] primary`: a single prefix operator at most.
    fn unary(&mut self) -> Result<Expr> {
        let op = match self.peek() {
            TokenKind::Minus => UnaryOp::Neg,
            TokenKind::Not => UnaryOp::Not,
            _ => return self.primary(),
        };
        let pos = self.pos();
        self.advance();
        let operand = self.primary()?;
        Ok(Expr {
            kind: ExprKind::Unary(op, Box::new(operand)),
            pos,
        })
    }

    fn primary(&mut self) -> Result<Expr> {
        let pos = self.pos();
        let kind = match self.peek() {
            TokenKind::Number(n) => ExprKind::Int(*n),
            TokenKind::True => ExprKind::Bool(true),
            TokenKind::False => ExprKind::Bool(false),
            TokenKind::Ident(_) if *self.peek2() == TokenKind::LParen => {
                return Ok(Expr {
                    kind: ExprKind::Call(self.call()?),
                    pos,
                });
            }
            TokenKind::Ident(name) => ExprKind::Var(name.clone()),
            TokenKind::LParen => {
                self.advance();
                let inner = self.expr()?;
                self.expect(TokenKind::RParen)?;
                return Ok(inner);
            }
            _ => return Err(self.error(Expected::Expression)),
        };
        self.advance();
        Ok(Expr { kind, pos })
    }
}

fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
    Expr {
        pos: left.pos,
        kind: ExprKind::Binary(op, Box::new(left), Box::new(right)),
    }
}
