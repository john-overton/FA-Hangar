//! Inert x86 length decoding and bounded stub analysis for SH F0 blocks.
//! The opcode subset is OpenFA's i386 table for FA shapes; anything else stops
//! decoding. Nothing here executes resource code: flow is followed statically,
//! and the stub evaluator only tracks a whitelisted register subset symbolically.
use crate::{invalid, slice, u32_at, Result};
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mem {
    /// `[disp32]`, an absolute virtual address.
    Abs(u32),
    Reg {
        base: Option<u8>,
        index: Option<(u8, u8)>,
        disp: i32,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rm {
    Reg(u8),
    Mem(Mem),
}
#[derive(Clone, Copy, Debug)]
pub struct Insn {
    pub at: usize,
    pub len: usize,
    pub op: u16,
    pub ext: u8,
    /// Operand-size prefix (16-bit operation).
    pub word: bool,
    pub reg: u8,
    pub rm: Option<Rm>,
    pub imm: Option<i32>,
    /// Absolute CODE offset of a relative branch/call target.
    pub rel: Option<i64>,
    /// Width of the relative field (1 or 4) and its offset inside the instruction.
    pub rel_field: Option<(usize, u8)>,
    /// Offset inside the instruction of a 32-bit immediate or absolute displacement.
    pub abs_field: Option<usize>,
}
#[derive(Clone, Copy, PartialEq)]
enum Form {
    None,
    ModRm,
    ModRmImm8,
    ModRmImmV,
    Imm8,
    ImmV,
    Rel8,
    RelV,
    Moffs,
}
fn form(op: u16, ext: u8) -> Option<Form> {
    use Form::*;
    Some(match (op, ext) {
        (0x00 | 0x02 | 0x03 | 0x0a | 0x0b | 0x22 | 0x2a | 0x2b | 0x32 | 0x33, _) => ModRm,
        (0x39 | 0x3a | 0x3b | 0x88 | 0x89 | 0x8a | 0x8b | 0x8d, _) => ModRm,
        (0x05 | 0x0d | 0x25 | 0x2d | 0x3d | 0x68, _) => ImmV,
        (0xb8..=0xbf, _) => ImmV,
        (0x07 | 0x16 | 0x60 | 0x61 | 0x90 | 0x99 | 0xa4 | 0xc3 | 0xcc | 0xfc, _) => None,
        (0x40..=0x5f, _) => None,
        (0x3c, _) => Imm8,
        (0x6b, _) => ModRmImm8,
        (0x70..=0x7f | 0xeb, _) => Rel8,
        (0x80, 0 | 2 | 7) | (0x82, 0) | (0x83, 0 | 1 | 4 | 5 | 7) => ModRmImm8,
        (0xc1, 4 | 5 | 7) | (0xc6, 0) => ModRmImm8,
        (0x81, 0 | 1 | 4 | 7) | (0xc7, 0) | (0xf7, 0) => ModRmImmV,
        (0xa1 | 0xa3, _) => Moffs,
        (0xd1, 1 | 4 | 5 | 7) | (0xf7, 3..=7) | (0xff, 1 | 4 | 6) => ModRm,
        (0xe8 | 0xe9, _) => RelV,
        (0x0f82..=0x0f85 | 0x0f8e, _) => RelV,
        (0x0faf | 0x0fb6 | 0x0fb7, _) => ModRm,
        _ => return Option::None,
    })
}
fn group(op: u16) -> bool {
    matches!(
        op,
        0x80 | 0x81 | 0x82 | 0x83 | 0xc1 | 0xc6 | 0xc7 | 0xd1 | 0xf7 | 0xff
    )
}
fn byte(code: &[u8], at: usize) -> Result<u8> {
    Ok(slice(code, at, 1)?[0])
}
/// Decode one instruction at `at`; unknown opcodes and prefixes are errors.
pub fn decode(code: &[u8], at: usize) -> Result<Insn> {
    let mut p = at;
    let mut word = false;
    for _ in 0..4 {
        match byte(code, p)? {
            0x66 => word = true,
            // FS segment and REP are the only other OpenFA-accepted prefixes.
            0x64 | 0xf3 => {}
            _ => break,
        }
        p += 1;
    }
    let mut op = byte(code, p)? as u16;
    p += 1;
    if op == 0x0f {
        op = 0x0f00 | byte(code, p)? as u16;
        p += 1;
    }
    let modrm = if group(op) {
        Some(byte(code, p)?)
    } else {
        None
    };
    let ext = modrm.map_or(0, |m| (m >> 3) & 7);
    let f = form(op, ext).ok_or_else(|| format!("Unknown x86 opcode {op:02X}/{ext} at {at:X}"))?;
    let mut insn = Insn {
        at,
        len: 0,
        op,
        ext,
        word,
        reg: (op & 7) as u8,
        rm: None,
        imm: None,
        rel: None,
        rel_field: None,
        abs_field: None,
    };
    if matches!(f, Form::ModRm | Form::ModRmImm8 | Form::ModRmImmV) {
        let m = byte(code, p)?;
        p += 1;
        let md = m >> 6;
        let rm = m & 7;
        insn.reg = (m >> 3) & 7;
        insn.rm = Some(if md == 3 {
            Rm::Reg(rm)
        } else {
            let (mut base, mut index) = (Some(rm), None);
            if rm == 4 {
                let sib = byte(code, p)?;
                p += 1;
                let i = (sib >> 3) & 7;
                index = (i != 4).then_some((i, 1 << (sib >> 6)));
                base = Some(sib & 7);
            }
            if md == 0 && base == Some(5) {
                let disp = u32_at(code, p)? as u32;
                insn.abs_field = Some(p - at);
                p += 4;
                if index.is_none() {
                    Rm::Mem(Mem::Abs(disp))
                } else {
                    Rm::Mem(Mem::Reg {
                        base: None,
                        index,
                        disp: disp as i32,
                    })
                }
            } else {
                let disp = match md {
                    1 => {
                        p += 1;
                        byte(code, p - 1)? as i8 as i32
                    }
                    2 => {
                        insn.abs_field = Some(p - at);
                        p += 4;
                        u32_at(code, p - 4)? as i32
                    }
                    _ => 0,
                };
                base = base.filter(|_| !(md == 0 && base == Some(5)));
                Rm::Mem(Mem::Reg { base, index, disp })
            }
        });
        if op == 0x8d && matches!(insn.rm, Some(Rm::Reg(_))) {
            return Err(invalid("Register LEA operand"));
        }
    }
    let v = if word { 2 } else { 4 };
    match f {
        Form::ModRmImm8 | Form::Imm8 => {
            insn.imm = Some(byte(code, p)? as i8 as i32);
            p += 1;
        }
        Form::ModRmImmV | Form::ImmV => {
            if v == 4 {
                insn.abs_field = insn.abs_field.or(Some(p - at));
                insn.imm = Some(u32_at(code, p)? as i32);
            } else {
                insn.imm = Some(crate::u16_at(code, p)? as u16 as i16 as i32);
            }
            p += v;
        }
        Form::Rel8 => {
            insn.rel_field = Some((p - at, 1));
            let d = byte(code, p)? as i8 as i64;
            p += 1;
            insn.rel = Some(p as i64 + d);
        }
        Form::RelV => {
            insn.rel_field = Some((p - at, v as u8));
            let d = if v == 4 {
                u32_at(code, p)? as u32 as i32 as i64
            } else {
                crate::u16_at(code, p)? as u16 as i16 as i64
            };
            p += v;
            insn.rel = Some(p as i64 + d);
        }
        Form::Moffs => {
            insn.abs_field = Some(p - at);
            insn.rm = Some(Rm::Mem(Mem::Abs(u32_at(code, p)? as u32)));
            p += 4;
        }
        _ => {}
    }
    insn.len = p - at;
    Ok(insn)
}
impl Insn {
    pub fn end(&self) -> usize {
        self.at + self.len
    }
    fn is_jcc(&self) -> bool {
        matches!(self.op, 0x70..=0x7f | 0x0f82..=0x0f85 | 0x0f8e)
    }
    fn unconditional(&self) -> bool {
        matches!(self.op, 0xc3 | 0xe9 | 0xeb) || (self.op == 0xff && self.ext == 4)
    }
}

/// How a retpoline (`push A; push B; ret`) leaves x86.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// `push SH; push do_start_interp; ret`: resume SH interpretation at a CODE offset.
    Sh(usize),
    /// Exit through another non-returning interpreter entry such as `_ErrorExit`.
    Stop,
}
/// Static code blocks reached from one F0 entry, sorted by start offset.
#[derive(Clone, Debug, Default)]
pub struct Flow {
    pub blocks: Vec<(usize, usize)>,
    pub exits: Vec<(usize, Exit)>,
    pub insns: Vec<Insn>,
}
pub struct Context<'a> {
    pub code: &'a [u8],
    /// Decoding stops at this CODE offset (the end marker or tail).
    pub limit: usize,
    pub code_va: usize,
    /// Trampoline CODE offsets with their import names.
    pub trampolines: &'a BTreeMap<usize, String>,
}
impl Context<'_> {
    fn offset(&self, va: i64) -> Option<usize> {
        let at = va - self.code_va as i64;
        (at >= 0 && (at as usize) < self.code.len()).then_some(at as usize)
    }
    fn trampoline(&self, va: i64) -> Option<&str> {
        self.offset(va)
            .and_then(|at| self.trampolines.get(&at))
            .map(String::as_str)
    }
}
fn interpreter_exit(name: &str) -> bool {
    matches!(name, "do_start_interp" | "_ErrorExit")
}
/// Follow jumps and non-interpreter retpolines from `entry`, OpenFA-style.
pub fn flow(cx: &Context, entry: usize, known: &[(usize, usize)]) -> Result<Flow> {
    let mut out = Flow::default();
    let mut work = alloc::vec![entry];
    let inside =
        |blocks: &[(usize, usize)], at: usize| blocks.iter().any(|(s, e)| *s <= at && at < *e);
    while let Some(start) = work.pop() {
        if inside(&out.blocks, start) || inside(known, start) {
            continue;
        }
        if out.blocks.len() >= 256 || out.insns.len() >= 8192 {
            return Err(invalid("x86 flow exceeds limits"));
        }
        let mut p = start;
        let mut block = Vec::new();
        loop {
            if p >= cx.limit {
                return Err(invalid("x86 runs into the SH end marker"));
            }
            // Falling into an already decoded block joins it.
            if p != start && (inside(&out.blocks, p) || inside(known, p)) {
                break;
            }
            let i = decode(cx.code, p)?;
            if i.end() > cx.limit {
                return Err(invalid("x86 instruction crosses the SH end marker"));
            }
            block.push(i);
            p = i.end();
            if let Some(t) = i.rel.filter(|_| i.is_jcc() || matches!(i.op, 0xe9 | 0xeb)) {
                if t < 0 || t as usize >= cx.limit {
                    return Err(invalid("x86 branch outside CODE"));
                }
                work.push(t as usize);
            }
            if i.op == 0xc3 {
                let n = block.len();
                let push = |k: usize| {
                    (n > k)
                        .then(|| block[n - 1 - k])
                        .filter(|i: &Insn| i.op == 0x68 && !i.word)
                        .and_then(|i| i.imm)
                };
                if let Some(callee) = push(1) {
                    let name = cx
                        .trampoline(callee as u32 as i64)
                        .ok_or_else(|| invalid("x86 return to a non-import address"))?;
                    if interpreter_exit(name) {
                        let exit = if name == "do_start_interp" {
                            let target =
                                push(2).ok_or_else(|| invalid("Retpoline without SH target"))?;
                            Exit::Sh(
                                cx.offset(target as u32 as i64)
                                    .ok_or_else(|| invalid("SH resume outside CODE"))?,
                            )
                        } else {
                            Exit::Stop
                        };
                        out.exits.push((i.at, exit));
                    } else {
                        let back = push(2).ok_or_else(|| invalid("Import call without return"))?;
                        work.push(
                            cx.offset(back as u32 as i64)
                                .ok_or_else(|| invalid("Import return outside CODE"))?,
                        );
                    }
                }
            }
            if i.unconditional() {
                break;
            }
        }
        out.blocks.push((start, p));
        out.insns.extend(block);
    }
    out.blocks.sort_unstable();
    out.insns.sort_unstable_by_key(|i| i.at);
    out.exits.sort_unstable_by_key(|e| e.0);
    for w in out.blocks.windows(2) {
        if w[0].1 > w[1].0 {
            return Err(invalid("Overlapping x86 blocks"));
        }
    }
    Ok(out)
}

/// One step of an xform law, in reverse Polish order. `w16` ops wrap to 16 bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LawOp {
    /// Push a word variable (symbol name).
    Var(String),
    Const(i32),
    Neg {
        w16: bool,
    },
    Add {
        w16: bool,
    },
    Sub {
        w16: bool,
    },
    Mul {
        w16: bool,
    },
    Sar {
        n: u8,
        w16: bool,
    },
    Shl {
        n: u8,
        w16: bool,
    },
    Shr {
        n: u8,
        w16: bool,
    },
}
fn wrap(v: i64, w16: bool) -> i64 {
    if w16 {
        v as i16 as i64
    } else {
        v as i32 as i64
    }
}
/// Evaluate a law; unknown variables read as zero. Results are stored words.
pub fn evaluate(law: &[LawOp], value: &dyn Fn(&str) -> i32) -> Option<i16> {
    let mut stack: Vec<i64> = Vec::new();
    for op in law {
        if stack.len() > 32 {
            return None;
        }
        let v = match op {
            LawOp::Var(name) => value(name) as i16 as i64,
            LawOp::Const(n) => *n as i64,
            LawOp::Neg { w16 } => wrap(-stack.pop()?, *w16),
            LawOp::Sar { n, w16 } => wrap(wrap(stack.pop()?, *w16) >> n, *w16),
            LawOp::Shl { n, w16 } => wrap(stack.pop()? << n, *w16),
            LawOp::Shr { n, w16 } => {
                let v = stack.pop()?;
                let u = if *w16 {
                    v as u16 as i64
                } else {
                    v as u32 as i64
                };
                wrap(u >> n, *w16)
            }
            LawOp::Add { w16 } | LawOp::Sub { w16 } | LawOp::Mul { w16 } => {
                let b = stack.pop()?;
                let a = stack.pop()?;
                wrap(
                    match op {
                        LawOp::Add { .. } => a.wrapping_add(b),
                        LawOp::Sub { .. } => a.wrapping_sub(b),
                        _ => a.wrapping_mul(b),
                    },
                    *w16,
                )
            }
        };
        stack.push(v);
    }
    (stack.len() == 1).then(|| stack[0] as i16)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rel {
    Eq,
    Ne,
    Lt,
    Ge,
    Le,
    Gt,
    Below,
    AboveEq,
    BelowEq,
    Above,
}
impl Rel {
    fn of(op: u16) -> Option<Self> {
        Some(match op & 0xff {
            0x74 | 0x84 => Self::Eq,
            0x75 | 0x85 => Self::Ne,
            0x7c => Self::Lt,
            0x7d => Self::Ge,
            0x7e | 0x8e => Self::Le,
            0x7f => Self::Gt,
            0x72 | 0x82 => Self::Below,
            0x73 | 0x83 => Self::AboveEq,
            0x76 => Self::BelowEq,
            0x77 => Self::Above,
            _ => return None,
        })
    }
    pub fn negated(self) -> Self {
        match self {
            Self::Eq => Self::Ne,
            Self::Ne => Self::Eq,
            Self::Lt => Self::Ge,
            Self::Ge => Self::Lt,
            Self::Le => Self::Gt,
            Self::Gt => Self::Le,
            Self::Below => Self::AboveEq,
            Self::AboveEq => Self::Below,
            Self::BelowEq => Self::Above,
            Self::Above => Self::BelowEq,
        }
    }
    pub fn holds(self, a: i32, b: i32, w16: bool) -> bool {
        let (ua, ub) = if w16 {
            (a as u16 as u32, b as u16 as u32)
        } else {
            (a as u32, b as u32)
        };
        let (a, b) = if w16 {
            (a as i16 as i32, b as i16 as i32)
        } else {
            (a, b)
        };
        match self {
            Self::Eq => a == b,
            Self::Ne => a != b,
            Self::Lt => a < b,
            Self::Ge => a >= b,
            Self::Le => a <= b,
            Self::Gt => a > b,
            Self::Below => ua < ub,
            Self::AboveEq => ua >= ub,
            Self::BelowEq => ua <= ub,
            Self::Above => ua > ub,
        }
    }
}
/// `variable rel value`, as compared by the stub (word or dword compare).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub variable: String,
    pub rel: Rel,
    pub value: i32,
    pub w16: bool,
}
/// A self-modifying store of a law result into a CODE word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Store {
    /// CODE offset of the stored word.
    pub at: usize,
    pub law: Vec<LawOp>,
}
/// One static path through a stub, ending in an SH resume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub conditions: Vec<Condition>,
    pub stores: Vec<Store>,
    pub resume: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Analysis {
    pub outcomes: Vec<Outcome>,
    /// First unsupported instruction on any path, if the stub was not fully understood.
    pub unrecognised: Option<(usize, String)>,
}
#[derive(Clone, Debug, PartialEq)]
enum Val {
    Unknown,
    Const(i64),
    /// A law over variables; `full` when all 32 bits are determined.
    Law(Vec<LawOp>, bool),
}
#[derive(Clone)]
struct Path {
    at: usize,
    regs: [Val; 8],
    flags: Option<(Val, Val, bool)>,
    conditions: Vec<Condition>,
    stores: Vec<Store>,
    stack: Vec<Val>,
    steps: usize,
}
fn law_of(v: &Val) -> Option<Vec<LawOp>> {
    match v {
        Val::Const(n) => Some(alloc::vec![LawOp::Const(*n as i32)]),
        Val::Law(l, _) => Some(l.clone()),
        Val::Unknown => None,
    }
}
fn full(v: &Val) -> bool {
    matches!(v, Val::Const(_) | Val::Law(_, true))
}
/// Symbolically walk every path of an F0 stub. Supported: word/dword compares of
/// imported variables against immediates, jcc/jmp, `call $+5; pop`, constant
/// adds, loads of imported words, sar/shl/shr/neg/add/sub/imul, register moves, nop
/// and word stores through a register holding a CODE address.
pub fn analyze(cx: &Context, entry: usize) -> Analysis {
    let mut out = Analysis::default();
    let mut work = alloc::vec![Path {
        at: entry,
        regs: core::array::from_fn(|_| Val::Unknown),
        flags: None,
        conditions: Vec::new(),
        stores: Vec::new(),
        stack: Vec::new(),
        steps: 0,
    }];
    let mut paths = 0;
    while let Some(mut path) = work.pop() {
        paths += 1;
        if paths > 64 {
            out.unrecognised = Some((entry, "too many stub paths".into()));
            return out;
        }
        loop {
            match step(cx, &mut path) {
                Ok(Step::Next) => {}
                Ok(Step::Fork(other)) => work.push(*other),
                Ok(Step::Resume(resume)) => {
                    out.outcomes.push(Outcome {
                        conditions: path.conditions,
                        stores: path.stores,
                        resume,
                    });
                    break;
                }
                Ok(Step::Stop) => break,
                Err(e) => {
                    if out.unrecognised.is_none() {
                        out.unrecognised = Some((path.at, e));
                    }
                    break;
                }
            }
        }
    }
    out
}
enum Step {
    Next,
    Fork(alloc::boxed::Box<Path>),
    Resume(usize),
    Stop,
}
fn variable(cx: &Context, rm: &Option<Rm>) -> Option<String> {
    match rm {
        Some(Rm::Mem(Mem::Abs(va))) => cx.trampoline(*va as i64).map(String::from),
        _ => None,
    }
}
/// A 16-bit register write leaves the upper half unrelated to the law.
fn narrow(v: Val, w16: bool) -> Val {
    match (v, w16) {
        (Val::Const(n), true) => Val::Law(alloc::vec![LawOp::Const(n as i32)], false),
        (Val::Law(l, f), w) => Val::Law(l, f && !w),
        (v, _) => v,
    }
}
fn apply(v: &Val, ops: &[LawOp], w16: bool) -> Val {
    // 32-bit right shifts need upper bits that a word load does not determine.
    if !w16
        && !full(v)
        && ops
            .iter()
            .any(|o| matches!(o, LawOp::Sar { .. } | LawOp::Shr { .. }))
    {
        return Val::Unknown;
    }
    match law_of(v) {
        Some(mut l) => {
            l.extend_from_slice(ops);
            Val::Law(l, full(v) && !w16)
        }
        None => Val::Unknown,
    }
}
fn step(cx: &Context, s: &mut Path) -> core::result::Result<Step, String> {
    s.steps += 1;
    if s.steps > 256 {
        return Err("stub path too long".into());
    }
    let i = decode(cx.code, s.at)?;
    if i.end() > cx.limit {
        return Err("stub crosses the end marker".into());
    }
    let next = i.end();
    let reg_rm = match i.rm {
        Some(Rm::Reg(r)) => Some(r as usize),
        _ => None,
    };
    let unsupported = || format!("x86 {:02X}/{} at {:X}", i.op, i.ext, i.at);
    let w = i.word;
    let var = variable(cx, &i.rm);
    match (i.op, i.ext) {
        (0x68, _) if !w => s.stack.push(Val::Const(i.imm.unwrap_or(0) as u32 as i64)),
        // `nop` / `xchg ax, ax`: Hangar's 2-byte filler in a resized law slot.
        (0x90, _) => {}
        (0xc3, _) => {
            let n = s.stack.len();
            let pushed = |k: usize| match s.stack.get(n.wrapping_sub(1 + k)) {
                Some(Val::Const(v)) => Some(*v),
                _ => None,
            };
            let callee = pushed(0).ok_or("return without a constant retpoline")?;
            let name = cx
                .trampoline(callee)
                .ok_or("return to a non-import address")?;
            return match name {
                "do_start_interp" => {
                    let t = pushed(1).ok_or("retpoline without SH target")?;
                    Ok(Step::Resume(cx.offset(t).ok_or("SH resume outside CODE")?))
                }
                "_ErrorExit" => Ok(Step::Stop),
                _ => Err(format!("calls {name}")),
            };
        }
        // call $+5 pushes the address of the next instruction.
        (0xe8, _) if i.rel == Some(next as i64) => {
            s.stack.push(Val::Const((cx.code_va + next) as i64))
        }
        (0x58..=0x5f, _) => {
            s.regs[(i.op & 7) as usize] = s.stack.pop().ok_or("pop of an unknown stack")?;
        }
        (0x50..=0x57, _) => {
            let v = s.regs[(i.op & 7) as usize].clone();
            s.stack.push(v);
        }
        // Compare an imported variable (memory or loaded register) with an immediate.
        (0x81 | 0x83, 7) | (0x3d, _) => {
            let lhs = match (&var, reg_rm) {
                (Some(v), _) => Val::Law(alloc::vec![LawOp::Var(v.clone())], false),
                (None, Some(r)) => s.regs[r].clone(),
                _ if i.op == 0x3d => s.regs[0].clone(),
                _ => return Err(unsupported()),
            };
            s.flags = Some((lhs, Val::Const(i.imm.unwrap_or(0) as i64), w));
        }
        (0x70..=0x7f, _) | (0x0f82..=0x0f85 | 0x0f8e, _) => {
            let rel = Rel::of(i.op).ok_or_else(unsupported)?;
            let (lhs, rhs, w16) = s.flags.clone().ok_or("branch without a reviewed compare")?;
            let (Val::Law(l, _), Val::Const(value)) = (lhs, rhs) else {
                return Err("branch on an unknown compare".into());
            };
            let [LawOp::Var(variable)] = l.as_slice() else {
                return Err("branch on a computed compare".into());
            };
            let mut taken = s.clone();
            taken.at = usize::try_from(i.rel.ok_or_else(unsupported)?)
                .map_err(|_| "branch outside CODE")?;
            for (path, rel) in [(&mut taken, rel), (&mut *s, rel.negated())] {
                path.conditions.push(Condition {
                    variable: variable.clone(),
                    rel,
                    value: value as i32,
                    w16,
                });
            }
            s.at = next;
            return Ok(Step::Fork(alloc::boxed::Box::new(taken)));
        }
        (0xeb | 0xe9, _) => {
            s.at =
                usize::try_from(i.rel.ok_or_else(unsupported)?).map_err(|_| "jump outside CODE")?;
            return Ok(Step::Next);
        }
        // Loads of imported words, register moves and constants.
        (0xa1 | 0x8b, _) if var.is_some() => {
            let r = if i.op == 0xa1 { 0 } else { i.reg as usize };
            s.regs[r] = Val::Law(alloc::vec![LawOp::Var(var.unwrap_or_default())], false);
        }
        (0x8b | 0x89, _) if reg_rm.is_some() => {
            let rm = reg_rm.unwrap_or(0);
            let (dst, src) = if i.op == 0x8b {
                (i.reg as usize, rm)
            } else {
                (rm, i.reg as usize)
            };
            s.regs[dst] = narrow(s.regs[src].clone(), w);
        }
        (0xb8..=0xbf, _) => {
            s.regs[(i.op & 7) as usize] = narrow(Val::Const(i.imm.unwrap_or(0) as i64), w)
        }
        // Register arithmetic.
        (0x81 | 0x83, 0 | 5) | (0x05 | 0x2d, _) => {
            let r = if matches!(i.op, 0x05 | 0x2d) {
                0
            } else {
                reg_rm.ok_or_else(unsupported)?
            };
            let sub = i.ext == 5 || i.op == 0x2d;
            let imm = i.imm.unwrap_or(0) as i64;
            s.regs[r] = match &s.regs[r] {
                Val::Const(v) if !w => {
                    Val::Const((if sub { v - imm } else { v + imm }) as u32 as i64)
                }
                v => {
                    let op = if sub {
                        LawOp::Sub { w16: w }
                    } else {
                        LawOp::Add { w16: w }
                    };
                    apply(v, &[LawOp::Const(imm as i32), op], w)
                }
            };
        }
        (0x03 | 0x2b, _) if reg_rm.is_some() => {
            let a = s.regs[i.reg as usize].clone();
            let b = s.regs[reg_rm.unwrap_or(0)].clone();
            s.regs[i.reg as usize] = match law_of(&b) {
                Some(mut ops) => {
                    ops.push(if i.op == 0x03 {
                        LawOp::Add { w16: w }
                    } else {
                        LawOp::Sub { w16: w }
                    });
                    let v = apply(&a, &ops, w);
                    if full(&b) {
                        v
                    } else {
                        narrow(v, true)
                    }
                }
                None => Val::Unknown,
            };
        }
        (0xd1 | 0xc1, 4 | 5 | 7) if reg_rm.is_some() => {
            let r = reg_rm.unwrap_or(0);
            let n = if i.op == 0xd1 {
                1
            } else {
                (i.imm.unwrap_or(0) & 31) as u8
            };
            let op = match i.ext {
                4 => LawOp::Shl { n, w16: w },
                5 => LawOp::Shr { n, w16: w },
                _ => LawOp::Sar { n, w16: w },
            };
            s.regs[r] = apply(&s.regs[r], &[op], w);
        }
        (0xf7, 3) if reg_rm.is_some() => {
            let r = reg_rm.unwrap_or(0);
            s.regs[r] = apply(&s.regs[r], &[LawOp::Neg { w16: w }], w);
        }
        (0x6b, _) if reg_rm.is_some() => {
            let src = s.regs[reg_rm.unwrap_or(0)].clone();
            s.regs[i.reg as usize] = apply(
                &src,
                &[LawOp::Const(i.imm.unwrap_or(0)), LawOp::Mul { w16: w }],
                w,
            );
        }
        // Word stores through a register holding a CODE address (self-modifying SH words).
        (0x89, _) if w => {
            let Some(Rm::Mem(Mem::Reg {
                base: Some(b),
                index: None,
                disp,
            })) = i.rm
            else {
                return Err(unsupported());
            };
            let Val::Const(addr) = s.regs[b as usize] else {
                return Err("store through an unknown address".into());
            };
            let at = cx
                .offset(addr + disp as i64)
                .filter(|a| *a + 2 <= cx.limit)
                .ok_or("store outside CODE")?;
            let law = law_of(&s.regs[i.reg as usize]).ok_or("store of an unknown value")?;
            s.stores.push(Store { at, law });
        }
        _ => return Err(unsupported()),
    }
    s.at = next;
    Ok(Step::Next)
}
/// Byte signature of decoded stub instructions with addresses masked: absolute
/// operands become `<NAME>` for imports, `<sh>` for CODE addresses, `<abs>`
/// otherwise; branch displacements become `<j>` and `add r32, K` after
/// `call $+5; pop` becomes `<k>`. Opcodes, immediates and store offsets stay.
pub fn signature(cx: &Context, insns: &[Insn]) -> String {
    let mut out = String::new();
    let mut after_pop = false;
    for i in insns {
        if !out.is_empty() {
            out.push(' ');
        }
        let bytes = &cx.code[i.at..i.end()];
        let mut k = 0;
        while k < bytes.len() {
            if Some(k) == i.abs_field {
                let v = u32_at(bytes, k).unwrap_or(0) as i64;
                if i.op == 0x81 && i.ext == 0 && after_pop {
                    out.push_str("<k>");
                } else if let Some(name) = cx.trampoline(v) {
                    out.push('<');
                    out.push_str(name);
                    out.push('>');
                } else if cx.offset(v).is_some() {
                    out.push_str("<sh>");
                } else {
                    out.push_str("<abs>");
                }
                k += 4;
                continue;
            }
            if let Some((at, n)) = i.rel_field.filter(|(at, _)| *at == k) {
                // call $+5 is structural; keep it literal.
                if i.op == 0xe8 && i.rel == Some(i.end() as i64) {
                    out.push_str("00000000");
                } else {
                    out.push_str("<j>");
                }
                k = at + n as usize;
                continue;
            }
            out.push_str(&format!("{:02X}", bytes[k]));
            k += 1;
        }
        after_pop = matches!(i.op, 0x58..=0x5f) || (after_pop && i.op != 0x81);
    }
    out
}
