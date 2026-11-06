//! Spell stack virtual machine: executes one [`Chunk`] per cast with a fuel
//! limit, by the "Execution" and "Fuel" sections of `docs/SPEC.md`.
//!
//! Single-threaded and deterministic: the same chunk and the same [`Cast`]
//! always produce the same [`Outcome`]. Never panics on bad bytecode or input;
//! those end the cast with `R010`.

use lang::bytecode::{BUILTINS, Chunk, EFFECTS, Instr, MAX_LOCALS};

/// Most frames on the frame stack.
pub const MAX_FRAMES: usize = 64;
/// Most values on the operand stack.
pub const MAX_OPERANDS: usize = 256;
/// Most live (allocated, not yet freed) memory cells per cast.
pub const MAX_CELLS: usize = 32;
/// Fuel cost of each builtin, by `CALL_BUILTIN` operand.
const BUILTIN_FUEL: [u64; BUILTINS.len()] = [8, 4, 2, 2, 2];

/// An entity position in the arena snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

/// Input of one cast.
#[derive(Debug, Clone, Copy)]
pub struct Cast<'a> {
    /// Arena snapshot; an `Entity` value is an index into it.
    pub entities: &'a [Point],
    /// Index of the caster.
    pub caster: u32,
    /// Entity indices passed as the spell parameters, in order.
    pub args: &'a [u32],
    /// Fuel available to this cast.
    pub fuel: u64,
}

/// A runtime value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// Signed 64-bit integer.
    Int(i64),
    /// Boolean.
    Bool(bool),
    /// Index of an entity in the arena snapshot.
    Entity(u32),
    /// Number of a memory cell, in `alloc` order from 0.
    Cell(u32),
}

/// An effect produced by `EMIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Effect {
    /// Index in [`EFFECTS`].
    pub effect: u8,
    /// Entity where the effect appears.
    pub at: u32,
    /// Effect power.
    pub power: i64,
}

/// History of a memory cell, for the trace and memory errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellInfo {
    /// Cell number.
    pub cell: u32,
    /// Index of the `alloc` instruction that created it.
    pub allocated_at: u32,
    /// Index of the `free` instruction that released it, if any.
    pub freed_at: Option<u32>,
}

/// One executed instruction in the trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceEntry {
    /// Instruction index.
    pub index: u32,
    /// The instruction.
    pub instr: Instr,
    /// Fuel left after paying for it.
    pub fuel_left: u64,
    /// The cell touched by `alloc`, `free`, `read` or `write`.
    pub memory: Option<CellInfo>,
}

/// Why bytecode or cast input is invalid (`R010`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corruption {
    /// The number of cast arguments differs from the chunk parameters.
    ArgCount,
    /// An entity index outside the arena snapshot.
    BadEntity(u32),
    /// The chunk declares more than [`MAX_LOCALS`] slots or fewer slots
    /// than parameters.
    BadLocals,
    /// A jump past the end of the code.
    BadJump(u32),
    /// A local slot outside the frame.
    BadSlot(u16),
    /// A read of a local slot that holds no value.
    EmptySlot(u16),
    /// A pop from an empty operand stack.
    StackUnderflow,
    /// A value of the wrong type on the operand stack.
    WrongType,
    /// A `PUSH_BOOL` operand other than 0 or 1.
    BadBool(u8),
    /// A `CALL_BUILTIN` operand outside [`BUILTINS`].
    UnknownBuiltin(u8),
    /// An `EMIT` operand outside [`EFFECTS`].
    UnknownEffect(u8),
    /// Execution ran past the last instruction.
    NoReturn,
}

/// Kind of a runtime error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// `R001`: frame stack overflow. Unreachable in v0.1.
    FrameOverflow,
    /// `R002`: a cell that does not exist. Unreachable in v0.1.
    NoSuchCell(u32),
    /// `R003`: `read` or `write` of a freed cell. Does not end the cast.
    UseAfterFree(CellInfo),
    /// `R004`: `free` of a freed cell.
    DoubleFree(CellInfo),
    /// `R005`: division by zero.
    DivisionByZero,
    /// `R006`: integer overflow.
    IntegerOverflow,
    /// `R007`: more than [`MAX_CELLS`] live cells (or more cells in one cast
    /// than fit in `u32`).
    TooManyCells,
    /// `R008`: not enough fuel for the next instruction.
    OutOfFuel,
    /// `R009`: operand stack overflow.
    OperandOverflow,
    /// `R010`: invalid bytecode or cast input.
    Corrupt(Corruption),
}

impl ErrorKind {
    /// Error code from `docs/SPEC.md`, such as `"R005"`.
    pub fn code(&self) -> &'static str {
        match self {
            ErrorKind::FrameOverflow => "R001",
            ErrorKind::NoSuchCell(_) => "R002",
            ErrorKind::UseAfterFree(_) => "R003",
            ErrorKind::DoubleFree(_) => "R004",
            ErrorKind::DivisionByZero => "R005",
            ErrorKind::IntegerOverflow => "R006",
            ErrorKind::TooManyCells => "R007",
            ErrorKind::OutOfFuel => "R008",
            ErrorKind::OperandOverflow => "R009",
            ErrorKind::Corrupt(_) => "R010",
        }
    }
}

/// A runtime error: what went wrong and at which instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeError {
    /// What went wrong.
    pub kind: ErrorKind,
    /// Index of the instruction; 0 for invalid cast input.
    pub at: u32,
}

/// Result of a cast. Effects applied before an error stay in force.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Effects in the order they were emitted.
    pub effects: Vec<Effect>,
    /// Every executed instruction, the failing one included.
    pub trace: Vec<TraceEntry>,
    /// Fuel spent.
    pub fuel_used: u64,
    /// The first `R003`, if any. It does not end the cast, but every effect
    /// after it appears at the caster.
    pub fault: Option<RuntimeError>,
    /// The error that ended the cast, if any.
    pub error: Option<RuntimeError>,
}

impl Outcome {
    /// Number of executed instructions.
    pub fn executed(&self) -> usize {
        self.trace.len()
    }
}

/// Runs one cast of `chunk`.
///
/// # Examples
///
/// ```
/// use vm::{Cast, Point, run};
///
/// let chunks = lang::compile("spell S(t):\n    emit Flame(at: t, power: 2 * 3)\nend").unwrap();
/// let entities = [Point { x: 0, y: 0 }, Point { x: 3, y: 4 }];
/// let cast = Cast { entities: &entities, caster: 0, args: &[1], fuel: 100 };
///
/// let outcome = run(&chunks[0], &cast);
/// assert_eq!(outcome.error, None);
/// assert_eq!((outcome.effects[0].at, outcome.effects[0].power), (1, 6));
/// assert_eq!(outcome.fuel_used, 21); // 4 instructions at 1, EMIT at 16, RET at 1
/// ```
pub fn run(chunk: &Chunk, cast: &Cast) -> Outcome {
    let mut m = Machine {
        chunk,
        cast,
        stack: Vec::new(),
        frames: Vec::new(),
        fuel: cast.fuel,
        effects: Vec::new(),
        trace: Vec::new(),
        cells: Vec::new(),
        live_cells: 0,
        fault: None,
    };
    let error = m.exec().err();
    Outcome {
        effects: m.effects,
        trace: m.trace,
        fuel_used: cast.fuel - m.fuel,
        fault: m.fault,
        error,
    }
}

/// What to do after an instruction.
enum Flow {
    Next,
    Jump(usize),
    Done,
}

struct Machine<'a> {
    chunk: &'a Chunk,
    cast: &'a Cast<'a>,
    stack: Vec<Value>,
    /// Local slots of each frame; `None` is an empty slot.
    frames: Vec<Vec<Option<Value>>>,
    fuel: u64,
    effects: Vec<Effect>,
    trace: Vec<TraceEntry>,
    /// Every cell of the cast, freed ones included; the index is the number.
    cells: Vec<Cell>,
    live_cells: usize,
    /// The first `R003`.
    fault: Option<RuntimeError>,
}

struct Cell {
    value: i64,
    allocated_at: u32,
    freed_at: Option<u32>,
}

type Step<T> = Result<T, ErrorKind>;

fn corrupt<T>(c: Corruption) -> Step<T> {
    Err(ErrorKind::Corrupt(c))
}

/// Fuel cost of an instruction; an unknown builtin has no cost and is `R010`.
fn cost(instr: Instr) -> Step<u64> {
    match instr {
        Instr::CallBuiltin(id) => match BUILTIN_FUEL.get(usize::from(id)) {
            Some(&fuel) => Ok(fuel),
            None => corrupt(Corruption::UnknownBuiltin(id)),
        },
        Instr::Emit(_) => Ok(16),
        Instr::Halt => Ok(0),
        _ => Ok(1),
    }
}

impl Machine<'_> {
    fn exec(&mut self) -> Result<(), RuntimeError> {
        self.start().map_err(|kind| RuntimeError { kind, at: 0 })?;
        let mut pc = 0;
        loop {
            let at = u32::try_from(pc).unwrap_or(u32::MAX);
            let fail = |kind| RuntimeError { kind, at };
            let Some(&instr) = self.chunk.code.get(pc) else {
                return Err(fail(ErrorKind::Corrupt(Corruption::NoReturn)));
            };
            let cost = cost(instr).map_err(fail)?;
            if cost > self.fuel {
                return Err(fail(ErrorKind::OutOfFuel));
            }
            self.fuel -= cost;
            self.trace.push(TraceEntry {
                index: at,
                instr,
                fuel_left: self.fuel,
                memory: None,
            });
            match self.step(instr, at).map_err(fail)? {
                Flow::Next => pc += 1,
                Flow::Jump(target) => pc = target,
                Flow::Done => return Ok(()),
            }
        }
    }

    /// Validates the cast input and pushes the first frame.
    fn start(&mut self) -> Step<()> {
        let (params, locals) = (
            usize::from(self.chunk.params),
            usize::from(self.chunk.locals),
        );
        if locals > MAX_LOCALS || params > locals {
            return corrupt(Corruption::BadLocals);
        }
        if self.cast.args.len() != params {
            return corrupt(Corruption::ArgCount);
        }
        let known = |e: u32| usize::try_from(e).is_ok_and(|e| e < self.cast.entities.len());
        if let Some(&bad) = std::iter::once(&self.cast.caster)
            .chain(self.cast.args)
            .find(|&&e| !known(e))
        {
            return corrupt(Corruption::BadEntity(bad));
        }
        let mut frame = vec![None; locals];
        for (slot, &e) in frame.iter_mut().zip(self.cast.args) {
            *slot = Some(Value::Entity(e));
        }
        self.push_frame(frame)
    }

    fn push_frame(&mut self, frame: Vec<Option<Value>>) -> Step<()> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(ErrorKind::FrameOverflow);
        }
        self.frames.push(frame);
        Ok(())
    }

    fn push(&mut self, value: Value) -> Step<Flow> {
        if self.stack.len() >= MAX_OPERANDS {
            return Err(ErrorKind::OperandOverflow);
        }
        self.stack.push(value);
        Ok(Flow::Next)
    }

    fn pop(&mut self) -> Step<Value> {
        self.stack
            .pop()
            .ok_or(ErrorKind::Corrupt(Corruption::StackUnderflow))
    }

    fn pop_int(&mut self) -> Step<i64> {
        match self.pop()? {
            Value::Int(n) => Ok(n),
            _ => corrupt(Corruption::WrongType),
        }
    }

    fn pop_bool(&mut self) -> Step<bool> {
        match self.pop()? {
            Value::Bool(b) => Ok(b),
            _ => corrupt(Corruption::WrongType),
        }
    }

    fn pop_entity(&mut self) -> Step<u32> {
        match self.pop()? {
            Value::Entity(e) => Ok(e),
            _ => corrupt(Corruption::WrongType),
        }
    }

    /// Pops a cell handle and records the cell in the current trace entry.
    fn pop_cell(&mut self) -> Step<CellInfo> {
        let Value::Cell(cell) = self.pop()? else {
            return corrupt(Corruption::WrongType);
        };
        let Some(c) = usize::try_from(cell).ok().and_then(|i| self.cells.get(i)) else {
            return Err(ErrorKind::NoSuchCell(cell));
        };
        let info = CellInfo {
            cell,
            allocated_at: c.allocated_at,
            freed_at: c.freed_at,
        };
        self.note(info);
        Ok(info)
    }

    fn note(&mut self, info: CellInfo) {
        if let Some(entry) = self.trace.last_mut() {
            entry.memory = Some(info);
        }
    }

    fn cell_mut(&mut self, info: CellInfo) -> &mut Cell {
        // pop_cell only returns cells that exist.
        &mut self.cells[info.cell as usize]
    }

    /// Records the first `R003`; the cast goes on.
    fn use_after_free(&mut self, info: CellInfo, at: u32) {
        self.fault.get_or_insert(RuntimeError {
            kind: ErrorKind::UseAfterFree(info),
            at,
        });
    }

    fn builtin(&mut self, id: u8, at: u32) -> Step<Flow> {
        match BUILTINS.get(usize::from(id)).copied() {
            Some("alloc") => {
                let value = self.pop_int()?;
                if self.live_cells >= MAX_CELLS {
                    return Err(ErrorKind::TooManyCells);
                }
                let cell = u32::try_from(self.cells.len()).map_err(|_| ErrorKind::TooManyCells)?;
                self.cells.push(Cell {
                    value,
                    allocated_at: at,
                    freed_at: None,
                });
                self.live_cells += 1;
                self.note(CellInfo {
                    cell,
                    allocated_at: at,
                    freed_at: None,
                });
                self.push(Value::Cell(cell))
            }
            Some("free") => {
                let info = self.pop_cell()?;
                if info.freed_at.is_some() {
                    return Err(ErrorKind::DoubleFree(info));
                }
                self.cell_mut(info).freed_at = Some(at);
                self.live_cells -= 1;
                self.note(CellInfo {
                    freed_at: Some(at),
                    ..info
                });
                Ok(Flow::Next)
            }
            Some("read") => {
                let info = self.pop_cell()?;
                if info.freed_at.is_some() {
                    self.use_after_free(info, at);
                }
                let value = self.cell_mut(info).value;
                self.push(Value::Int(value))
            }
            Some("write") => {
                let value = self.pop_int()?;
                let info = self.pop_cell()?;
                if info.freed_at.is_some() {
                    self.use_after_free(info, at);
                } else {
                    self.cell_mut(info).value = value;
                }
                Ok(Flow::Next)
            }
            Some("distance") => {
                let b = self.pop_entity()?;
                let a = self.pop_entity()?;
                let d = self.distance(a, b)?;
                self.push(Value::Int(d))
            }
            _ => corrupt(Corruption::UnknownBuiltin(id)),
        }
    }

    /// Pops `b`, then `a`, and pushes `f(a, b)`; `None` from `f` is `R006`.
    fn arith(&mut self, f: fn(i64, i64) -> Option<i64>) -> Step<Flow> {
        let b = self.pop_int()?;
        let a = self.pop_int()?;
        let n = f(a, b).ok_or(ErrorKind::IntegerOverflow)?;
        self.push(Value::Int(n))
    }

    fn compare(&mut self, f: fn(i64, i64) -> bool) -> Step<Flow> {
        let b = self.pop_int()?;
        let a = self.pop_int()?;
        self.push(Value::Bool(f(a, b)))
    }

    /// `CMP_EQ` / `CMP_NE`: two `Int` or two `Bool`.
    fn equal(&mut self, want: bool) -> Step<Flow> {
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(_), Value::Int(_)) | (Value::Bool(_), Value::Bool(_)) => {
                self.push(Value::Bool((a == b) == want))
            }
            _ => corrupt(Corruption::WrongType),
        }
    }

    fn slot(&mut self, slot: u16) -> Step<&mut Option<Value>> {
        let frame = self.frames.last_mut().expect("a frame while running");
        frame
            .get_mut(usize::from(slot))
            .ok_or(ErrorKind::Corrupt(Corruption::BadSlot(slot)))
    }

    fn target(&self, target: u32) -> Step<usize> {
        match usize::try_from(target) {
            Ok(t) if t < self.chunk.code.len() => Ok(t),
            _ => corrupt(Corruption::BadJump(target)),
        }
    }

    fn step(&mut self, instr: Instr, at: u32) -> Step<Flow> {
        match instr {
            Instr::PushInt(n) => self.push(Value::Int(n)),
            Instr::PushBool(b @ (0 | 1)) => self.push(Value::Bool(b == 1)),
            Instr::PushBool(b) => corrupt(Corruption::BadBool(b)),
            Instr::LoadLocal(s) => match *self.slot(s)? {
                Some(v) => self.push(v),
                None => corrupt(Corruption::EmptySlot(s)),
            },
            Instr::StoreLocal(s) => {
                let v = self.pop()?;
                *self.slot(s)? = Some(v);
                Ok(Flow::Next)
            }
            Instr::Add => self.arith(i64::checked_add),
            Instr::Sub => self.arith(i64::checked_sub),
            Instr::Mul => self.arith(i64::checked_mul),
            Instr::Div => {
                let b = self.pop_int()?;
                let a = self.pop_int()?;
                if b == 0 {
                    return Err(ErrorKind::DivisionByZero);
                }
                let n = a.checked_div(b).ok_or(ErrorKind::IntegerOverflow)?;
                self.push(Value::Int(n))
            }
            Instr::Neg => {
                let n = self.pop_int()?;
                let n = n.checked_neg().ok_or(ErrorKind::IntegerOverflow)?;
                self.push(Value::Int(n))
            }
            Instr::Not => {
                let b = self.pop_bool()?;
                self.push(Value::Bool(!b))
            }
            Instr::CmpLt => self.compare(|a, b| a < b),
            Instr::CmpGt => self.compare(|a, b| a > b),
            Instr::CmpEq => self.equal(true),
            Instr::CmpNe => self.equal(false),
            Instr::Jmp(t) => Ok(Flow::Jump(self.target(t)?)),
            Instr::JmpIfFalse(t) => {
                let t = self.target(t)?;
                let cond = self.pop_bool()?;
                Ok(if cond { Flow::Next } else { Flow::Jump(t) })
            }
            Instr::CallBuiltin(id) => self.builtin(id, at),
            Instr::Emit(id) => {
                if usize::from(id) >= EFFECTS.len() {
                    return corrupt(Corruption::UnknownEffect(id));
                }
                // Flame(at: Entity, power: Int) is the only effect in v0.1.
                let power = self.pop_int()?;
                let target = self.pop_entity()?;
                // After an R003 every effect lands on the caster.
                let at = if self.fault.is_some() {
                    self.cast.caster
                } else {
                    target
                };
                self.effects.push(Effect {
                    effect: id,
                    at,
                    power,
                });
                Ok(Flow::Next)
            }
            Instr::Ret => {
                self.frames.pop();
                Ok(if self.frames.is_empty() {
                    Flow::Done
                } else {
                    Flow::Next
                })
            }
            Instr::Halt => Ok(Flow::Done),
        }
    }

    /// Euclidean distance rounded down, in integers.
    fn distance(&self, a: u32, b: u32) -> Step<i64> {
        let point = |e: u32| {
            usize::try_from(e)
                .ok()
                .and_then(|e| self.cast.entities.get(e))
                .copied()
                .ok_or(ErrorKind::Corrupt(Corruption::BadEntity(e)))
        };
        let (a, b) = (point(a)?, point(b)?);
        let dx = i128::from(a.x) - i128::from(b.x);
        let dy = i128::from(a.y) - i128::from(b.y);
        let d = (dx * dx + dy * dy).unsigned_abs().isqrt();
        // |dx|, |dy| < 2^32, so d < 2^33.
        Ok(i64::try_from(d).expect("distance fits in i64"))
    }
}
