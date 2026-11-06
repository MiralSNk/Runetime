//! Name and type checking (`C002`–`C007`) by the static rules in
//! `docs/SPEC.md`.

use std::collections::HashMap;

use crate::ast::{Arg, BinaryOp, Call, Expr, ExprKind, Ident, Program, Stmt, UnaryOp};
use crate::diag::{Diagnostic, DiagnosticKind, Pos};

/// A value type of the spell language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    /// Signed 64-bit integer.
    Int,
    /// `true` or `false`.
    Bool,
    /// Opaque memory cell handle returned by `alloc`.
    Cell,
    /// Opaque reference to an entity, given as a spell parameter.
    Entity,
}

/// Signature of a builtin function or an effect.
struct Signature {
    name: &'static str,
    params: &'static [(&'static str, Ty)],
    ret: Option<Ty>,
}

const BUILTINS: &[Signature] = &[
    Signature {
        name: "alloc",
        params: &[("value", Ty::Int)],
        ret: Some(Ty::Cell),
    },
    Signature {
        name: "free",
        params: &[("cell", Ty::Cell)],
        ret: None,
    },
    Signature {
        name: "read",
        params: &[("cell", Ty::Cell)],
        ret: Some(Ty::Int),
    },
    Signature {
        name: "write",
        params: &[("cell", Ty::Cell), ("value", Ty::Int)],
        ret: None,
    },
    Signature {
        name: "distance",
        params: &[("a", Ty::Entity), ("b", Ty::Entity)],
        ret: Some(Ty::Int),
    },
];

const EFFECTS: &[Signature] = &[Signature {
    name: "Flame",
    params: &[("at", Ty::Entity), ("power", Ty::Int)],
    ret: None,
}];

/// Checks names, scopes, arguments and types of a parsed program.
///
/// # Errors
///
/// Returns the first `C002`–`C007` [`Diagnostic`] in source order.
///
/// # Examples
///
/// ```
/// use lang::{check::check, parser::parse};
///
/// let ok = parse("spell S(t):\n    emit Flame(at: t, power: 3)\nend").unwrap();
/// assert!(check(&ok).is_ok());
///
/// let bad = parse("spell S(t):\n    if 1:\n    end\nend").unwrap();
/// assert_eq!(check(&bad).unwrap_err().code(), "C004");
/// ```
pub fn check(program: &Program) -> std::result::Result<(), Diagnostic> {
    let mut spell_names = Vec::new();
    for spell in &program.spells {
        if spell_names.contains(&&spell.name.name) {
            return fail(
                DiagnosticKind::Redeclared(spell.name.name.clone()),
                spell.name.pos,
            );
        }
        spell_names.push(&spell.name.name);
        let mut cx = Checker::default();
        for param in &spell.params {
            cx.declare(param, Ty::Entity)?;
        }
        cx.stmts(&spell.body)?;
    }
    Ok(())
}

type Result<T> = std::result::Result<T, Diagnostic>;

fn fail<T>(kind: DiagnosticKind, pos: Pos) -> Result<T> {
    Err(Diagnostic { kind, pos })
}

struct Checker<'a> {
    /// Innermost scope last.
    scopes: Vec<HashMap<&'a str, Ty>>,
}

impl Default for Checker<'_> {
    fn default() -> Self {
        Checker {
            scopes: vec![HashMap::new()],
        }
    }
}

impl<'a> Checker<'a> {
    fn declare(&mut self, name: &'a Ident, ty: Ty) -> Result<()> {
        let scope = self.scopes.last_mut().expect("at least one scope");
        if scope.insert(&name.name, ty).is_some() {
            return fail(DiagnosticKind::Redeclared(name.name.clone()), name.pos);
        }
        Ok(())
    }

    fn lookup(&self, name: &str, pos: Pos) -> Result<Ty> {
        match self.scopes.iter().rev().find_map(|s| s.get(name)) {
            Some(&ty) => Ok(ty),
            None => fail(DiagnosticKind::UnknownName(name.to_owned()), pos),
        }
    }

    fn stmts(&mut self, stmts: &'a [Stmt]) -> Result<()> {
        stmts.iter().try_for_each(|s| self.stmt(s))
    }

    fn nested(&mut self, stmts: &'a [Stmt]) -> Result<()> {
        self.scopes.push(HashMap::new());
        self.stmts(stmts)?;
        self.scopes.pop();
        Ok(())
    }

    fn stmt(&mut self, stmt: &'a Stmt) -> Result<()> {
        match stmt {
            Stmt::Let { name, value } => {
                let ty = self.expr(value)?;
                self.declare(name, ty)
            }
            Stmt::Assign { name, value } => {
                let ty = self.lookup(&name.name, name.pos)?;
                self.expect(value, ty)
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expect(cond, Ty::Bool)?;
                self.nested(then)?;
                otherwise.as_deref().map_or(Ok(()), |o| self.nested(o))
            }
            Stmt::While { cond, body } => {
                self.expect(cond, Ty::Bool)?;
                self.nested(body)
            }
            Stmt::Emit { effect, args } => {
                let Some(sig) = EFFECTS.iter().find(|s| s.name == effect.name) else {
                    return fail(
                        DiagnosticKind::UnknownEffect(effect.name.clone()),
                        effect.pos,
                    );
                };
                if args.iter().any(|a| a.name.is_none()) {
                    return fail(DiagnosticKind::PositionalEmitArg, effect.pos);
                }
                self.args(sig, effect.pos, args)
            }
            Stmt::Call(call) => match self.call(call)? {
                Some(_) => fail(
                    DiagnosticKind::UnusedValue(call.name.name.clone()),
                    call.name.pos,
                ),
                None => Ok(()),
            },
        }
    }

    fn call(&mut self, call: &Call) -> Result<Option<Ty>> {
        let Some(sig) = BUILTINS.iter().find(|s| s.name == call.name.name) else {
            return fail(
                DiagnosticKind::UnknownBuiltin(call.name.name.clone()),
                call.name.pos,
            );
        };
        self.args(sig, call.name.pos, &call.args)?;
        Ok(sig.ret)
    }

    /// Argument shape errors are reported at the callee (`pos`), which keeps
    /// them ahead of any error inside the arguments in source order.
    fn args(&mut self, sig: &Signature, pos: Pos, args: &[Arg]) -> Result<()> {
        let named = args.iter().filter(|a| a.name.is_some()).count();
        if named != 0 && named != args.len() {
            return fail(DiagnosticKind::MixedArgs, pos);
        }
        if args.len() != sig.params.len() {
            let (expected, found) = (sig.params.len(), args.len());
            return fail(DiagnosticKind::ArgCount { expected, found }, pos);
        }
        let mut seen = Vec::new();
        for (i, arg) in args.iter().enumerate() {
            let ty = match &arg.name {
                None => sig.params[i].1,
                Some(name) => {
                    let Some(&(_, ty)) = sig.params.iter().find(|p| p.0 == name.name) else {
                        return fail(DiagnosticKind::UnknownArg(name.name.clone()), name.pos);
                    };
                    if seen.contains(&name.name) {
                        return fail(DiagnosticKind::DuplicateArg(name.name.clone()), name.pos);
                    }
                    seen.push(name.name.clone());
                    ty
                }
            };
            self.expect(&arg.value, ty)?;
        }
        Ok(())
    }

    fn expect(&mut self, expr: &Expr, expected: Ty) -> Result<()> {
        let found = self.expr(expr)?;
        if found != expected {
            return fail(DiagnosticKind::TypeMismatch { expected, found }, expr.pos);
        }
        Ok(())
    }

    /// An operand of arithmetic: `Int`, `C007` for opaque types, `C004` otherwise.
    fn arith(&mut self, expr: &Expr) -> Result<()> {
        match self.expr(expr)? {
            Ty::Int => Ok(()),
            ty @ (Ty::Cell | Ty::Entity) => fail(DiagnosticKind::OpaqueArithmetic(ty), expr.pos),
            found => fail(
                DiagnosticKind::TypeMismatch {
                    expected: Ty::Int,
                    found,
                },
                expr.pos,
            ),
        }
    }

    fn expr(&mut self, expr: &Expr) -> Result<Ty> {
        match &expr.kind {
            ExprKind::Int(_) => Ok(Ty::Int),
            ExprKind::Bool(_) => Ok(Ty::Bool),
            ExprKind::Var(name) => self.lookup(name, expr.pos),
            ExprKind::Call(call) => match self.call(call)? {
                Some(ty) => Ok(ty),
                None => fail(DiagnosticKind::NoValue(call.name.name.clone()), expr.pos),
            },
            ExprKind::Unary(UnaryOp::Neg, x) => self.arith(x).map(|()| Ty::Int),
            ExprKind::Unary(UnaryOp::Not, x) => self.expect(x, Ty::Bool).map(|()| Ty::Bool),
            ExprKind::Binary(op, l, r) => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                    self.arith(l)?;
                    self.arith(r)?;
                    Ok(Ty::Int)
                }
                BinaryOp::Lt | BinaryOp::Gt => {
                    self.expect(l, Ty::Int)?;
                    self.expect(r, Ty::Int)?;
                    Ok(Ty::Bool)
                }
                BinaryOp::Eq | BinaryOp::Ne => {
                    let ty = self.expr(l)?;
                    if !matches!(ty, Ty::Int | Ty::Bool) {
                        let kind = DiagnosticKind::TypeMismatch {
                            expected: Ty::Int,
                            found: ty,
                        };
                        return fail(kind, l.pos);
                    }
                    self.expect(r, ty)?;
                    Ok(Ty::Bool)
                }
                BinaryOp::And | BinaryOp::Or => {
                    self.expect(l, Ty::Bool)?;
                    self.expect(r, Ty::Bool)?;
                    Ok(Ty::Bool)
                }
            },
        }
    }
}
