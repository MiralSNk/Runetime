//! Bytecode generation for a checked program, by the conventions in the
//! "Bytecode" section of `docs/SPEC.md`.

use std::collections::HashMap;

use crate::ast::{Arg, BinaryOp, Call, Expr, ExprKind, Program, Spell, Stmt, UnaryOp};
use crate::bytecode::{Chunk, Instr};
use crate::check::{self, BUILTINS, EFFECTS, Signature};
use crate::diag::Diagnostic;
use crate::parser::parse;

/// Parses, checks and compiles `src`: one [`Chunk`] per spell, in source
/// order.
///
/// # Errors
///
/// Returns the compile error from `docs/SPEC.md` (`C001`–`C008`): the first
/// syntax error if there is one, otherwise the first check error in source
/// order.
///
/// # Examples
///
/// ```
/// let chunks = lang::compile("spell Spark(t):\n    emit Flame(at: t, power: 1)\nend").unwrap();
/// assert_eq!(chunks[0].name, "Spark");
///
/// let err = lang::compile("spell Spark(t):\n    let x <- y\nend").unwrap_err();
/// assert_eq!(err.code(), "C002");
/// ```
pub fn compile(src: &str) -> Result<Vec<Chunk>, Diagnostic> {
    let program = parse(src)?;
    check::check(&program)?;
    Ok(generate(&program))
}

/// Generates code for a program that already passed [`check::check`].
fn generate(program: &Program) -> Vec<Chunk> {
    program.spells.iter().map(spell).collect()
}

fn spell(spell: &Spell) -> Chunk {
    let mut g = Gen {
        code: Vec::new(),
        scopes: vec![HashMap::new()],
        slots: 0,
    };
    for param in &spell.params {
        g.declare(&param.name);
    }
    g.stmts(&spell.body);
    g.code.push(Instr::Ret);
    Chunk {
        name: spell.name.name.clone(),
        params: slot(spell.params.len()),
        locals: g.slots,
        code: g.code,
    }
}

/// The checker caps slots at `MAX_LOCALS` (64), so this never fails.
fn slot(n: usize) -> u16 {
    u16::try_from(n).expect("slot count is checked by C008")
}

struct Gen<'a> {
    code: Vec<Instr>,
    /// Name to local slot; innermost scope last.
    scopes: Vec<HashMap<&'a str, u16>>,
    slots: u16,
}

impl<'a> Gen<'a> {
    fn declare(&mut self, name: &'a str) -> u16 {
        let slot = self.slots;
        self.slots += 1;
        self.scopes
            .last_mut()
            .expect("at least one scope")
            .insert(name, slot);
        slot
    }

    fn lookup(&self, name: &str) -> u16 {
        *self
            .scopes
            .iter()
            .rev()
            .find_map(|s| s.get(name))
            .expect("names are checked by C002")
    }

    fn here(&self) -> u32 {
        // ponytail: u32 jump targets cap a spell at 4G instructions; far past
        // anything 64 locals and fuel limits let a player write.
        u32::try_from(self.code.len()).expect("spell too large")
    }

    /// Emits a jump with a placeholder target; returns its index for [`Self::patch`].
    fn jump(&mut self, instr: fn(u32) -> Instr) -> usize {
        self.code.push(instr(0));
        self.code.len() - 1
    }

    /// Points the jump at `at` to the next instruction to be emitted.
    fn patch(&mut self, at: usize) {
        let target = self.here();
        self.code[at] = match self.code[at] {
            Instr::Jmp(_) => Instr::Jmp(target),
            Instr::JmpIfFalse(_) => Instr::JmpIfFalse(target),
            other => unreachable!("patching a non-jump {other}"),
        };
    }

    fn stmts(&mut self, stmts: &'a [Stmt]) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn nested(&mut self, stmts: &'a [Stmt]) {
        self.scopes.push(HashMap::new());
        self.stmts(stmts);
        self.scopes.pop();
    }

    fn stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Let { name, value } => {
                self.expr(value);
                let slot = self.declare(&name.name);
                self.code.push(Instr::StoreLocal(slot));
            }
            Stmt::Assign { name, value } => {
                self.expr(value);
                let slot = self.lookup(&name.name);
                self.code.push(Instr::StoreLocal(slot));
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond);
                let to_else = self.jump(Instr::JmpIfFalse);
                self.nested(then);
                match otherwise {
                    None => self.patch(to_else),
                    Some(otherwise) => {
                        let to_end = self.jump(Instr::Jmp);
                        self.patch(to_else);
                        self.nested(otherwise);
                        self.patch(to_end);
                    }
                }
            }
            Stmt::While { cond, body } => {
                let start = self.here();
                self.expr(cond);
                let to_end = self.jump(Instr::JmpIfFalse);
                self.nested(body);
                self.code.push(Instr::Jmp(start));
                self.patch(to_end);
            }
            Stmt::Emit { effect, args } => {
                let id = index(EFFECTS, &effect.name);
                self.args(&EFFECTS[id], args);
                self.code.push(Instr::Emit(op(id)));
            }
            Stmt::Call(call) => self.call(call),
        }
    }

    fn call(&mut self, call: &Call) {
        let id = index(BUILTINS, &call.name.name);
        self.args(&BUILTINS[id], &call.args);
        self.code.push(Instr::CallBuiltin(op(id)));
    }

    /// Pushes arguments in parameter order, whatever order they were written in.
    fn args(&mut self, sig: &Signature, args: &[Arg]) {
        for (i, (param, _)) in sig.params.iter().enumerate() {
            let arg = args
                .iter()
                .find(|a| a.name.as_ref().is_some_and(|n| n.name == *param))
                .unwrap_or(&args[i]);
            self.expr(&arg.value);
        }
    }

    fn expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Int(n) => self.code.push(Instr::PushInt(*n)),
            ExprKind::Bool(b) => self.code.push(Instr::PushBool(u8::from(*b))),
            ExprKind::Var(name) => {
                let slot = self.lookup(name);
                self.code.push(Instr::LoadLocal(slot));
            }
            ExprKind::Call(call) => self.call(call),
            ExprKind::Unary(op, x) => {
                self.expr(x);
                self.code.push(match op {
                    UnaryOp::Neg => Instr::Neg,
                    UnaryOp::Not => Instr::Not,
                });
            }
            // l and r: if l is false the result is false, r is not evaluated.
            ExprKind::Binary(BinaryOp::And, l, r) => {
                self.expr(l);
                let to_false = self.jump(Instr::JmpIfFalse);
                self.expr(r);
                let to_end = self.jump(Instr::Jmp);
                self.patch(to_false);
                self.code.push(Instr::PushBool(0));
                self.patch(to_end);
            }
            // l or r: if l is true the result is true, r is not evaluated.
            ExprKind::Binary(BinaryOp::Or, l, r) => {
                self.expr(l);
                let to_right = self.jump(Instr::JmpIfFalse);
                self.code.push(Instr::PushBool(1));
                let to_end = self.jump(Instr::Jmp);
                self.patch(to_right);
                self.expr(r);
                self.patch(to_end);
            }
            ExprKind::Binary(op, l, r) => {
                self.expr(l);
                self.expr(r);
                self.code.push(match op {
                    BinaryOp::Add => Instr::Add,
                    BinaryOp::Sub => Instr::Sub,
                    BinaryOp::Mul => Instr::Mul,
                    BinaryOp::Div => Instr::Div,
                    BinaryOp::Lt => Instr::CmpLt,
                    BinaryOp::Gt => Instr::CmpGt,
                    BinaryOp::Eq => Instr::CmpEq,
                    BinaryOp::Ne => Instr::CmpNe,
                    BinaryOp::And | BinaryOp::Or => unreachable!("handled above"),
                });
            }
        }
    }
}

fn index(table: &[Signature], name: &str) -> usize {
    table
        .iter()
        .position(|s| s.name == name)
        .expect("callees are checked by C003")
}

fn op(id: usize) -> u8 {
    u8::try_from(id).expect("builtin and effect tables are tiny")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode;

    /// The checker's signature tables and the public operand tables must
    /// agree, since the index is the operand.
    #[test]
    fn signature_tables_match_operand_tables() {
        let names = |t: &[Signature]| t.iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(names(BUILTINS), bytecode::BUILTINS);
        assert_eq!(names(EFFECTS), bytecode::EFFECTS);
    }
}
