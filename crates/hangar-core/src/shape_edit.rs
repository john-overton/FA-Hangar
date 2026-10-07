//! First SH authoring layer: source-provenance edits and bounded continuations.
//! Existing instruction RVAs are retained, including opaque code and relocations.
use crate::{invalid, model::Model, slice, u16_at, u32_at, Result};
use alloc::{collections::BTreeSet, vec::Vec};
#[derive(Debug)]
pub struct PanelTexture {
    pub shape: Vec<u8>,
    pub picture: Vec<u8>,
    pub face: usize,
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
fn ensure_face_not_relocated(source: &[u8], start: usize, end: usize) -> Result<()> {
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
            }
            at += size;
        }
    }
    Ok(())
}
fn jump(from: usize, to: usize) -> Result<[u8; 4]> {
    let d = to as i64 - from as i64 - 4;
    let d = i16::try_from(d)
        .map_err(|_| invalid("Panel continuation exceeds the 16-bit SH jump reach"))?;
    let b = d.to_le_bytes();
    Ok([0x48, 0, b[0], b[1]])
}
fn raw_picture(size: usize, color: u8, palette: &[[u8; 3]; 256]) -> Result<Vec<u8>> {
    if !(8..=256).contains(&size) || !size.is_power_of_two() {
        return Err(invalid(
            "Texture sheet size must be 8, 16, 32, 64, 128 or 256",
        ));
    }
    let mut out = vec![0; 64 + size * size + 768];
    for (at, n) in [
        (2, size),
        (6, size),
        (10, 64),
        (14, size * size),
        (18, 64 + size * size),
        (22, 768),
    ] {
        put32(&mut out, at, n)?;
    }
    out[64..64 + size * size].fill(color);
    for (i, rgb) in palette.iter().enumerate() {
        for c in 0..3 {
            out[64 + size * size + i * 3 + c] = ((rgb[c] as u16 * 63 + 127) / 255) as u8;
        }
    }
    crate::picture::Pic::parse(&out)?;
    Ok(out)
}
/// Paintable flat-color face -> private PIC and UVs. The old face site becomes
/// one relative jump; the new draw sequence returns to the original successor.
/// Code grows only within its existing virtual-address gap.
pub fn texture_panel(
    source: &[u8],
    face_index: usize,
    name: &str,
    size: usize,
    palette: &[[u8; 3]; 256],
) -> Result<PanelTexture> {
    if !(8..=256).contains(&size) || !size.is_power_of_two() {
        return Err(invalid(
            "Texture sheet size must be a power of two from 8 to 256",
        ));
    }
    crate::archive::validate_name(name)?;
    if !name.ends_with(".PIC") {
        return Err(invalid("Panel texture must be a PIC"));
    }
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
    if face.material_selector.is_empty() && !original.writable {
        return Err(invalid("This panel inherits an unresolved material state; use its base color until the SH state writer supports it"));
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
    record[2] |= 1; // appended UVs use bytes
    let points: Vec<_> = face
        .indices
        .iter()
        .map(|i| original.vertices[*i].point)
        .collect();
    let mut ranges: [(usize, i32, i32); 3] = core::array::from_fn(|i| {
        (
            i,
            points.iter().map(|p| p[i]).min().unwrap(),
            points.iter().map(|p| p[i]).max().unwrap(),
        )
    });
    ranges.sort_unstable_by_key(|(_, min, max)| core::cmp::Reverse(*max as i64 - *min as i64));
    if ranges[1].1 == ranges[1].2 {
        return Err(invalid("Degenerate panel cannot be auto-mapped"));
    }
    for point in &points {
        for (axis, min, max) in ranges.iter().take(2) {
            record.push(
                ((point[*axis] as i64 - *min as i64) * (size - 1) as i64
                    / (*max as i64 - *min as i64)) as u8,
            );
        }
    }
    let mut extension = vec![0xe2, 0];
    extension.extend(name.as_bytes());
    extension.resize(16, 0);
    let new_face = code.len + 16;
    extension.extend(record);
    if face.material_selector.is_empty() {
        extension.extend([0xe0, 0, 0, 0]);
    } else {
        extension.extend(&face.material_selector);
    }
    extension.extend(jump(code.len + extension.len(), successor)?);
    let new_len = code.len + extension.len();
    if new_len > code.limit {
        return Err(invalid(
            "CODE has no virtual-address room for this panel; relocation support is required",
        ));
    }
    let mut payload = source[code.start..code.start + code.len].to_vec();
    // The original polygon was copied into the continuation. Replace its dead
    // bytes with valid padding so whole-module readers keep later boundaries.
    if successor - local < 8 {
        return Err(invalid("Face too short for a continuation stub"));
    }
    payload[local..successor].fill(0x1e);
    payload[local..local + 4].copy_from_slice(&jump(local, code.len)?);
    payload[successor - 4..successor].copy_from_slice(&jump(successor - 4, successor)?);
    payload.extend(extension);
    let mut out = source.to_vec();
    let new_start;
    if new_len <= code.raw_size
        && source[code.start + code.len..code.start + new_len]
            .iter()
            .all(|b| *b == 0)
    {
        new_start = code.start;
        out[new_start..new_start + new_len].copy_from_slice(&payload);
    } else {
        new_start = align(out.len(), code.alignment)?;
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
    if out.len() > crate::archive::RESOURCE_LIMIT {
        return Err(invalid("Expanded shape exceeds resource limit"));
    }
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
    for (i, (a, b)) in original.faces.iter().zip(&checked.faces).enumerate() {
        if a.indices != b.indices
            || (i != new_index && (a.uv != b.uv || a.texture != b.texture && a.sub & 4 != 0))
        {
            return Err(invalid("Panel conversion altered a different face"));
        }
    }
    Ok(PanelTexture {
        shape: out,
        picture: raw_picture(size, face.color, palette)?,
        face: new_index,
    })
}
/// Safe selected-vertex edits on the understood spatial-record-free subset.
pub fn move_vertices(source: &[u8], selected: &[usize], delta: [i32; 3]) -> Result<Vec<u8>> {
    let mut model = Model::parse(source)?;
    if !model.writable {
        return Err(model.reason.clone());
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
        let palette = core::array::from_fn(|i| [i as u8; 3]);
        let panel = texture_panel(&source, 0, "PANEL.PIC", 64, &palette).unwrap();
        let result = Model::parse(&panel.shape).unwrap();
        assert!(result.writable);
        assert_eq!(result.faces[0].texture, "PANEL.PIC");
        assert_eq!(result.faces[0].uv.len(), 3);
        assert!(texture_panel(&panel.shape, 0, "OTHER.PIC", 64, &palette)
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
        assert_eq!(
            crate::picture::Pic::parse(&panel.picture).unwrap().pixels,
            vec![32; 4096]
        );
    }
    #[test]
    fn exhausted_virtual_gap_and_long_jump_are_refused() {
        let mut source = crate::model::demo_shape();
        let len = u32_at(&source, 128).unwrap();
        source[70..72].copy_from_slice(&2u16.to_le_bytes());
        source[160..166].copy_from_slice(b".idata");
        put32(&mut source, 172, 4096 + len).unwrap();
        let palette = [[0; 3]; 256];
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64, &palette).is_err());
        let mut source = crate::model::demo_shape();
        source.resize(256 + 40000, 0);
        put32(&mut source, 128, 40000).unwrap();
        put32(&mut source, 136, 40000).unwrap();
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64, &palette).is_err());
        assert!(texture_panel(&crate::model::demo_shape(), 0, "PANEL.PIC", 0, &palette).is_err());
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
        assert!(texture_panel(&source, 0, "PANEL.PIC", 64, &[[0; 3]; 256])
            .unwrap_err()
            .contains("relocation"));
    }
}
