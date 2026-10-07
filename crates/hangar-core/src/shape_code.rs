//! Whole-CODE SH inventory: every byte of CODE in a record with an exact span,
//! relative and absolute pointers, imports, embedded x86 extents, written
//! vertex slots, static reachability and stub-derived part bindings.
//! Record grammar follows OpenFA's instruction table; x86 extents follow its
//! jump-following disassembly. Bytes outside that grammar become opaque spans.
#[path = "shape_x86.rs"]
pub mod x86;

use crate::{invalid, model::section, slice, u16_at, u32_at, Result};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
pub use x86::{Condition, LawOp, Rel};

pub const END_MARKER: &[u8] = &[1, 2, 3, 2, 1, 2, 3, 2, 1];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// An SH record identified by its opcode (0xFF is the FF FF header).
    Sh(u8),
    /// A run of 0x1E bytes: padding, or the end of a called block.
    Pad,
    /// 0x00 end-of-object record (normally 18 bytes).
    EndObject,
    /// Embedded x86; `header` when the span starts with its `F0 00`.
    X86 { header: bool },
    /// Bytes inside an x86 extent that are neither decoded code nor SH records.
    X86Data,
    /// End marker and its zero padding.
    EndShape,
    /// `FF 25 <IAT slot>` import trampoline.
    Trampoline,
    /// Bytes the grammar does not explain. Preserved, never interpreted.
    Opaque,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    /// Signed 16-bit displacement from `base`.
    Rel16,
    /// Signed 32-bit displacement from `base`.
    Rel32,
    /// Unsigned forward 16-bit displacement from `base` (F2).
    Off16,
    /// HIGHLOW-relocated 32-bit virtual address.
    Abs32,
    /// x86 rel8 branch.
    X86Rel8,
    /// x86 rel32 branch or call.
    X86Rel32,
    /// `add r32, K` after `call $+5; pop r32`: K is relative to `base`.
    SelfOffset,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// CODE offset.
    Code(usize),
    /// Virtual address outside CODE (an import address slot).
    External(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pointer {
    /// CODE offset of the pointer field.
    pub field: usize,
    pub kind: PointerKind,
    /// CODE offset the displacement is measured from (0 for Abs32).
    pub base: usize,
    pub target: Target,
}
#[derive(Clone, Debug)]
pub struct Record {
    /// CODE-relative offset; add `Inventory::code_start` for a file offset.
    pub offset: usize,
    pub len: usize,
    pub kind: Kind,
    pub pointers: Vec<Pointer>,
    /// Reached by the static traversal (any state, any LOD, any x86 path).
    pub reached: bool,
}
impl Record {
    pub fn end(&self) -> usize {
        self.offset + self.len
    }
}
#[derive(Clone, Debug)]
pub struct Import {
    pub name: String,
    /// Virtual address of the import address table slot.
    pub iat: usize,
    /// CODE offset of its `FF 25` trampoline (the alias stubs reference).
    pub alias: Option<usize>,
}
/// One F0 entry point and what the bounded evaluator found on its paths.
#[derive(Clone, Debug)]
pub struct Stub {
    /// CODE offset of the `F0 00` record.
    pub offset: usize,
    /// Decoded code blocks reached from this entry.
    pub blocks: Vec<(usize, usize)>,
    pub outcomes: Vec<x86::Outcome>,
    pub unrecognised: Option<(usize, String)>,
    /// Address-masked byte signature of the reached instructions.
    pub signature: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingKind {
    /// The target is drawn when one of the condition sets holds.
    Toggle,
    /// The stub stores laws into the target C4/C6 words before drawing it.
    Xform { writes: Vec<(usize, Vec<LawOp>)> },
}
/// A stub-gated or stub-driven SH target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub stub: usize,
    /// CODE offset the stub resumes at.
    pub target: usize,
    /// Opcode of the target record (12, 6E, C4, C6 for geometry calls).
    pub target_op: u8,
    /// Called block for 12/6E/C4/C6 targets.
    pub block: Option<usize>,
    /// C4/C6 translation in vertex order (right, forward, up).
    pub pivot: Option<[i32; 3]>,
    /// Alternative condition sets (OR of ANDs) under which the target is reached.
    pub when: Vec<Vec<Condition>>,
    pub kind: BindingKind,
}
impl Binding {
    pub fn variables(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for c in self.when.iter().flatten() {
            out.insert(c.variable.clone());
        }
        if let BindingKind::Xform { writes } = &self.kind {
            for (_, law) in writes {
                for op in law {
                    if let LawOp::Var(v) = op {
                        out.insert(v.clone());
                    }
                }
            }
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct Inventory {
    /// File offset of CODE.
    pub code_start: usize,
    pub code_len: usize,
    /// Virtual address of CODE offset 0.
    pub code_va: usize,
    pub records: Vec<Record>,
    pub imports: Vec<Import>,
    /// CODE offset of the end marker, when present.
    pub end_marker: Option<usize>,
    /// Vertex slots written by any 82 record anywhere in CODE.
    pub slots: BTreeSet<usize>,
    pub stubs: Vec<Stub>,
    pub bindings: Vec<Binding>,
}
fn word(c: &[u8], at: usize) -> Result<i32> {
    Ok(u16_at(c, at)? as u16 as i16 as i32)
}
fn rel(c: &[u8], kind: PointerKind, field: usize, base: usize) -> Result<Pointer> {
    let d = match kind {
        PointerKind::Rel32 => u32_at(c, field)? as u32 as i32 as i64,
        PointerKind::Off16 => u16_at(c, field)? as i64,
        _ => word(c, field)? as i64,
    };
    let t = base as i64 + d;
    if t < 0 || t as usize >= c.len() {
        return Err(invalid("SH pointer outside CODE"));
    }
    Ok(Pointer {
        field,
        kind,
        base,
        target: Target::Code(t as usize),
    })
}
/// Size and pointers of one SH record at `p` (not F0, 00 or 1E).
fn sh_record(c: &[u8], p: usize) -> Result<(usize, Vec<Pointer>)> {
    use PointerKind::*;
    let op = slice(c, p, 1)?[0];
    let byte_magic = matches!(op, 0x38 | 0xbc | 0xf6 | 0xfc | 0xff);
    if !byte_magic && slice(c, p + 1, 1)?[0] != 0 {
        return Err(invalid("SH word opcode with a non-zero second byte"));
    }
    let mut ptr = Vec::new();
    let len = match op {
        0x08 | 0x2e | 0x44 | 0x72 | 0xb8 | 0xca | 0xd0 | 0xda | 0xe0 => 4,
        0x46 | 0x4e | 0xb2 | 0xbc | 0xee => 2,
        0x3a | 0x50 | 0x96 | 0xe8 => 6,
        0x68 | 0xd2 | 0xea => 8,
        0x66 | 0x76 | 0x7a | 0xe6 => 10,
        0x78 | 0xdc => 12,
        0xe2 => 16,
        0xe4 => 20,
        0xce => 40,
        0xf6 => 7,
        0xff => {
            if slice(c, p, 2)? != [0xff, 0xff] {
                return Err(invalid("Malformed SH header"));
            }
            14
        }
        0x12 | 0x48 | 0xac => {
            ptr.push(rel(c, Rel16, p + 2, p + 4)?);
            4
        }
        0xf2 => {
            ptr.push(rel(c, Off16, p + 2, p + 4)?);
            4
        }
        0x6e => {
            ptr.push(rel(c, Rel32, p + 2, p + 6)?);
            6
        }
        0xa6 => {
            ptr.push(rel(c, Rel16, p + 2, p + 6)?);
            6
        }
        0xc8 => {
            ptr.push(rel(c, Rel16, p + 6, p + 8)?);
            8
        }
        0xc4 => {
            ptr.push(rel(c, Rel16, p + 14, p + 16)?);
            16
        }
        0xc6 => {
            ptr.push(rel(c, Rel32, p + 14, p + 18)?);
            18
        }
        0x38 => {
            // The operand is not a code pointer: it lands on a record start for
            // under 0.1% of retail 38s under any tested base. Kept as unknown data.
            3
        }
        // OpenFA reads 06 as 16 + count (count 5: a 38 follows; 8: a 50 follows)
        // and 6C by the byte at +10. Both trailers are self-describing records,
        // so the inventory keeps them separate to expose their pointers. The
        // count at +14 covers the rel16 at +16 plus the trailer.
        0x06 => {
            let count = u16_at(c, p + 14)?;
            let next = slice(c, p + 18, 1)?[0];
            if !matches!((count, next), (5, 0x38) | (8, 0x50)) {
                return Err(invalid("Unrecognized 06 record trailer"));
            }
            // Measured from the end of the 18-byte head: 99.95% of retail
            // targets are record starts.
            ptr.push(rel(c, Rel16, p + 16, p + 18)?);
            18
        }
        0x6c => {
            if !matches!(slice(c, p + 10, 1)?[0], 0x38 | 0x48 | 0x50) {
                return Err(invalid("Unrecognized 6C record trailer"));
            }
            10
        }
        // OpenFA: 12 + count at +10. Retail counts are 0, 5 or 8 and, like 06,
        // 5/8 cover a rel16 plus a 38/50 trailer record.
        0x0c | 0x0e | 0x10 => match (u16_at(c, p + 10)?, slice(c, p + 14, 1).map(|b| b[0])) {
            (0, _) => 12,
            (5, Ok(0x38)) | (8, Ok(0x50)) => {
                ptr.push(rel(c, Rel16, p + 12, p + 14)?);
                14
            }
            _ => return Err(invalid("Unrecognized 0C/0E/10 record trailer")),
        },
        0x40 => {
            let n = u16_at(c, p + 2)?;
            if n == 0 || n > 1024 {
                return Err(invalid("Invalid SH frame list"));
            }
            for i in 0..n {
                ptr.push(rel(c, Rel16, p + 4 + 2 * i, p + 4 + 2 * i)?);
            }
            4 + 2 * n
        }
        0x42 => {
            let rest = slice(c, p + 2, c.len().saturating_sub(p + 2))?;
            3 + rest
                .iter()
                .take(256)
                .position(|b| *b == 0)
                .ok_or_else(|| invalid("Unterminated SH source name"))?
        }
        0x82 => {
            let n = u16_at(c, p + 2)?;
            if u16_at(c, p + 4)? % 8 != 0 || n > 8192 {
                return Err(invalid("Invalid SH vertex buffer"));
            }
            6 + 6 * n
        }
        0xfc => face_len(c, p)?,
        _ => return Err(format!("Unknown SH opcode {op:02X}")),
    };
    slice(c, p, len)?;
    Ok((len, ptr))
}
fn face_len(c: &[u8], p: usize) -> Result<usize> {
    let h = slice(c, p, 5)?;
    let (content, layout) = (h[1], h[2]);
    if layout & 0xf0 != 0 {
        return Err(invalid("Invalid face layout flags"));
    }
    let mut at = p + 5;
    if content & 0x40 != 0 {
        at += 6 + if layout & 2 != 0 { 3 } else { 6 };
    }
    let n = slice(c, at, 1)?[0] as usize;
    at += 1 + n * if layout & 4 != 0 { 2 } else { 1 };
    if content & 4 != 0 {
        at += n * if layout & 1 != 0 { 2 } else { 4 };
    }
    Ok(at - p)
}
/// HIGHLOW relocation sites (RVAs) from `.reloc`.
pub fn relocations(data: &[u8]) -> Result<Vec<usize>> {
    let pe = u32_at(data, 60)?;
    let count = u16_at(data, pe + 6)?;
    let table = pe + 24 + u16_at(data, pe + 20)?;
    let mut out = Vec::new();
    for i in 0..count.min(32) {
        let s = slice(data, table + i * 40, 40)?;
        if &s[..6] != b".reloc" {
            continue;
        }
        let bytes = slice(data, u32_at(s, 20)?, u32_at(s, 8)?.min(u32_at(s, 16)?))?;
        let mut at = 0;
        while at + 8 <= bytes.len() && out.len() < 65536 {
            let page = u32_at(bytes, at)?;
            let size = u32_at(bytes, at + 4)?;
            if page == 0 && size == 0 {
                break;
            }
            if size < 8 || size % 2 != 0 {
                return Err(invalid("Malformed relocation block"));
            }
            let block = slice(bytes, at, size)?;
            for j in (8..size).step_by(2) {
                let v = u16_at(block, j)?;
                match v >> 12 {
                    0 => {}
                    3 => out.push(page + (v & 4095)),
                    _ => return Err(invalid("Unsupported relocation type")),
                }
            }
            at += size;
        }
    }
    out.sort_unstable();
    Ok(out)
}
/// End marker, its zero padding and the trampoline stubs that follow it.
fn find_tail(c: &[u8]) -> Option<(usize, usize, usize)> {
    let mut found = None;
    for start in (0..c.len().saturating_sub(END_MARKER.len() - 1)).rev() {
        if &c[start..start + END_MARKER.len()] != END_MARKER {
            continue;
        }
        let mut p = start + END_MARKER.len();
        while c.get(p) == Some(&0) && p - start - END_MARKER.len() < 16 {
            p += 1;
        }
        let stubs = p;
        while c.get(p..p + 2) == Some(&[0xff, 0x25]) && p + 6 <= c.len() {
            p += 6;
        }
        if p == c.len() || c.get(p..p + 2) == Some(&[0xe2, 0]) {
            found = Some((start, stubs, p));
            break;
        }
    }
    found
}
impl Inventory {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (code_start, code_len, code_va) = section(data)?;
        let c = slice(data, code_start, code_len)?;
        let symbols = crate::animation::symbols(data).unwrap_or_default();
        let mut trampolines = BTreeMap::new();
        for (va, name) in &symbols {
            if let Some(at) = va.checked_sub(code_va).filter(|a| *a + 6 <= code_len) {
                if c[at..at + 2] == [0xff, 0x25] {
                    trampolines.insert(at, name.clone());
                }
            }
        }
        let mut imports: Vec<Import> = symbols
            .iter()
            .filter(|(va, _)| va.checked_sub(code_va).is_none_or(|a| a >= code_len))
            .map(|(va, name)| Import {
                name: name.clone(),
                iat: *va,
                alias: None,
            })
            .collect();
        for at in trampolines.keys() {
            let iat = u32_at(c, at + 2)?;
            if let Some(i) = imports.iter_mut().find(|i| i.iat == iat) {
                i.alias.get_or_insert(*at);
            }
        }
        let tail = find_tail(c);
        let limit = tail.map_or(code_len, |t| t.0);
        let cx = x86::Context {
            code: c,
            limit,
            code_va,
            trampolines: &trampolines,
        };
        let mut out = Self {
            code_start,
            code_len,
            code_va,
            records: Vec::new(),
            imports,
            end_marker: tail.map(|t| t.0),
            slots: BTreeSet::new(),
            stubs: Vec::new(),
            bindings: Vec::new(),
        };
        let mut known: BTreeMap<usize, usize> = BTreeMap::new();
        let mut extents: Vec<(usize, usize)> = Vec::new();
        let mut flows: Vec<x86::Flow> = Vec::new();
        out.linear(&cx, 0, limit, &mut known, &mut extents, &mut flows)?;
        if let Some((marker, stubs, end)) = tail {
            out.push(marker, stubs - marker, Kind::EndShape, Vec::new());
            for at in (stubs..end).step_by(6) {
                let iat = u32_at(c, at + 2)?;
                out.push(
                    at,
                    6,
                    Kind::Trampoline,
                    alloc::vec![Pointer {
                        field: at + 2,
                        kind: PointerKind::Abs32,
                        base: 0,
                        target: Target::External(iat),
                    }],
                );
            }
            // Legacy Hangar 0.7 panel continuations follow the stubs.
            out.linear(&cx, end, code_len, &mut known, &mut extents, &mut flows)?;
        }
        out.attach_relocations(data, c)?;
        for f in &flows {
            out.attach_x86(f);
        }
        // Spans that x86 code addresses through call/pop/add self-offsets are
        // x86 data tables, not unexplained SH bytes.
        let data_refs: Vec<usize> = out
            .records
            .iter()
            .filter(|r| matches!(r.kind, Kind::X86 { .. }))
            .flat_map(|r| r.pointers.iter())
            .filter(|p| p.kind == PointerKind::SelfOffset)
            .filter_map(|p| match p.target {
                Target::Code(t) => Some(t),
                _ => None,
            })
            .collect();
        for r in &mut out.records {
            if r.kind == Kind::Opaque && data_refs.iter().any(|t| r.offset <= *t && *t < r.end()) {
                r.kind = Kind::X86Data;
            }
        }
        for r in &out.records {
            if r.kind == Kind::Sh(0x82) {
                let n = u16_at(c, r.offset + 2)?;
                let slot = u16_at(c, r.offset + 4)? / 8;
                out.slots.extend(slot..slot + n);
            }
        }
        out.reach(&flows);
        out.analyze_stubs(&cx)?;
        Ok(out)
    }
    fn push(&mut self, offset: usize, len: usize, kind: Kind, pointers: Vec<Pointer>) {
        if let Some(last) = self.records.last_mut() {
            // Adjacent opaque or x86-data spans merge into one.
            if last.kind == kind
                && matches!(kind, Kind::Opaque | Kind::X86Data)
                && last.end() == offset
            {
                last.len += len;
                return;
            }
        }
        self.records.push(Record {
            offset,
            len,
            kind,
            pointers,
            reached: false,
        });
    }
    fn linear(
        &mut self,
        cx: &x86::Context,
        from: usize,
        limit: usize,
        known: &mut BTreeMap<usize, usize>,
        extents: &mut Vec<(usize, usize)>,
        flows: &mut Vec<x86::Flow>,
    ) -> Result<()> {
        let c = cx.code;
        let mut p = from;
        let mut end_object: Option<usize> = None;
        let mut guard = 0;
        while p < limit {
            guard += 1;
            if guard > 1 << 20 {
                return Err(invalid("SH inventory limit"));
            }
            if let Some(end) = known.get(&p).copied() {
                self.push(p, end - p, Kind::X86 { header: false }, Vec::new());
                p = end;
                continue;
            }
            if c[p] == 0xf0 && c.get(p + 1) == Some(&0) && p + 2 < limit {
                if !known.contains_key(&(p + 2)) {
                    let blocks: Vec<_> = known.iter().map(|(s, e)| (*s, *e)).collect();
                    match x86::flow(cx, p + 2, &blocks) {
                        Ok(f) => {
                            if let (Some(a), Some(b)) = (f.blocks.first(), f.blocks.last()) {
                                extents.push((a.0, b.1));
                            }
                            for (s, e) in &f.blocks {
                                known.insert(*s, *e);
                            }
                            flows.push(f);
                        }
                        Err(_) => {
                            self.push(p, limit - p, Kind::Opaque, Vec::new());
                            return Ok(());
                        }
                    }
                }
                if let Some(end) = known.get(&(p + 2)).copied() {
                    self.push(p, end - p, Kind::X86 { header: true }, Vec::new());
                    p = end;
                    continue;
                }
            }
            let cap = known
                .range(p + 1..)
                .next()
                .map_or(limit, |(s, _)| (*s).min(limit));
            let cap = match (known.range(p + 1..).next(), cap) {
                // A header F0 00 immediately precedes its code block.
                (Some((s, _)), cap)
                    if *s >= 2 && c[*s - 2] == 0xf0 && c[*s - 1] == 0 && *s - 2 >= p =>
                {
                    (*s - 2).min(cap)
                }
                (_, cap) => cap,
            };
            let in_extent = extents.iter().any(|(a, b)| *a <= p && p < *b);
            let parsed = match c[p] {
                // One record per byte: retail pointers land inside 1E runs.
                0x1e => Ok((1, Kind::Pad, Vec::new())),
                0x00 if !in_extent => {
                    let rest = cap - p;
                    let n = if rest < 18 {
                        rest
                    } else if (c[p + 16] == 0 && c[p + 17] == 0 && c[p + 1..p + 6] != [0; 5])
                        || (end_object.is_none() && rest > 80 && c[p + 2] == 0x10)
                    {
                        18
                    } else {
                        rest
                    };
                    Ok((n, Kind::EndObject, Vec::new()))
                }
                0x00 => Err(invalid("End object inside x86")),
                op => sh_record(c, p).map(|(n, ptr)| (n, Kind::Sh(op), ptr)),
            };
            match parsed {
                Ok((n, kind, ptr))
                    if p + n <= cap && end_object.is_none_or(|e| p >= e || p + n <= e) =>
                {
                    if kind == Kind::Sh(0xf2) {
                        if let Some(Target::Code(t)) = ptr.first().map(|q| q.target) {
                            end_object = Some(t);
                        }
                    }
                    self.push(p, n, kind, ptr);
                    p += n;
                }
                Ok((n, _, _))
                    if end_object.is_some_and(|e| p < e && p + n > e)
                        && end_object.unwrap_or(0) <= cap =>
                {
                    // OpenFA: dead bytes before the end object (e.g. SOLDIER, CATGUY).
                    let e = end_object.unwrap_or(cap);
                    self.push(p, e - p, Kind::Opaque, Vec::new());
                    p = e;
                }
                _ => {
                    let kind = if in_extent {
                        Kind::X86Data
                    } else {
                        Kind::Opaque
                    };
                    // Inside an x86 extent the gap ends at the next code block.
                    let stop = if in_extent { cap } else { limit };
                    self.push(p, stop - p, kind, Vec::new());
                    p = stop;
                }
            }
        }
        Ok(())
    }
    /// Index of the record containing a CODE offset.
    pub fn record_index(&self, at: usize) -> Option<usize> {
        let i = self.records.partition_point(|r| r.offset <= at);
        (i > 0 && at < self.records[i - 1].end()).then(|| i - 1)
    }
    /// Index of the record starting exactly at a CODE offset.
    pub fn starting_at(&self, at: usize) -> Option<usize> {
        self.records.binary_search_by_key(&at, |r| r.offset).ok()
    }
    fn attach_relocations(&mut self, data: &[u8], c: &[u8]) -> Result<()> {
        let code_rva = self.code_va - u32_at(data, u32_at(data, 60)? + 52)?;
        for site in relocations(data)? {
            let Some(at) = site.checked_sub(code_rva).filter(|a| *a + 4 <= c.len()) else {
                continue;
            };
            let Some(i) = self.record_index(at) else {
                continue;
            };
            if self.records[i].pointers.iter().any(|p| p.field == at) {
                continue;
            }
            let v = u32_at(c, at)?;
            let target = match v.checked_sub(self.code_va).filter(|t| *t < c.len()) {
                Some(t) => Target::Code(t),
                None => Target::External(v),
            };
            self.records[i].pointers.push(Pointer {
                field: at,
                kind: PointerKind::Abs32,
                base: 0,
                target,
            });
        }
        Ok(())
    }
    fn attach_x86(&mut self, f: &x86::Flow) {
        let mut popped: Option<(u8, usize)> = None;
        for (n, i) in f.insns.iter().enumerate() {
            let Some(r) = self.record_index(i.at) else {
                continue;
            };
            if let (Some((field, w)), Some(t)) = (i.rel_field, i.rel) {
                self.records[r].pointers.push(Pointer {
                    field: i.at + field,
                    kind: if w == 1 {
                        PointerKind::X86Rel8
                    } else {
                        PointerKind::X86Rel32
                    },
                    base: i.end(),
                    target: Target::Code(t.max(0) as usize),
                });
            }
            // call $+5; pop r; add r, K
            if matches!(i.op, 0x58..=0x5f)
                && n > 0
                && f.insns[n - 1].op == 0xe8
                && f.insns[n - 1].rel == Some(i.at as i64)
            {
                popped = Some(((i.op & 7) as u8, i.at));
            } else if let (Some((reg, base)), 0x81, 0) = (popped, i.op, i.ext) {
                if i.rm == Some(x86::Rm::Reg(reg)) {
                    let field = i.at + i.abs_field.unwrap_or(2);
                    let k = i.imm.unwrap_or(0) as i64;
                    self.records[r].pointers.push(Pointer {
                        field,
                        kind: PointerKind::SelfOffset,
                        base,
                        target: Target::Code((base as i64 + k).max(0) as usize),
                    });
                }
                popped = None;
            }
        }
        for r in &mut self.records {
            r.pointers.sort_unstable_by_key(|p| p.field);
        }
    }
    /// Static traversal over every branch: calls (12/6E/C4/C6) return to the next
    /// record, 06/A6/C8/AC continue on both paths, 48 and 40 jump, 1E and 00
    /// end a path, and F0 entries reach every SH resume of their x86 flow.
    fn reach(&mut self, flows: &[x86::Flow]) {
        let mut seen = BTreeSet::new();
        let mut work: Vec<usize> = alloc::vec![0];
        while let Some(at) = work.pop() {
            let Some(i) = self.starting_at(at) else {
                continue;
            };
            if !seen.insert(at) || seen.len() > 1 << 20 {
                continue;
            }
            self.records[i].reached = true;
            let r = &self.records[i];
            let targets = r.pointers.iter().filter_map(|p| match (p.kind, p.target) {
                (PointerKind::Rel16 | PointerKind::Rel32, Target::Code(t)) => Some(t),
                _ => None,
            });
            match r.kind {
                Kind::Sh(0x48 | 0x40) => work.extend(targets),
                Kind::Sh(0x06 | 0x12 | 0x6e | 0xc4 | 0xc6 | 0xa6 | 0xc8 | 0xac) => {
                    work.extend(targets);
                    work.push(r.end());
                }
                Kind::Sh(_) => work.push(r.end()),
                Kind::X86 { .. } => {
                    let o = r.offset;
                    for f in flows
                        .iter()
                        .filter(|f| f.blocks.iter().any(|b| b.0 <= o + 2 && o < b.1))
                    {
                        for (s, _) in &f.blocks {
                            if let Some(j) = self
                                .starting_at(*s)
                                .or_else(|| self.starting_at(s.saturating_sub(2)))
                            {
                                self.records[j].reached = true;
                            }
                        }
                        for (_, e) in &f.exits {
                            if let x86::Exit::Sh(t) = e {
                                work.push(*t);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    fn analyze_stubs(&mut self, cx: &x86::Context) -> Result<()> {
        let c = cx.code;
        let heads: Vec<usize> = self
            .records
            .iter()
            .filter(|r| r.kind == Kind::X86 { header: true })
            .map(|r| r.offset)
            .collect();
        for at in heads.into_iter().take(4096) {
            let f = x86::flow(cx, at + 2, &[]).unwrap_or_default();
            let a = x86::analyze(cx, at + 2);
            self.stubs.push(Stub {
                offset: at,
                blocks: f.blocks.clone(),
                outcomes: a.outcomes,
                unrecognised: a.unrecognised,
                signature: x86::signature(cx, &f.insns),
            });
        }
        let mut bindings: Vec<Binding> = Vec::new();
        for s in &self.stubs {
            for o in &s.outcomes {
                let Some(i) = self.starting_at(o.resume) else {
                    continue;
                };
                let r = &self.records[i];
                // Resumes at padding, another stub or the end are skip paths. Any
                // other conditional resume (direct geometry, a jump) is reported.
                let Kind::Sh(op) = r.kind else { continue };
                let call = matches!(op, 0x12 | 0x6e | 0xc4 | 0xc6);
                let writes: Vec<(usize, Vec<LawOp>)> = o
                    .stores
                    .iter()
                    .filter(|st| {
                        matches!(op, 0xc4 | 0xc6)
                            && st.at >= r.offset + 2
                            && st.at + 2 <= r.offset + 14
                            && (st.at - r.offset).is_multiple_of(2)
                    })
                    .map(|st| ((st.at - r.offset - 2) / 2, st.law.clone()))
                    .collect();
                if o.conditions.is_empty() && writes.is_empty() {
                    continue;
                }
                let kind = if writes.is_empty() {
                    BindingKind::Toggle
                } else {
                    BindingKind::Xform { writes }
                };
                if let Some(b) = bindings
                    .iter_mut()
                    .find(|b| b.stub == s.offset && b.target == o.resume && b.kind == kind)
                {
                    if !b.when.contains(&o.conditions) {
                        b.when.push(o.conditions.clone());
                    }
                    continue;
                }
                let block = r.pointers.iter().find_map(|p| match p.target {
                    Target::Code(t) if call => Some(t),
                    _ => None,
                });
                let pivot = if matches!(op, 0xc4 | 0xc6) {
                    Some([
                        word(c, r.offset + 2)?,
                        word(c, r.offset + 6)?,
                        word(c, r.offset + 4)?,
                    ])
                } else {
                    None
                };
                bindings.push(Binding {
                    stub: s.offset,
                    target: o.resume,
                    target_op: op,
                    block,
                    pivot,
                    when: alloc::vec![o.conditions.clone()],
                    kind,
                });
            }
        }
        self.bindings = bindings;
        Ok(())
    }
    pub fn opaque_bytes(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.kind == Kind::Opaque)
            .map(|r| r.len)
            .sum()
    }
    /// True when every CODE byte is in exactly one record, in order.
    pub fn contiguous(&self) -> bool {
        let mut at = 0;
        for r in &self.records {
            if r.offset != at || r.len == 0 {
                return false;
            }
            at = r.end();
        }
        at == self.code_len
    }
    /// First run of `n` consecutive vertex slots no 82 record writes, below 8192.
    pub fn free_slots(&self, n: usize) -> Option<usize> {
        let mut start = 0;
        for s in self.slots.iter().copied().chain(core::iter::once(8192)) {
            if s >= start + n {
                return (start + n <= 8192).then_some(start);
            }
            start = start.max(s + 1);
        }
        None
    }
    /// Import name of a trampoline alias virtual address.
    pub fn alias_name(&self, va: usize) -> Option<&str> {
        let at = va.checked_sub(self.code_va)?;
        self.imports
            .iter()
            .find(|i| i.alias == Some(at))
            .map(|i| i.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Model, Pose};
    const IMPORTS: [&str; 4] = ["_PLgearDown", "_PLgearPos", "_PLhook", "do_start_interp"];
    const STUBS: usize = 0x310;
    fn alias(i: usize) -> u32 {
        (0x1000 + STUBS + 6 * i) as u32
    }
    fn va(at: usize) -> u32 {
        (0x1000 + at) as u32
    }
    struct Asm {
        c: Vec<u8>,
        relocs: Vec<usize>,
    }
    impl Asm {
        fn b(&mut self, b: &[u8]) -> &mut Self {
            self.c.extend(b);
            self
        }
        fn abs(&mut self, v: u32) -> &mut Self {
            self.relocs.push(self.c.len());
            self.c.extend(v.to_le_bytes());
            self
        }
        fn w(&mut self, v: i16) -> &mut Self {
            self.c.extend(v.to_le_bytes());
            self
        }
        fn at(&self, n: usize) {
            assert_eq!(self.c.len(), n, "fixture layout");
        }
        fn verts(&mut self, slot: u16, v: &[[i16; 3]]) -> &mut Self {
            self.b(&[0x82, 0]).w(v.len() as i16).w((slot * 8) as i16);
            for p in v {
                for x in p {
                    self.w(*x);
                }
            }
            self
        }
    }
    /// Synthetic gear/hook shape: an xform stub (cmp/jne, call/pop/add ebx,
    /// mov ax/sar/neg/store into C4 r2), a toggle stub over a 12 call, an
    /// unreached orphan, end object, end marker and four import trampolines.
    fn code(rotation: [i16; 3]) -> Asm {
        let mut a = Asm {
            c: Vec::new(),
            relocs: Vec::new(),
        };
        a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
        a.b(&[0xf2, 0]).w(0x107 - 0x12);
        a.b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
        a.at(0x22);
        a.verts(0, &[[0, 0, 0], [10, 0, 0], [0, 10, 0]]);
        a.b(&[0xfc, 0, 0, 32, 0, 3, 0, 1, 2]);
        a.at(0x43);
        a.b(&[0xf0, 0, 0x66, 0x83, 0x3d])
            .abs(alias(0))
            .b(&[1, 0x75, 0x39]);
        a.b(&[0xe8, 0, 0, 0, 0, 0x5b, 0x81, 0xc3])
            .b(&(0x78u32 - 0x54).to_le_bytes());
        a.b(&[0x66, 0xa1]).abs(alias(1));
        a.b(&[0x66, 0xd1, 0xf8, 0x66, 0xf7, 0xd8, 0x66, 0x89, 0x43, 0x0a]);
        a.b(&[0x68])
            .abs(va(0x76))
            .b(&[0x68])
            .abs(alias(3))
            .b(&[0xc3]);
        a.at(0x76);
        a.b(&[0xc4, 0]).w(5).w(-3).w(2);
        for r in rotation {
            a.w(r);
        }
        a.w(0xc3 - 0x86);
        a.b(&[0xf0, 0, 0x68])
            .abs(va(0x93))
            .b(&[0x68])
            .abs(alias(3))
            .b(&[0xc3]);
        a.at(0x93);
        a.b(&[0xf0, 0, 0x66, 0x83, 0x3d])
            .abs(alias(2))
            .b(&[1, 0x75, 0x11]);
        a.b(&[0x68])
            .abs(va(0xaa))
            .b(&[0x68])
            .abs(alias(3))
            .b(&[0xc3]);
        a.b(&[0x12, 0]).w(0xe5 - 0xae);
        a.b(&[0xf0, 0, 0x68])
            .abs(va(0xbb))
            .b(&[0x68])
            .abs(alias(3))
            .b(&[0xc3]);
        a.at(0xbb);
        a.b(&[0x48, 0]).w(0x107 - 0xbf);
        a.b(&[0xca, 0, 0, 0]);
        a.at(0xc3);
        a.verts(3, &[[0, 0, 0], [0, 0, -10], [1, 0, -10]]);
        a.b(&[0xfc, 0, 0, 40, 0, 3, 3, 4, 5, 0x1e]);
        a.at(0xe5);
        a.verts(6, &[[0, -5, 0], [0, -9, 0], [0, -9, -2]]);
        a.b(&[0xfc, 0, 0, 50, 0, 3, 6, 7, 8, 0x1e]);
        a.at(0x107);
        a.b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
        a.c.resize(0x300, 0x1e);
        a.b(END_MARKER);
        a.c.resize(STUBS, 0);
        for i in 0..IMPORTS.len() {
            a.b(&[0xff, 0x25]).abs(0x5060 + 4 * i as u32);
        }
        a
    }
    fn put(b: &mut [u8], at: usize, v: usize) {
        b[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
    }
    fn module(a: &Asm) -> Vec<u8> {
        let code = &a.c;
        let mut b = vec![0; 1024 + 4096 + 512 + 512];
        b[..2].copy_from_slice(b"MZ");
        put(&mut b, 60, 128);
        b[128..132].copy_from_slice(b"PL\0\0");
        b[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
        b[134..136].copy_from_slice(&3u16.to_le_bytes());
        b[148..150].copy_from_slice(&224u16.to_le_bytes());
        b[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
        let reloc_size = 8 + 2 * (a.relocs.len() + a.relocs.len() % 2);
        for (at, v) in [
            (156, code.len()),
            (184, 4096),
            (188, 512),
            (208, 0x7000),
            (212, 1024),
            (244, 16),
            (256, 0x5000),
            (260, 40),
            (288, 0x6000),
            (292, reloc_size),
        ] {
            put(&mut b, at, v);
        }
        for (n, (name, rva, virt, raw, size)) in [
            ("CODE", 0x1000, code.len(), 1024, 4096),
            (".idata", 0x5000, 0x200, 5120, 512),
            (".reloc", 0x6000, 512, 5632, 512),
        ]
        .into_iter()
        .enumerate()
        {
            let h = 376 + n * 40;
            b[h..h + name.len()].copy_from_slice(name.as_bytes());
            for (off, value) in [(8, virt), (12, rva), (16, size), (20, raw)] {
                put(&mut b, h + off, value);
            }
        }
        b[1024..1024 + code.len()].copy_from_slice(code);
        let idata = 5120;
        put(&mut b, idata, 0x5040);
        put(&mut b, idata + 12, 0x5080);
        put(&mut b, idata + 16, 0x5060);
        b[idata + 0x80..idata + 0x83].copy_from_slice(b"FA\0");
        for (i, name) in IMPORTS.iter().enumerate() {
            let rva = 0x5090 + 32 * i;
            put(&mut b, idata + 0x40 + 4 * i, rva);
            put(&mut b, idata + 0x60 + 4 * i, rva);
            let at = idata + rva - 0x5000 + 2;
            b[at..at + name.len()].copy_from_slice(name.as_bytes());
        }
        put(&mut b, 5632, 0x1000);
        put(&mut b, 5636, reloc_size);
        for (i, site) in a.relocs.iter().enumerate() {
            b[5640 + 2 * i..5642 + 2 * i].copy_from_slice(&(0x3000 | *site as u16).to_le_bytes());
        }
        b
    }
    fn gear() -> Vec<u8> {
        module(&code([0; 3]))
    }
    fn pointer(inv: &Inventory, field: usize) -> Pointer {
        *inv.records
            .iter()
            .flat_map(|r| r.pointers.iter())
            .find(|p| p.field == field)
            .unwrap_or_else(|| panic!("no pointer at {field:X}"))
    }
    #[test]
    fn inventory_covers_every_byte_with_exact_spans() {
        let b = gear();
        let inv = Inventory::parse(&b).unwrap();
        assert!(inv.contiguous());
        assert_eq!(inv.opaque_bytes(), 0);
        assert_eq!(inv.end_marker, Some(0x300));
        let spans: Vec<_> = inv
            .records
            .iter()
            .take(14)
            .map(|r| (r.offset, r.len, r.kind))
            .collect();
        assert_eq!(
            spans,
            [
                (0, 14, Kind::Sh(0xff)),
                (0x0e, 4, Kind::Sh(0xf2)),
                (0x12, 16, Kind::Sh(0xe2)),
                (0x22, 24, Kind::Sh(0x82)),
                (0x3a, 9, Kind::Sh(0xfc)),
                (0x43, 0x33, Kind::X86 { header: true }),
                (0x76, 16, Kind::Sh(0xc4)),
                (0x86, 13, Kind::X86 { header: true }),
                (0x93, 0x17, Kind::X86 { header: true }),
                (0xaa, 4, Kind::Sh(0x12)),
                (0xae, 13, Kind::X86 { header: true }),
                (0xbb, 4, Kind::Sh(0x48)),
                (0xbf, 4, Kind::Sh(0xca)),
                (0xc3, 24, Kind::Sh(0x82)),
            ]
        );
        let at = |o: usize| &inv.records[inv.starting_at(o).unwrap()];
        assert_eq!((at(0x107).kind, at(0x107).len), (Kind::EndObject, 18));
        assert_eq!((at(0x300).kind, at(0x300).len), (Kind::EndShape, 16));
        assert_eq!(
            inv.records
                .iter()
                .filter(|r| r.kind == Kind::Trampoline)
                .count(),
            4
        );
        // Reachability: the orphan behind the 48 jump is the only unreached SH record.
        let unreached: Vec<_> = inv
            .records
            .iter()
            .filter(|r| matches!(r.kind, Kind::Sh(_)) && !r.reached)
            .map(|r| r.offset)
            .collect();
        assert_eq!(unreached, [0xbf]);
        assert_eq!(
            inv.slots.iter().copied().collect::<Vec<_>>(),
            (0..9).collect::<Vec<_>>()
        );
        assert_eq!(inv.free_slots(4), Some(9));
        let names: Vec<_> = inv
            .imports
            .iter()
            .map(|i| (i.name.as_str(), i.alias))
            .collect();
        assert!(names.contains(&("_PLgearPos", Some(STUBS + 6))));
        assert_eq!(inv.alias_name(alias(3) as usize), Some("do_start_interp"));
    }
    #[test]
    fn every_pointer_kind_is_recorded_with_its_base_and_target() {
        let inv = Inventory::parse(&gear()).unwrap();
        use PointerKind::*;
        let cases = [
            (0x10, Off16, 0x12, Target::Code(0x107)),
            (0x84, Rel16, 0x86, Target::Code(0xc3)),
            (0xac, Rel16, 0xae, Target::Code(0xe5)),
            (0xbd, Rel16, 0xbf, Target::Code(0x107)),
            (0x48, Abs32, 0, Target::Code(STUBS)),
            (0x6c, Abs32, 0, Target::Code(0x76)),
            (0x4e, X86Rel8, 0x4f, Target::Code(0x88)),
            (0x50, X86Rel32, 0x54, Target::Code(0x54)),
            (0x57, SelfOffset, 0x54, Target::Code(0x78)),
            (STUBS + 2, Abs32, 0, Target::External(0x5060)),
        ];
        for (field, kind, base, target) in cases {
            assert_eq!(
                pointer(&inv, field),
                Pointer {
                    field,
                    kind,
                    base,
                    target
                },
                "{field:X}"
            );
        }
        // Linear SH records with the remaining pointer forms.
        let mut c = Vec::new();
        c.extend([0x6e, 0, 4, 0, 0, 0]); // -> 0x0a
        c.extend([0xc6, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 0, 0, 0, 0]); // -> 0x18
        c.extend([0xa6, 0, 0, 0, 1, 0]); // -> 0x1e
        c.extend([0xac, 0, 0, 0]); // -> 0x22
        c.extend([0xc8, 0, 1, 0, 2, 0, 0, 0]); // -> 0x2a
        c.extend([0x40, 0, 2, 0, 4, 0, 2, 0]); // frames -> 0x30, 0x30
        c.extend([6, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 5, 0, 3, 0]); // -> 0x45
        c.extend([0x38, 0x99, 0x99]);
        c.extend([0x0c, 0, 1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 0, 0, 0x38, 0, 0]); // -> 0x51
        c.extend([0x6c, 0, 1, 0, 2, 0, 3, 0, 4, 0, 0x48, 0, 0, 0]); // 48 -> 0x5f
        c.push(0);
        let mut a = code([0; 3]);
        a.c = c;
        a.relocs.clear();
        let inv = Inventory::parse(&module(&a)).unwrap();
        assert!(inv.contiguous() && inv.opaque_bytes() == 0);
        for (field, kind, base, t) in [
            (2, Rel32, 6, 0x0a),
            (0x14, Rel32, 0x18, 0x18),
            (0x1a, Rel16, 0x1e, 0x1e),
            (0x20, Rel16, 0x22, 0x22),
            (0x28, Rel16, 0x2a, 0x2a),
            (0x2e, Rel16, 0x2e, 0x32),
            (0x30, Rel16, 0x30, 0x32),
            (0x42, Rel16, 0x44, 0x47),
            (0x53, Rel16, 0x55, 0x55),
            (0x64, Rel16, 0x66, 0x66),
        ] {
            assert_eq!(
                pointer(&inv, field),
                Pointer {
                    field,
                    kind,
                    base,
                    target: Target::Code(t)
                },
                "{field:X}"
            );
        }
        let kinds: Vec<_> = inv.records.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Sh(0x38)) && kinds.contains(&Kind::Sh(0x6c)));
    }
    #[test]
    fn truncated_and_corrupted_input_never_panics() {
        let b = gear();
        for n in 0..b.len() {
            let _ = Inventory::parse(&b[..n]);
        }
        for at in 1024..1024 + 0x330 {
            for v in [0x00, 0x1e, 0x48, 0x82, 0xf0, 0xff] {
                let mut m = b.clone();
                m[at] = v;
                let _ = Inventory::parse(&m);
                let _ = Model::with_pose(&m, &Pose::from([("_PLgearPos".into(), -8192)]));
            }
        }
        // An unknown SH opcode becomes an opaque span, never a guess.
        let mut a = code([0; 3]);
        a.c[0xbf] = 0x9e;
        let inv = Inventory::parse(&module(&a)).unwrap();
        assert!(inv.contiguous());
        assert!(inv.opaque_bytes() > 0);
        assert_eq!(
            inv.records
                .iter()
                .find(|r| r.kind == Kind::Opaque)
                .unwrap()
                .offset,
            0xbf
        );
    }
    #[test]
    fn xform_and_toggle_stubs_are_recognised() {
        let inv = Inventory::parse(&gear()).unwrap();
        let cond = |v: &str, rel, value| Condition {
            variable: v.into(),
            rel,
            value,
            w16: true,
        };
        assert_eq!(inv.bindings.len(), 3);
        let gear = &inv.bindings[0];
        assert_eq!(
            (gear.stub, gear.target, gear.target_op, gear.block),
            (0x43, 0x76, 0xc4, Some(0xc3))
        );
        assert_eq!(gear.pivot, Some([5, 2, -3]));
        assert_eq!(gear.when, [[cond("_PLgearDown", Rel::Eq, 1)]]);
        assert_eq!(
            gear.kind,
            BindingKind::Xform {
                writes: vec![(
                    5,
                    vec![
                        LawOp::Var("_PLgearPos".into()),
                        LawOp::Sar { n: 1, w16: true },
                        LawOp::Neg { w16: true }
                    ]
                )]
            }
        );
        let hook = &inv.bindings[1];
        assert_eq!(
            (hook.target, hook.block, hook.kind.clone()),
            (0xaa, Some(0xe5), BindingKind::Toggle)
        );
        assert_eq!(hook.when, [[cond("_PLhook", Rel::Eq, 1)]]);
        assert_eq!(
            hook.variables().into_iter().collect::<Vec<_>>(),
            ["_PLhook"]
        );
        // The gear skip resumes at the next stub (no binding); the hook skip resumes
        // at a 48 jump and is reported as a branch of the same variable.
        let other = &inv.bindings[2];
        assert_eq!(
            (other.target, other.target_op, other.block),
            (0xbb, 0x48, None)
        );
        assert_eq!(other.when, [[cond("_PLhook", Rel::Ne, 1)]]);
        assert_eq!(inv.stubs.len(), 4);
        assert!(inv.stubs.iter().all(|s| s.unrecognised.is_none()));
        assert_eq!(
            inv.stubs[0].signature,
            "66833D<_PLgearDown>01 75<j> E800000000 5B 81C3<k> 66A1<_PLgearPos> 66D1F8 66F7D8 6689430A 68<sh> 68<do_start_interp> C3 68<sh> 68<do_start_interp> C3"
        );
        let law = match &gear.kind {
            BindingKind::Xform { writes } => writes[0].1.clone(),
            _ => unreachable!(),
        };
        assert_eq!(x86::evaluate(&law, &|_| -8192), Some(4096));
        assert_eq!(x86::evaluate(&law, &|_| 0), Some(0));
        assert_eq!(x86::evaluate(&[LawOp::Neg { w16: true }], &|_| 0), None);
    }
    #[test]
    fn unrecognised_stubs_are_reported_not_guessed() {
        // pushad, a call into an import, a branch on a computed value and a store
        // through an unknown register are all outside the reviewed subset.
        for (at, bytes) in [
            (0x45, &[0x60][..]),
            (0x45, &[0x0b, 0xc0]),
            (0x67, &[0x66, 0x89, 0x46, 0x0a]),
        ] {
            let mut a = code([0; 3]);
            a.c[at..at + bytes.len()].copy_from_slice(bytes);
            if bytes.len() < 8 && at == 0x45 {
                // Keep the x86 extent decodable: pad the rest of the compare with inc eax.
                a.c[at + bytes.len()..0x4d].fill(0x40);
            }
            let inv = Inventory::parse(&module(&a)).unwrap();
            let stub = inv.stubs.iter().find(|s| s.offset == 0x43).unwrap();
            assert!(stub.unrecognised.is_some(), "{bytes:02X?}");
            assert!(inv.bindings.iter().all(|b| b.stub != 0x43), "{bytes:02X?}");
            assert!(inv.contiguous() && inv.opaque_bytes() == 0);
        }
        // A retpoline to a non-interpreter import is a call returning to x86; here
        // the return address holds SH bytes, so the extent is opaque, not guessed.
        let mut a = code([0; 3]);
        a.c[0xa5..0xa9].copy_from_slice(&alias(2).to_le_bytes());
        let inv = Inventory::parse(&module(&a)).unwrap();
        assert!(inv.contiguous());
        let opaque = inv.records.iter().find(|r| r.kind == Kind::Opaque).unwrap();
        assert_eq!(opaque.offset, 0x93);
        assert!(inv.bindings.iter().all(|b| b.stub != 0x93));
    }
    #[test]
    fn model_tags_parts_and_keeps_local_coordinates() {
        let b = gear();
        let pose = Pose::from([("_PLgearDown".into(), 1), ("_PLhook".into(), 1)]);
        let m = Model::with_pose(&b, &pose).unwrap();
        assert_eq!(m.parts.len(), 1);
        assert_eq!(m.parts[0].offset, 1024 + 0x76);
        assert_eq!(m.groups.len(), 2);
        assert_eq!((m.groups[0].opcode, m.groups[0].part), (0xc4, Some(0)));
        assert_eq!((m.groups[1].opcode, m.groups[1].part), (0x12, None));
        assert_eq!(m.vertex_tags.len(), m.vertices.len());
        let tags: Vec<_> = m.vertex_tags.iter().map(|t| (t.part, t.group)).collect();
        assert_eq!(tags[..3], [(None, None); 3]);
        assert_eq!(tags[3..6], [(Some(0), Some(0)); 3]);
        assert_eq!(tags[6..], [(None, Some(1)); 3]);
        assert_eq!(m.vertex_tags[4].local, [0, 0, -10]);
        // Stored rotation is zero and gearPos is not supplied: translation only.
        assert_eq!(m.vertices[4].point, [5, 2, -13]);
        let faces: Vec<_> = m.faces.iter().map(|f| (f.part, f.group)).collect();
        assert_eq!(faces, [(None, None), (Some(0), Some(0)), (None, Some(1))]);
        assert_eq!(
            m.state_names
                .values()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["_PLgearDown", "_PLhook"]
        );
        // Neutral pose skips both gated blocks.
        assert_eq!(Model::parse(&b).unwrap().vertices.len(), 3);
    }
    #[test]
    fn rotation_preview_applies_stored_angles_and_evaluated_laws() {
        let b = gear();
        let up = Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -8192)]);
        let m = Model::with_pose(&b, &up).unwrap();
        // r2 = -(gearPos >> 1) = 4096: a quarter turn about forward.
        assert_eq!(m.parts[0].posed_rotation, [0, 0, 4096]);
        assert_eq!(m.parts[0].rotation, [0, 0, 0]);
        assert_eq!(m.vertices[4].point, [5 - 10, 2, -3]);
        assert_eq!(m.vertices[5].point, [5 - 10, 2, -3 - 1]);
        let half = Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -4096)]);
        let h = Model::with_pose(&b, &half).unwrap();
        assert_eq!(h.vertices[4].point, [5 - 7, 2, -3 - 7]);
        // Stored angles apply without any pose: r1 = -4096 swings down to forward.
        let stored = module(&code([0, -4096, 0]));
        let s = Model::with_pose(&stored, &Pose::from([("_PLgearDown".into(), 1)])).unwrap();
        assert_eq!(s.vertices[4].point, [5, 2 + 10, -3]);
        // r0 turns about up: right becomes forward for +4096 (OpenFA sign).
        let yaw = module(&code([4096, 0, 0]));
        let y = Model::with_pose(&yaw, &Pose::from([("_PLgearDown".into(), 1)])).unwrap();
        assert_eq!(y.vertices[5].point, [5, 2 + 1, -13]);
        // The address-keyed state reads the same laws.
        let state = m.state_from_pose(&up);
        assert_eq!(state.len(), 1);
        let mut va = state.clone();
        va.insert(alias(1) as usize, -8192);
        assert_eq!(
            Model::with_state(&b, &va).unwrap().vertices[4].point,
            m.vertices[4].point
        );
    }
    #[test]
    fn symbolic_state_keys_survive_a_tail_shift() {
        let b = gear();
        let pose = Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -8192)]);
        let before = Model::with_pose(&b, &pose).unwrap();
        let state = before.state_from_pose(&pose);
        assert_eq!(
            state.keys().copied().collect::<Vec<_>>(),
            [alias(0) as usize]
        );
        let shifted = crate::shape_edit::texture_panel(&b, 0, "NEW.PIC", 64, &[[0; 3]; 256])
            .unwrap()
            .shape;
        let inv = Inventory::parse(&shifted).unwrap();
        assert!(inv.contiguous() && inv.opaque_bytes() == 0);
        assert!(inv.end_marker.unwrap() > 0x300);
        assert_eq!(inv.bindings.len(), 3);
        let after = Model::with_pose(&shifted, &pose).unwrap();
        let points = |m: &Model| m.vertices.iter().map(|v| v.point).collect::<Vec<_>>();
        assert_eq!(points(&after), points(&before));
        // The old address no longer names the gear switch after the shift.
        assert!(!after.state_words.contains(&(alias(0) as usize)));
        assert_ne!(
            points(&Model::with_state(&shifted, &state).unwrap()),
            points(&before)
        );
        assert_eq!(
            after
                .pose_from_state(&after.state_from_pose(&pose))
                .get("_PLgearDown"),
            Some(&1)
        );
    }
}
