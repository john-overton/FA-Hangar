//! First SH authoring layer: source-provenance edits and bounded continuations.
//! Body instruction RVAs stay fixed; the end/import tail is relocated explicitly.
#[path = "shape_layout.rs"]
mod layout;

use crate::{
    invalid,
    model::Model,
    shape_texture::{Fit, Plane, PANEL_MAX, PANEL_MIN},
    slice, u16_at, u32_at, Result,
};
use alloc::{collections::BTreeSet, vec::Vec};
#[derive(Debug)]
pub struct PanelTexture {
    pub shape: Vec<u8>,
    pub picture: Vec<u8>,
    /// Model face index of the first converted face.
    pub face: usize,
    /// Model face indices of every converted face (they keep their index).
    pub faces: Vec<usize>,
    /// Width and height of the panel's sub-rectangle at the sheet's left
    /// edge. The sheet is a retail texture: 256 wide and this tall.
    pub size: [u32; 2],
}
fn put32(bytes: &mut [u8], at: usize, value: usize) -> Result<()> {
    let value = u32::try_from(value).map_err(|_| invalid("Module field exceeds 32 bits"))?;
    bytes
        .get_mut(at..at + 4)
        .ok_or("Module header truncated")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn align(n: usize, a: usize) -> Result<usize> {
    n.checked_add(a - 1)
        .map(|n| n & !(a - 1))
        .ok_or_else(|| invalid("Alignment overflow"))
}
struct Code {
    header: usize,
    optional: usize,
    optional_size: usize,
    start: usize,
    len: usize,
    raw_size: usize,
    rva: usize,
    limit: usize,
    alignment: usize,
    section_alignment: usize,
}
fn code(source: &[u8]) -> Result<Code> {
    let p = u32_at(source, 60)?;
    let count = u16_at(source, p + 6)?;
    let size = u16_at(source, p + 20)?;
    let optional = p + 24;
    if count == 0 || count > 32 || size < 32 {
        return Err(invalid("Unsupported module section table"));
    }
    let mut found = None;
    let mut starts = Vec::new();
    let mut ranges = Vec::new();
    for i in 0..count {
        let h = optional + size + i * 40;
        let s = slice(source, h, 40)?;
        let raw = u32_at(s, 16)?;
        let at = u32_at(s, 20)?;
        let rva = u32_at(s, 12)?;
        slice(source, at, raw)?;
        if raw > 0
            && (at < optional + size + count * 40
                || ranges.iter().any(|(a, n)| at < *a + *n && *a < at + raw))
        {
            return Err(invalid("Overlapping module sections"));
        }
        ranges.push((at, raw));
        starts.push(rva);
        if &s[..4] == b"CODE" {
            if found.is_some() {
                return Err(invalid("Multiple CODE sections"));
            }
            if u32_at(s, 8)? > raw {
                return Err(invalid(
                    "CODE has uninitialized virtual data; a full layout writer is required",
                ));
            }
            found = Some((h, at, u32_at(s, 8)?.min(raw), raw, rva));
        }
    }
    let (header, start, len, raw_size, rva) = found.ok_or("No CODE section")?;
    let limit = starts
        .into_iter()
        .filter(|a| *a > rva)
        .min()
        .unwrap_or(u32::MAX as usize)
        .saturating_sub(rva);
    let alignment = if size >= 40 {
        u32_at(source, optional + 36)?
    } else {
        1
    };
    let section_alignment = if size >= 40 {
        u32_at(source, optional + 32)?
    } else {
        4096
    };
    if !alignment.is_power_of_two()
        || alignment > 65536
        || !section_alignment.is_power_of_two()
        || section_alignment > 65536
    {
        return Err(invalid("Unsupported module alignment"));
    }
    Ok(Code {
        header,
        optional,
        optional_size: size,
        start,
        len,
        raw_size,
        rva,
        limit,
        alignment,
        section_alignment,
    })
}
pub(crate) fn ensure_face_not_relocated(source: &[u8], start: usize, end: usize) -> Result<()> {
    let p = u32_at(source, 60)?;
    let count = u16_at(source, p + 6)?;
    let table = p + 24 + u16_at(source, p + 20)?;
    for i in 0..count {
        let s = slice(source, table + i * 40, 40)?;
        if &s[..6] != b".reloc" {
            continue;
        }
        let bytes = slice(source, u32_at(s, 20)?, u32_at(s, 8)?.min(u32_at(s, 16)?))?;
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at..].iter().all(|b| *b == 0) {
                break;
            }
            let page = u32_at(bytes, at)?;
            let size = u32_at(bytes, at + 4)?;
            if size < 8 || size % 2 != 0 {
                return Err(invalid("Malformed relocation block"));
            }
            let block = slice(bytes, at, size)?;
            for j in (8..size).step_by(2) {
                let value = u16_at(block, j)?;
                let target = page
                    .checked_add(value & 4095)
                    .ok_or("Relocation address overflow")?;
                if value >> 12 != 0 && target < end && target.saturating_add(4) > start {
                    return Err(invalid(
                        "Face overlaps a module relocation; structural rewrite refused",
                    ));
                }
                if value >> 12 == 3 {
                    let base = u32_at(source, p + 52)?;
                    for n in 0..count {
                        let h = slice(source, table + n * 40, 40)?;
                        let rva = u32_at(h, 12)?;
                        let size = u32_at(h, 8)?.min(u32_at(h, 16)?);
                        if let Some(offset) = target.checked_sub(rva) {
                            if offset.checked_add(4).is_some_and(|end| end <= size) {
                                let address = u32_at(source, u32_at(h, 20)? + offset)?;
                                if address
                                    .checked_sub(base)
                                    .is_some_and(|v| v > start && v < end)
                                {
                                    return Err(invalid("Native code references fields inside this face; a full record writer is required"));
                                }
                            }
                        }
                    }
                }
            }
            at += size;
        }
    }
    Ok(())
}
pub(crate) fn jump(from: usize, to: usize) -> Result<[u8; 4]> {
    let d = to as i64 - from as i64 - 4;
    let d = i16::try_from(d)
        .map_err(|_| invalid("Panel continuation exceeds the 16-bit SH jump reach"))?;
    let b = d.to_le_bytes();
    Ok([0x48, 0, b[0], b[1]])
}
/// CODE offset at which `append_continuation` places an extension: the end
/// marker when the module has the native tail, otherwise the end of CODE.
pub(crate) fn continuation_start(source: &[u8]) -> Result<usize> {
    let code = code(source)?;
    if repair_panel_layout(source)?.is_some() {
        return Err(invalid(
            "Repair the legacy generated-panel layout before adding geometry",
        ));
    }
    Ok(layout::tail(source, &code)?.map_or(code.len, |t| t.start))
}
/// Replace a record at `[at, end)` of a CODE payload with a relative jump to
/// `to`, 0x1E padding and, for records of 8 bytes or more, a closing jump to
/// `end`, so readers that walk the dead bytes keep later record boundaries.
pub(crate) fn stub_out(payload: &mut [u8], at: usize, end: usize, to: usize) -> Result<()> {
    if end < at + 4 || end > payload.len() {
        return Err(invalid("Record too short for a jump stub"));
    }
    payload[at..end].fill(0x1e);
    payload[at..at + 4].copy_from_slice(&jump(at, to)?);
    if end - at >= 8 {
        payload[end - 4..end].copy_from_slice(&jump(end - 4, end)?);
    }
    Ok(())
}
/// Install an edited CODE payload (same length as CODE) plus an extension that
/// was built to start at `continuation_start`. Body RVAs stay fixed; with the
/// native tail the end marker and import stubs move and relocations follow.
pub(crate) fn append_continuation(
    source: &[u8],
    mut payload: Vec<u8>,
    extension: Vec<u8>,
) -> Result<Vec<u8>> {
    let code = code(source)?;
    if payload.len() != code.len {
        return Err(invalid("Edited CODE payload changed size"));
    }
    let new_len = code.len + extension.len();
    if new_len > code.limit {
        return Err(invalid(
            "CODE has no virtual-address room for this continuation; relocation support is required",
        ));
    }
    let out = if let Some(tail) = layout::tail(source, &code)? {
        layout::append_before_tail(source, payload, extension, tail)?
    } else {
        payload.extend(extension);
        let mut out = source.to_vec();
        if new_len <= code.raw_size
            && source[code.start + code.len..code.start + new_len]
                .iter()
                .all(|b| *b == 0)
        {
            out[code.start..code.start + new_len].copy_from_slice(&payload);
        } else {
            let new_start = align(out.len(), code.alignment)?;
            out.resize(new_start, 0);
            out.extend(&payload);
            let raw = align(new_len, code.alignment)?;
            out.resize(new_start + raw, 0);
            put32(&mut out, code.header + 20, new_start)?;
            put32(&mut out, code.header + 16, raw)?;
            if code.optional_size >= 64 {
                let old = u32_at(source, code.optional + 4)?;
                put32(
                    &mut out,
                    code.optional + 4,
                    old.saturating_sub(code.raw_size) + raw,
                )?;
            }
        }
        put32(&mut out, code.header + 8, new_len)?;
        if code.optional_size >= 68 {
            let image = u32_at(source, code.optional + 56)?
                .max(align(code.rva + new_len, code.section_alignment)?);
            put32(&mut out, code.optional + 56, image)?;
            put32(&mut out, code.optional + 64, 0)?;
        }
        out
    };
    if out.len() > crate::archive::RESOURCE_LIMIT {
        return Err(invalid("Expanded shape exceeds resource limit"));
    }
    Ok(out)
}
/// Paintable flat-color face -> private PIC with planar UVs on the face's own
/// plane in a `size` square at its left edge (square texels, longer side
/// along U). The PIC is a retail texture, 256 wide and `size` tall. The old
/// face site becomes one relative jump; the new draw sequence returns to the
/// original successor. Code grows only within its existing virtual-address gap.
pub fn texture_panel(
    source: &[u8],
    face_index: usize,
    name: &str,
    size: usize,
) -> Result<PanelTexture> {
    if !(8..=256).contains(&size) || !size.is_power_of_two() {
        return Err(invalid(
            "Texture sheet size must be a power of two from 8 to 256",
        ));
    }
    panels(source, &[face_index], name, Fit::Size([size as u32; 2]))
}
/// Flat-color faces -> one private PIC sized from `density` (Q16 texels per
/// source unit, see `shape_texture::atlas_density`). The faces are projected
/// together onto their plane, the longer extent along U, with square texels;
/// each side is clamped to 8..256 with the aspect kept. The panel fills that
/// sub-rectangle at the left edge of a retail texture (256 wide, kind 0, row
/// table, no palette) of the panel's height, so FA's texture mapper can read
/// it; the rest of the sheet repeats the face color.
pub fn texture_panels(
    source: &[u8],
    faces: &[usize],
    name: &str,
    density: u32,
) -> Result<PanelTexture> {
    panels(source, faces, name, Fit::Density(density))
}
/// Flat faces that can share one generated sheet with model face `face`:
/// connected to it through shared stored vertices, with the same colour,
/// subtype and part, and coplanar with it (normals within about 2.5 degrees,
/// corners within one unit of its plane). At most `limit` faces, `face` first.
pub fn coplanar_panels(model: &Model, face: usize, limit: usize) -> Vec<usize> {
    let Some(f0) = model.faces.get(face) else {
        return Vec::new();
    };
    let points = |f: &crate::model::Face| -> Vec<[i32; 3]> {
        f.indices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.point))
            .collect()
    };
    let p0 = points(f0);
    let Some(n0) = crate::model::face_normal(&p0) else {
        return vec![face];
    };
    let unit = 32765i64 * 32765;
    let on_plane = |q: [i32; 3]| {
        (0..3)
            .map(|k| (q[k] as i64 - p0[0][k] as i64) * n0[k] as i64)
            .sum::<i64>()
            .abs()
            <= 32765
    };
    let fits = |f: &crate::model::Face| {
        f.sub & 4 == 0
            && f.sub == f0.sub
            && f.color == f0.color
            && f.part == f0.part
            && crate::model::face_normal(&points(f)).is_some_and(|n| {
                (0..3).map(|k| n[k] as i64 * n0[k] as i64).sum::<i64>() * 1000 >= unit * 999
            })
            && points(f).into_iter().all(on_plane)
    };
    let offsets = |f: &crate::model::Face| -> Vec<usize> {
        f.indices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.offset))
            .collect()
    };
    let mut group = vec![face];
    // Inserted one by one: collecting a set pulls in a stable sort's
    // large stack buffer.
    let mut shared = BTreeSet::new();
    shared.extend(offsets(f0));
    while group.len() < limit {
        let next = (0..model.faces.len()).find(|i| {
            !group.contains(i)
                && model.faces[*i].offset != f0.offset
                && fits(&model.faces[*i])
                && offsets(&model.faces[*i]).iter().any(|o| shared.contains(o))
        });
        let Some(i) = next else { break };
        shared.extend(offsets(&model.faces[i]));
        group.push(i);
    }
    group
}
fn panels(source: &[u8], faces: &[usize], name: &str, fit: Fit) -> Result<PanelTexture> {
    crate::archive::validate_name(name)?;
    if !name.ends_with(".PIC") {
        return Err(invalid("Panel texture must be a PIC"));
    }
    if faces.is_empty() || faces.len() > 256 {
        return Err(invalid("Convert 1 to 256 panels at a time"));
    }
    let repaired = repair_panel_layout(source)?;
    let source = repaired.as_ref().map_or(source, |r| r.shape.as_slice());
    let original = Model::parse(source)?;
    let mut group = Vec::new();
    for i in faces {
        let face = original.faces.get(*i).ok_or("Select a model face")?;
        let points = face
            .indices
            .iter()
            .map(|v| original.vertices.get(*v).map(|v| v.point))
            .collect::<Option<Vec<_>>>()
            .ok_or("Unresolved panel corner")?;
        group.push((points, face.normal));
    }
    let planar = crate::shape_texture::planar(&group, Plane::Auto, fit)?;
    let limits = PANEL_MIN..=PANEL_MAX;
    if !planar.size.iter().all(|s| limits.contains(s)) {
        return Err(invalid("Texture sheet sides must be 8 to 256 pixels"));
    }
    // The face's own palette byte is a game-palette index: retail textures
    // carry no palette, and FA draws them with the game palette.
    let color = original.faces[faces[0]].color;
    let rows = planar.size[1] as usize;
    let mut shape = source.to_vec();
    for (i, uv) in faces.iter().zip(&planar.uv) {
        shape = convert_face(&shape, *i, name, uv)?;
    }
    Ok(PanelTexture {
        shape,
        picture: crate::picture::retail_texture(
            rows,
            &vec![color; crate::picture::TEXTURE_WIDTH * rows],
        )?,
        face: faces[0],
        faces: faces.to_vec(),
        size: planar.size,
    })
}
/// One flat face -> texture `name` with these UVs, through the panel
/// continuation `E2 name; face; restore; 48 back`.
fn convert_face(source: &[u8], face_index: usize, name: &str, uv: &[[i32; 2]]) -> Result<Vec<u8>> {
    let original = Model::parse(source)?;
    let face = original
        .faces
        .get(face_index)
        .ok_or("Select a model face")?;
    if face.sub & 4 != 0 && !face.texture.is_empty() && !face.uv.is_empty() {
        return Err(invalid(
            "Panel already has a named texture; load or copy its PIC before painting",
        ));
    }
    if face.sub & !0x67 != 0 {
        return Err(invalid(
            "This polygon shading subtype is not supported for automatic texturing",
        ));
    }
    let mut restore = face.material_selector.clone();
    if restore.is_empty() && !original.writable {
        // The model reader forgets the state after native code; the
        // whole-CODE proof can still name the selector that holds here.
        restore = crate::shape_geometry::Geometry::parse(source)
            .ok()
            .and_then(|g| {
                let i = g.face_at(face.offset)?;
                g.material(i).ok().map(|at| g.selector(at).to_vec())
            })
            .filter(|r| !r.is_empty())
            .ok_or_else(|| invalid("This panel inherits an unresolved material state; use its base color until the SH state writer supports it"))?;
    }
    if uv.len() != face.indices.len() || uv.iter().flatten().any(|v| !(0..=65535).contains(v)) {
        return Err(invalid("A panel needs one UV per corner, 0..65535"));
    }
    let code = code(source)?;
    let local = face
        .offset
        .checked_sub(code.start)
        .ok_or("Face outside CODE")?;
    let successor = face.end - code.start;
    if u16_at(source, face.offset + 3)? > 255 {
        return Err(invalid("Panel uses a special color encoding"));
    }
    ensure_face_not_relocated(source, code.rva + local, code.rva + successor)?;
    let uv_start = face.uv_offsets.first().copied().unwrap_or(face.end);
    let mut record = source[face.offset..uv_start].to_vec();
    record[1] = (face.sub & 0x60) | 4;
    // Byte UVs unless a coordinate needs a word.
    let word = uv.iter().flatten().any(|v| *v > 255);
    record[2] = (record[2] & !1) | u8::from(!word);
    for p in uv {
        for v in p {
            if word {
                record.extend((*v as u16).to_le_bytes());
            } else {
                record.push(*v as u8);
            }
        }
    }
    let mut extension = vec![0xe2, 0];
    extension.extend(name.as_bytes());
    extension.resize(16, 0);
    let extension_start = continuation_start(source)?;
    let new_face = extension_start + 16;
    extension.extend(record);
    if restore.is_empty() {
        extension.extend([0xe0, 0, 0, 0]);
    } else {
        extension.extend(&restore);
    }
    extension.extend(jump(extension_start + extension.len(), successor)?);
    let mut payload = source[code.start..code.start + code.len].to_vec();
    // The original polygon was copied into the continuation. Replace its dead
    // bytes with valid padding so whole-module readers keep later boundaries.
    stub_out(&mut payload, local, successor, extension_start)?;
    let out = append_continuation(source, payload, extension)?;
    let new_start = self::code(&out)?.start;
    let checked = Model::parse(&out)?;
    if checked.vertices.iter().map(|v| v.point).collect::<Vec<_>>()
        != original
            .vertices
            .iter()
            .map(|v| v.point)
            .collect::<Vec<_>>()
        || checked.faces.len() != original.faces.len()
    {
        return Err(invalid("Panel conversion changed the decoded geometry"));
    }
    let new_index = checked
        .faces
        .iter()
        .position(|f| f.offset == new_start + new_face)
        .ok_or("Generated face was not reached")?;
    if new_index != face_index {
        return Err(invalid("Panel conversion changed the face order"));
    }
    for (i, (a, b)) in original.faces.iter().zip(&checked.faces).enumerate() {
        if a.indices != b.indices
            || (i != new_index && (a.uv != b.uv || a.texture != b.texture && a.sub & 4 != 0))
        {
            return Err(invalid("Panel conversion altered a different face"));
        }
    }
    Ok(out)
}
/// Repair only the exact legacy Hangar continuation layout; leave PICs untouched.
pub struct PanelRepair {
    pub shape: Vec<u8>,
    pub panels: usize,
}
pub fn repair_panel_layout(source: &[u8]) -> Result<Option<PanelRepair>> {
    layout::repair(source).map(|r| r.map(|(shape, panels)| PanelRepair { shape, panels }))
}
/// Safe selected-vertex edits on the understood spatial-record-free subset.
/// Shapes outside the whole-shape writable subset go through the region
/// writer (`shape_geometry::move_model_vertices`), which proves each moved
/// vertex's faces and writes part vertices in their local frame.
pub fn move_vertices(source: &[u8], selected: &[usize], delta: [i32; 3]) -> Result<Vec<u8>> {
    let mut model = Model::parse(source)?;
    if !model.writable {
        return crate::shape_geometry::move_model_vertices(source, &model, selected, delta);
    }
    let mut offsets = BTreeSet::new();
    for i in selected {
        offsets.insert(model.vertices.get(*i).ok_or("No selected vertex")?.offset);
    }
    if offsets.is_empty() {
        return Err(invalid("Select at least one vertex"));
    }
    for v in &mut model.vertices {
        if offsets.contains(&v.offset) {
            for (k, d) in delta.iter().enumerate() {
                v.point[k] = v.point[k].checked_add(*d).ok_or("Coordinate overflow")?;
            }
        }
    }
    model.write(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_panel_keeps_geometry_and_supports_followup_vertex_edits() {
        let source = crate::model::demo_shape();
        let model = Model::parse(&source).unwrap();
        let panel = texture_panel(&source, 0, "PANEL.PIC", 64).unwrap();
        let result = Model::parse(&panel.shape).unwrap();
        assert!(result.writable);
        assert_eq!(result.faces[0].texture, "PANEL.PIC");
        assert_eq!(result.faces[0].uv.len(), 3);
        assert!(texture_panel(&panel.shape, 0, "OTHER.PIC", 64)
            .unwrap_err()
            .contains("already has a named texture"));
        assert_eq!(
            model.vertices.iter().map(|v| v.point).collect::<Vec<_>>(),
            result.vertices.iter().map(|v| v.point).collect::<Vec<_>>()
        );
        assert_eq!(result.write(&panel.shape).unwrap(), panel.shape);
        let changed = move_vertices(&panel.shape, &[0], [1, 2, 3]).unwrap();
        let moved = Model::parse(&changed).unwrap();
        for i in 1..moved.vertices.len() {
            assert_eq!(moved.vertices[i].point, result.vertices[i].point);
        }
        assert_eq!(moved.vertices[0].point, [1, 102, 3]);
        assert!(move_vertices(&panel.shape, &[0], [32768, 0, 0]).is_err());
        let sheet = crate::picture::Pic::parse(&panel.picture).unwrap();
        assert_eq!((sheet.width, sheet.height), (256, 64));
        assert_eq!(sheet.pixels, vec![32; 256 * 64]);
        assert!(crate::picture::retail_texture_check(&panel.picture).is_ok());
    }
    /// Two flat coplanar quads sharing an edge (together 160 x 40 units),
    /// one more of another colour and a tilted one, under BASE.PIC.
    fn panel_fixture() -> Vec<u8> {
        let mut a = crate::shape_testkit::Asm::default();
        a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
        a.b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
        let p: [[i16; 3]; 8] = [
            [0, 0, 0],
            [80, 0, 0],
            [80, 40, 0],
            [0, 40, 0],
            [160, 0, 0],
            [160, 40, 0],
            [0, 0, 30],
            [80, 0, 30],
        ];
        a.verts(0, &p);
        let n = [0, 0, 32765];
        a.face(0x23, 9, Some((n, [40, 20, 0])), &[0, 1, 2, 3], &[]);
        a.face(0x23, 9, Some((n, [120, 20, 0])), &[1, 4, 5, 2], &[]);
        a.face(0x23, 10, Some((n, [80, 20, 0])), &[0, 4, 5, 3], &[]);
        a.face(
            0x23,
            9,
            Some(([0, -32765, 0], [40, 0, 15])),
            &[0, 1, 7, 6],
            &[],
        );
        a.b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
        a.finish()
    }
    #[test]
    fn generated_sheets_follow_the_panel_size_and_aspect() {
        let src = panel_fixture();
        let model = Model::parse(&src).unwrap();
        assert_eq!(coplanar_panels(&model, 0, 32), [0, 1]);
        assert_eq!(coplanar_panels(&model, 0, 1), [0]);
        assert_eq!(coplanar_panels(&model, 3, 32), [3]);
        // One texel per unit: an 80 x 40 quad -> an 80 x 40 sub-rectangle
        // of a 256 x 40 retail texture.
        let one = texture_panels(&src, &[0], "ONE.PIC", 1 << 16).unwrap();
        assert_eq!(one.size, [80, 40]);
        let pic = crate::picture::Pic::parse(&one.picture).unwrap();
        assert_eq!((pic.width, pic.height), (256, 40));
        assert!(crate::originals::panel_sheet(&one.picture));
        let m = Model::parse(&one.shape).unwrap();
        assert_eq!(m.faces[0].texture, "ONE.PIC");
        let max = |k: usize| m.faces[0].uv.iter().map(|p| p[k]).max().unwrap();
        assert_eq!((max(0), max(1)), (79, 39));
        // The coplanar pair shares one 160 x 40 sheet with continuous UVs.
        let pair = texture_panels(&src, &[0, 1], "PAIR.PIC", 1 << 16).unwrap();
        assert_eq!(pair.size, [160, 40]);
        let m = Model::parse(&pair.shape).unwrap();
        assert_eq!(m.faces[1].texture, "PAIR.PIC");
        assert_eq!(m.faces[0].uv[1], m.faces[1].uv[0]);
        assert_eq!(m.faces[2].texture, "BASE.PIC");
        // Clamping keeps the aspect: 4 texels a unit would be 640 x 160.
        let big = texture_panels(&src, &[0, 1], "BIG.PIC", 4 << 16).unwrap();
        assert_eq!(big.size, [256, 64]);
        // A standing panel: the longer extent still runs along U.
        let side = texture_panels(&src, &[3], "SIDE.PIC", 1 << 16).unwrap();
        assert_eq!(side.size, [80, 30]);
        // The square entry point still fits the face into its sheet.
        let square = texture_panel(&src, 0, "SQ.PIC", 64).unwrap();
        assert_eq!(square.size, [64, 64]);
        let m = Model::parse(&square.shape).unwrap();
        assert!(m.faces[0].uv.iter().all(|p| p[0] <= 63 && p[1] <= 32));
        // Every generated sheet is a retail texture as tall as its panel,
        // solid in the face's own game-palette index.
        for (panel, height) in [
            (&one, 40),
            (&pair, 40),
            (&big, 64),
            (&side, 30),
            (&square, 64),
        ] {
            assert!(crate::picture::retail_texture_check(&panel.picture).is_ok());
            let pic = crate::picture::Pic::parse(&panel.picture).unwrap();
            assert_eq!((pic.width, pic.height), (256, height));
            assert!(pic.palette.is_empty());
            assert!(pic.pixels.iter().all(|p| *p == 9));
        }
    }
    #[test]
    fn exhausted_virtual_gap_and_long_jump_are_refused() {
        let mut source = crate::model::demo_shape();
        let len = u32_at(&source, 128).unwrap();
        source[70..72].copy_from_slice(&2u16.to_le_bytes());
        source[160..166].copy_from_slice(b".idata");
        put32(&mut source, 172, 4096 + len).unwrap();
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64).is_err());
        let mut source = crate::model::demo_shape();
        source.resize(256 + 40000, 0);
        put32(&mut source, 128, 40000).unwrap();
        put32(&mut source, 136, 40000).unwrap();
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64).is_err());
        assert!(texture_panel(&crate::model::demo_shape(), 0, "PANEL.PIC", 0).is_err());
    }
    #[test]
    fn face_relocations_are_not_overwritten() {
        let mut source = crate::model::demo_shape();
        let f = Model::parse(&source).unwrap().faces[0].offset - 256;
        source[70..72].copy_from_slice(&2u16.to_le_bytes());
        source[160..166].copy_from_slice(b".reloc");
        let at = source.len();
        put32(&mut source, 168, 12).unwrap();
        put32(&mut source, 172, 8192).unwrap();
        put32(&mut source, 176, 12).unwrap();
        put32(&mut source, 180, at).unwrap();
        source.extend(4096u32.to_le_bytes());
        source.extend(12u32.to_le_bytes());
        source.extend((0x3000 | f as u16).to_le_bytes());
        source.extend(0u16.to_le_bytes());
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64)
            .unwrap_err()
            .contains("relocation"));
    }
}
