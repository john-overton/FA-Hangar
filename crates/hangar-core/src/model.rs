//! Bounded SH static-pose reader. Only fully understood straight-line shapes are writable.
use crate::{invalid, slice, u16_at, u32_at, Result};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
#[derive(Clone, Debug)]
pub struct Vertex {
    pub point: [i32; 3],
    pub offset: usize,
}
#[derive(Clone, Debug)]
pub struct Face {
    pub indices: Vec<usize>,
    pub offset: usize,
    pub sub: u8,
    pub flags: u8,
    pub color: u8,
    pub colors: Vec<u8>,
    pub normal: Option<[i32; 3]>,
    pub texture: String,
    pub uv: Vec<[i32; 2]>,
    pub uv_offsets: Vec<usize>,
    pub end: usize,
    pub material_selector: Vec<u8>,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
    pub textures: BTreeSet<String>,
    pub texture_records: BTreeMap<usize, String>,
    pub writable: bool,
    pub reason: String,
    pub records: Vec<Record>,
    pub state_words: BTreeSet<usize>,
    pub parts: Vec<Part>,
}
#[derive(Clone, Debug)]
pub struct Part {
    pub offset: usize,
    pub target: usize,
    pub position: [i32; 3],
    /// Stored C4 angles; native arithmetic can overwrite these at runtime.
    pub rotation: [i32; 3],
}
#[derive(Clone, Debug)]
pub struct Record {
    pub offset: usize,
    pub length: usize,
    pub opcode: u8,
}
#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Move(usize, i32),
    Rotate(usize, i32),
    Scale(Option<usize>, i32),
}
// Integer trig keeps the core independent of a C math runtime. Angle is degrees;
// Bhaskara's approximation is for editor interaction, not original game simulation.
pub fn sin_cos(degrees: i32) -> (i32, i32) {
    fn sin(d: i32) -> i32 {
        let mut d = d.rem_euclid(360);
        let sign = if d > 180 {
            d -= 180;
            -1
        } else {
            1
        };
        let t = d * (180 - d);
        sign * (4 * t * 1024 / (40500 - t))
    }
    let degrees = degrees.rem_euclid(360);
    (sin(degrees), sin(degrees + 90))
}
pub fn rotate(p: [i32; 3], axis: usize, degrees: i32) -> [i32; 3] {
    let (s, c) = sin_cos(degrees);
    let a = (axis + 1) % 3;
    let b = (axis + 2) % 3;
    let mut q = p;
    q[a] = ((p[a] as i64 * c as i64 - p[b] as i64 * s as i64) / 1024) as i32;
    q[b] = ((p[a] as i64 * s as i64 + p[b] as i64 * c as i64) / 1024) as i32;
    q
}
fn word(b: &[u8], p: usize) -> Result<i32> {
    Ok(u16_at(b, p)? as u16 as i16 as i32)
}
fn target(p: usize, d: i32, len: usize) -> Result<usize> {
    let t = p as i64 + d as i64;
    if t < 0 || t >= len as i64 {
        Err(invalid("SH branch outside CODE"))
    } else {
        Ok(t as usize)
    }
}
fn section(data: &[u8]) -> Result<(usize, usize, usize)> {
    if data.len() > 16 * 1024 * 1024 || slice(data, 0, 2)? != b"MZ" {
        return Err(invalid("Not a PL/PE shape module"));
    }
    let p = u32_at(data, 60)?;
    if !matches!(slice(data, p, 4)?, b"PL\0\0" | b"PE\0\0") || u16_at(data, p + 4)? != 0x14c {
        return Err(invalid("Invalid SH module"));
    }
    let n = u16_at(data, p + 6)?;
    let optional = u16_at(data, p + 20)?;
    if n > 32 || optional < 32 {
        return Err(invalid("Invalid section table"));
    }
    let base = u32_at(data, p + 52)?;
    for i in 0..n {
        let s = slice(data, p + 24 + optional + i * 40, 40)?;
        if &s[..4] == b"CODE" {
            let size = u32_at(s, 8)?.min(u32_at(s, 16)?);
            let start = u32_at(s, 20)?;
            slice(data, start, size)?;
            return Ok((
                start,
                size,
                base.checked_add(u32_at(s, 12)?)
                    .ok_or_else(|| invalid("SH address overflow"))?,
            ));
        }
    }
    Err(invalid("Missing CODE section"))
}
impl Model {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::with_state(data, &BTreeMap::new())
    }
    /// Reviewed state switches only. Imported code and angle arithmetic are inert.
    pub fn with_state(data: &[u8], state: &BTreeMap<usize, i32>) -> Result<Self> {
        let (start, len, base) = section(data)?;
        let c = &data[start..start + len];
        let mut out = Self {
            vertices: Vec::new(),
            faces: Vec::new(),
            textures: BTreeSet::new(),
            texture_records: BTreeMap::new(),
            writable: true,
            reason: String::new(),
            records: Vec::new(),
            state_words: BTreeSet::new(),
            parts: Vec::new(),
        };
        let mut slots = BTreeMap::<usize, usize>::new();
        let mut vertex_colors = BTreeMap::<usize, u8>::new();
        let mut seen = BTreeSet::new();
        let mut texture = String::new();
        let mut material_selector = Vec::new();
        let mut p = 0;
        let mut end = None;
        let mut trans = [0; 3];
        type Frame = (usize, Option<usize>, [i32; 3], Option<(String, Vec<u8>)>);
        let mut stack: Vec<Frame> = Vec::new();
        let mut done = false;
        for _ in 0..30000 {
            if stack.len() > 64 {
                return Err(invalid("SH call depth exceeded"));
            }
            let op = slice(c, p, 1)?[0];
            if op == 0 || (op == 0x1e && end.is_none_or(|e| p >= e)) {
                if let Some((ret, e, t, old_texture)) = stack.pop() {
                    p = ret;
                    end = e;
                    trans = t;
                    if let Some((name, selector)) = old_texture {
                        texture = name;
                        material_selector = selector;
                    }
                    continue;
                }
                done = true;
                break;
            }
            let record_start = p;
            match op {
                0xeb => {
                    out.writable = false;
                    // Reviewed CHAP/SA2 loaded-launcher selection envelope.
                    // This chooses the loaded static branch; no HARD callback runs.
                    if slice(c, p, 7)? != [0xeb, 5, 0xb8, 1, 0, 0, 0] {
                        return Err(invalid("Unsupported launcher selection envelope"));
                    }
                    let draw = if slice(c, p + 7, 2)? == [0x83, 0xf8]
                        && (1..=4).contains(&slice(c, p + 9, 1)?[0])
                        && slice(c, p + 10, 2)? == [0x72, 0x11]
                    {
                        p + 12
                    } else if slice(c, p + 7, 4)? == [0x0b, 0xc0, 0x74, 0x11] {
                        p + 11
                    } else {
                        return Err(invalid("Unsupported launcher load comparison"));
                    };
                    if slice(c, draw, 1)? != [0x68]
                        || slice(c, draw + 5, 1)? != [0x68]
                        || slice(c, draw + 10, 1)? != [0xc3]
                    {
                        return Err(invalid("Invalid launcher drawing reference"));
                    }
                    p = u32_at(c, draw + 1)?
                        .checked_sub(base)
                        .ok_or("Launcher drawing address underflow")?;
                    if slice(c, p, 2)? != [0x12, 0] {
                        return Err(invalid("Launcher reference is not an SH drawing call"));
                    }
                }
                0x38 => {
                    out.writable = false;
                    let t = target(p + 3, word(c, p + 1)?, len)?;
                    if t <= p {
                        return Err(invalid("Invalid SH scope"));
                    }
                    end = Some(end.unwrap_or(t).max(t));
                    p += 3;
                }
                0x12 => {
                    out.writable = false;
                    let t = target(p + 4, word(c, p + 2)?, len)?;
                    stack.push((p + 4, end, trans, None));
                    p = t;
                    end = None;
                }
                0x48 => {
                    p = target(p + 4, word(c, p + 2)?, len)?;
                }
                0xc4 => {
                    out.writable = false;
                    let t = target(p + 16, word(c, p + 14)?, len)?;
                    if !out.parts.iter().any(|part| part.offset == start + p) {
                        out.parts.push(Part {
                            offset: start + p,
                            target: start + t,
                            position: [word(c, p + 2)?, word(c, p + 6)?, word(c, p + 4)?],
                            rotation: [word(c, p + 8)?, word(c, p + 10)?, word(c, p + 12)?],
                        });
                    }
                    stack.push((
                        p + 16,
                        end,
                        trans,
                        Some((texture.clone(), material_selector.clone())),
                    ));
                    trans[0] += word(c, p + 2)?;
                    trans[1] += word(c, p + 6)?;
                    trans[2] += word(c, p + 4)?;
                    p = t;
                    end = None;
                }
                0xf0 => {
                    out.writable = false;
                    material_selector.clear();
                    let mut at = p + 2;
                    if slice(c, at, 2)? == [0x83, 0x0d]
                        && slice(c, at + 6, 1)? == [2]
                        && slice(c, at + 7, 3)? == [0x66, 0x83, 0x3d]
                    {
                        at += 7;
                    }
                    for _ in 0..16 {
                        if slice(c, at, 3)? != [0x66, 0x83, 0x3d] {
                            break;
                        }
                        let address = u32_at(c, at + 3)?;
                        out.state_words.insert(address);
                        let value = state.get(&address).copied().unwrap_or(0);
                        let imm = slice(c, at + 7, 1)?[0] as i8 as i32;
                        let take = match slice(c, at + 8, 1)?[0] {
                            0x74 => value == imm,
                            0x75 => value != imm,
                            _ => return Err(invalid("Unsupported SH pose guard")),
                        };
                        if take {
                            at = target(at + 10, slice(c, at + 9, 1)?[0] as i8 as i32, len)?;
                        } else {
                            at += 10;
                            break;
                        }
                    }
                    // Bounded inert reference pattern, never execute imported machine code.
                    let mut next = None;
                    for r in at..(at + 128).min(len.saturating_sub(10)) {
                        if c[r] == 0x68 && c[r + 5] == 0x68 && c[r + 10] == 0xc3 {
                            next = Some(
                                u32_at(c, r + 1)?
                                    .checked_sub(base)
                                    .ok_or_else(|| invalid("SH reentry underflow"))?,
                            );
                            break;
                        }
                    }
                    p = next
                        .filter(|t| *t < len)
                        .ok_or_else(|| invalid("Unsupported SH reentry"))?;
                }
                0x82 => {
                    let count = u16_at(c, p + 2)?;
                    let dest = u16_at(c, p + 4)?;
                    if dest % 8 != 0 || count > 8192 || out.vertices.len() + count > 65536 {
                        return Err(invalid("Invalid SH vertex buffer"));
                    }
                    slice(c, p, 6 + count * 6)?;
                    for i in 0..count {
                        let at = p + 6 + i * 6;
                        let point = [
                            word(c, at)? + trans[0],
                            word(c, at + 2)? + trans[1],
                            word(c, at + 4)? + trans[2],
                        ];
                        slots.insert(dest / 8 + i, out.vertices.len());
                        out.vertices.push(Vertex {
                            point,
                            offset: start + at,
                        });
                    }
                    p += 6 + count * 6;
                }
                0xf6 => {
                    out.writable = false;
                    vertex_colors.insert(u16_at(c, p + 1)?, slice(c, p + 3, 1)?[0]);
                    slice(c, p, 7)?;
                    p += 7;
                }
                0xfc => {
                    let addr = p;
                    let h = slice(c, p, 5)?;
                    let sub = h[1];
                    let flags = h[2];
                    p += 5;
                    if sub & 0x60 != 0 {
                        p += 6 + if flags & 2 != 0 { 3 } else { 6 };
                    }
                    let count = slice(c, p, 1)?[0] as usize;
                    p += 1;
                    if !(3..=64).contains(&count) {
                        return Err(invalid("Invalid SH polygon"));
                    }
                    let mut indices = Vec::with_capacity(count);
                    let mut colors = Vec::with_capacity(count);
                    for _ in 0..count {
                        let i = if flags & 4 != 0 {
                            let i = u16_at(c, p)?;
                            p += 2;
                            i
                        } else {
                            let i = slice(c, p, 1)?[0] as usize;
                            p += 1;
                            i
                        };
                        colors.push(if sub == 0xee {
                            *vertex_colors.get(&i).unwrap_or(&c[addr + 3])
                        } else {
                            c[addr + 3]
                        });
                        indices.push(
                            *slots
                                .get(&i)
                                .ok_or_else(|| invalid("Unresolved SH vertex"))?,
                        );
                    }
                    let mut uv = Vec::new();
                    let mut uv_offsets = Vec::new();
                    if sub & 4 != 0 {
                        for _ in 0..count {
                            uv_offsets.push(start + p);
                            if flags & 1 != 0 {
                                let b = slice(c, p, 2)?;
                                uv.push([b[0] as i32, b[1] as i32]);
                                p += 2;
                            } else {
                                uv.push([u16_at(c, p)? as i32, u16_at(c, p + 2)? as i32]);
                                p += 4;
                            }
                        }
                    }
                    slice(c, addr, p - addr)?;
                    if seen.insert((addr, trans)) {
                        out.faces.push(Face {
                            indices,
                            offset: start + addr,
                            sub,
                            flags,
                            color: c[addr + 3],
                            colors,
                            normal: if sub & 0x60 != 0 {
                                Some([word(c, addr + 5)?, word(c, addr + 9)?, word(c, addr + 7)?])
                            } else {
                                None
                            },
                            texture: texture.clone(),
                            uv,
                            uv_offsets,
                            end: start + p,
                            material_selector: material_selector.clone(),
                        });
                    }
                }
                0xe2 => {
                    material_selector = slice(c, p, 16)?.to_vec();
                    let name = slice(c, p + 2, 14)?.split(|b| *b == 0).next().unwrap();
                    let name = core::str::from_utf8(name)
                        .map_err(|_| invalid("Non-ASCII texture reference"))?
                        .to_ascii_uppercase();
                    if !name.is_empty() {
                        out.texture_records.insert(start + p + 2, name.clone());
                        texture = name.clone();
                        out.textures.insert(name);
                    }
                    p += 16;
                }
                0xe0 => {
                    material_selector = slice(c, p, 4)?.to_vec();
                    texture.clear();
                    p += 4;
                }
                0xca => p += 4,
                0x42 => {
                    let rest = slice(c, p + 2, len.saturating_sub(p + 2))?;
                    p += 3 + rest
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or_else(|| invalid("Unterminated SH string"))?;
                }
                0xff if slice(c, p, 2)? == [255, 255] => {
                    out.writable = false;
                    p += 14;
                }
                _ => {
                    out.writable = false;
                    p += match op {
                        0x1e => 1,
                        0x46 | 0xb2 | 0x4e | 0xee => 2,
                        0xf2 | 0xb8 | 0x4d | 0xd0 | 0xda | 0x05 | 0x14 | 0x18 | 0x4a | 0xac => 4,
                        0xa6 => 6,
                        0xf6 => 7,
                        0x2e | 0x50 | 0x68 | 0xea | 0xc8 => 8,
                        0x7a | 0x0c | 0x0e | 0x10 | 0x66 | 0xe6 | 0x76 | 0x08 | 0x6c => 10,
                        0x78 => 12,
                        0x06 => 14,
                        0xe4 => 20,
                        0xce => 40,
                        0x40 => 4 + 2 * u16_at(c, p + 2)?,
                        0x44 => 8 + 2 * u16_at(c, p + 6)?,
                        0xbc => match slice(c, p + 2, 1)?[0] {
                            0x72 | 0x08 => 6,
                            0x96 | 0x3a => 8,
                            0x68 => 10,
                            _ => return Err(invalid("Unsupported SH primitive")),
                        },
                        _ => return Err(format!("Unsupported SH opcode {op:02X} at CODE+{p:X}")),
                    };
                }
            }
            let length = match op {
                0x12 | 0x48 => 4,
                0xc4 => 16,
                0xf0 | 0xeb => 2,
                _ => p.saturating_sub(record_start),
            };
            out.records.push(Record {
                offset: start + record_start,
                length,
                opcode: op,
            });
            if p > len {
                return Err(invalid("Truncated SH record"));
            }
        }
        if !done || out.faces.is_empty() {
            return Err(invalid("SH instruction limit or no visible geometry"));
        }
        if !out.writable {
            out.reason="Preview only: branches, bounds, animation or other spatial records need a complete writer".into();
        }
        Ok(out)
    }
    /// Preview transform works for any decoded pose. Packaging edits is restricted.
    pub fn transformed(&self, t: Transform) -> Result<Self> {
        let mut out = self.clone();
        match t {
            Transform::Move(a, _) | Transform::Rotate(a, _) if a > 2 => {
                return Err(invalid("Invalid axis"))
            }
            Transform::Scale(Some(a), _) if a > 2 => return Err(invalid("Invalid axis")),
            Transform::Scale(_, n) if !(1..=10000).contains(&n) => {
                return Err(invalid("Scale must be 1..10000 percent"))
            }
            _ => {}
        }
        for v in &mut out.vertices {
            v.point = match t {
                Transform::Move(a, n) => {
                    let mut p = v.point;
                    p[a] = p[a]
                        .checked_add(n)
                        .ok_or_else(|| invalid("Coordinate overflow"))?;
                    p
                }
                Transform::Rotate(a, n) => rotate(v.point, a, n),
                Transform::Scale(a, n) => {
                    let mut p = v.point;
                    for (i, x) in p.iter_mut().enumerate() {
                        if a.is_none_or(|a| a == i) {
                            *x = (*x as i64 * n as i64 / 100) as i32;
                        }
                    }
                    p
                }
            };
            if v.point.iter().any(|n| !(-32768..=32767).contains(n)) {
                return Err(invalid("Transform exceeds signed 16-bit coordinates"));
            }
        }
        Ok(out)
    }
    pub fn write(&self, source: &[u8]) -> Result<Vec<u8>> {
        if !self.writable {
            return Err(self.reason.clone());
        }
        let original = Self::parse(source)?;
        if !original.writable {
            return Err(original.reason.clone());
        }
        if self
            .vertices
            .iter()
            .any(|v| v.point.iter().any(|x| !(-32768..=32767).contains(x)))
        {
            return Err(invalid("Vertex exceeds signed 16-bit source coordinates"));
        }
        if original.vertices.len() != self.vertices.len()
            || original.faces.len() != self.faces.len()
        {
            return Err(invalid("Model does not match its source"));
        }
        if original
            .vertices
            .iter()
            .zip(&self.vertices)
            .any(|(a, b)| a.offset != b.offset)
            || original.faces.iter().zip(&self.faces).any(|(a, b)| {
                a.offset != b.offset
                    || a.end != b.end
                    || a.indices != b.indices
                    || a.flags != b.flags
                    || a.sub != b.sub
                    || a.uv != b.uv
            })
        {
            return Err(invalid(
                "Geometry edits must preserve source record topology/provenance",
            ));
        }
        let moved: Vec<bool> = original
            .vertices
            .iter()
            .zip(&self.vertices)
            .map(|(a, b)| a.point != b.point)
            .collect();
        if !moved.contains(&true) {
            return Ok(source.to_vec());
        }
        let mut out = source.to_vec();
        for (v, _) in self.vertices.iter().zip(&moved).filter(|(_, m)| **m) {
            for (j, n) in v.point.iter().enumerate() {
                out[v.offset + j * 2..v.offset + j * 2 + 2]
                    .copy_from_slice(&(*n as i16).to_le_bytes());
            }
        }
        // Only faces with a moved vertex own changed normal/centre bytes. Recompute from
        // positions (never from a caller's cached normal) and keep the stored normal when
        // every vertex triple is degenerate.
        let mut updated = self.clone();
        updated.refresh_normals(&original);
        for (i, f) in updated.faces.iter().enumerate() {
            let Some(normal) = f.normal else {
                continue;
            };
            if !f.indices.iter().any(|v| moved[*v]) {
                continue;
            }
            // Stored order is right, up, forward; the model uses right, forward, up.
            for (j, axis) in [0, 2, 1].into_iter().enumerate() {
                out[f.offset + 5 + j * 2..f.offset + 7 + j * 2]
                    .copy_from_slice(&(normal[axis] as i16).to_le_bytes());
            }
            let pts = self.face_points(i);
            for (j, axis) in [0, 2, 1].into_iter().enumerate() {
                let val = pts.iter().map(|p| p[axis] as i64).sum::<i64>() / pts.len() as i64;
                if f.flags & 2 != 0 {
                    if !(-128..=127).contains(&val) {
                        return Err(invalid(
                            "Face center exceeds byte width; relocation required",
                        ));
                    }
                    out[f.offset + 11 + j] = val as i8 as u8;
                } else {
                    out[f.offset + 11 + j * 2..f.offset + 13 + j * 2]
                        .copy_from_slice(&(val as i16).to_le_bytes());
                }
            }
        }
        Self::parse(&out)?;
        Ok(out)
    }
    fn face_points(&self, face: usize) -> Vec<[i32; 3]> {
        self.faces[face]
            .indices
            .iter()
            .map(|i| self.vertices[*i].point)
            .collect()
    }
    /// Update cached normals of faces whose vertices differ from `before`, exactly as
    /// `write` stores them, so previews cull the same faces the written shape will.
    pub fn refresh_normals(&mut self, before: &Model) {
        for i in 0..self.faces.len() {
            let f = &self.faces[i];
            if f.normal.is_none()
                || !f.indices.iter().any(|v| {
                    before
                        .vertices
                        .get(*v)
                        .is_none_or(|b| b.point != self.vertices[*v].point)
                })
            {
                continue;
            }
            let stored = before.faces.get(i).and_then(|b| b.normal).or(f.normal);
            self.faces[i].normal = face_normal(&self.face_points(i)).or(stored);
        }
    }
}
/// Unit normal (scale 32765) in model order right/forward/up, from the first
/// non-degenerate vertex triple. Retail FA faces store (c-a)x(b-a): the opposite of
/// the right-handed cross product for their vertex order. None if all are collinear.
pub fn face_normal(points: &[[i32; 3]]) -> Option<[i32; 3]> {
    let n = points.len();
    let sub = |a: [i32; 3], b: [i32; 3]| -> [i64; 3] {
        core::array::from_fn(|k| b[k] as i64 - a[k] as i64)
    };
    for a in 0..n {
        for b in a + 1..n {
            for c in b + 1..n {
                let u = sub(points[a], points[b]);
                let v = sub(points[a], points[c]);
                let mut n = [
                    v[1] * u[2] - v[2] * u[1],
                    v[2] * u[0] - v[0] * u[2],
                    v[0] * u[1] - v[1] * u[0],
                ];
                // Rescale before squaring to avoid overflow for wide 16-bit triangles.
                let max = n.iter().map(|n| n.abs()).max().unwrap();
                if max == 0 {
                    continue;
                }
                for x in &mut n {
                    *x = *x * 32765 / max;
                }
                let mag = isqrt(n.iter().map(|x| (x * x) as u64).sum()).max(1) as i64;
                return Some(core::array::from_fn(|k| {
                    ((n[k] * 32765 + n[k].signum() * mag / 2) / mag) as i32
                }));
            }
        }
    }
    None
}
impl Model {
    /// Change only reviewed FC palette bytes, never geometry/links/relocations.
    /// Scope is untextured faces reached by this static-pose traversal.
    pub fn retarget_texture(source: &[u8], from: &str, to: &str) -> Result<(Vec<u8>, usize)> {
        crate::archive::validate_name(to)?;
        if !to.to_ascii_uppercase().ends_with(".PIC") {
            return Err(invalid("Texture name must end in .PIC"));
        }
        let model = Self::parse(source)?;
        let mut out = source.to_vec();
        let mut n = 0;
        for (at, name) in model.texture_records {
            let full = if name.contains('.') {
                name
            } else {
                format!("{name}.PIC")
            };
            if full.eq_ignore_ascii_case(from) {
                out[at..at + 14].fill(0);
                out[at..at + to.len()].copy_from_slice(to.as_bytes());
                n += 1;
            }
        }
        if n == 0 {
            return Err(invalid("No decoded texture references matched"));
        }
        Self::parse(&out)?;
        Ok((out, n))
    }
    pub fn recolor_face(source: &[u8], face: usize, to: u8) -> Result<Vec<u8>> {
        let model = Self::parse(source)?;
        let f = model.faces.get(face).ok_or("No selected face")?;
        if f.sub & 4 != 0 || u16_at(source, f.offset + 3)? > 255 {
            return Err(invalid("This face does not use a plain palette color"));
        }
        let mut out = source.to_vec();
        out[f.offset + 3] = to;
        Ok(out)
    }
    pub fn recolor(source: &[u8], from: u8, to: u8) -> Result<(Vec<u8>, usize)> {
        let model = Self::parse(source)?;
        let mut out = source.to_vec();
        let mut seen = BTreeSet::new();
        for f in &model.faces {
            if f.sub & 4 == 0
                && u16_at(source, f.offset + 3)? == from as usize
                && seen.insert(f.offset)
            {
                out[f.offset + 3] = to;
            }
        }
        let n = seen.len();
        if n == 0 {
            return Err(invalid(
                "No matching untextured face colors in the decoded pose",
            ));
        }
        Self::parse(&out)?;
        Ok((out, n))
    }
    pub fn obj(&self) -> String {
        let mut out =
            String::from("# TORE Hangar static pose; geometry only, no textures or animation\n");
        for v in &self.vertices {
            out.push_str(&format!("v {} {} {}\n", v.point[0], v.point[1], v.point[2]));
        }
        for f in &self.faces {
            out.push('f');
            for i in &f.indices {
                out.push_str(&format!(" {}", i + 1));
            }
            out.push('\n');
        }
        out
    }
}
fn isqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x / 2 + 1;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
/// Synthetic fixture, never retail media. Also used by the interactive demo.
pub fn demo_shape() -> Vec<u8> {
    let mut c = vec![0x82, 0, 6, 0, 0, 0];
    for p in [
        [0i16, 100, 0],
        [-80, -35, 0],
        [80, -35, 0],
        [0, -50, 20],
        [0, -30, -12],
        [0, -70, 0],
    ] {
        for x in p {
            c.extend(x.to_le_bytes());
        }
    }
    for face in [
        [0, 1, 3],
        [0, 3, 2],
        [0, 2, 4],
        [0, 4, 1],
        [1, 5, 3],
        [2, 3, 5],
        [1, 4, 5],
        [2, 5, 4],
    ] {
        c.extend([0xfc, 0, 0, 32, 0, 3]);
        c.extend(face);
    }
    c.push(0);
    module(c)
}
/// Minimal PL module around a synthetic CODE section (file offset 256).
fn module(c: Vec<u8>) -> Vec<u8> {
    let mut b = vec![0; 256];
    b[..2].copy_from_slice(b"MZ");
    b[60..64].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PL\0\0");
    b[68..70].copy_from_slice(&0x14cu16.to_le_bytes());
    b[70..72].copy_from_slice(&1u16.to_le_bytes());
    b[84..86].copy_from_slice(&32u16.to_le_bytes());
    b[120..124].copy_from_slice(b"CODE");
    for (at, n) in [(128, c.len()), (132, 4096), (136, c.len()), (140, 256)] {
        b[at..at + 4].copy_from_slice(&(n as u32).to_le_bytes());
    }
    b.extend(c);
    b
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_transform_and_preserve_other_bytes() {
        let b = demo_shape();
        let m = Model::parse(&b).unwrap();
        assert!(m.writable);
        assert_eq!(m.write(&b).unwrap(), b);
        assert_eq!(m.faces.len(), 8);
        let changed = m
            .transformed(Transform::Move(0, 10))
            .unwrap()
            .write(&b)
            .unwrap();
        assert_eq!(
            Model::parse(&changed).unwrap().vertices[0].point,
            [10, 100, 0]
        );
        assert_eq!(&b[..262], &changed[..262]);
        assert!(m.transformed(Transform::Move(0, 32768)).is_err());
        for n in 0..b.len() {
            assert!(Model::parse(&b[..n]).is_err());
        }
    }
    #[test]
    fn rotations_and_limits() {
        assert_eq!(rotate([100, 0, 0], 2, 90), [0, 100, 0]);
        assert_eq!(sin_cos(-90), (-1024, 0));
        assert_eq!(sin_cos(i32::MAX), sin_cos(i32::MAX % 360));
        assert!(Model::parse(&demo_shape())
            .unwrap()
            .transformed(Transform::Scale(None, 0))
            .is_err());
    }
    #[test]
    fn unknown_records_never_written() {
        let mut b = demo_shape();
        b[256] = 0xff;
        b[257] = 0xff;
        assert!(Model::parse(&b).is_err());
    }
    /// FC with stored normal (right, up, forward words) and a byte or word centre.
    fn lit_face(
        sub: u8,
        byte_centre: bool,
        normal: [i16; 3],
        centre: [i16; 3],
        v: &[u8],
    ) -> Vec<u8> {
        let mut f = vec![0xfc, sub, if byte_centre { 2 } else { 0 }, 32, 0];
        for n in normal {
            f.extend(n.to_le_bytes());
        }
        for n in centre {
            if byte_centre {
                f.push(n as i8 as u8);
            } else {
                f.extend(n.to_le_bytes());
            }
        }
        f.push(v.len() as u8);
        f.extend(v);
        f
    }
    fn lit_shape() -> Vec<u8> {
        let mut c = vec![0x82, 0, 6, 0, 0, 0];
        for p in [
            [0i16, 0, 0],
            [10, 0, 0],
            [0, 10, 0],
            [20, 0, 0],
            [0, 0, 10],
            [30, 0, 0],
        ] {
            for x in p {
                c.extend(x.to_le_bytes());
            }
        }
        // Deliberately stale stored values prove which faces are rewritten.
        c.extend(lit_face(0x41, true, [1, 2, 3], [4, 5, 6], &[0, 1, 2]));
        c.extend(lit_face(0x41, false, [1, 2, 3], [4, 5, 6], &[0, 3, 4]));
        c.extend(lit_face(0x41, true, [7, 8, 9], [1, 2, 3], &[1, 2, 4]));
        c.extend(lit_face(0x61, false, [11, 12, 13], [4, 5, 6], &[1, 3, 5]));
        c.extend(lit_face(0x40, false, [1, 2, 3], [4, 5, 6], &[0, 1, 3, 2]));
        c.push(0);
        module(c)
    }
    fn face_bytes(b: &[u8], m: &Model, i: usize) -> Vec<u8> {
        b[m.faces[i].offset..m.faces[i].end].to_vec()
    }
    fn words(b: &[u8], at: usize) -> [i16; 3] {
        core::array::from_fn(|k| i16::from_le_bytes([b[at + k * 2], b[at + k * 2 + 1]]))
    }
    #[test]
    fn write_updates_only_faces_with_moved_vertices() {
        let b = lit_shape();
        let m = Model::parse(&b).unwrap();
        assert!(m.writable);
        assert_eq!(m.faces.len(), 5);
        assert_eq!(m.write(&b).unwrap(), b);
        let mut edit = m.clone();
        edit.vertices[0].point = [-5, 0, 0];
        let out = edit.write(&b).unwrap();
        let f = |i: usize| m.faces[i].offset;
        // Face 0: plane up=0 wound (0,0)->(10,0)->(0,10); retail normal points down.
        assert_eq!(words(&out, f(0) + 5), [0, -32765, 0]);
        assert_eq!(&out[f(0) + 11..f(0) + 14], &[1, 0, 3]);
        // Face 1: word centre, normal along +forward (stored third).
        assert_eq!(words(&out, f(1) + 5), [0, 0, 32765]);
        assert_eq!(words(&out, f(1) + 11), [5, 3, 0]);
        // Faces 2 and 3 do not use vertex 0: every byte is unchanged.
        assert_eq!(face_bytes(&out, &m, 2), face_bytes(&b, &m, 2));
        assert_eq!(face_bytes(&out, &m, 3), face_bytes(&b, &m, 3));
        // Face 4: first three vertices are collinear; the next triple decides.
        assert_eq!(words(&out, f(4) + 5), [0, -32765, 0]);
        assert_eq!(words(&out, f(4) + 11), [6, 0, 2]);
        let changed: Vec<_> = (0..b.len()).filter(|i| b[*i] != out[*i]).collect();
        assert!(changed
            .iter()
            .all(
                |i| (m.vertices[0].offset..m.vertices[0].offset + 6).contains(i)
                    || [0, 1, 4]
                        .iter()
                        .any(|n| (f(*n) + 5..m.faces[*n].end).contains(i))
            ));
        let reread = Model::parse(&out).unwrap();
        assert_eq!(reread.vertices[0].point, [-5, 0, 0]);
        assert_eq!(reread.faces[0].normal, Some([0, 0, -32765]));
    }
    #[test]
    fn degenerate_faces_keep_stored_normals_and_byte_centres_are_bounded() {
        let b = lit_shape();
        let m = Model::parse(&b).unwrap();
        let mut edit = m.clone();
        edit.vertices[1].point = [11, 0, 0];
        let out = edit.write(&b).unwrap();
        let at = m.faces[3].offset;
        assert_eq!(words(&out, at + 5), [11, 12, 13]);
        assert_eq!(words(&out, at + 11), [20, 0, 0]);
        let mut edit = m.clone();
        edit.vertices[0].point = [400, 0, 0];
        assert!(edit.write(&b).unwrap_err().contains("byte width"));
        assert_eq!(face_normal(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]), None);
        assert_eq!(
            face_normal(&[[0, 0, 0], [30000, 0, 0], [0, 30000, 0]]),
            Some([0, 0, -32765])
        );
    }
}

/// Synthetic textured geometry for UI and paint-coordinate tests.
pub fn demo_textured() -> Vec<u8> {
    let original = demo_shape();
    let mut code = vec![0xe2, 0];
    code.extend(b"DEMO.PIC\0\0\0\0\0\0");
    code.extend(&original[256..298]);
    for face in original[298..original.len() - 1].chunks_exact(9) {
        let mut f = face.to_vec();
        f[1] = 4;
        f[2] = 1;
        code.extend(f);
        code.extend([0, 0, 31, 0, 16, 31]);
    }
    code.push(0);
    let mut out = original[..256].to_vec();
    for at in [128, 136] {
        out[at..at + 4].copy_from_slice(&(code.len() as u32).to_le_bytes());
    }
    out.extend(code);
    out
}
#[cfg(test)]
mod material_tests {
    use super::*;
    #[test]
    fn face_color_changes_only_reviewed_palette_byte() {
        let b = demo_shape();
        let (after, n) = Model::recolor(&b, 32, 150).unwrap();
        assert_eq!(n, 8);
        let m = Model::parse(&b).unwrap();
        let changed: Vec<_> = b
            .iter()
            .zip(&after)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            changed,
            m.faces.iter().map(|f| f.offset + 3).collect::<Vec<_>>()
        );
        assert_eq!(
            Model::parse(&after).unwrap().vertices[0].point,
            m.vertices[0].point
        );
    }
    #[test]
    fn uv_texture_decode() {
        let b = demo_textured();
        let m = Model::parse(&b).unwrap();
        assert_eq!(m.faces[0].texture, "DEMO.PIC");
        assert_eq!(m.faces[0].uv, vec![[0, 0], [31, 0], [16, 31]]);
        assert!(Model::recolor(&b, 32, 2).is_err());
    }
}
