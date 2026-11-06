//! Bytecode shared by the compiler and the VM: the v0.1 instruction set from
//! `docs/SPEC.md` and a disassembler.

use std::fmt;

/// Most local slots a spell frame may have (parameters plus every `let`).
pub const MAX_LOCALS: usize = 64;

/// Builtin function names; the index is the `CALL_BUILTIN` operand.
pub const BUILTINS: [&str; 5] = ["alloc", "free", "read", "write", "distance"];

/// Effect names; the index is the `EMIT` operand.
pub const EFFECTS: [&str; 1] = ["Flame"];

/// One instruction: an opcode and at most one operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instr {
    /// `PUSH_INT`: push a constant.
    PushInt(i64),
    /// `PUSH_BOOL`: push `false` (`0`) or `true` (`1`).
    PushBool(u8),
    /// `LOAD_LOCAL`: push the value of a local slot.
    LoadLocal(u16),
    /// `STORE_LOCAL`: pop into a local slot.
    StoreLocal(u16),
    /// `ADD`
    Add,
    /// `SUB`
    Sub,
    /// `MUL`
    Mul,
    /// `DIV`
    Div,
    /// `NEG`
    Neg,
    /// `NOT`
    Not,
    /// `CMP_LT`
    CmpLt,
    /// `CMP_GT`
    CmpGt,
    /// `CMP_EQ`
    CmpEq,
    /// `CMP_NE`
    CmpNe,
    /// `JMP`: jump to an instruction index.
    Jmp(u32),
    /// `JMP_IF_FALSE`: pop a `Bool`, jump to an instruction index if `false`.
    JmpIfFalse(u32),
    /// `CALL_BUILTIN`: call the builtin with this index in [`BUILTINS`].
    CallBuiltin(u8),
    /// `EMIT`: apply the effect with this index in [`EFFECTS`].
    Emit(u8),
    /// `RET`: leave the current frame.
    Ret,
    /// `HALT`: stop execution.
    Halt,
}

/// Compiled code of one spell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Spell name.
    pub name: String,
    /// Number of parameters; they occupy local slots `0..params`.
    pub params: u16,
    /// Number of local slots used, parameters included.
    pub locals: u16,
    /// Instructions; jump operands are indices into this vector.
    pub code: Vec<Instr>,
}

impl fmt::Display for Instr {
    /// Prints the opcode name from `docs/SPEC.md` and its operand. Builtin and
    /// effect operands also get their name, e.g. `CALL_BUILTIN 0 (alloc)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name =
            |names: &[&'static str], i: u8| names.get(usize::from(i)).copied().unwrap_or("?");
        match *self {
            Instr::PushInt(n) => write!(f, "PUSH_INT {n}"),
            Instr::PushBool(b) => write!(f, "PUSH_BOOL {b}"),
            Instr::LoadLocal(s) => write!(f, "LOAD_LOCAL {s}"),
            Instr::StoreLocal(s) => write!(f, "STORE_LOCAL {s}"),
            Instr::Add => f.write_str("ADD"),
            Instr::Sub => f.write_str("SUB"),
            Instr::Mul => f.write_str("MUL"),
            Instr::Div => f.write_str("DIV"),
            Instr::Neg => f.write_str("NEG"),
            Instr::Not => f.write_str("NOT"),
            Instr::CmpLt => f.write_str("CMP_LT"),
            Instr::CmpGt => f.write_str("CMP_GT"),
            Instr::CmpEq => f.write_str("CMP_EQ"),
            Instr::CmpNe => f.write_str("CMP_NE"),
            Instr::Jmp(t) => write!(f, "JMP {t}"),
            Instr::JmpIfFalse(t) => write!(f, "JMP_IF_FALSE {t}"),
            Instr::CallBuiltin(i) => write!(f, "CALL_BUILTIN {i} ({})", name(&BUILTINS, i)),
            Instr::Emit(i) => write!(f, "EMIT {i} ({})", name(&EFFECTS, i)),
            Instr::Ret => f.write_str("RET"),
            Instr::Halt => f.write_str("HALT"),
        }
    }
}

/// Renders a chunk as text: a header line, then one numbered instruction per
/// line.
///
/// # Examples
///
/// ```
/// let chunks = lang::compile("spell S(t):\n    let x <- 1\nend").unwrap();
/// assert_eq!(
///     lang::bytecode::disassemble(&chunks[0]),
///     "spell S params=1 locals=2\n0000 PUSH_INT 1\n0001 STORE_LOCAL 1\n0002 RET\n"
/// );
/// ```
pub fn disassemble(chunk: &Chunk) -> String {
    let mut out = format!(
        "spell {} params={} locals={}\n",
        chunk.name, chunk.params, chunk.locals
    );
    for (i, instr) in chunk.code.iter().enumerate() {
        out += &format!("{i:04} {instr}\n");
    }
    out
}
