//! Per-face texture assignment. SH faces do not name textures: an E2 record
//! sets the texture state and later textured FC faces draw with it. To give
//! some faces their own PIC, each contiguous run of selected faces is detoured
//! into a continuation placed before the end marker:
//!
//! `E2 NEW.PIC; copies; restore selector; 48 back; stored originals`
//!
//! The original records become same-size jump stubs (`48` to the
//! continuation for the first face of a run, to their own end for the others,
//! `1E` fill and a closing `48`). The restore selector is the E2/E0 record
//! proved current at the run (`Geometry::material`), so faces after the run
//! keep their texture. The stored originals are never reached; they make the
//! continuation self-describing, so `restore_texture_assignment` puts the
//! exact original records back. A continuation is recognised structurally
//! from its bytes and its sites, never from a marker.
//!
//! Every result is re-parsed: CODE coverage, stubs and bindings unchanged,
//! the same number of drawn faces in every checked pose, the moved faces on
//! the new texture in the same part, every other face unchanged.
use crate::{
    archive::validate_name,
    invalid,
    model::{face_normal, Model, Pose},
    shape_code::Kind,
    shape_edit::{append_continuation, continuation_start, jump},
    shape_geometry::{face_layout, pose_drawing, verify_structure, Frame, Geometry},
    Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

/// Projection plane for planar UVs, in model order (right, forward, up).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// The faces' own plane; the longest edge sets the U direction.
    Auto,
    /// Seen from above: U right, V forward.
    Top,
    /// Seen from the right: U forward, V up.
    Side,
    /// Seen from the front: U right, V up.
    Front,
}
/// How the faces' UVs reach the new texture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UvMode {
    /// Keep the stored UVs. The caller checks that both PICs have the same
    /// size (`keep_refusal`).
    Keep,
    /// Scale stored UVs from one PIC size to another (integer, rounded).
    Scale { from: [u32; 2], to: [u32; 2] },
    /// Planar projection fitted into a PIC of `size`, uniform texels.
    Project { plane: Plane, size: [u32; 2] },
}
/// Why Keep cannot map faces from a PIC of size `from` onto one of size `to`.
pub fn keep_refusal(from: Option<[u32; 2]>, to: [u32; 2]) -> Option<String> {
    match from {
        Some(f) if f == to => None,
        Some(f) => Some(format!(
            "Keep needs the same size: {}x{} is not {}x{}",
            f[0], f[1], to[0], to[1]
        )),
        None => Some("Keep needs textured faces with a known PIC size".into()),
    }
}
/// Result of an assignment edit.
#[derive(Clone, Debug)]
pub struct Assigned {
    pub shape: Vec<u8>,
    /// New file offsets of the requested faces, in request order.
    pub faces: Vec<usize>,
}
/// A Hangar texture-assignment continuation found in a shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    /// Texture the continuation selects, as stored (upper case).
    pub texture: String,
    /// CODE span from the E2 record to the end of the stored originals.
    pub start: usize,
    pub end: usize,
    /// CODE spans of the original faces' sites, now jump stubs.
    pub sites: Vec<(usize, usize)>,
    /// CODE offsets of the drawn copies, in site order.
    pub copies: Vec<usize>,
    /// CODE offsets of the stored original records, in site order.
    pub originals: Vec<usize>,
    /// CODE span of the restore selector.
    pub restore: (usize, usize),
    /// CODE offset of the `48` back to the end of the run.
    pub back: usize,
}
/// Texel density used when a shape has no measurable textured faces:
/// 3 texels per source unit (Q16), near the FA_2.LIB median of 2.8.
pub const DEFAULT_DENSITY: u32 = 3 << 16;
/// Generated panel sheet limits per side.
pub const PANEL_MIN: u32 = 8;
pub const PANEL_MAX: u32 = 256;

fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << (128 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}
const ONE: i64 = 1 << 14;
fn dot(a: [i64; 3], b: [i64; 3]) -> i128 {
    (0..3).map(|k| a[k] as i128 * b[k] as i128).sum()
}
fn cross(a: [i64; 3], b: [i64; 3]) -> [i128; 3] {
    [
        a[1] as i128 * b[2] as i128 - a[2] as i128 * b[1] as i128,
        a[2] as i128 * b[0] as i128 - a[0] as i128 * b[2] as i128,
        a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128,
    ]
}
/// `v` scaled to length `ONE`, or `None` when it is (almost) zero.
fn unit(v: [i128; 3]) -> Option<[i64; 3]> {
    let len = isqrt(v.iter().map(|x| (x * x) as u128).sum());
    if len == 0 {
        return None;
    }
    let out = v.map(|x| (x * ONE as i128 / len as i128) as i64);
    (out != [0; 3]).then_some(out)
}
/// Right-handed in-plane basis (u, v) of length `ONE` for `normal`, with u
/// along `dir` projected onto the plane (or the axis least aligned with the
/// normal when `dir` is parallel to it). Seen from the normal side, u points
/// right and v up.
fn basis(normal: [i64; 3], dir: [i64; 3]) -> Option<([i64; 3], [i64; 3])> {
    let n = unit(normal.map(|x| x as i128))?;
    let d = dir.map(|x| x * ONE);
    let k = dot(d, n);
    let flat = core::array::from_fn(|i| d[i] as i128 - n[i] as i128 * k / (ONE * ONE) as i128);
    let u = unit(flat).or_else(|| {
        let axis = (0..3).min_by_key(|i| n[*i].abs()).unwrap_or(0);
        let mut a = [0i64; 3];
        a[axis] = ONE;
        unit(cross(a, n))
    })?;
    let v = unit(cross(n, u))?;
    Some((u, v))
}
/// How planar UVs are sized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Fit into a PIC of this size (width, height), keeping texels square.
    Size([u32; 2]),
    /// Choose the size from a texel density (Q16 texels per source unit),
    /// clamped to `PANEL_MIN..=PANEL_MAX` per side with the aspect kept.
    Density(u32),
}
/// Corner points (model order) of one face and its stored normal.
pub type PanelFace = (Vec<[i32; 3]>, Option<[i32; 3]>);
/// Planar UVs for a group of faces sharing one texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planar {
    /// One UV per corner of each face, in PIC pixels (V as stored).
    pub uv: Vec<Vec<[i32; 2]>>,
    pub size: [u32; 2],
}
/// Sheet size for planar extents `es` x `et` (source units scaled by
/// `ONE`) at `density`: square texels, the longer side clamped to
/// `PANEL_MAX`, the shorter raised to `PANEL_MIN`, both scaled together.
fn sheet_size(es: i128, et: i128, density: u32) -> [u32; 2] {
    let px = |e: i128| ((e * density as i128 + (1i128 << 29)) >> 30).max(1);
    let (mut w, mut h) = (px(es), px(et));
    let big = w.max(h);
    if big > PANEL_MAX as i128 {
        w = (w * PANEL_MAX as i128 + big / 2) / big;
        h = (h * PANEL_MAX as i128 + big / 2) / big;
    }
    let small = w.min(h).max(1);
    if small < PANEL_MIN as i128 {
        w = (w * PANEL_MIN as i128 + small - 1) / small;
        h = (h * PANEL_MIN as i128 + small - 1) / small;
    }
    let clamp = |v: i128| v.clamp(PANEL_MIN as i128, PANEL_MAX as i128) as u32;
    [clamp(w), clamp(h)]
}
/// Planar UVs for faces given as corner points (model order) with their
/// stored normals. All faces share one projection, so neighbours meet with
/// continuous UVs. Texels are square: the extent is fitted with one scale.
pub fn planar(faces: &[PanelFace], plane: Plane, fit: Fit) -> Result<Planar> {
    let degenerate = || invalid("Degenerate panel cannot be auto-mapped");
    if faces.is_empty() || faces.iter().any(|(p, _)| p.len() < 3) {
        return Err(degenerate());
    }
    let (normal, dir) = match plane {
        Plane::Auto => {
            let mut n = [0i64; 3];
            let mut longest = (0i64, [0i64; 3]);
            for (points, stored) in faces {
                let f = stored.or_else(|| face_normal(points)).unwrap_or([0; 3]);
                for k in 0..3 {
                    n[k] += f[k] as i64;
                }
                for i in 0..points.len() {
                    let (a, b) = (points[i], points[(i + 1) % points.len()]);
                    let d: [i64; 3] = core::array::from_fn(|k| b[k] as i64 - a[k] as i64);
                    let l = d.iter().map(|x| x * x).sum::<i64>();
                    if l > longest.0 {
                        longest = (l, d);
                    }
                }
            }
            (n, longest.1)
        }
        Plane::Top => ([0, 0, 1], [1, 0, 0]),
        Plane::Side => ([1, 0, 0], [0, 1, 0]),
        Plane::Front => ([0, -1, 0], [1, 0, 0]),
    };
    let (mut u, mut v) = basis(normal, dir).ok_or_else(degenerate)?;
    let project = |u: [i64; 3], v: [i64; 3]| -> Vec<Vec<[i128; 2]>> {
        faces
            .iter()
            .map(|(points, _)| {
                points
                    .iter()
                    .map(|p| {
                        let p = p.map(|x| x as i64);
                        [dot(p, u), dot(p, v)]
                    })
                    .collect()
            })
            .collect()
    };
    let extents = |st: &[Vec<[i128; 2]>]| -> ([i128; 2], [i128; 2]) {
        let mut lo = [i128::MAX; 2];
        let mut hi = [i128::MIN; 2];
        for p in st.iter().flatten() {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, [hi[0] - lo[0], hi[1] - lo[1]])
    };
    let mut st = project(u, v);
    let (_, e) = extents(&st);
    // Orientation follows the panel: the longer extent goes to the longer
    // side of the sheet (U for generated sheets). Turning the basis a
    // quarter keeps it right-handed, so the image is never mirrored.
    let wide = match fit {
        Fit::Size([w, h]) => w >= h,
        Fit::Density(_) => true,
    };
    if plane == Plane::Auto && (e[0] >= e[1]) != wide {
        (u, v) = (v, u.map(|x| -x));
        st = project(u, v);
    }
    let (lo, e) = extents(&st);
    if e[0] == 0 && e[1] == 0 {
        return Err(degenerate());
    }
    let size = match fit {
        Fit::Size(s) => s,
        Fit::Density(d) => sheet_size(e[0], e[1], d.max(1)),
    };
    if size.iter().any(|s| *s == 0 || *s > 65536) {
        return Err(invalid("Texture size must be 1 to 65536 pixels per side"));
    }
    let (w, h) = (size[0] as i128, size[1] as i128);
    // One scale for both axes, the tighter of W/Es and H/Et; the far edge
    // lands on the last texel.
    let (num, den) = if e[1] == 0 || (e[0] != 0 && w * e[1] <= h * e[0]) {
        (w, e[0])
    } else {
        (h, e[1])
    };
    let map = |x: i128, last: i128| -> i32 { ((2 * x * num + den) / (2 * den)).min(last) as i32 };
    let uv = st
        .iter()
        .map(|f| {
            f.iter()
                .map(|p| [map(p[0] - lo[0], w - 1), map(p[1] - lo[1], h - 1)])
                .collect()
        })
        .collect();
    Ok(Planar { uv, size })
}
/// Average texel density of a model's textured faces (Q16 texels per source
/// unit): UV edge length over model edge length, summed over every edge of
/// every face with a named texture and one UV per corner.
pub fn atlas_density(model: &Model) -> Option<u32> {
    let (mut texels, mut units) = (0u128, 0u128);
    for f in model.faces.iter().take(65536) {
        if f.sub & 4 == 0 || f.texture.is_empty() || f.uv.len() != f.indices.len() {
            continue;
        }
        let n = f.indices.len();
        for k in 0..n {
            let (a, b) = (f.indices[k], f.indices[(k + 1) % n]);
            let (Some(p), Some(q)) = (model.vertices.get(a), model.vertices.get(b)) else {
                continue;
            };
            let d: u128 = (0..3)
                .map(|i| (p.point[i] as i128 - q.point[i] as i128).pow(2) as u128)
                .sum();
            let (s, t) = (f.uv[k], f.uv[(k + 1) % n]);
            let e: u128 = (0..2)
                .map(|i| (s[i] as i128 - t[i] as i128).pow(2) as u128)
                .sum();
            texels += isqrt(e << 16);
            units += isqrt(d << 16);
        }
    }
    if units == 0 || texels == 0 {
        return None;
    }
    Some((texels * 65536 / units).clamp(1 << 10, 256 << 16) as u32)
}

/// Upper-case texture name an E2 selector names; empty for E0.
fn selector_texture(bytes: &[u8]) -> String {
    if bytes.first() != Some(&0xe2) {
        return String::new();
    }
    let name = bytes.get(2..16).unwrap_or(&[]);
    let name = name.split(|b| *b == 0).next().unwrap_or(&[]);
    name.iter()
        .map(|b| b.to_ascii_uppercase() as char)
        .collect()
}
/// The same-size stub `stub_out` writes over `[a, e)` jumping to `to`.
fn stub_bytes(a: usize, e: usize, to: usize) -> Result<Vec<u8>> {
    if e < a + 4 {
        return Err(invalid("Record too short for a jump stub"));
    }
    let mut out = alloc::vec![0x1e; e - a];
    out[..4].copy_from_slice(&jump(a, to)?);
    if e - a >= 8 {
        let n = out.len();
        out[n - 4..].copy_from_slice(&jump(e - 4, e)?);
    }
    Ok(out)
}
fn record(g: &Geometry, at: usize) -> Option<&crate::shape_code::Record> {
    g.inventory.starting_at(at).map(|i| &g.inventory.records[i])
}
/// The face record (decoded) starting at a CODE offset.
fn face_code(g: &Geometry, at: usize) -> Option<usize> {
    g.face_at(g.inventory.code_start + at)
}
fn sorted_slots(g: &Geometry, at: usize) -> Option<Vec<usize>> {
    let mut s = g.faces[face_code(g, at)?].slots.clone();
    s.sort_unstable();
    Some(s)
}
/// Recognise the continuation whose E2 record starts at CODE offset `t`.
fn recognise(g: &Geometry, t: usize) -> Option<Assignment> {
    let inv = &g.inventory;
    let i = inv.starting_at(t)?;
    let r = &inv.records[i];
    if r.kind != Kind::Sh(0xe2) || r.len != 16 {
        return None;
    }
    let mut j = i + 1;
    let mut copies = Vec::new();
    while let Some(c) = inv.records.get(j).filter(|c| c.kind == Kind::Sh(0xfc)) {
        copies.push(c.offset);
        j += 1;
        if copies.len() > 4096 {
            return None;
        }
    }
    let restore = inv.records.get(j)?;
    if copies.is_empty()
        || !matches!(
            (restore.kind, restore.len),
            (Kind::Sh(0xe2), 16) | (Kind::Sh(0xe0), 4)
        )
    {
        return None;
    }
    let back = inv.records.get(j + 1)?;
    if back.kind != Kind::Sh(0x48) {
        return None;
    }
    let ret = back.pointers.iter().find_map(|p| match p.target {
        crate::shape_code::Target::Code(x) => Some(x),
        _ => None,
    })?;
    let mut originals = Vec::new();
    let mut size = 0;
    for k in 0..copies.len() {
        let o = inv.records.get(j + 2 + k)?;
        if o.kind != Kind::Sh(0xfc) {
            return None;
        }
        originals.push(o.offset);
        size += o.len;
    }
    let end = inv.records[j + 1 + copies.len()].end();
    let mut a = ret.checked_sub(size)?;
    let mut sites = Vec::new();
    for (k, o) in originals.iter().enumerate() {
        let len = record(g, *o)?.len;
        let e = a + len;
        // The sites lie outside the continuation, before or after it.
        if a < end && t < e {
            return None;
        }
        let to = if k == 0 { t } else { e };
        if g.code.get(a..e)? != stub_bytes(a, e, to).ok()?.as_slice() {
            return None;
        }
        // A copy draws the same corners as its original, in any order.
        if sorted_slots(g, copies[k])? != sorted_slots(g, *o)? {
            return None;
        }
        sites.push((a, e));
        a = e;
    }
    Some(Assignment {
        texture: selector_texture(g.selector(t)),
        start: t,
        end,
        sites,
        copies,
        originals,
        restore: (restore.offset, restore.end()),
        back: back.offset,
    })
}
/// Every Hangar texture-assignment continuation in an analysed shape.
pub fn assignments(g: &Geometry) -> Vec<Assignment> {
    g.inventory
        .records
        .iter()
        .filter(|r| r.kind == Kind::Sh(0xe2))
        .filter_map(|r| recognise(g, r.offset))
        .collect()
}
/// File offsets of faces drawn from Hangar texture assignments, with the
/// texture each one selects.
pub fn assigned_faces(source: &[u8]) -> Result<BTreeMap<usize, String>> {
    let g = Geometry::parse(source)?;
    let cs = g.inventory.code_start;
    let mut out = BTreeMap::new();
    for a in assignments(&g) {
        for c in &a.copies {
            out.insert(cs + c, a.texture.clone());
        }
    }
    Ok(out)
}

/// A face drawn from a continuation after the edit.
struct Draw {
    site: (usize, usize),
    copy: Vec<u8>,
    original: Vec<u8>,
    texture: String,
    restore: Vec<u8>,
    /// CODE offset where the face is drawn now.
    old: usize,
    request: Option<usize>,
}
/// A site that gets a record back.
struct Put {
    site: (usize, usize),
    bytes: Vec<u8>,
    texture: String,
    old: usize,
    request: Option<usize>,
}
/// Where a requested face is drawn now.
enum Origin {
    /// A face drawn where it is stored (face index).
    Body(usize),
    /// Copy `k` of recognised assignment `a`.
    Copy(usize, usize),
}
fn origin(
    g: &Geometry,
    copies: &BTreeMap<usize, (usize, usize)>,
    offset: usize,
) -> Result<(usize, Origin)> {
    let cs = g.inventory.code_start;
    let i = g
        .face_at(offset)
        .ok_or_else(|| format!("No FC face record starts at {offset:X}"))?;
    let code = offset - cs;
    if let Some((a, k)) = copies.get(&code) {
        return Ok((i, Origin::Copy(*a, *k)));
    }
    if let Some(e) = g.face_refusal(i) {
        return Err(format!("Face at {offset:X}: {e}"));
    }
    if g.inventory.bindings.iter().any(|b| b.target == code) {
        return Err(format!(
            "Face at {offset:X}: a part stub resumes drawing at this face, so it cannot be moved"
        ));
    }
    Ok((i, Origin::Body(i)))
}
fn unproved(offset: usize, why: &str) -> String {
    format!("Face at {offset:X}: Hangar cannot prove which texture draws it: {why}")
}
/// Why faces (file offsets) cannot take another texture, if one cannot: it
/// is not a face record, its bytes are guarded, a part stub resumes at it,
/// or its texture state cannot be proved. Clone, Assign and Remap refuse
/// such a selection with this reason, so dialogs can say so before Apply.
pub fn assign_refusal(source: &[u8], faces: &[usize]) -> Option<String> {
    let g = match Geometry::parse(source) {
        Ok(g) => g,
        Err(e) => return Some(e),
    };
    let copies = copy_index(&assignments(&g));
    for offset in faces.iter().take(4096) {
        match origin(&g, &copies, *offset) {
            Err(e) => return Some(e),
            Ok((_, Origin::Body(i))) => {
                if let Err(e) = g.material(i) {
                    return Some(unproved(*offset, &e));
                }
            }
            Ok(_) => {}
        }
    }
    None
}
fn copy_index(found: &[Assignment]) -> BTreeMap<usize, (usize, usize)> {
    let mut out = BTreeMap::new();
    for (a, x) in found.iter().enumerate() {
        for (k, c) in x.copies.iter().enumerate() {
            out.insert(*c, (a, k));
        }
    }
    out
}
fn bytes_at(g: &Geometry, at: usize) -> Result<Vec<u8>> {
    let len = record(g, at).ok_or("Record lost")?.len;
    Ok(g.code.get(at..at + len).ok_or("Record truncated")?.to_vec())
}
/// The faces of `a` other than those in `taken`, drawn as they are.
fn survivors(g: &Geometry, a: &Assignment, taken: &BTreeSet<usize>) -> Result<Vec<Draw>> {
    let restore = g.code[a.restore.0..a.restore.1].to_vec();
    let mut out = Vec::new();
    for k in 0..a.copies.len() {
        if taken.contains(&k) {
            continue;
        }
        out.push(Draw {
            site: a.sites[k],
            copy: bytes_at(g, a.copies[k])?,
            original: bytes_at(g, a.originals[k])?,
            texture: a.texture.clone(),
            restore: restore.clone(),
            old: a.copies[k],
            request: None,
        });
    }
    Ok(out)
}
/// `record` with new content flags and UVs, byte or word wide.
fn with_uvs(record: &[u8], content: u8, uv: &[[i32; 2]], word: bool) -> Result<Vec<u8>> {
    let l = face_layout(record, 0)?;
    if uv.len() != l.count {
        return Err(invalid("One UV per corner is required"));
    }
    if uv.iter().flatten().any(|v| !(0..=65535).contains(v)) {
        return Err(invalid("A UV falls outside 0..65535"));
    }
    let word = word || uv.iter().flatten().any(|v| *v > 255);
    let head = l.uv.map_or(l.end, |(p, _)| p);
    let mut out = record[..head].to_vec();
    out[1] = content;
    out[2] = (out[2] & !1) | u8::from(!word);
    for p in uv {
        for v in p {
            if word {
                out.extend((*v as u16).to_le_bytes());
            } else {
                out.push(*v as u8);
            }
        }
    }
    Ok(out)
}
/// Stored UVs and whether they are words.
fn stored_uvs(record: &[u8]) -> Result<Option<(Vec<[i32; 2]>, bool)>> {
    let l = face_layout(record, 0)?;
    let Some((p, bytes)) = l.uv else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for k in 0..l.count {
        out.push(if bytes {
            [record[p + 2 * k] as i32, record[p + 2 * k + 1] as i32]
        } else {
            let w = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]) as i32;
            [w(p + 4 * k), w(p + 4 * k + 2)]
        });
    }
    Ok(Some((out, !bytes)))
}
/// Textured content for a flat face, as generated panels use it.
fn textured_content(record: &[u8], offset: usize) -> Result<u8> {
    let c = record[1];
    if c & !0x67 != 0 {
        return Err(format!(
            "Face at {offset:X}: this polygon shading subtype is not supported for automatic texturing"
        ));
    }
    if record[4] != 0 {
        return Err(format!("Face at {offset:X} uses a special color encoding"));
    }
    Ok((c & 0x60) | 4)
}
/// The original record to put back for a copy that later edits may have
/// changed (moved corners rewrite its normal and centre; a flip reverses its
/// corners): the original's content, layout and UVs over the copy's
/// geometry, UVs following their corners.
fn restored_record(copy: &[u8], original: &[u8]) -> Result<Vec<u8>> {
    let (lc, lo) = (face_layout(copy, 0)?, face_layout(original, 0)?);
    let geometry = |l: &crate::shape_geometry::FaceLayout, r: &[u8]| -> Vec<u8> {
        r[5..l.uv.map_or(l.end, |(p, _)| p)].to_vec()
    };
    if geometry(&lc, copy) == geometry(&lo, original) {
        return Ok(original.to_vec());
    }
    if lc.count != lo.count
        || lc.wide != lo.wide
        || lc.normal.is_some() != lo.normal.is_some()
        || lc.centre.map(|c| c.1) != lo.centre.map(|c| c.1)
    {
        return Err(invalid(
            "The assigned face was reshaped; its original record no longer fits",
        ));
    }
    let mut out = original.to_vec();
    // Normal and centre from the copy (same widths).
    out[5..lo.index - 1].copy_from_slice(&copy[5..lc.index - 1]);
    let w = if lo.wide { 2 } else { 1 };
    let slot = |r: &[u8], l: &crate::shape_geometry::FaceLayout, k: usize| -> Vec<u8> {
        r[l.index + w * k..l.index + w * (k + 1)].to_vec()
    };
    let uv_w = lo.uv.map_or(0, |(_, b)| if b { 2 } else { 4 });
    for k in 0..lc.count {
        let s = slot(copy, &lc, k);
        let q = (0..lo.count)
            .find(|q| slot(original, &lo, *q) == s)
            .ok_or("The assigned face's corners changed; its original record no longer fits")?;
        out[lo.index + w * k..lo.index + w * (k + 1)].copy_from_slice(&s);
        if let Some((p, _)) = lo.uv {
            let from = original[p + uv_w * q..p + uv_w * (q + 1)].to_vec();
            out[p + uv_w * k..p + uv_w * (k + 1)].copy_from_slice(&from);
        }
    }
    Ok(out)
}
fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}
/// Lay out the edit: dissolve `dissolved` assignments (1E fill), put
/// records back at `puts`, and draw every `draws` face from a continuation,
/// one per contiguous run with the same texture and restore selector.
/// Returns the shape and, per moved face, its old CODE offset mapped to its
/// new CODE offset and expected texture.
#[allow(clippy::type_complexity)]
fn rebuild(
    source: &[u8],
    g: &Geometry,
    dissolved: &[&Assignment],
    mut draws: Vec<Draw>,
    puts: Vec<Put>,
) -> Result<(
    Vec<u8>,
    BTreeMap<usize, (usize, String)>,
    BTreeMap<usize, usize>,
)> {
    let mut rewritten: Vec<(usize, usize)> = Vec::new();
    for a in dissolved {
        rewritten.push((a.start, a.end));
    }
    let spans: Vec<(usize, usize)> = draws
        .iter()
        .map(|d| d.site)
        .chain(puts.iter().map(|p| p.site))
        .collect();
    for (k, s) in spans.iter().enumerate() {
        if spans[..k]
            .iter()
            .chain(&rewritten)
            .any(|x| overlaps(*x, *s))
        {
            return Err(invalid("Two faces of this edit share bytes"));
        }
    }
    rewritten.extend(&spans);
    let inside = |at: usize| rewritten.iter().any(|(a, e)| (*a..*e).contains(&at));
    for a in dissolved {
        let stub = a.sites[0].0 + 2;
        for (t, field) in &g.targets {
            if (a.start..a.end).contains(t) && *field != stub && !(a.start..a.end).contains(field) {
                return Err(format!(
                    "CODE+{field:X} jumps into the texture assignment at CODE+{:X}; it cannot be rebuilt",
                    a.start
                ));
            }
        }
        for (f, _) in &g.fields {
            if (a.start..a.end).contains(f) && *f != a.back + 2 {
                return Err(format!(
                    "The texture assignment at CODE+{:X} carries a pointer at CODE+{f:X}; it cannot be rebuilt",
                    a.start
                ));
            }
        }
        if g.data.iter().any(|(t, w)| *t < a.end && t + w > a.start) {
            return Err(format!(
                "Native code addresses the texture assignment at CODE+{:X}",
                a.start
            ));
        }
    }
    let mut payload = g.code.clone();
    let mut free: Vec<(usize, usize)> = Vec::new();
    for a in dissolved {
        payload[a.start..a.end].fill(0x1e);
        free.push((a.start, a.end));
    }
    let mut moves = BTreeMap::new();
    let mut placed = BTreeMap::new();
    for p in &puts {
        if p.bytes.len() != p.site.1 - p.site.0 {
            return Err(invalid("A restored face no longer fits its site"));
        }
        payload[p.site.0..p.site.1].copy_from_slice(&p.bytes);
        moves.insert(p.old, (p.site.0, p.texture.clone()));
        if let Some(r) = p.request {
            placed.insert(r, p.site.0);
        }
    }
    // Runs: contiguous sites, one texture and restore, no entry between.
    draws.sort_unstable_by_key(|d| d.site.0);
    let entered = |at: usize| g.targets.iter().any(|(t, f)| *t == at && !inside(*f));
    let mut groups: Vec<Vec<Draw>> = Vec::new();
    for d in draws {
        match groups.last_mut() {
            Some(run)
                if run.last().is_some_and(|p| {
                    p.site.1 == d.site.0 && p.texture == d.texture && p.restore == d.restore
                }) && !entered(d.site.0) =>
            {
                run.push(d)
            }
            _ => groups.push(alloc::vec![d]),
        }
    }
    let mut ext = Vec::new();
    let mut ext_start = None;
    for run in &groups {
        let size = 16
            + run
                .iter()
                .map(|d| d.copy.len() + d.original.len())
                .sum::<usize>()
            + run[0].restore.len()
            + 4;
        let slot = free.iter_mut().find(|(a, e)| e - a >= size);
        let at = match slot {
            Some((a, _)) => {
                let at = *a;
                *a += size;
                at
            }
            None => {
                if g.inventory.end_marker.is_none() {
                    return Err(invalid(
                        "Appending a texture assignment needs the native end marker and import tail",
                    ));
                }
                let s = match ext_start {
                    Some(s) => s,
                    None => *ext_start.insert(continuation_start(source)?),
                };
                s + ext.len()
            }
        };
        let name = &run[0].texture;
        if name.len() > 13 {
            return Err(invalid("Texture names are at most 13 characters"));
        }
        let mut b = alloc::vec![0xe2, 0];
        let mut field = [0u8; 14];
        field[..name.len()].copy_from_slice(name.as_bytes());
        b.extend(field);
        for d in run {
            moves.insert(d.old, (at + b.len(), d.texture.clone()));
            if let Some(r) = d.request {
                placed.insert(r, at + b.len());
            }
            b.extend(&d.copy);
        }
        b.extend(&run[0].restore);
        let end = run.last().map_or(0, |d| d.site.1);
        b.extend(jump(at + b.len(), end)?);
        for d in run {
            b.extend(&d.original);
        }
        debug_assert_eq!(b.len(), size);
        if ext_start.is_some_and(|s| at >= s) {
            ext.extend(b);
        } else {
            payload[at..at + size].copy_from_slice(&b);
        }
        for (k, d) in run.iter().enumerate() {
            let to = if k == 0 { at } else { d.site.1 };
            let stub = stub_bytes(d.site.0, d.site.1, to)?;
            payload[d.site.0..d.site.1].copy_from_slice(&stub);
        }
    }
    let out = if ext.is_empty() {
        g.install(source, &payload)?
    } else {
        append_continuation(source, payload, ext)?
    };
    Ok((out, moves, placed))
}
type Key = (
    usize,
    String,
    Vec<[i32; 3]>,
    Option<usize>,
    Option<(Vec<[i32; 2]>, u8, u8)>,
);
fn keys(m: &Model, cs: usize, map: &dyn Fn(usize, &str) -> (usize, String, bool)) -> Vec<Key> {
    let mut out: Vec<Key> = m
        .faces
        .iter()
        .map(|f| {
            let (at, texture, moved) = map(f.offset - cs, &f.texture);
            let points = f
                .indices
                .iter()
                .map(|i| m.vertices.get(*i).map_or([0; 3], |v| v.point))
                .collect();
            let part = f.part.and_then(|p| m.parts.get(p)).map(|p| p.offset - cs);
            let detail = (!moved).then(|| (f.uv.clone(), f.sub, f.color));
            (at, texture.to_ascii_uppercase(), points, part, detail)
        })
        .collect();
    out.sort_unstable();
    out
}
/// Re-parse the edit: structure, recognised continuations and every drawn
/// face in the neutral pose and the poses that draw the moved faces.
fn verify(
    source: &[u8],
    g: &Geometry,
    out: &[u8],
    moves: &BTreeMap<usize, (usize, String)>,
    drawn: &[(usize, (usize, usize), String)],
) -> Result<usize> {
    let after = verify_structure(g, source, out)?;
    let found = assignments(&after);
    for (at, site, texture) in drawn {
        let ok = found.iter().any(|a| {
            &a.texture == texture
                && a.copies
                    .iter()
                    .zip(&a.sites)
                    .any(|(c, s)| c == at && s == site)
        });
        if !ok {
            return Err(invalid(
                "A texture assignment did not re-parse as a Hangar continuation",
            ));
        }
    }
    let (cs, cs2) = (g.inventory.code_start, after.inventory.code_start);
    let mut targets = BTreeSet::new();
    targets.extend(moves.values().map(|(n, _)| *n));
    let mut poses: Vec<Pose> = alloc::vec![Pose::new()];
    for old in moves.keys() {
        if let Some((pose, _)) = pose_drawing(source, &g.inventory, cs + old) {
            if !poses.contains(&pose) && poses.len() < 16 {
                poses.push(pose);
            }
        }
    }
    for pose in &poses {
        let Ok(before) = Model::with_pose(source, pose) else {
            continue;
        };
        let now = Model::with_pose(out, pose)
            .map_err(|e| format!("The edited shape no longer parses in a checked pose: {e}"))?;
        if before.faces.len() != now.faces.len() {
            return Err(invalid(
                "The texture edit changed the number of drawn faces",
            ));
        }
        let a = keys(&before, cs, &|at, t| match moves.get(&at) {
            Some((n, texture)) => (*n, texture.clone(), true),
            None => (at, t.into(), false),
        });
        let b = keys(&now, cs2, &|at, t| (at, t.into(), targets.contains(&at)));
        if a != b {
            return Err(invalid(
                "The texture edit changed another face, or a moved face did not draw the expected texture",
            ));
        }
    }
    Ok(cs2)
}
/// Draw the given faces (file offsets) from texture `name`, a PIC. Each
/// contiguous run of them is detoured through one continuation; faces that
/// already come from a Hangar assignment move to the new one, keeping their
/// stored originals, so the assignment never nests. With `Keep`, faces
/// already on `name` are left as they are.
pub fn assign_texture(
    source: &[u8],
    faces: &[usize],
    name: &str,
    mode: UvMode,
) -> Result<Assigned> {
    assign(source, faces, name, Uvs::Mode(mode))
}
/// `assign_texture` with the UVs given: one list per requested face, one UV
/// per corner in record order (PIC pixels, V as stored). Byte UVs widen to
/// words when a value exceeds 255; untextured faces become textured as for
/// Project.
pub fn assign_texture_uvs(
    source: &[u8],
    faces: &[usize],
    name: &str,
    uvs: &[Vec<[i32; 2]>],
) -> Result<Assigned> {
    if uvs.len() != faces.len() {
        return Err(invalid("One UV list per face is required"));
    }
    assign(source, faces, name, Uvs::Given(uvs))
}
/// Where an assignment's UVs come from.
#[derive(Clone, Copy)]
enum Uvs<'a> {
    Mode(UvMode),
    Given(&'a [Vec<[i32; 2]>]),
}
fn assign(source: &[u8], faces: &[usize], name: &str, uvs: Uvs) -> Result<Assigned> {
    let name = name.trim().to_ascii_uppercase();
    validate_name(&name)?;
    if !name.ends_with(".PIC") {
        return Err(invalid("Assign a PIC texture"));
    }
    if name.len() > 13 {
        return Err(invalid("Texture names are at most 13 characters"));
    }
    if faces.is_empty() {
        return Err(invalid("Select faces to texture"));
    }
    if faces.len() > 4096 {
        return Err(invalid("Assign at most 4096 faces at a time"));
    }
    let g = Geometry::parse(source)?;
    let cs = g.inventory.code_start;
    let found = assignments(&g);
    let copies = copy_index(&found);
    struct Item {
        request: usize,
        face: usize,
        site: (usize, usize),
        record: Vec<u8>,
        original: Vec<u8>,
        restore: Vec<u8>,
        texture: String,
        old: usize,
        from: Option<(usize, usize)>,
    }
    let mut items: Vec<Item> = Vec::new();
    let mut seen = BTreeSet::new();
    for (request, offset) in faces.iter().enumerate() {
        if !seen.insert(*offset) {
            continue;
        }
        let (face, from) = origin(&g, &copies, *offset)?;
        let old = offset - cs;
        let item = match from {
            Origin::Body(i) => {
                let site = g.span(i);
                let at = g.material(i).map_err(|e| unproved(*offset, &e))?;
                let restore = g.selector(at).to_vec();
                let record = g.code[site.0..site.1].to_vec();
                Item {
                    request,
                    face,
                    site,
                    original: record.clone(),
                    record,
                    texture: selector_texture(&restore),
                    restore,
                    old,
                    from: None,
                }
            }
            Origin::Copy(a, k) => {
                let x = &found[a];
                Item {
                    request,
                    face,
                    site: x.sites[k],
                    record: bytes_at(&g, x.copies[k])?,
                    original: bytes_at(&g, x.originals[k])?,
                    restore: g.code[x.restore.0..x.restore.1].to_vec(),
                    texture: x.texture.clone(),
                    old,
                    from: Some((a, k)),
                }
            }
        };
        items.push(item);
    }
    if matches!(uvs, Uvs::Mode(UvMode::Keep)) {
        items.retain(|i| i.texture != name);
        if items.is_empty() {
            return Ok(Assigned {
                shape: source.to_vec(),
                faces: faces.to_vec(),
            });
        }
    }
    // New records.
    let mut records = Vec::new();
    let mode = match uvs {
        Uvs::Mode(mode) => Some(mode),
        Uvs::Given(given) => {
            for it in &items {
                let (content, word) = match stored_uvs(&it.record)? {
                    Some((_, word)) => (it.record[1], word),
                    None => (textured_content(&it.record, cs + it.old)?, false),
                };
                records.push(with_uvs(&it.record, content, &given[it.request], word)?);
            }
            None
        }
    };
    match mode {
        None => {}
        Some(UvMode::Keep | UvMode::Scale { .. }) => {
            for it in &items {
                let (uv, word) = stored_uvs(&it.record)?.ok_or_else(|| {
                    format!(
                        "Face at {:X} is untextured; Keep and Scale need stored UVs, choose Project",
                        cs + it.old
                    )
                })?;
                let uv: Vec<[i32; 2]> = match mode {
                    Some(UvMode::Scale { from, to }) => {
                        if from.contains(&0) || to.contains(&0) {
                            return Err(invalid("Texture sizes must be positive"));
                        }
                        uv.iter()
                            .map(|p| {
                                core::array::from_fn(|k| {
                                    ((p[k] as u64 * to[k] as u64 + from[k] as u64 / 2)
                                        / from[k] as u64)
                                        .min(u32::MAX as u64)
                                        as i32
                                })
                            })
                            .collect()
                    }
                    _ => uv,
                };
                records.push(with_uvs(&it.record, it.record[1], &uv, word)?);
            }
        }
        Some(UvMode::Project { plane, size }) => {
            let mut frames: BTreeMap<Frame, Vec<usize>> = BTreeMap::new();
            for (k, it) in items.iter().enumerate() {
                frames.entry(g.faces[it.face].frame).or_default().push(k);
            }
            records = alloc::vec![Vec::new(); items.len()];
            for list in frames.values() {
                let mut group = Vec::new();
                for k in list {
                    let f = items[*k].face;
                    group.push((g.face_points(f)?, g.faces[f].normal));
                }
                let p = planar(&group, plane, Fit::Size(size))?;
                for (k, uv) in list.iter().zip(p.uv) {
                    let it = &items[*k];
                    let (content, word) = match stored_uvs(&it.record)? {
                        Some((_, word)) => (it.record[1], word),
                        None => (textured_content(&it.record, cs + it.old)?, false),
                    };
                    records[*k] = with_uvs(&it.record, content, &uv, word)?;
                }
            }
        }
    }
    let mut touched: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut draws = Vec::new();
    for (it, copy) in items.iter().zip(records) {
        if let Some((a, k)) = it.from {
            touched.entry(a).or_default().insert(k);
        }
        draws.push(Draw {
            site: it.site,
            copy,
            original: it.original.clone(),
            texture: name.clone(),
            restore: it.restore.clone(),
            old: it.old,
            request: Some(it.request),
        });
    }
    for (a, taken) in &touched {
        draws.extend(survivors(&g, &found[*a], taken)?);
    }
    let dissolved: Vec<&Assignment> = touched.keys().map(|a| &found[*a]).collect();
    finish(source, &g, &dissolved, draws, Vec::new(), faces)
}
fn finish(
    source: &[u8],
    g: &Geometry,
    dissolved: &[&Assignment],
    draws: Vec<Draw>,
    puts: Vec<Put>,
    faces: &[usize],
) -> Result<Assigned> {
    let drawn_sites: Vec<((usize, usize), usize, String)> = draws
        .iter()
        .map(|d| (d.site, d.old, d.texture.clone()))
        .collect();
    let (out, moves, placed) = rebuild(source, g, dissolved, draws, puts)?;
    let drawn: Vec<(usize, (usize, usize), String)> = drawn_sites
        .into_iter()
        .filter_map(|(site, old, t)| moves.get(&old).map(|(n, _)| (*n, site, t)))
        .collect();
    let cs2 = verify(source, g, &out, &moves, &drawn)?;
    let cs = g.inventory.code_start;
    // Unchanged requests (Keep on the same texture) keep their offset.
    let new: Vec<usize> = faces
        .iter()
        .enumerate()
        .map(|(r, o)| {
            placed.get(&r).map_or_else(
                || {
                    let code = o - cs;
                    moves.get(&code).map_or(cs2 + code, |(n, _)| cs2 + n)
                },
                |n| cs2 + n,
            )
        })
        .collect();
    Ok(Assigned {
        shape: out,
        faces: new,
    })
}
/// Return faces drawn from Hangar texture assignments to the shape's own
/// texture state: each original record goes back to its site (keeping later
/// moves and flips of the copy), and the rest of its continuation is rebuilt.
/// Faces on retail texture state are refused.
pub fn restore_texture_assignment(source: &[u8], faces: &[usize]) -> Result<Assigned> {
    if faces.is_empty() {
        return Err(invalid("Select faces to restore"));
    }
    let g = Geometry::parse(source)?;
    let cs = g.inventory.code_start;
    let found = assignments(&g);
    let copies = copy_index(&found);
    let mut touched: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut puts = Vec::new();
    let mut seen = BTreeSet::new();
    let mut missing = None;
    for (request, offset) in faces.iter().enumerate() {
        if !seen.insert(*offset) {
            continue;
        }
        let Some((a, k)) = offset.checked_sub(cs).and_then(|c| copies.get(&c)) else {
            missing.get_or_insert(*offset);
            continue;
        };
        let x = &found[*a];
        touched.entry(*a).or_default().insert(*k);
        puts.push(Put {
            site: x.sites[*k],
            bytes: restored_record(
                &bytes_at(&g, x.copies[*k])?,
                &bytes_at(&g, x.originals[*k])?,
            )?,
            texture: selector_texture(&g.code[x.restore.0..x.restore.1]),
            old: x.copies[*k],
            request: Some(request),
        });
    }
    match missing {
        Some(_) if puts.is_empty() => {
            return Err(invalid("No Hangar texture assignment to remove"))
        }
        Some(o) => {
            return Err(format!(
                "Face at {o:X}: No Hangar texture assignment to remove"
            ))
        }
        None => {}
    }
    let mut draws = Vec::new();
    for (a, taken) in &touched {
        draws.extend(survivors(&g, &found[*a], taken)?);
    }
    let dissolved: Vec<&Assignment> = touched.keys().map(|a| &found[*a]).collect();
    finish(source, &g, &dissolved, draws, puts, faces)
}

#[cfg(test)]
#[path = "shape_texture_tests.rs"]
mod tests;
