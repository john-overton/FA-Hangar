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
    pub texture: String,
    pub uv: Vec<[i32; 2]>,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub vertices: Vec<Vertex>,
    pub faces: Vec<Face>,
    pub textures: BTreeSet<String>,
    pub texture_records: BTreeMap<usize, String>,
    pub writable: bool,
    pub reason: String,
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
        let (start, len, base) = section(data)?;
        let c = &data[start..start + len];
        let mut out = Self {
            vertices: Vec::new(),
            faces: Vec::new(),
            textures: BTreeSet::new(),
            texture_records: BTreeMap::new(),
            writable: true,
            reason: String::new(),
        };
        let mut slots = BTreeMap::<usize, usize>::new();
        let mut seen = BTreeSet::new();
        let mut texture = String::new();
        let mut p = 0;
        let mut end = None;
        let mut trans = [0; 3];
        type Frame = (usize, Option<usize>, [i32; 3], Option<String>);
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
                    if let Some(name) = old_texture {
                        texture = name;
                    }
                    continue;
                }
                done = true;
                break;
            }
            match op {
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
                    out.writable = false;
                    p = target(p + 4, word(c, p + 2)?, len)?;
                }
                0xc4 => {
                    out.writable = false;
                    let t = target(p + 16, word(c, p + 14)?, len)?;
                    stack.push((p + 16, end, trans, Some(texture.clone())));
                    trans[0] += word(c, p + 2)?;
                    trans[1] += word(c, p + 6)?;
                    trans[2] += word(c, p + 4)?;
                    p = t;
                    end = None;
                }
                0xf0 => {
                    out.writable = false;
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
                        let imm = slice(c, at + 7, 1)?[0] as i8;
                        let take = match slice(c, at + 8, 1)?[0] {
                            0x74 => imm == 0,
                            0x75 => imm != 0,
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
                        indices.push(
                            *slots
                                .get(&i)
                                .ok_or_else(|| invalid("Unresolved SH vertex"))?,
                        );
                    }
                    let mut uv = Vec::new();
                    if sub & 4 != 0 {
                        for _ in 0..count {
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
                            texture: texture.clone(),
                            uv,
                        });
                    }
                }
                0xe2 => {
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
        if original.vertices.len() != self.vertices.len()
            || original.faces.len() != self.faces.len()
        {
            return Err(invalid("Model does not match its source"));
        }
        if original
            .vertices
            .iter()
            .zip(&self.vertices)
            .all(|(a, b)| a.offset == b.offset && a.point == b.point)
        {
            return Ok(source.to_vec());
        }
        let mut out = source.to_vec();
        for v in &self.vertices {
            for (j, n) in v.point.iter().enumerate() {
                out[v.offset + j * 2..v.offset + j * 2 + 2]
                    .copy_from_slice(&(*n as i16).to_le_bytes());
            }
        }
        for f in &self.faces {
            if f.sub & 0x60 == 0 {
                continue;
            }
            let pts: Vec<_> = f.indices.iter().map(|i| self.vertices[*i].point).collect();
            let a = pts[0];
            let b = pts[1];
            let c = pts[2];
            let u = [
                b[0] as i64 - a[0] as i64,
                b[1] as i64 - a[1] as i64,
                b[2] as i64 - a[2] as i64,
            ];
            let v = [
                c[0] as i64 - a[0] as i64,
                c[1] as i64 - a[1] as i64,
                c[2] as i64 - a[2] as i64,
            ];
            let mut n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            // Rescale before squaring to avoid overflow for wide 16-bit triangles.
            let max = n.iter().map(|n| n.abs()).max().unwrap().max(1);
            for x in &mut n {
                *x = *x * 32765 / max;
            }
            let mag = isqrt(n.iter().map(|x| (x * x) as u64).sum()).max(1) as i64;
            for (j, axis) in [0, 2, 1].into_iter().enumerate() {
                let val = (n[axis] * 32765 / mag) as i16;
                out[f.offset + 5 + j * 2..f.offset + 7 + j * 2].copy_from_slice(&val.to_le_bytes());
            }
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
