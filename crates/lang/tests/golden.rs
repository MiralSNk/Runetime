//! Golden tests: every `golden/<dir>/NAME.spell` is run through a stage and
//! the output is compared with `golden/<dir>/NAME.<ext>`.
//! Run with `BLESS=1` to rewrite the expected files.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use lang::ast::{Arg, BinaryOp, Expr, ExprKind, Stmt, UnaryOp};
use lang::diag::Diagnostic;
use lang::{check::check, lexer::tokenize, parser::parse};

fn run(dir: &str, ext: &str, render: fn(&str) -> String) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(dir);
    let bless = std::env::var_os("BLESS").is_some();
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "spell") {
            continue;
        }
        count += 1;
        let actual = render(&fs::read_to_string(&path).unwrap());
        let expected_path = path.with_extension(ext);
        if bless {
            fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "{}\n--- expected\n{expected}--- actual\n{actual}",
                path.display()
            ));
        }
    }
    assert!(count > 0, "no golden files in {}", dir.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn error(e: &Diagnostic) -> String {
    format!(
        "{}:{} error {} {:?}\n",
        e.pos.line,
        e.pos.col,
        e.code(),
        e.kind
    )
}

#[test]
fn lex() {
    run("lex", "tokens", |src| match tokenize(src) {
        Ok(tokens) => tokens.iter().fold(String::new(), |mut out, t| {
            writeln!(out, "{}:{} {:?}", t.pos.line, t.pos.col, t.kind).unwrap();
            out
        }),
        Err(e) => error(&e),
    });
}

/// Parses and checks; prints the AST as an indented outline on success.
#[test]
fn parse_and_check() {
    run("parse", "ast", |src| {
        let program = match parse(src) {
            Ok(p) => p,
            Err(e) => return error(&e),
        };
        if let Err(e) = check(&program) {
            return error(&e);
        }
        let mut out = String::new();
        for spell in &program.spells {
            let params: Vec<_> = spell.params.iter().map(|p| p.name.as_str()).collect();
            writeln!(out, "spell {}({})", spell.name.name, params.join(", ")).unwrap();
            stmts(&mut out, &spell.body, 1);
        }
        out
    });
}

fn stmts(out: &mut String, stmts: &[Stmt], depth: usize) {
    let pad = "  ".repeat(depth);
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, value } => writeln!(out, "{pad}let {} {}", name.name, expr(value)),
            Stmt::Assign { name, value } => writeln!(out, "{pad}set {} {}", name.name, expr(value)),
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                writeln!(out, "{pad}if {}", expr(cond)).unwrap();
                self::stmts(out, then, depth + 1);
                if let Some(o) = otherwise {
                    writeln!(out, "{pad}else").unwrap();
                    self::stmts(out, o, depth + 1);
                }
                writeln!(out, "{pad}end")
            }
            Stmt::While { cond, body } => {
                writeln!(out, "{pad}while {}", expr(cond)).unwrap();
                self::stmts(out, body, depth + 1);
                writeln!(out, "{pad}end")
            }
            Stmt::Emit { effect, args: a } => {
                writeln!(out, "{pad}emit ({}{})", effect.name, args(a))
            }
            Stmt::Call(c) => writeln!(out, "{pad}call ({}{})", c.name.name, args(&c.args)),
        }
        .unwrap();
    }
}

fn args(args: &[Arg]) -> String {
    args.iter()
        .map(|a| match &a.name {
            Some(n) => format!(" {}:{}", n.name, expr(&a.value)),
            None => format!(" {}", expr(&a.value)),
        })
        .collect()
}

fn expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Var(v) => v.clone(),
        ExprKind::Call(c) => format!("({}{})", c.name.name, args(&c.args)),
        ExprKind::Unary(op, x) => {
            let op = match op {
                UnaryOp::Neg => "neg",
                UnaryOp::Not => "not",
            };
            format!("({op} {})", expr(x))
        }
        ExprKind::Binary(op, l, r) => {
            let op = match op {
                BinaryOp::Or => "or",
                BinaryOp::And => "and",
                BinaryOp::Lt => "<",
                BinaryOp::Gt => ">",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
            };
            format!("({op} {} {})", expr(l), expr(r))
        }
    }
}
