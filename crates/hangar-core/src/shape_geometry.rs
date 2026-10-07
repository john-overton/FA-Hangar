//! Region-scoped SH geometry edits over the whole-CODE inventory.
//!
//! Every FC face and 82 vertex buffer in CODE is decoded, whether or not a
//! pose reaches it, and tagged with the coordinate frame that draws it (the
//! root or an innermost C4/C6 part). Edits are pure functions from SH bytes to
//! SH bytes:
//! - in place and same size: deleting faces (a jump stub over the record),
//!   flipping faces, and vertex writes that update the normal and centre of
//!   exactly the faces whose slots resolve to the moved vertex;
//! - appended: new faces and vertices go in a continuation placed before the
//!   end marker, reached by detouring an existing face of the same frame, so
//!   they inherit its frame, material and slot state.
//!
//! Slot liveness is proved, never assumed: a slot used at a record resolves to
//! one 82 buffer only when, walking back, nothing rewrites it (no other
//! writer, no call whose block writes it) and every pointer entering that span
//! from outside resolves to the same buffer. Faces with pointer or relocation
//! fields, pointer targets, self-offsets or x86 stores inside their bytes are
//! never rewritten. Each result is re-parsed and checked before it is returned.
//! Coordinates are stored units in model order (right, forward, up); vertices
//! in a part are local to its pivot frame.
use crate::{
    invalid,
    model::{face_normal, Model, Pose},
    shape_code::{Condition, Inventory, Kind, PointerKind, Rel, Target},
    shape_edit::{append_continuation, continuation_start, jump, stub_out},
    slice, u16_at, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
use core::cell::RefCell;

/// Highest vertex-slot count of any retail FA_2.LIB shape (CITY2.SH, 640).
/// New slots stay below it, so no shape asks the engine's vertex table for
/// more entries than retail data proves it holds.
pub const SLOT_CEILING: usize = 640;

/// The coordinate frame that draws a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Frame {
    /// Shape coordinates (no enclosing C4/C6).
    Root,
    /// Local to the pivot of the C4/C6 record at this file offset.
    Part(usize),
    /// Reached under more than one frame; local coordinates are ambiguous.
    Mixed,
    /// No static path reaches it; it is never drawn.
    Unreached,
}
/// One FC record, decoded from its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceRecord {
    /// File offset, as `model::Face::offset`.
    pub offset: usize,
    pub len: usize,
    pub content: u8,
    pub layout: u8,
    /// Colour word at +3 (a palette index when below 256).
    pub color: u16,
    /// Stored normal and centre in model order (right, forward, up).
    pub normal: Option<[i32; 3]>,
    pub centre: Option<[i32; 3]>,
    pub slots: Vec<usize>,
    pub uv: Vec<[i32; 2]>,
    pub frame: Frame,
}
impl FaceRecord {
    pub fn end(&self) -> usize {
        self.offset + self.len
    }
}
/// One 82 vertex buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Buffer {
    /// File offset of the 82 record.
    pub offset: usize,
    /// First slot written.
    pub slot: usize,
    /// Stored coordinates in model order; local to the frame.
    pub points: Vec<[i32; 3]>,
    pub frame: Frame,
}
impl Buffer {
    /// File offset of vertex `i`'s coordinates, as `model::Vertex::offset`.
    pub fn vertex(&self, i: usize) -> usize {
        self.offset + 6 + 6 * i
    }
    pub fn end(&self) -> usize {
        self.offset + 6 + 6 * self.points.len()
    }
}
/// Writability of one stored vertex.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexStatus {
    /// File offset of the coordinates, as `model::Vertex::offset`.
    pub offset: usize,
    pub slot: usize,
    pub local: [i32; 3],
    pub frame: Frame,
    /// File offsets of the drawn faces whose slot resolves to this vertex.
    pub faces: Vec<usize>,
    /// Why the vertex cannot be written, if it cannot.
    pub refusal: Option<String>,
}
type Resolved = core::result::Result<usize, String>;
/// Bytes and analysis of one shape.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub inventory: Inventory,
    /// Every FC record in CODE order.
    pub faces: Vec<FaceRecord>,
    /// Every 82 record in CODE order.
    pub buffers: Vec<Buffer>,
    code: Vec<u8>,
    /// Frame of each inventory record.
    frames: Vec<Frame>,
    /// 1E records inside a 38 scope of their walk: padding, not a return.
    pads: BTreeSet<usize>,
    /// Slot -> indices of buffers writing it, in CODE order.
    writers: BTreeMap<usize, Vec<usize>>,
    /// Slot -> indices of faces referencing it.
    users: BTreeMap<usize, Vec<usize>>,
    /// Control-flow pointers as (target, field), CODE offsets, sorted.
    entries: Vec<(usize, usize)>,
    /// Every CODE pointer target as (target, field), sorted.
    targets: Vec<(usize, usize)>,
    /// CODE ranges addressed as data: self-offset targets and x86 stores.
    data: Vec<(usize, usize)>,
    /// Pointer fields as (CODE offset, width).
    fields: Vec<(usize, usize)>,
    /// Slots written by everything reachable from each call target.
    called: BTreeMap<usize, core::result::Result<BTreeSet<usize>, String>>,
    /// (slot, CODE position) -> writer buffer; `None` while in progress.
    memo: RefCell<BTreeMap<(usize, usize), Option<Resolved>>>,
}
enum Flow {
    Stop,
    Unexplained,
    Go {
        next: Vec<usize>,
        call: Option<usize>,
    },
}
fn word(c: &[u8], at: usize) -> Result<i32> {
    Ok(u16_at(c, at)? as u16 as i16 as i32)
}
/// Field offsets inside an FC record.
struct FaceLayout {
    normal: Option<usize>,
    centre: Option<(usize, bool)>,
    count: usize,
    index: usize,
    wide: bool,
    uv: Option<(usize, bool)>,
    end: usize,
}
fn face_layout(c: &[u8], at: usize) -> Result<FaceLayout> {
    let h = slice(c, at, 5)?;
    let (content, layout) = (h[1], h[2]);
    let mut p = at + 5;
    let (mut normal, mut centre) = (None, None);
    if content & 0x40 != 0 {
        normal = Some(p);
        centre = Some((p + 6, layout & 2 != 0));
        p += 6 + if layout & 2 != 0 { 3 } else { 6 };
    }
    let count = slice(c, p, 1)?[0] as usize;
    let wide = layout & 4 != 0;
    let index = p + 1;
    p = index + count * if wide { 2 } else { 1 };
    let mut uv = None;
    if content & 4 != 0 {
        uv = Some((p, layout & 1 != 0));
        p += count * if layout & 1 != 0 { 2 } else { 4 };
    }
    slice(c, at, p - at)?;
    Ok(FaceLayout {
        normal,
        centre,
        count,
        index,
        wide,
        uv,
        end: p,
    })
}
/// Stored right, up, forward; returned in model order right, forward, up.
fn read3(c: &[u8], at: usize, byte: bool) -> Result<[i32; 3]> {
    let v = |k: usize| -> Result<i32> {
        if byte {
            Ok(slice(c, at + k, 1)?[0] as i8 as i32)
        } else {
            word(c, at + 2 * k)
        }
    };
    Ok([v(0)?, v(2)?, v(1)?])
}
fn put3(c: &mut [u8], at: usize, byte: bool, v: [i32; 3]) -> Result<()> {
    for (k, axis) in [0, 2, 1].into_iter().enumerate() {
        let x = v[axis];
        if byte {
            if !(-128..=127).contains(&x) {
                return Err(invalid(
                    "A face centre exceeds its byte width; the record would have to grow",
                ));
            }
            *c.get_mut(at + k).ok_or("Face truncated")? = x as i8 as u8;
        } else {
            if !(-32768..=32767).contains(&x) {
                return Err(invalid("A face centre exceeds a signed word"));
            }
            c.get_mut(at + 2 * k..at + 2 * k + 2)
                .ok_or("Face truncated")?
                .copy_from_slice(&(x as i16).to_le_bytes());
        }
    }
    Ok(())
}
fn average(points: &[[i32; 3]]) -> [i32; 3] {
    let n = points.len().max(1) as i64;
    core::array::from_fn(|k| (points.iter().map(|p| p[k] as i64).sum::<i64>() / n) as i32)
}
fn in_word(p: &[i32; 3]) -> bool {
    p.iter().all(|v| (-32768..=32767).contains(v))
}
fn slots_of(c: &[u8], at: usize) -> Result<(usize, usize)> {
    Ok((u16_at(c, at + 4)? / 8, u16_at(c, at + 2)?))
}
impl Geometry {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let inventory = Inventory::parse(data)?;
        if !inventory.contiguous() {
            return Err(invalid("SH inventory does not cover CODE"));
        }
        let code = slice(data, inventory.code_start, inventory.code_len)?.to_vec();
        let cs = inventory.code_start;
        let mut g = Self {
            faces: Vec::new(),
            buffers: Vec::new(),
            code,
            frames: Vec::new(),
            pads: BTreeSet::new(),
            writers: BTreeMap::new(),
            users: BTreeMap::new(),
            entries: Vec::new(),
            targets: Vec::new(),
            data: Vec::new(),
            fields: Vec::new(),
            called: BTreeMap::new(),
            memo: RefCell::new(BTreeMap::new()),
            inventory,
        };
        for r in &g.inventory.records {
            for p in &r.pointers {
                let width = match p.kind {
                    PointerKind::Rel16 | PointerKind::Off16 => 2,
                    PointerKind::X86Rel8 => 1,
                    _ => 4,
                };
                g.fields.push((p.field, width));
                let Target::Code(t) = p.target else { continue };
                g.targets.push((t, p.field));
                match p.kind {
                    PointerKind::SelfOffset => g.data.push((t, 1)),
                    PointerKind::Off16 => {}
                    _ => g.entries.push((t, p.field)),
                }
            }
        }
        for s in &g.inventory.stubs {
            for o in &s.outcomes {
                for st in &o.stores {
                    g.data.push((st.at, 2));
                }
            }
        }
        g.entries.sort_unstable();
        g.targets.sort_unstable();
        g.data.sort_unstable();
        g.fields.sort_unstable();
        g.frames = g.walk_frames();
        let mut calls = BTreeSet::new();
        for i in 0..g.inventory.records.len() {
            if let Flow::Go { call: Some(t), .. } = g.flow(i) {
                calls.insert(t);
            }
        }
        for t in calls {
            let slots = g.reach_slots(t);
            g.called.insert(t, slots);
        }
        for i in 0..g.inventory.records.len() {
            let r = &g.inventory.records[i];
            let frame = g.frames[i];
            match r.kind {
                Kind::Sh(0x82) => {
                    let (slot, n) = slots_of(&g.code, r.offset)?;
                    let points = (0..n)
                        .map(|k| {
                            let at = r.offset + 6 + 6 * k;
                            Ok([
                                word(&g.code, at)?,
                                word(&g.code, at + 2)?,
                                word(&g.code, at + 4)?,
                            ])
                        })
                        .collect::<Result<Vec<_>>>()?;
                    for k in 0..n {
                        g.writers.entry(slot + k).or_default().push(g.buffers.len());
                    }
                    g.buffers.push(Buffer {
                        offset: cs + r.offset,
                        slot,
                        points,
                        frame,
                    });
                }
                Kind::Sh(0xfc) => {
                    let f = g.decode_face(r.offset, frame)?;
                    if f.len != r.len {
                        return Err(invalid("Face length disagrees with the inventory"));
                    }
                    for s in &f.slots {
                        let list = g.users.entry(*s).or_default();
                        if list.last() != Some(&g.faces.len()) {
                            list.push(g.faces.len());
                        }
                    }
                    g.faces.push(f);
                }
                _ => {}
            }
        }
        Ok(g)
    }
    fn decode_face(&self, at: usize, frame: Frame) -> Result<FaceRecord> {
        let c = &self.code;
        let l = face_layout(c, at)?;
        let slots = (0..l.count)
            .map(|k| {
                if l.wide {
                    u16_at(c, l.index + 2 * k)
                } else {
                    Ok(slice(c, l.index + k, 1)?[0] as usize)
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let mut uv = Vec::new();
        if let Some((p, bytes)) = l.uv {
            for k in 0..l.count {
                uv.push(if bytes {
                    let b = slice(c, p + 2 * k, 2)?;
                    [b[0] as i32, b[1] as i32]
                } else {
                    [
                        u16_at(c, p + 4 * k)? as i32,
                        u16_at(c, p + 4 * k + 2)? as i32,
                    ]
                });
            }
        }
        Ok(FaceRecord {
            offset: self.inventory.code_start + at,
            len: l.end - at,
            content: c[at + 1],
            layout: c[at + 2],
            color: u16_at(c, at + 3)? as u16,
            normal: l.normal.map(|p| read3(c, p, false)).transpose()?,
            centre: l.centre.map(|(p, b)| read3(c, p, b)).transpose()?,
            slots,
            uv,
            frame,
        })
    }
    /// Index of the face whose record starts at this file offset.
    pub fn face_at(&self, offset: usize) -> Option<usize> {
        self.faces.binary_search_by_key(&offset, |f| f.offset).ok()
    }
    /// Buffer index and vertex index of a coordinate file offset.
    pub fn vertex_at(&self, offset: usize) -> Option<(usize, usize)> {
        let b = self
            .buffers
            .partition_point(|b| b.offset <= offset)
            .checked_sub(1)?;
        let buffer = &self.buffers[b];
        let rel = offset.checked_sub(buffer.offset + 6)?;
        (rel % 6 == 0 && rel / 6 < buffer.points.len()).then_some((b, rel / 6))
    }
    fn span(&self, face: usize) -> (usize, usize) {
        let a = self.faces[face].offset - self.inventory.code_start;
        (a, a + self.faces[face].len)
    }
    fn flow(&self, i: usize) -> Flow {
        let inv = &self.inventory;
        let r = &inv.records[i];
        let kind_at = |t: usize| inv.record_index(t).map(|q| inv.records[q].kind);
        let targets = |x86: bool| -> Vec<usize> {
            r.pointers
                .iter()
                .filter(|p| !matches!(p.kind, PointerKind::SelfOffset | PointerKind::Off16))
                .filter_map(|p| match p.target {
                    Target::Code(t) => Some(t),
                    _ => None,
                })
                // Import operands and x86 data tables are not control flow.
                .filter(|t| !x86 || !matches!(kind_at(*t), Some(Kind::Trampoline | Kind::X86Data)))
                // x86 branches land on instructions inside x86 records.
                .filter_map(|t| {
                    if x86 {
                        inv.record_index(t).map(|q| inv.records[q].offset)
                    } else {
                        Some(t)
                    }
                })
                .collect()
        };
        match r.kind {
            Kind::Pad if self.pads.contains(&i) => Flow::Go {
                next: alloc::vec![r.end()],
                call: None,
            },
            Kind::Pad | Kind::EndObject => Flow::Stop,
            Kind::EndShape | Kind::Trampoline | Kind::Opaque | Kind::X86Data => Flow::Unexplained,
            Kind::X86 { .. } => Flow::Go {
                next: targets(true),
                call: None,
            },
            Kind::Sh(0x48 | 0x40) => Flow::Go {
                next: targets(false),
                call: None,
            },
            Kind::Sh(0x12 | 0x6e | 0xc4 | 0xc6) => Flow::Go {
                next: alloc::vec![r.end()],
                call: targets(false).first().copied(),
            },
            Kind::Sh(_) => {
                let mut next = targets(false);
                next.push(r.end());
                Flow::Go { next, call: None }
            }
        }
    }
    /// Frame of every record: a walk from the shape start that does not enter
    /// calls, then one walk per reached call target. 12/6E calls keep the
    /// caller's frame; C4/C6 calls start their own. As in `Model`, a 1E that a
    /// preceding 38 scope of the same walk covers is padding; walks repeat
    /// until that padding set is stable.
    fn walk_frames(&mut self) -> Vec<Frame> {
        let n = self.inventory.records.len();
        let mut frames: Vec<Option<Frame>> = alloc::vec![None; n];
        for _ in 0..16 {
            let inv = &self.inventory;
            frames = alloc::vec![None; n];
            let mut found = BTreeSet::new();
            let mut starts = alloc::vec![(0usize, Frame::Root)];
            let mut started = BTreeSet::new();
            let mut k = 0;
            while k < starts.len() && k <= 4 * n {
                let (start, frame) = starts[k];
                k += 1;
                let mut seen = BTreeSet::new();
                let mut scopes = Vec::new();
                let mut stops = Vec::new();
                let mut work = alloc::vec![start];
                while let Some(at) = work.pop() {
                    let Some(i) = inv.starting_at(at) else {
                        continue;
                    };
                    if !seen.insert(i) {
                        continue;
                    }
                    frames[i] = match frames[i] {
                        None => Some(frame),
                        Some(f) if f == frame => Some(f),
                        Some(_) => Some(Frame::Mixed),
                    };
                    let r = &inv.records[i];
                    if r.kind == Kind::Sh(0x38) {
                        if let Ok(d) = word(&self.code, r.offset + 1) {
                            let end = r.end() as i64 + d as i64;
                            if end > r.offset as i64 {
                                scopes.push((r.offset, end as usize));
                            }
                        }
                    }
                    if r.kind == Kind::Pad && !self.pads.contains(&i) {
                        stops.push(i);
                    }
                    if let Flow::Go { next, call } = self.flow(i) {
                        work.extend(next);
                        if let Some(t) = call {
                            let f = if matches!(r.kind, Kind::Sh(0xc4 | 0xc6)) {
                                Frame::Part(inv.code_start + r.offset)
                            } else {
                                frame
                            };
                            if started.insert((t, f)) {
                                starts.push((t, f));
                            }
                        }
                    }
                }
                for i in stops {
                    let p = inv.records[i].offset;
                    if scopes.iter().any(|(q, e)| *q < p && p < *e) {
                        found.insert(i);
                    }
                }
            }
            if found.is_empty() {
                break;
            }
            self.pads.extend(found);
        }
        frames
            .into_iter()
            .map(|f| f.unwrap_or(Frame::Unreached))
            .collect()
    }
    /// Slots written by any 82 record reachable from a call target, through
    /// nested calls, until each path returns.
    fn reach_slots(&self, target: usize) -> core::result::Result<BTreeSet<usize>, String> {
        let inv = &self.inventory;
        let mut seen = BTreeSet::new();
        let mut work = alloc::vec![target];
        let mut slots = BTreeSet::new();
        while let Some(at) = work.pop() {
            let Some(i) = inv.starting_at(at) else {
                return Err(format!(
                    "a called path lands inside a record at CODE+{at:X}"
                ));
            };
            if !seen.insert(i) {
                continue;
            }
            let r = &inv.records[i];
            if r.kind == Kind::Sh(0x82) {
                let (s, n) = slots_of(&self.code, r.offset)?;
                slots.extend(s..s + n);
            }
            match self.flow(i) {
                Flow::Stop => {}
                Flow::Unexplained => {
                    return Err(format!(
                        "a called path reaches unexplained bytes at CODE+{:X}",
                        r.offset
                    ))
                }
                Flow::Go { next, call } => {
                    work.extend(next);
                    work.extend(call);
                }
            }
        }
        Ok(slots)
    }
    /// Walking back, a record lets the path continue from the one before it.
    /// Every 1E counts as falling through: whichever reading holds (return or
    /// padding), this only adds paths to check.
    fn continues(&self, i: usize) -> bool {
        let r = &self.inventory.records[i];
        r.kind == Kind::Pad
            || matches!(self.flow(i), Flow::Go { next, .. } if next.contains(&r.end()))
    }
    /// The 82 buffer whose vertex `slot` holds on every path reaching the
    /// record at CODE offset `x`. Walking back from `x`, no record may rewrite
    /// the slot (directly or in a called block) before the writer or a
    /// barrier, and every pointer entering that span from outside it must
    /// resolve to the same writer.
    fn resolve(&self, slot: usize, x: usize, depth: usize) -> Resolved {
        if let Some(known) = self.memo.borrow().get(&(slot, x)) {
            return known
                .clone()
                .unwrap_or_else(|| Err("its control flow loops".into()));
        }
        if depth > 32 {
            return Err("its control flow is too deep to prove".into());
        }
        self.memo.borrow_mut().insert((slot, x), None);
        let r = self.resolve_span(slot, x, depth);
        self.memo.borrow_mut().insert((slot, x), Some(r.clone()));
        r
    }
    fn resolve_span(&self, slot: usize, x: usize, depth: usize) -> Resolved {
        let inv = &self.inventory;
        let mut found = None;
        let mut start = 0;
        for j in (0..inv.records.partition_point(|r| r.offset < x)).rev() {
            let r = &inv.records[j];
            match r.kind {
                Kind::Sh(0x82) => {
                    let (s, n) = slots_of(&self.code, r.offset)?;
                    if (s..s + n).contains(&slot) {
                        found = self
                            .buffers
                            .binary_search_by_key(&(inv.code_start + r.offset), |b| b.offset)
                            .ok();
                        start = r.end();
                        break;
                    }
                }
                Kind::Sh(0x12 | 0x6e | 0xc4 | 0xc6) => {
                    let t = r.pointers.iter().find_map(|p| match p.target {
                        Target::Code(t) => Some(t),
                        _ => None,
                    });
                    match t.and_then(|t| self.called.get(&t)) {
                        Some(Ok(s)) if !s.contains(&slot) => {}
                        Some(Ok(_)) => {
                            return Err(format!(
                                "the block called at CODE+{:X} rewrites slot {slot}",
                                r.offset
                            ))
                        }
                        Some(Err(e)) => return Err(e.clone()),
                        None => {
                            return Err(format!("the call at CODE+{:X} is unresolved", r.offset))
                        }
                    }
                }
                Kind::Opaque | Kind::X86Data | Kind::EndShape | Kind::Trampoline => {
                    return Err(format!(
                        "unexplained bytes at CODE+{:X} precede it",
                        r.offset
                    ))
                }
                _ => {}
            }
            if !self.continues(j) {
                start = r.end();
                break;
            }
        }
        if found.is_none() && start == 0 {
            return Err(format!(
                "slot {slot} is not written on the path from the shape start"
            ));
        }
        let first = self.entries.partition_point(|(t, _)| *t < start);
        let mut sources = BTreeSet::new();
        for (_, f) in self.entries[first..].iter().take_while(|(t, _)| *t <= x) {
            if !(start..x).contains(f) {
                if let Some(i) = inv.record_index(*f) {
                    sources.insert(inv.records[i].offset);
                }
            }
        }
        for source in sources {
            let w = self
                .resolve(slot, source, depth + 1)
                .map_err(|e| format!("CODE+{source:X} enters before CODE+{x:X} and {e}"))?;
            match found {
                None => found = Some(w),
                Some(f) if f == w => {}
                Some(_) => {
                    return Err(format!(
                        "slot {slot} holds a different vertex when entered from CODE+{source:X}"
                    ))
                }
            }
        }
        found.ok_or_else(|| format!("nothing reaches CODE+{x:X}"))
    }
    /// The buffer whose vertex a face's slot shows, when that is provable.
    pub fn writer(&self, face: usize, slot: usize) -> Resolved {
        let f = self.faces.get(face).ok_or("No such face")?;
        self.resolve(slot, f.offset - self.inventory.code_start, 0)
    }
    /// Refusal when CODE bytes `[a, e)` may not be rewritten: a pointer or
    /// relocation field inside, a pointer target strictly inside, or a
    /// self-offset or x86 store addressing them.
    fn guard_bytes(&self, a: usize, e: usize, label: &str) -> core::result::Result<(), String> {
        let first = self.fields.partition_point(|(f, _)| *f + 4 <= a);
        if let Some((f, _)) = self.fields[first..]
            .iter()
            .take_while(|(f, _)| *f < e)
            .find(|(f, w)| *f + *w > a)
        {
            return Err(format!(
                "{label} contains a pointer or relocation field at CODE+{f:X}"
            ));
        }
        let first = self.targets.partition_point(|(t, _)| *t <= a);
        if let Some((t, field)) = self.targets[first..].iter().find(|(t, _)| *t < e) {
            return Err(format!(
                "CODE+{field:X} points into the middle of {label} at CODE+{t:X}"
            ));
        }
        let first = self.data.partition_point(|(t, w)| *t + *w <= a);
        if let Some((t, _)) = self.data[first..].iter().find(|(t, _)| *t < e) {
            return Err(format!("native code addresses {label} at CODE+{t:X}"));
        }
        Ok(())
    }
    /// Why a face may not be deleted, flipped, rewritten or used as a host.
    pub fn face_refusal(&self, face: usize) -> Option<String> {
        let f = self.faces.get(face)?;
        if f.frame == Frame::Unreached {
            return Some("This face is never drawn".into());
        }
        let (a, e) = self.span(face);
        self.guard_bytes(a, e, "this face").err()
    }
    /// Writability of every stored vertex.
    pub fn vertex_report(&self) -> Vec<VertexStatus> {
        let mut out = Vec::new();
        for b in 0..self.buffers.len() {
            for i in 0..self.buffers[b].points.len() {
                out.push(self.vertex_status(b, i));
            }
        }
        out
    }
    /// Writability of vertex `i` of buffer `b` and the faces it drives.
    pub fn vertex_status(&self, b: usize, i: usize) -> VertexStatus {
        let buffer = &self.buffers[b];
        let slot = buffer.slot + i;
        let mut status = VertexStatus {
            offset: buffer.vertex(i),
            slot,
            local: buffer.points[i],
            frame: buffer.frame,
            faces: Vec::new(),
            refusal: None,
        };
        let cs = self.inventory.code_start;
        if let Err(e) = self.guard_bytes(buffer.offset - cs, buffer.end() - cs, "its vertex buffer")
        {
            status.refusal = Some(e);
            return status;
        }
        for f in self.users.get(&slot).into_iter().flatten() {
            let face = &self.faces[*f];
            if face.frame == Frame::Unreached {
                continue;
            }
            match self.writer(*f, slot) {
                Ok(w) if w == b => {
                    if let Some(e) = self.face_refusal(*f).filter(|_| face.normal.is_some()) {
                        status.refusal = Some(format!(
                            "The face at {:X} using it cannot be updated: {e}",
                            face.offset
                        ));
                        return status;
                    }
                    status.faces.push(face.offset);
                }
                Ok(_) => {}
                Err(e) => {
                    status.refusal = Some(format!(
                        "The face at {:X} uses slot {slot}, but {e}",
                        face.offset
                    ));
                    return status;
                }
            }
        }
        status
    }
    /// Local corner points of a face, each slot resolved to its writer.
    fn face_points(&self, face: usize) -> Result<Vec<[i32; 3]>> {
        self.faces[face]
            .slots
            .iter()
            .map(|s| {
                let b = self.writer(face, *s)?;
                Ok(self.buffers[b].points[*s - self.buffers[b].slot])
            })
            .collect()
    }
    /// Copy an edited CODE payload (same length) into a shape.
    fn install(&self, source: &[u8], payload: &[u8]) -> Result<Vec<u8>> {
        let cs = self.inventory.code_start;
        if payload.len() != self.inventory.code_len {
            return Err(invalid("Edited CODE changed size"));
        }
        let mut out = source.to_vec();
        out.get_mut(cs..cs + payload.len())
            .ok_or("Shape truncated")?
            .copy_from_slice(payload);
        Ok(out)
    }
}

/// The edit kept CODE explained, the stub analysis and bindings identical,
/// and a shape the model reader accepted still parses.
fn verify_structure(before: &Geometry, source: &[u8], out: &[u8]) -> Result<Geometry> {
    let after = Geometry::parse(out)?;
    if after.inventory.opaque_bytes() != before.inventory.opaque_bytes() {
        return Err(invalid("The edit changed how much of CODE is explained"));
    }
    if after.inventory.bindings != before.inventory.bindings
        || after.inventory.stubs.len() != before.inventory.stubs.len()
    {
        return Err(invalid("The edit changed a part binding"));
    }
    if Model::parse(source).is_ok() && Model::parse(out).is_err() {
        return Err(invalid("The edited shape no longer parses as a model"));
    }
    Ok(after)
}
/// Same decoded face at the same CODE offset, ignoring the file position.
fn same_face(a: &Geometry, i: usize, b: &Geometry, j: usize) -> bool {
    let (x, y) = (&a.faces[i], &b.faces[j]);
    x.offset - a.inventory.code_start == y.offset - b.inventory.code_start
        && x.len == y.len
        && x.content == y.content
        && x.layout == y.layout
        && x.color == y.color
        && x.normal == y.normal
        && x.centre == y.centre
        && x.slots == y.slots
        && x.uv == y.uv
}
/// Faces by file offset, distinct, in CODE order, each passing `face_refusal`.
fn pick_faces(g: &Geometry, faces: &[usize]) -> Result<Vec<usize>> {
    let mut out = BTreeSet::new();
    for f in faces {
        let i = g
            .face_at(*f)
            .ok_or_else(|| format!("No FC face record starts at {f:X}"))?;
        if let Some(e) = g.face_refusal(i) {
            return Err(format!("Face at {f:X}: {e}"));
        }
        out.insert(i);
    }
    Ok(out.into_iter().collect())
}
/// Check that every face outside `changed` (CODE offsets) is unchanged.
fn others_unchanged(before: &Geometry, after: &Geometry, changed: &BTreeSet<usize>) -> Result<()> {
    for i in 0..before.faces.len() {
        let at = before.faces[i].offset - before.inventory.code_start;
        if changed.contains(&at) {
            continue;
        }
        let j = after
            .face_at(after.inventory.code_start + at)
            .ok_or("The edit lost another face")?;
        if !same_face(before, i, after, j) {
            return Err(invalid("The edit changed another face"));
        }
    }
    Ok(())
}
/// Replace each face record in place with a same-size jump stub (`48` to its
/// end, `1E` fill), so every path that drew it continues after it. Pointers
/// to the record start still land on a valid record.
pub fn delete_faces(source: &[u8], faces: &[usize]) -> Result<Vec<u8>> {
    if faces.is_empty() {
        return Ok(source.to_vec());
    }
    let g = Geometry::parse(source)?;
    let picked = pick_faces(&g, faces)?;
    let mut payload = g.code.clone();
    let mut changed = BTreeSet::new();
    for &i in &picked {
        let (a, e) = g.span(i);
        stub_out(&mut payload, a, e, e)?;
        changed.insert(a);
    }
    let out = g.install(source, &payload)?;
    let after = verify_structure(&g, source, &out)?;
    if after.faces.len() + picked.len() != g.faces.len() {
        return Err(invalid(
            "Face deletion did not remove exactly the selection",
        ));
    }
    others_unchanged(&g, &after, &changed)?;
    Ok(out)
}
fn flip_bytes(c: &[u8], at: usize) -> Result<Vec<u8>> {
    let l = face_layout(c, at)?;
    let mut b = slice(c, at, l.end - at)?.to_vec();
    if let Some(n) = l.normal {
        for k in 0..3 {
            let v = -word(c, n + 2 * k)?;
            let o = n - at + 2 * k;
            b[o..o + 2].copy_from_slice(&(v.clamp(-32767, 32767) as i16).to_le_bytes());
        }
    }
    let w = if l.wide { 2 } else { 1 };
    let i = l.index - at;
    let mut chunks: Vec<Vec<u8>> = b[i..i + l.count * w]
        .chunks(w)
        .map(|x| x.to_vec())
        .collect();
    chunks.reverse();
    b[i..i + l.count * w].copy_from_slice(&chunks.concat());
    if let Some((p, bytes)) = l.uv {
        let w = if bytes { 2 } else { 4 };
        let p = p - at;
        let mut chunks: Vec<Vec<u8>> = b[p..p + l.count * w]
            .chunks(w)
            .map(|x| x.to_vec())
            .collect();
        chunks.reverse();
        b[p..p + l.count * w].copy_from_slice(&chunks.concat());
    }
    Ok(b)
}
/// Reverse each face's corner order and UVs and negate its stored normal, in
/// place. The centre is unchanged.
pub fn flip_faces(source: &[u8], faces: &[usize]) -> Result<Vec<u8>> {
    if faces.is_empty() {
        return Ok(source.to_vec());
    }
    let g = Geometry::parse(source)?;
    let picked = pick_faces(&g, faces)?;
    let mut payload = g.code.clone();
    let mut changed = BTreeSet::new();
    for &i in &picked {
        let (a, e) = g.span(i);
        payload[a..e].copy_from_slice(&flip_bytes(&g.code, a)?);
        changed.insert(a);
    }
    let out = g.install(source, &payload)?;
    let after = verify_structure(&g, source, &out)?;
    others_unchanged(&g, &after, &changed)?;
    for &i in &picked {
        let f = &g.faces[i];
        let j = after.face_at(f.offset).ok_or("Flipped face lost")?;
        let n = &after.faces[j];
        let mut slots = f.slots.clone();
        slots.reverse();
        let mut uv = f.uv.clone();
        uv.reverse();
        if n.slots != slots || n.uv != uv || n.centre != f.centre {
            return Err(invalid("Flip did not reverse the face"));
        }
    }
    Ok(out)
}
/// Write stored (local) vertex coordinates, given by coordinate file offset
/// (`model::Vertex::offset`). Each vertex must be writable (`vertex_status`);
/// the normals of the faces it drives are recomputed retail-style and their
/// centres move by the change of the corner average, keeping whatever offset
/// the original tool stored. A face whose stored normal opposed the retail
/// winding keeps that orientation.
pub fn write_vertices(source: &[u8], moves: &[(usize, [i32; 3])]) -> Result<Vec<u8>> {
    let g = Geometry::parse(source)?;
    let mut targets = BTreeMap::new();
    for (offset, p) in moves {
        let at = g
            .vertex_at(*offset)
            .ok_or_else(|| format!("No stored vertex at {offset:X}"))?;
        if !in_word(p) {
            return Err(invalid("Vertex exceeds signed 16-bit source coordinates"));
        }
        if targets.insert(at, *p).is_some() {
            return Err(invalid("A vertex is listed twice"));
        }
    }
    let changed: Vec<_> = targets
        .iter()
        .filter(|((b, i), p)| g.buffers[*b].points[*i] != **p)
        .map(|(k, p)| (*k, *p))
        .collect();
    if changed.is_empty() {
        return Ok(source.to_vec());
    }
    let mut faces = BTreeSet::new();
    for ((b, i), _) in &changed {
        let status = g.vertex_status(*b, *i);
        if let Some(e) = status.refusal {
            return Err(format!("Vertex at {:X}: {e}", status.offset));
        }
        faces.extend(status.faces.iter().filter_map(|f| g.face_at(*f)));
    }
    let mut moved = g.clone();
    let cs = g.inventory.code_start;
    let mut payload = g.code.clone();
    for ((b, i), p) in &changed {
        moved.buffers[*b].points[*i] = *p;
        let at = g.buffers[*b].vertex(*i) - cs;
        for (k, v) in p.iter().enumerate() {
            payload[at + 2 * k..at + 2 * k + 2].copy_from_slice(&(*v as i16).to_le_bytes());
        }
    }
    let mut rewritten = BTreeSet::new();
    for f in faces {
        let face = &g.faces[f];
        let Some(stored) = face.normal else { continue };
        let old = g.face_points(f)?;
        let new = moved.face_points(f)?;
        let l = face_layout(&g.code, face.offset - cs)?;
        let mut normal = face_normal(&new).unwrap_or(stored);
        if let Some(was) = face_normal(&old) {
            if (0..3)
                .map(|k| was[k] as i64 * stored[k] as i64)
                .sum::<i64>()
                < 0
            {
                normal = normal.map(|v| -v);
            }
        }
        if let Some(n) = l.normal {
            put3(&mut payload, n, false, normal)?;
        }
        if let (Some((at, byte)), Some(c)) = (l.centre, face.centre) {
            let (a, b) = (average(&old), average(&new));
            put3(
                &mut payload,
                at,
                byte,
                core::array::from_fn(|k| c[k] + b[k] - a[k]),
            )?;
        }
        rewritten.insert(face.offset - cs);
    }
    let out = g.install(source, &payload)?;
    let after = verify_structure(&g, source, &out)?;
    for ((b, i), p) in &changed {
        if after.buffers.get(*b).map(|x| x.points[*i]) != Some(*p) {
            return Err(invalid("Vertex write did not land"));
        }
    }
    others_unchanged(&g, &after, &rewritten)?;
    Ok(out)
}
/// Scale the corners of the given faces about their common local centroid by
/// `percent` (100 = unchanged). The faces must share one frame.
pub fn scale_faces(source: &[u8], faces: &[usize], percent: i32) -> Result<Vec<u8>> {
    if !(1..=10000).contains(&percent) {
        return Err(invalid("Scale must be 1..10000 percent"));
    }
    let g = Geometry::parse(source)?;
    let picked = pick_faces(&g, faces)?;
    let mut corners = BTreeMap::new();
    let mut frame = None;
    for &f in &picked {
        for s in &g.faces[f].slots {
            let b = g.writer(f, *s)?;
            corners.insert(
                g.buffers[b].vertex(*s - g.buffers[b].slot),
                g.buffers[b].points[*s - g.buffers[b].slot],
            );
            if *frame.get_or_insert(g.buffers[b].frame) != g.buffers[b].frame {
                return Err(invalid("Scale faces of one part at a time"));
            }
        }
    }
    let points: Vec<_> = corners.values().copied().collect();
    let c = average(&points);
    let moves: Vec<_> = corners
        .iter()
        .map(|(o, p)| {
            (
                *o,
                core::array::from_fn(|k| {
                    c[k] + ((p[k] - c[k]) as i64 * percent as i64 / 100) as i32
                }),
            )
        })
        .collect();
    write_vertices(source, &moves)
}

/// Appearance of a new face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceStyle {
    /// FC content flags: 0x40 stores a normal and centre, 0x04 carries UVs.
    /// Per-vertex shading (0x80) is refused: new slots have no F6 records.
    pub content: u8,
    pub color: u8,
    /// Texture to select for the face; `None` draws in the host's state.
    pub texture: Option<String>,
    /// One UV per corner (PIC pixels) when the face is textured.
    pub uv: Vec<[i32; 2]>,
}
impl FaceStyle {
    /// Lit flat face, content 0x63: the most common retail body face.
    pub fn flat(color: u8) -> Self {
        Self {
            content: 0x63,
            color,
            texture: None,
            uv: Vec::new(),
        }
    }
    /// An existing face's appearance. Per-vertex shaded content maps to flat
    /// 0x63 or textured 0x6C, because new corners carry no F6 records.
    pub fn like(face: &FaceRecord) -> Self {
        let content = match face.content {
            c if c & 0x80 != 0 && c & 4 != 0 => 0x6c,
            c if c & 0x80 != 0 => 0x63,
            c => c,
        };
        Self {
            content,
            color: face.color as u8,
            texture: None,
            uv: if content & 4 != 0 {
                face.uv.clone()
            } else {
                Vec::new()
            },
        }
    }
}
/// A corner of a new face: an existing stored vertex (coordinate file
/// offset) or an index into `Addition::points`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    Vertex(usize),
    New(usize),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewFace {
    pub corners: Vec<Corner>,
    pub style: FaceStyle,
}
/// What happens to the host face (or an extruded original).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Keep,
    Flip,
    Remove,
}
/// New geometry drawn where the host face is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Addition {
    /// Host face file offset; `None` picks a face of the corners' frame at
    /// which every existing corner is provably current.
    pub host: Option<usize>,
    /// New vertices, local to the host frame.
    pub points: Vec<[i32; 3]>,
    pub faces: Vec<NewFace>,
    pub host_copy: Base,
    /// Other faces to flip or delete in place in the same edit.
    pub flip: Vec<usize>,
    pub delete: Vec<usize>,
}
/// Result of an appended edit; offsets refer to the new shape.
#[derive(Clone, Debug)]
pub struct Added {
    pub shape: Vec<u8>,
    /// File offset of the host face's copy, if it was kept.
    pub host: Option<usize>,
    /// File offsets of the new faces, in `Addition::faces` order.
    pub faces: Vec<usize>,
    /// Coordinate file offsets of the new vertices, in `points` order.
    pub vertices: Vec<usize>,
    pub slots: Vec<usize>,
}
fn texture_key(name: &str) -> String {
    let up = name.to_ascii_uppercase();
    if up.contains('.') {
        up
    } else {
        format!("{up}.PIC")
    }
}
/// Pose satisfying one stub condition.
fn satisfying(c: &Condition) -> i32 {
    match c.rel {
        Rel::Eq | Rel::Ge | Rel::Le | Rel::AboveEq | Rel::BelowEq => c.value,
        Rel::Ne | Rel::Gt | Rel::Above => c.value.wrapping_add(1),
        Rel::Lt | Rel::Below => c.value.wrapping_sub(1),
    }
}
/// A model pose that draws the face at this file offset, if one is found
/// among the neutral pose and each binding's condition sets.
pub fn model_drawing(source: &[u8], inventory: &Inventory, face: usize) -> Option<Model> {
    let has = |m: &Model| m.faces.iter().any(|f| f.offset == face);
    if let Ok(m) = Model::parse(source) {
        if has(&m) {
            return Some(m);
        }
    }
    let mut all = Pose::new();
    for b in inventory.bindings.iter().take(64) {
        for set in b.when.iter().take(8) {
            let mut pose = Pose::new();
            for c in set {
                pose.insert(c.variable.clone(), satisfying(c));
            }
            for (k, v) in &pose {
                all.entry(k.clone()).or_insert(*v);
            }
            if let Ok(m) = Model::with_pose(source, &pose) {
                if has(&m) {
                    return Some(m);
                }
            }
        }
    }
    Model::with_pose(source, &all).ok().filter(has)
}
/// Texture name and selector record active at a face, when a pose draws it.
fn material_at(source: &[u8], g: &Geometry, face: usize) -> Option<(String, Vec<u8>)> {
    let m = model_drawing(source, &g.inventory, face)?;
    let f = m.faces.iter().find(|f| f.offset == face)?;
    Some((f.texture.clone(), f.material_selector.clone()))
}
fn encode_face(style: &FaceStyle, slots: &[usize], points: &[[i32; 3]]) -> Result<Vec<u8>> {
    let n = slots.len();
    if !(3..=64).contains(&n) {
        return Err(invalid("A face needs 3 to 64 corners"));
    }
    if style.content & 0x80 != 0 {
        return Err(invalid(
            "Per-vertex shaded faces need F6 vertex records; choose a flat or textured style",
        ));
    }
    if (1..n).any(|i| slots[..i].contains(&slots[i])) {
        return Err(invalid("A face cannot use a vertex twice"));
    }
    let textured = style.content & 4 != 0;
    if (textured && style.uv.len() != n) || (!textured && !style.uv.is_empty()) {
        return Err(invalid(
            "Textured faces need one UV per corner; untextured faces take none",
        ));
    }
    if style.uv.iter().flatten().any(|v| !(0..=65535).contains(v)) {
        return Err(invalid("UV outside 0..65535"));
    }
    if slots.iter().any(|s| *s >= 8192) {
        return Err(invalid("Vertex slot beyond 8191"));
    }
    let wide = slots.iter().any(|s| *s > 255);
    let lit = style.content & 0x40 != 0;
    let normal = if lit {
        Some(face_normal(points).ok_or("Degenerate face: its corners are collinear")?)
    } else {
        None
    };
    let centre = average(points);
    let byte_centre = lit && centre.iter().all(|v| (-128..=127).contains(v));
    let byte_uv = textured && style.uv.iter().flatten().all(|v| *v <= 255);
    let layout = (wide as u8) << 2 | (byte_centre as u8) << 1 | byte_uv as u8;
    let mut b = alloc::vec![0xfc, style.content, layout, style.color, 0];
    if let Some(normal) = normal {
        let mut f = [0u8; 12];
        put3(&mut f, 0, false, normal)?;
        b.extend(&f[..6]);
        let w = if byte_centre { 3 } else { 6 };
        put3(&mut f, 6, byte_centre, centre)?;
        b.extend(&f[6..6 + w]);
    }
    b.push(n as u8);
    for s in slots {
        if wide {
            b.extend((*s as u16).to_le_bytes());
        } else {
            b.push(*s as u8);
        }
    }
    for uv in &style.uv {
        for v in uv {
            if byte_uv {
                b.push(*v as u8);
            } else {
                b.extend((*v as u16).to_le_bytes());
            }
        }
    }
    Ok(b)
}
/// Pick a host face in `frame` where every `(buffer, vertex)` corner is current.
fn pick_host(g: &Geometry, corners: &[(usize, usize)], frame: Frame) -> Result<usize> {
    let cs = g.inventory.code_start;
    let mut wanted = BTreeSet::new();
    for (b, i) in corners {
        wanted.insert(g.buffers[*b].slot + i);
    }
    let mut candidates: Vec<(usize, usize)> = (0..g.faces.len())
        .filter(|f| g.faces[*f].frame == frame && g.face_refusal(*f).is_none())
        .map(|f| {
            (
                g.faces[f]
                    .slots
                    .iter()
                    .filter(|s| wanted.contains(s))
                    .count(),
                f,
            )
        })
        .collect();
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    let mut last = String::from("no face of this frame can host it");
    for (_, f) in candidates.into_iter().take(512) {
        let h = g.faces[f].offset - cs;
        match corners
            .iter()
            .try_for_each(|(b, i)| match g.resolve(g.buffers[*b].slot + i, h, 0) {
                Ok(w) if w == *b => Ok(()),
                Ok(_) => Err(format!(
                    "slot {} shows another vertex there",
                    g.buffers[*b].slot + i
                )),
                Err(e) => Err(e),
            }) {
            Ok(()) => return Ok(f),
            Err(e) => last = e,
        }
    }
    Err(format!(
        "No existing face can host the new geometry: {last}"
    ))
}
/// Append new vertices and faces in a continuation that the host face's
/// record detours to: `[host copy] [82 new vertices] [faces] [texture
/// switches and restore] [48 back]`. Existing corners must be current at the
/// host and in its frame; new vertices take free slots below `SLOT_CEILING`
/// that no face references, so nothing drawn later reads them.
pub fn append_geometry(source: &[u8], add: &Addition) -> Result<Added> {
    if add.faces.is_empty() && add.points.is_empty() && add.flip.is_empty() && add.delete.is_empty()
    {
        return Ok(Added {
            shape: source.to_vec(),
            host: None,
            faces: Vec::new(),
            vertices: Vec::new(),
            slots: Vec::new(),
        });
    }
    if add.points.len() > 1024 || add.faces.len() > 4096 {
        return Err(invalid("Too much new geometry for one edit"));
    }
    if add.points.iter().any(|p| !in_word(p)) {
        return Err(invalid("Vertex exceeds signed 16-bit source coordinates"));
    }
    let g = Geometry::parse(source)?;
    if g.inventory.end_marker.is_none() {
        // Without the marker, the trailing end object would absorb the
        // continuation and no whole-CODE reader would see the new records.
        return Err(invalid(
            "Appending geometry needs the native end marker and import tail",
        ));
    }
    let cs = g.inventory.code_start;
    let mut existing = BTreeSet::new();
    for face in &add.faces {
        for c in &face.corners {
            match c {
                Corner::Vertex(o) => {
                    existing.insert(
                        g.vertex_at(*o)
                            .ok_or_else(|| format!("No stored vertex at {o:X}"))?,
                    );
                }
                Corner::New(k) if *k < add.points.len() => {}
                Corner::New(_) => return Err(invalid("New corner index outside the new vertices")),
            }
        }
    }
    let existing: Vec<(usize, usize)> = existing.into_iter().collect();
    let host = match add.host {
        Some(h) => g
            .face_at(h)
            .ok_or_else(|| format!("No FC face record starts at {h:X}"))?,
        None => {
            let frame = existing
                .first()
                .map(|(b, _)| g.buffers[*b].frame)
                .ok_or("Name a host face for geometry without existing corners")?;
            pick_host(&g, &existing, frame)?
        }
    };
    if let Some(e) = g.face_refusal(host) {
        return Err(format!("Host face: {e}"));
    }
    let frame = g.faces[host].frame;
    if frame == Frame::Mixed {
        return Err(invalid(
            "The host face is drawn under several frames; its local coordinates are ambiguous",
        ));
    }
    let (a, e) = g.span(host);
    for (b, i) in &existing {
        let slot = g.buffers[*b].slot + i;
        if g.buffers[*b].frame != frame {
            return Err(format!(
                "Vertex at {:X} belongs to another part than the host face",
                g.buffers[*b].vertex(*i)
            ));
        }
        match g.resolve(slot, a, 0) {
            Ok(w) if w == *b => {}
            Ok(_) => {
                return Err(format!(
                "Vertex at {:X} is not current at the host face: slot {slot} shows another vertex",
                g.buffers[*b].vertex(*i)
            ))
            }
            Err(err) => {
                return Err(format!(
                    "Vertex at {:X} is not provably current at the host face: {err}",
                    g.buffers[*b].vertex(*i)
                ))
            }
        }
    }
    let n = add.points.len();
    let first = if n == 0 {
        0
    } else {
        let s = g
            .inventory
            .free_slots(n)
            .filter(|s| s + n <= SLOT_CEILING)
            .ok_or_else(|| {
                format!("No {n} free vertex slots below the retail ceiling of {SLOT_CEILING}")
            })?;
        if (s..s + n).any(|x| g.users.contains_key(&x)) {
            return Err(invalid("Free slots are referenced by an existing face"));
        }
        s
    };
    let new_slots: Vec<usize> = (first..first + n).collect();
    let corner = |c: &Corner| -> (usize, [i32; 3]) {
        match c {
            Corner::Vertex(o) => {
                let (b, i) = g.vertex_at(*o).unwrap_or((0, 0));
                (g.buffers[b].slot + i, g.buffers[b].points[i])
            }
            Corner::New(k) => (first + k, add.points[*k]),
        }
    };
    // Material state at the host.
    let needs_state = add.faces.iter().any(|f| f.style.content & 4 != 0);
    let state = if needs_state {
        material_at(source, &g, g.faces[host].offset)
    } else {
        None
    };
    let ext_start = continuation_start(source)?;
    let mut ext = Vec::new();
    let host_copy = match add.host_copy {
        Base::Keep => {
            ext.extend(&g.code[a..e]);
            Some(0)
        }
        Base::Flip => {
            ext.extend(flip_bytes(&g.code, a)?);
            Some(0)
        }
        Base::Remove => None,
    };
    let vertex_at = ext.len();
    if n > 0 {
        ext.extend([0x82, 0]);
        ext.extend((n as u16).to_le_bytes());
        ext.extend(((first * 8) as u16).to_le_bytes());
        for p in &add.points {
            for v in p {
                ext.extend((*v as i16).to_le_bytes());
            }
        }
    }
    let mut positions = alloc::vec![0usize; add.faces.len()];
    let mut switched: Vec<(String, Vec<usize>)> = Vec::new();
    for (k, face) in add.faces.iter().enumerate() {
        let (slots, points): (Vec<usize>, Vec<[i32; 3]>) = face.corners.iter().map(corner).unzip();
        let bytes = encode_face(&face.style, &slots, &points)?;
        let textured = face.style.content & 4 != 0;
        let switch = match (&face.style.texture, &state) {
            _ if !textured => None,
            (Some(t), Some((current, _))) if texture_key(t) == texture_key(current) => None,
            (Some(t), _) => Some(texture_key(t)),
            (None, Some((current, _))) if current.is_empty() => {
                return Err(invalid(
                    "No texture is active at the host face; name one for the textured face",
                ))
            }
            (None, None) if g.faces[host].content & 4 == 0 => {
                return Err(invalid(
                    "The texture state at the host face is unresolved; name a texture",
                ))
            }
            (None, _) => None,
        };
        match switch {
            None => {
                positions[k] = ext.len();
                ext.extend(bytes);
            }
            Some(name) => {
                let list = match switched.iter_mut().find(|(t, _)| *t == name) {
                    Some((_, list)) => list,
                    None => {
                        switched.push((name, Vec::new()));
                        &mut switched.last_mut().ok_or("Texture list")?.1
                    }
                };
                list.push(k);
            }
        }
    }
    if !switched.is_empty() {
        let restore = state
            .as_ref()
            .map(|(_, s)| s.clone())
            .filter(|s| !s.is_empty())
            .ok_or("The material at the host face is unresolved, so a texture switch cannot be restored")?;
        for (name, list) in &switched {
            crate::archive::validate_name(name)?;
            if name.len() > 13 {
                return Err(invalid("Texture names are at most 13 characters"));
            }
            ext.extend([0xe2, 0]);
            let mut field = [0u8; 14];
            field[..name.len()].copy_from_slice(name.as_bytes());
            ext.extend(field);
            for k in list {
                let face = &add.faces[*k];
                let (slots, points): (Vec<usize>, Vec<[i32; 3]>) =
                    face.corners.iter().map(corner).unzip();
                positions[*k] = ext.len();
                ext.extend(encode_face(&face.style, &slots, &points)?);
            }
        }
        ext.extend(restore);
    }
    ext.extend(jump(ext_start + ext.len(), e)?);
    let mut payload = g.code.clone();
    let mut changed = BTreeSet::new();
    changed.insert(a);
    for f in pick_faces(&g, &add.flip)? {
        if f == host {
            return Err(invalid("Flip the host through its copy instead"));
        }
        let (fa, fe) = g.span(f);
        payload[fa..fe].copy_from_slice(&flip_bytes(&g.code, fa)?);
        changed.insert(fa);
    }
    for f in pick_faces(&g, &add.delete)? {
        if f == host || add.flip.contains(&g.faces[f].offset) {
            return Err(invalid("A face cannot be deleted and edited in one step"));
        }
        let (fa, fe) = g.span(f);
        stub_out(&mut payload, fa, fe, fe)?;
        changed.insert(fa);
    }
    stub_out(&mut payload, a, e, ext_start)?;
    let out = append_continuation(source, payload, ext)?;
    // Verify the result from scratch.
    let after = verify_structure(&g, source, &out)?;
    let cs2 = after.inventory.code_start;
    let buffer_code = |x: &Geometry, b: usize| x.buffers[b].offset - x.inventory.code_start;
    let expect = |c: &Corner| -> usize {
        match c {
            Corner::Vertex(o) => g
                .vertex_at(*o)
                .map_or(usize::MAX, |(b, _)| buffer_code(&g, b)),
            Corner::New(_) => ext_start + vertex_at,
        }
    };
    let mut faces = Vec::new();
    for (k, face) in add.faces.iter().enumerate() {
        let off = cs2 + ext_start + positions[k];
        let j = after.face_at(off).ok_or("A new face did not decode")?;
        let f = &after.faces[j];
        if f.frame != frame && !matches!((f.frame, frame), (Frame::Part(_), Frame::Part(_))) {
            return Err(invalid("A new face is drawn in another frame"));
        }
        for (s, c) in f.slots.iter().zip(&face.corners) {
            let w = after
                .writer(j, *s)
                .map_err(|e| format!("A new face's slot {s} is not provably current: {e}"))?;
            if buffer_code(&after, w) != expect(c) {
                return Err(invalid("A new face shows the wrong vertex"));
            }
        }
        faces.push(off);
    }
    let host_out = host_copy.map(|p| cs2 + ext_start + p);
    if let Some(off) = host_out {
        let j = after.face_at(off).ok_or("The host copy did not decode")?;
        for s in &after.faces[j].slots {
            let before = g.writer(host, *s).map(|b| buffer_code(&g, b));
            let now = after.writer(j, *s).map(|b| buffer_code(&after, b));
            if before.is_err() || before != now {
                return Err(invalid("The host copy shows different vertices"));
            }
        }
    }
    for s in &new_slots {
        let users = after.users.get(s).map_or(0, |u| u.len());
        let ours = faces
            .iter()
            .filter(|o| {
                after
                    .face_at(**o)
                    .is_some_and(|j| after.faces[j].slots.contains(s))
            })
            .count();
        if users != ours {
            return Err(invalid("A new slot is referenced by another face"));
        }
    }
    for i in 0..g.faces.len() {
        let at = g.faces[i].offset - cs;
        if changed.contains(&at) {
            continue;
        }
        let j = after
            .face_at(cs2 + at)
            .ok_or("The edit lost another face")?;
        if !same_face(&g, i, &after, j) {
            return Err(invalid("The edit changed another face"));
        }
    }
    let vertices = (0..n)
        .map(|k| cs2 + ext_start + vertex_at + 6 + 6 * k)
        .collect();
    Ok(Added {
        shape: out,
        host: host_out,
        faces,
        vertices,
        slots: new_slots,
    })
}
/// Add one face over existing stored vertices (coordinate file offsets), in
/// the order given. The host is picked automatically.
pub fn add_face(source: &[u8], vertices: &[usize], style: &FaceStyle) -> Result<Added> {
    append_geometry(
        source,
        &Addition {
            host: None,
            points: Vec::new(),
            faces: alloc::vec![NewFace {
                corners: vertices.iter().map(|v| Corner::Vertex(*v)).collect(),
                style: style.clone(),
            }],
            host_copy: Base::Keep,
            flip: Vec::new(),
            delete: Vec::new(),
        },
    )
}
/// Add vertices (local to the host face's frame) and faces over them and
/// existing vertices, drawn where the host face is drawn.
pub fn add_vertices(
    source: &[u8],
    host: usize,
    points: &[[i32; 3]],
    faces: &[NewFace],
) -> Result<Added> {
    append_geometry(
        source,
        &Addition {
            host: Some(host),
            points: points.to_vec(),
            faces: faces.to_vec(),
            host_copy: Base::Keep,
            flip: Vec::new(),
            delete: Vec::new(),
        },
    )
}
/// Resolved corners of each selected face, and their common frame.
type Selection = (Vec<Vec<(usize, usize)>>, Frame);
fn selection(g: &Geometry, picked: &[usize]) -> Result<Selection> {
    let mut frame = None;
    let mut out = Vec::new();
    for &f in picked {
        let mut corners = Vec::new();
        for s in &g.faces[f].slots {
            let b = g.writer(f, *s)?;
            corners.push((b, *s - g.buffers[b].slot));
        }
        let fr = g.faces[f].frame;
        if fr == Frame::Mixed || *frame.get_or_insert(fr) != fr {
            return Err(invalid("Edit faces of one part at a time"));
        }
        out.push(corners);
    }
    Ok((out, frame.ok_or("Select at least one face")?))
}
/// Style for a copy of `face` drawn at `host`: the same texture when both
/// are drawn under the same material state.
fn copy_style(source: &[u8], g: &Geometry, face: usize, host: usize) -> Result<FaceStyle> {
    let mut style = FaceStyle::like(&g.faces[face]);
    if style.content & 4 != 0 && face != host {
        let a = material_at(source, g, g.faces[face].offset);
        let b = material_at(source, g, g.faces[host].offset);
        match (a, b) {
            (Some(a), Some(b)) if a == b => {}
            (Some((t, _)), Some(_)) if !t.is_empty() => style.texture = Some(t),
            _ => {
                return Err(invalid(
                    "A selected textured face has an unresolved material state",
                ))
            }
        }
    }
    Ok(style)
}
/// Copy faces onto new vertices offset by `offset` (local units), drawn
/// after the last selected face. The originals are unchanged.
pub fn duplicate_faces(source: &[u8], faces: &[usize], offset: [i32; 3]) -> Result<Added> {
    let g = Geometry::parse(source)?;
    let picked = pick_faces(&g, faces)?;
    let (corners, _) = selection(&g, &picked)?;
    let host = *picked.last().ok_or("Select at least one face")?;
    let mut index = BTreeMap::new();
    let mut points = Vec::new();
    let mut new_faces = Vec::new();
    for (k, f) in picked.iter().enumerate() {
        let mut list = Vec::new();
        for (b, i) in &corners[k] {
            let next = index.len();
            let at = *index.entry((*b, *i)).or_insert(next);
            if at == points.len() {
                let p = g.buffers[*b].points[*i];
                points.push(core::array::from_fn(|a| p[a] + offset[a]));
            }
            list.push(Corner::New(at));
        }
        new_faces.push(NewFace {
            corners: list,
            style: copy_style(source, &g, *f, host)?,
        });
    }
    append_geometry(
        source,
        &Addition {
            host: Some(g.faces[host].offset),
            points,
            faces: new_faces,
            host_copy: Base::Keep,
            flip: Vec::new(),
            delete: Vec::new(),
        },
    )
}
/// Extrude faces by `offset` (local units): new vertices for every corner, a
/// moved copy of each face, and a side quad for each boundary edge of the
/// selection, wound outward. `base` keeps, flips or removes the originals.
pub fn extrude_faces(
    source: &[u8],
    faces: &[usize],
    offset: [i32; 3],
    base: Base,
) -> Result<Added> {
    if offset == [0; 3] {
        return Err(invalid("Extrude needs a non-zero offset"));
    }
    let g = Geometry::parse(source)?;
    let picked = pick_faces(&g, faces)?;
    let (corners, _) = selection(&g, &picked)?;
    let mut edges = BTreeMap::<((usize, usize), (usize, usize)), usize>::new();
    for list in &corners {
        for k in 0..list.len() {
            let (p, q) = (list[k], list[(k + 1) % list.len()]);
            *edges
                .entry(if p < q { (p, q) } else { (q, p) })
                .or_default() += 1;
        }
    }
    let mut index = BTreeMap::new();
    let mut points = Vec::new();
    let mut new_of = |c: (usize, usize), points: &mut Vec<[i32; 3]>| -> usize {
        let next = index.len();
        let at = *index.entry(c).or_insert(next);
        if at == points.len() {
            let p = g.buffers[c.0].points[c.1];
            points.push(core::array::from_fn(|a| p[a] + offset[a]));
        }
        at
    };
    let mut boundary = BTreeSet::new();
    let mut plans = Vec::new();
    for (k, list) in corners.iter().enumerate() {
        let f = picked[k];
        let pts: Vec<[i32; 3]> = list.iter().map(|(b, i)| g.buffers[*b].points[*i]).collect();
        let outward = face_normal(&pts)
            .map(|n| (0..3).map(|a| n[a] as i64 * offset[a] as i64).sum::<i64>() >= 0)
            .unwrap_or(true);
        let cap: Vec<Corner> = list
            .iter()
            .map(|c| Corner::New(new_of(*c, &mut points)))
            .collect();
        let mut sides = Vec::new();
        for j in 0..list.len() {
            let (p, q) = (list[j], list[(j + 1) % list.len()]);
            if edges[&if p < q { (p, q) } else { (q, p) }] != 1 {
                continue;
            }
            let (pp, qp) = (g.buffers[p.0].points[p.1], g.buffers[q.0].points[q.1]);
            let moved = |x: [i32; 3]| -> [i32; 3] { core::array::from_fn(|a| x[a] + offset[a]) };
            if face_normal(&[pp, qp, moved(qp), moved(pp)]).is_none() {
                // An edge parallel to the offset sweeps no area.
                continue;
            }
            boundary.insert(p);
            boundary.insert(q);
            let (pn, qn) = (new_of(p, &mut points), new_of(q, &mut points));
            let v = |c: (usize, usize)| Corner::Vertex(g.buffers[c.0].vertex(c.1));
            let quad = if outward {
                alloc::vec![v(p), v(q), Corner::New(qn), Corner::New(pn)]
            } else {
                alloc::vec![v(q), v(p), Corner::New(pn), Corner::New(qn)]
            };
            let uv = &g.faces[f].uv;
            let side_uv = if uv.len() == list.len() {
                let (a, b) = (uv[j], uv[(j + 1) % list.len()]);
                if outward {
                    alloc::vec![a, b, b, a]
                } else {
                    alloc::vec![b, a, a, b]
                }
            } else {
                Vec::new()
            };
            sides.push((quad, side_uv));
        }
        plans.push((f, cap, sides));
    }
    // Host: a selected face at which every boundary corner is current.
    let cs = g.inventory.code_start;
    let host = picked
        .iter()
        .rev()
        .copied()
        .find(|h| {
            let at = g.faces[*h].offset - cs;
            boundary
                .iter()
                .all(|(b, i)| g.resolve(g.buffers[*b].slot + i, at, 0) == Ok(*b))
        })
        .ok_or("No selected face sees every edge vertex; extrude a smaller selection")?;
    let mut new_faces = Vec::new();
    for (f, cap, sides) in plans {
        let style = copy_style(source, &g, f, host)?;
        new_faces.push(NewFace {
            corners: cap,
            style: style.clone(),
        });
        for (quad, uv) in sides {
            let mut s = style.clone();
            if s.content & 4 != 0 {
                s.uv = uv;
            }
            new_faces.push(NewFace {
                corners: quad,
                style: s,
            });
        }
    }
    let others: Vec<usize> = picked
        .iter()
        .filter(|f| **f != host)
        .map(|f| g.faces[*f].offset)
        .collect();
    let (flip, delete) = match base {
        Base::Keep => (Vec::new(), Vec::new()),
        Base::Flip => (others, Vec::new()),
        Base::Remove => (Vec::new(), others),
    };
    append_geometry(
        source,
        &Addition {
            host: Some(g.faces[host].offset),
            points,
            faces: new_faces,
            host_copy: base,
            flip,
            delete,
        },
    )
}
/// Model-space translation of the frame drawing a model face: the common
/// difference between its corners' positions and stored coordinates. Fails
/// when the frame rotates them, so local and model axes differ.
pub fn frame_translation(model: &Model, face: usize) -> Result<[i32; 3]> {
    let f = model
        .faces
        .iter()
        .find(|f| f.offset == face)
        .ok_or("The face is not drawn in this pose")?;
    let mut out = None;
    for v in &f.indices {
        let p = model.vertices.get(*v).ok_or("Vertex")?.point;
        let l = model.vertex_tags.get(*v).ok_or("Vertex tag")?.local;
        let d: [i32; 3] = core::array::from_fn(|k| p[k] - l[k]);
        if *out.get_or_insert(d) != d {
            return Err(invalid(
                "This part is rotated in the shown pose; edit it in local coordinates",
            ));
        }
    }
    out.ok_or_else(|| invalid("Face has no corners"))
}
/// Move model vertices (indices into `model.vertices`) by a model-space
/// delta through the region writer. Every part frame above a vertex must be
/// unrotated in that model, so the local delta equals the model delta.
pub fn move_model_vertices(
    source: &[u8],
    model: &Model,
    selected: &[usize],
    delta: [i32; 3],
) -> Result<Vec<u8>> {
    let mut moves = BTreeMap::new();
    for i in selected {
        let v = model.vertices.get(*i).ok_or("No selected vertex")?;
        let tag = model.vertex_tags.get(*i).ok_or("No selected vertex")?;
        let mut group = tag.group;
        for _ in 0..64 {
            let Some(g) = group.and_then(|g| model.groups.get(g)) else {
                break;
            };
            if let Some(p) = g.part.and_then(|p| model.parts.get(p)) {
                if p.posed_rotation != [0; 3] {
                    return Err(invalid(
                        "A selected vertex is in a rotated part; edit it in local coordinates",
                    ));
                }
            }
            group = g.parent;
        }
        let mut local = tag.local;
        for (k, d) in delta.iter().enumerate() {
            local[k] = local[k].checked_add(*d).ok_or("Coordinate overflow")?;
        }
        moves.insert(v.offset, local);
    }
    if moves.is_empty() {
        return Err(invalid("Select at least one vertex"));
    }
    write_vertices(source, &moves.into_iter().collect::<Vec<_>>())
}

#[cfg(test)]
#[path = "shape_geometry_tests.rs"]
mod tests;
