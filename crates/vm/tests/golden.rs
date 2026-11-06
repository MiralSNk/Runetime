//! Golden tests: every `golden/NAME.spell` (compiled with `lang`) or
//! `golden/NAME.asm` (hand-written bytecode) is run as one cast and the
//! outcome is compared with `golden/NAME.out`. Run with `BLESS=1` to rewrite.
//!
//! Directives in the source, as comment lines:
//! - `# fuel: N` — fuel for the cast (default 10000);
//! - `# args: A B ...` — entity indices for the parameters
//!   (default `1 2 ...`, one per parameter);
//! - `# params: N`, `# locals: N` — chunk header for `.asm` files.
//!
//! The arena is always [`ENTITIES`] and the caster is entity 0. Every cast is
//! run twice and both outcomes must be equal (determinism).

use std::fmt::Write;
use std::fs;
use std::path::Path;

use lang::bytecode::{Chunk, EFFECTS, Instr};
use vm::{Cast, Point, run};

const ENTITIES: [Point; 3] = [
    Point { x: 0, y: 0 },
    Point { x: 3, y: 4 },
    Point { x: 6, y: 8 },
];

fn directive<'a>(src: &'a str, key: &str) -> Option<&'a str> {
    src.lines()
        .find_map(|l| l.strip_prefix("# ")?.strip_prefix(key)?.strip_prefix(':'))
        .map(str::trim)
}

fn number<T: std::str::FromStr>(s: &str) -> T {
    s.parse().unwrap_or_else(|_| panic!("bad number `{s}`"))
}

/// Parses one instruction per non-comment line, in disassembler syntax.
fn assemble(src: &str) -> Chunk {
    let code = src
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|line| {
            let (op, arg) = line.split_once(' ').unwrap_or((line, ""));
            match op {
                "PUSH_INT" => Instr::PushInt(number(arg)),
                "PUSH_BOOL" => Instr::PushBool(number(arg)),
                "LOAD_LOCAL" => Instr::LoadLocal(number(arg)),
                "STORE_LOCAL" => Instr::StoreLocal(number(arg)),
                "ADD" => Instr::Add,
                "SUB" => Instr::Sub,
                "MUL" => Instr::Mul,
                "DIV" => Instr::Div,
                "NEG" => Instr::Neg,
                "NOT" => Instr::Not,
                "CMP_LT" => Instr::CmpLt,
                "CMP_GT" => Instr::CmpGt,
                "CMP_EQ" => Instr::CmpEq,
                "CMP_NE" => Instr::CmpNe,
                "JMP" => Instr::Jmp(number(arg)),
                "JMP_IF_FALSE" => Instr::JmpIfFalse(number(arg)),
                "CALL_BUILTIN" => Instr::CallBuiltin(number(arg)),
                "EMIT" => Instr::Emit(number(arg)),
                "RET" => Instr::Ret,
                "HALT" => Instr::Halt,
                _ => panic!("unknown opcode `{op}`"),
            }
        })
        .collect();
    Chunk {
        name: "asm".into(),
        params: directive(src, "params").map_or(0, number),
        locals: directive(src, "locals").map_or(0, number),
        code,
    }
}

fn render(chunk: &Chunk, src: &str) -> String {
    let fuel = directive(src, "fuel").map_or(10_000, number);
    let args: Vec<u32> = match directive(src, "args") {
        Some(list) => list.split_whitespace().map(number).collect(),
        None => (1..=u32::from(chunk.params)).collect(),
    };
    let cast = Cast {
        entities: &ENTITIES,
        caster: 0,
        args: &args,
        fuel,
    };
    let outcome = run(chunk, &cast);
    assert_eq!(outcome, run(chunk, &cast), "cast is not deterministic");

    let mut out = String::new();
    for t in &outcome.trace {
        write!(out, "{:04} {} fuel={}", t.index, t.instr, t.fuel_left).unwrap();
        if let Some(m) = t.memory {
            write!(out, " cell={} alloc={:04}", m.cell, m.allocated_at).unwrap();
            if let Some(f) = m.freed_at {
                write!(out, " free={f:04}").unwrap();
            }
        }
        out += "\n";
    }
    for e in &outcome.effects {
        let name = EFFECTS[usize::from(e.effect)];
        writeln!(out, "effect {name} at={} power={}", e.at, e.power).unwrap();
    }
    writeln!(
        out,
        "fuel_used={} executed={}",
        outcome.fuel_used,
        outcome.executed()
    )
    .unwrap();
    if let Some(f) = outcome.fault {
        writeln!(out, "fault {} at {:04} {:?}", f.kind.code(), f.at, f.kind).unwrap();
    }
    match outcome.error {
        None => out += "ok\n",
        Some(e) => writeln!(out, "error {} at {:04} {:?}", e.kind.code(), e.at, e.kind).unwrap(),
    }
    out
}

#[test]
fn golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    let bless = std::env::var_os("BLESS").is_some();
    let mut failures = Vec::new();
    let mut count = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        let src = || fs::read_to_string(&path).unwrap();
        let chunk = match path.extension().and_then(|e| e.to_str()) {
            Some("spell") => lang::compile(&src())
                .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()))
                .remove(0),
            Some("asm") => assemble(&src()),
            _ => continue,
        };
        count += 1;
        let actual = render(&chunk, &src());
        let expected_path = path.with_extension("out");
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
