//! The VM must never panic on any bytecode (`docs/SPEC.md`, executor limits).
//! Runs thousands of pseudo-random chunks from a fixed seed, so the test is
//! deterministic.

use lang::bytecode::{Chunk, Instr};
use vm::{Cast, Point, run};

/// Linear congruential generator (Knuth's MMIX constants).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % below
    }
}

fn instr(r: &mut Lcg) -> Instr {
    let small = |r: &mut Lcg| r.next(12);
    match r.next(20) {
        0 => Instr::PushInt([0, 1, -1, i64::MAX, i64::MIN][r.next(5) as usize]),
        1 => Instr::PushBool(small(r) as u8),
        2 => Instr::LoadLocal(small(r) as u16),
        3 => Instr::StoreLocal(small(r) as u16),
        4 => Instr::Add,
        5 => Instr::Sub,
        6 => Instr::Mul,
        7 => Instr::Div,
        8 => Instr::Neg,
        9 => Instr::Not,
        10 => Instr::CmpLt,
        11 => Instr::CmpGt,
        12 => Instr::CmpEq,
        13 => Instr::CmpNe,
        14 => Instr::Jmp(small(r) as u32),
        15 => Instr::JmpIfFalse(small(r) as u32),
        // ponytail: ids 0-3 (memory builtins) are todo!() until v0.1 step 5;
        // widen to every id once they exist.
        16 => Instr::CallBuiltin(4 + small(r) as u8),
        17 => Instr::Emit(small(r) as u8 % 3),
        18 => Instr::Ret,
        _ => Instr::Halt,
    }
}

#[test]
fn random_bytecode_never_panics() {
    let entities = [
        Point {
            x: i32::MIN,
            y: i32::MAX,
        },
        Point {
            x: i32::MAX,
            y: i32::MIN,
        },
    ];
    let mut r = Lcg(0x5EED);
    for _ in 0..20_000 {
        let len = 1 + r.next(16) as usize;
        let chunk = Chunk {
            name: "fuzz".into(),
            params: r.next(3) as u16,
            locals: r.next(70) as u16,
            code: (0..len).map(|_| instr(&mut r)).collect(),
        };
        let args: Vec<u32> = (0..r.next(3)).map(|_| r.next(3) as u32).collect();
        let cast = Cast {
            entities: &entities,
            caster: 0,
            args: &args,
            fuel: r.next(200),
        };
        let outcome = run(&chunk, &cast);
        assert!(outcome.fuel_used <= cast.fuel);
    }
}
