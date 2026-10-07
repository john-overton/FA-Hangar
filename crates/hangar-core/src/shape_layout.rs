//! SH end-marker/import-tail layout. Preserve instruction RVAs before the tail,
//! relocate HIGHLOW sites and targets, and keep CODE first in the raw file.
use super::{align, code, jump, put32, Code};
use crate::{invalid, model::Model, slice, u16_at, u32_at, Result};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
const MARKER: &[u8] = &[1, 2, 3, 2, 1, 2, 3, 2, 1];
#[derive(Clone, Copy, Debug)]
pub(super) struct Tail {
    pub start: usize,
    pub end: usize,
}
pub(super) fn tail(source: &[u8], c: &Code) -> Result<Option<Tail>> {
    let bytes = slice(source, c.start, c.len)?;
    let names = crate::animation::symbols(source)?;
    let mut found = None;
    for (start, window) in bytes.windows(MARKER.len()).enumerate() {
        if window != MARKER {
            continue;
        }
        let mut p = start + MARKER.len();
        let padding = p;
        while bytes.get(p) == Some(&0) {
            p += 1;
        }
        if p - padding > 15 {
            continue;
        }
        while bytes.get(p..p + 2) == Some(&[0xff, 0x25]) {
            let address = u32_at(bytes, p + 2)?;
            if !names.contains_key(&address) {
                return Err(invalid("SH import stub has no matching import slot"));
            }
            p += 6;
        }
        if p - padding <= 15 && p != bytes.len() && bytes.get(p..p + 2) != Some(&[0xe2, 0]) {
            continue;
        }
        if found.is_some() {
            return Err(invalid("Ambiguous SH end marker"));
        }
        found = Some(Tail { start, end: p });
    }
    if found.is_none() && c.optional_size >= 96 {
        return Err(invalid(
            "SH end marker/import tail is not recognized; layout rewrite refused",
        ));
    }
    Ok(found)
}
#[derive(Clone)]
struct Section {
    header: usize,
    rva: usize,
    virtual_size: usize,
    raw: usize,
    bytes: Vec<u8>,
}
fn sections(source: &[u8], c: &Code) -> Result<Vec<Section>> {
    let pe = u32_at(source, 60)?;
    let count = u16_at(source, pe + 6)?;
    (0..count)
        .map(|i| {
            let h = c.optional + c.optional_size + i * 40;
            let raw = u32_at(source, h + 20)?;
            Ok(Section {
                header: h,
                rva: u32_at(source, h + 12)?,
                virtual_size: u32_at(source, h + 8)?,
                raw,
                bytes: slice(source, raw, u32_at(source, h + 16)?)?.to_vec(),
            })
        })
        .collect()
}
fn locate(sections: &[Section], rva: usize) -> Result<(usize, usize)> {
    for (i, s) in sections.iter().enumerate() {
        if let Some(at) = rva.checked_sub(s.rva) {
            if at
                .checked_add(4)
                .is_some_and(|end| end <= s.virtual_size.min(s.bytes.len()))
            {
                return Ok((i, at));
            }
        }
    }
    Err(invalid("Relocation outside initialized module data"))
}
/// The replacement keeps body RVAs fixed and moves just the marker/stub tail.
pub(super) fn repack(
    source: &[u8],
    payload: Vec<u8>,
    old_tail: Tail,
    shift: isize,
) -> Result<Vec<u8>> {
    let moved = |v: usize| -> Result<usize> {
        v.checked_add_signed(shift)
            .ok_or_else(|| invalid("Address overflow"))
    };
    let c = code(source)?;
    if c.optional_size < 224 {
        return Err(invalid("Incomplete SH module header"));
    }
    let pe = u32_at(source, 60)?;
    if u32_at(source, pe + 12)? != 0 || u32_at(source, pe + 16)? != 0 {
        return Err(invalid(
            "COFF symbol tables require a full file-layout writer",
        ));
    }
    for i in 0..16 {
        if !matches!(i, 1 | 5 | 12) && u32_at(source, c.optional + 96 + i * 8)? != 0 {
            return Err(invalid(
                "Additional module directories are not supported for SH tail relocation",
            ));
        }
    }
    if payload.len() > c.limit {
        return Err(invalid(
            "CODE has no virtual-address room for the panel/import tail",
        ));
    }
    let raw_size = align(payload.len(), c.alignment)?;
    if raw_size > c.limit {
        return Err(invalid("Aligned CODE would overlap the next section"));
    }
    let mut sections = sections(source, &c)?;
    let ci = sections
        .iter()
        .position(|s| s.header == c.header)
        .ok_or("No CODE section")?;
    let ri = sections
        .iter()
        .position(|s| slice(source, s.header, 6).is_ok_and(|n| n == b".reloc"));
    let base = u32_at(source, c.optional + 28)?;
    let tail_begin = c.rva + old_tail.start;
    let tail_end = c.rva + old_tail.end;
    let mut relocations = Vec::new();
    if let Some(ri) = ri {
        let r = &sections[ri];
        if u32_at(source, c.optional + 96 + 5 * 8)? != r.rva {
            return Err(invalid("Relocation directory/section mismatch"));
        }
        let bytes = &r.bytes[..r.virtual_size.min(r.bytes.len())];
        let mut at = 0;
        let mut seen = BTreeSet::new();
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
                let kind = value >> 12;
                if kind == 0 {
                    continue;
                }
                if kind != 3 {
                    return Err(invalid("Unsupported SH relocation type"));
                }
                let site = page
                    .checked_add(value & 4095)
                    .ok_or("Relocation overflow")?;
                if !seen.insert(site) {
                    return Err(invalid("Duplicate module relocation"));
                }
                let (si, offset) = locate(&sections, site)?;
                if si == ri {
                    return Err(invalid("Self-relocating relocation section"));
                }
                let value = u32_at(&sections[si].bytes, offset)?;
                if (tail_end..c.rva + c.len).contains(&site)
                    || value
                        .checked_sub(base)
                        .is_some_and(|v| (tail_end..c.rva + c.len).contains(&v))
                {
                    return Err(invalid(
                        "Legacy appended drawing records contain an absolute relocation",
                    ));
                }
                let new_site = if (tail_begin..tail_end).contains(&site) {
                    moved(site)?
                } else {
                    site
                };
                let new_value = if value
                    .checked_sub(base)
                    .is_some_and(|v| (tail_begin..tail_end).contains(&v))
                {
                    moved(value)?
                } else {
                    value
                };
                relocations.push((new_site, new_value));
            }
            at += size;
        }
    }
    // Every moved import operand needs its relocated HIGHLOW site.
    let old_code = slice(source, c.start, c.len)?;
    let mut stub = old_tail.start + MARKER.len();
    while old_code.get(stub) == Some(&0) {
        stub += 1;
    }
    while stub < old_tail.end {
        if old_code.get(stub..stub + 2) != Some(&[0xff, 0x25])
            || !relocations
                .iter()
                .any(|(site, _)| Ok(*site) == moved(c.rva + stub + 2))
        {
            return Err(invalid("Unrelocated SH import stub"));
        }
        stub += 6;
    }
    sections[ci].virtual_size = payload.len();
    sections[ci].bytes = payload;
    sections[ci].bytes.resize(raw_size, 0);
    for (site, value) in &relocations {
        let (si, at) = locate(&sections, *site)?;
        put32(&mut sections[si].bytes, at, *value)?;
    }
    let mut reloc_size = None;
    if let Some(ri) = ri {
        let mut pages = BTreeMap::<usize, Vec<u16>>::new();
        for (site, _) in relocations {
            pages
                .entry(site & !4095)
                .or_default()
                .push((0x3000 | (site & 4095)) as u16);
        }
        let mut encoded = Vec::new();
        for (page, mut entries) in pages {
            entries.sort_unstable();
            if entries.len() % 2 != 0 {
                entries.push(0);
            }
            encoded.extend((page as u32).to_le_bytes());
            encoded.extend(((8 + entries.len() * 2) as u32).to_le_bytes());
            for entry in entries {
                encoded.extend(entry.to_le_bytes());
            }
        }
        if encoded.len() > sections[ri].bytes.len().min(sections[ri].virtual_size) {
            return Err(invalid(
                "Relocation table has no room for the moved import tail",
            ));
        }
        reloc_size = Some(encoded.len());
        encoded.resize(sections[ri].bytes.len(), 0);
        sections[ri].bytes = encoded;
    } else if u32_at(source, c.optional + 96 + 5 * 8)? != 0 {
        return Err(invalid("Relocation directory has no section"));
    }
    let headers = if matches!(c.start, 512 | 1024) {
        c.start
    } else if u32_at(source, c.optional + 104)? != 0 || u32_at(source, c.optional + 140)? != 0 {
        1024
    } else {
        512
    };
    let min_headers = c.optional + c.optional_size + sections.len() * 40;
    if !matches!(headers, 512 | 1024) || headers < min_headers || headers % c.alignment != 0 {
        return Err(invalid("Unsupported native SH header size"));
    }
    let mut out = slice(source, 0, headers)?.to_vec();
    // Keep CODE in its native first-section position, even when repairing the
    // older writer's appended raw CODE copy. No unused old code is serialized.
    let mut order: Vec<_> = (0..sections.len()).collect();
    order.sort_unstable_by_key(|i| {
        if *i == ci {
            (0, 0)
        } else {
            (1, sections[*i].raw)
        }
    });
    for i in order {
        let at = align(out.len(), c.alignment)?;
        if Some(i) == ri {
            // Native PL files pad relocations to the next 4 KiB file boundary;
            // $$DOSX follows that boundary. Preserve the relocation entries.
            let padded = align(at + reloc_size.unwrap_or(0) + 1, 4096)? - at;
            let limit = sections
                .iter()
                .filter(|s| s.rva > sections[i].rva)
                .map(|s| s.rva - sections[i].rva)
                .min()
                .unwrap_or(usize::MAX);
            if padded > limit {
                return Err(invalid("Relocation padding would overlap another section"));
            }
            sections[i].virtual_size = padded;
            sections[i].bytes.resize(padded, 0);
        }
        let s = &sections[i];
        out.resize(at, 0);
        put32(&mut out, s.header + 8, s.virtual_size)?;
        put32(&mut out, s.header + 16, s.bytes.len())?;
        put32(&mut out, s.header + 20, at)?;
        out.extend(&s.bytes);
    }
    put32(&mut out, c.optional + 4, sections[ci].virtual_size)?;
    if let Some(ri) = ri {
        let initialized = u32_at(source, c.optional + 8)?
            .saturating_sub(u32_at(source, sections[ri].header + 8)?)
            .checked_add(sections[ri].virtual_size)
            .ok_or("Module size overflow")?;
        put32(&mut out, c.optional + 8, initialized)?;
    }
    put32(&mut out, c.optional + 64, 0)?;
    // Relocation padding can also grow past a page when .reloc is the last section.
    let mut extent = 0;
    for s in &sections {
        extent = extent.max(
            s.rva
                .checked_add(s.virtual_size)
                .ok_or("Module size overflow")?,
        );
    }
    let image = u32_at(source, c.optional + 56)?.max(align(extent, c.section_alignment)?);
    put32(&mut out, c.optional + 56, image)?;
    let old_end = sections
        .iter()
        .map(|s| s.raw + u32_at(source, s.header + 16).unwrap_or(0))
        .max()
        .unwrap_or(headers);
    out.extend(slice(
        source,
        old_end,
        source.len().saturating_sub(old_end),
    )?);
    if let Some(size) = reloc_size {
        put32(&mut out, c.optional + 96 + 5 * 8 + 4, size)?;
    }
    if out.len() > crate::archive::RESOURCE_LIMIT {
        return Err(invalid("Expanded SH exceeds resource limit"));
    }
    Ok(out)
}
pub(super) fn append_before_tail(
    source: &[u8],
    mut body: Vec<u8>,
    extension: Vec<u8>,
    tail: Tail,
) -> Result<Vec<u8>> {
    if tail.end != body.len() {
        return Err(invalid(
            "Repair the legacy panel tail before adding another panel",
        ));
    }
    let shift = align(extension.len(), 16)?;
    let old = body.split_off(tail.start);
    body.extend(extension);
    body.resize(tail.start + shift, 0x1e);
    body.extend(old);
    repack(source, body, tail, shift as isize)
}
/// Replace the CODE bytes from `from` up to the end marker with `extension`
/// (1E-padded to 16 bytes, as `append_before_tail` lays it out), moving the
/// marker and import tail back or forward. Removing everything a series of
/// appends added gives back the layout before them.
pub(super) fn replace_before_tail(
    source: &[u8],
    mut body: Vec<u8>,
    from: usize,
    extension: Vec<u8>,
    tail: Tail,
) -> Result<Vec<u8>> {
    if tail.end != body.len() || from > tail.start {
        return Err(invalid(
            "Repair the legacy panel tail before adding another panel",
        ));
    }
    let size = align(extension.len(), 16)?;
    let old = body.split_off(tail.start);
    body.truncate(from);
    body.extend(extension);
    body.resize(from + size, 0x1e);
    body.extend(old);
    let shift = (from + size) as isize - tail.start as isize;
    repack(source, body, tail, shift)
}
pub(super) fn repair(source: &[u8]) -> Result<Option<(Vec<u8>, usize)>> {
    let c = code(source)?;
    let Some(tail) = tail(source, &c)? else {
        return Ok(None);
    };
    if tail.end == c.len {
        return Ok(None);
    }
    let model = Model::parse(source)?;
    let mut body = slice(source, c.start, c.len)?.to_vec();
    let tail_size = tail.end - tail.start;
    let mut routine = tail.end;
    let mut count = 0;
    while routine < c.len {
        if slice(&body, routine, 2)? != [0xe2, 0] {
            return Err(invalid(
                "Trailing SH code is not a Hangar panel continuation",
            ));
        }
        let face = model
            .faces
            .iter()
            .find(|f| f.offset == c.start + routine + 16)
            .ok_or("Legacy panel is not reached by the reviewed pose")?;
        if face.sub & !0x64 != 0 || face.sub & 4 == 0 || face.flags & 1 == 0 {
            return Err(invalid(
                "Legacy panel record does not match Hangar's writer",
            ));
        }
        let mut end = face.end - c.start;
        end += match slice(&body, end, 2)? {
            [0xe2, 0] => 16,
            [0xe0, 0] => 4,
            _ => return Err(invalid("Unrecognized legacy material restore")),
        };
        if slice(&body, end, 2)? != [0x48, 0] {
            return Err(invalid("Legacy panel has no return jump"));
        }
        let successor = (end as i64 + 4 + u16_at(&body, end + 2)? as u16 as i16 as i64) as usize;
        if successor >= tail.start {
            return Err(invalid("Legacy panel return is outside the original body"));
        }
        let mut sites = BTreeSet::new();
        for record in &model.records {
            if record.opcode != 0x48
                || record.offset < c.start
                || record.offset >= c.start + tail.start
            {
                continue;
            }
            let at = record.offset - c.start;
            if at as i64 + 4 + u16_at(&body, at + 2)? as u16 as i16 as i64 == routine as i64 {
                sites.insert(at);
            }
        }
        if sites.is_empty() {
            return Err(invalid("Legacy panel has no reviewed source jump"));
        }
        for site in sites {
            let span = successor
                .checked_sub(site)
                .ok_or("Legacy panel return precedes its source")?;
            let face_len = face.end - face.offset;
            let uv_bytes = face.indices.len() * 2;
            if span < 8 || ![face_len, face_len - uv_bytes, face_len + uv_bytes].contains(&span) {
                return Err(invalid(
                    "Legacy source stub does not match the converted face",
                ));
            }
            super::ensure_face_not_relocated(source, c.rva + site, c.rva + successor)?;
            // Also repair 0.7 stubs whose dead bytes still contained an FC tail.
            body[site..successor].fill(0x1e);
            body[site..site + 4].copy_from_slice(&jump(site, routine - tail_size)?);
            body[successor - 4..successor].copy_from_slice(&jump(successor - 4, successor)?);
        }
        body[end..end + 4].copy_from_slice(&jump(end - tail_size, successor)?);
        routine = end + 4;
        count += 1;
        if count > 256 {
            return Err(invalid("Too many legacy panel continuations"));
        }
    }
    let extra = body.split_off(tail.end);
    let old_tail = body.split_off(tail.start);
    let shift = align(extra.len(), 16)?;
    body.extend(extra);
    body.resize(tail.start + shift, 0x1e);
    body.extend(old_tail);
    let output = repack(source, body, tail, shift as isize)?;
    let checked = Model::parse(&output)?;
    if model.faces.len() != checked.faces.len()
        || model
            .vertices
            .iter()
            .map(|v| v.point)
            .ne(checked.vertices.iter().map(|v| v.point))
        || model.faces.iter().zip(&checked.faces).any(|(a, b)| {
            a.indices != b.indices
                || a.uv != b.uv
                || a.texture != b.texture
                || a.sub != b.sub
                || a.color != b.color
                || a.colors != b.colors
                || a.normal != b.normal
        })
    {
        return Err(invalid(
            "Panel-layout repair changed decoded geometry or materials",
        ));
    }
    Ok(Some((output, count)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let marker = 0xfe7;
        let tramp = 0xff0;
        let mut code = vec![0xf0, 0, 0x66, 0x83, 0x3d];
        code.extend((0x1000u32 + tramp).to_le_bytes());
        code.extend([0, 0x75, 0, 0x68]);
        code.extend(0x1017u32.to_le_bytes());
        code.push(0x68);
        code.extend((0x1000u32 + tramp).to_le_bytes());
        code.push(0xc3);
        code.extend([0xe2, 0]);
        code.extend(b"BASE.PIC");
        code.resize(23 + 16, 0);
        code.extend(&crate::model::demo_shape()[256..]);
        code.resize(marker, 0);
        code.extend(MARKER);
        code.resize(tramp as usize, 0);
        code.extend([0xff, 0x25]);
        code.extend(0x5040u32.to_le_bytes());
        let mut b = vec![0; 1024 + 4096 + 512 + 512];
        b[..2].copy_from_slice(b"MZ");
        put32(&mut b, 60, 128).unwrap();
        b[128..132].copy_from_slice(b"PL\0\0");
        b[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
        b[134..136].copy_from_slice(&3u16.to_le_bytes());
        b[148..150].copy_from_slice(&224u16.to_le_bytes());
        b[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
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
            (292, 16),
        ] {
            put32(&mut b, at, v).unwrap();
        }
        for (n, (name, rva, virt, raw, size)) in [
            ("CODE", 0x1000, code.len(), 1024, 4096),
            (".idata", 0x5000, 0x90, 5120, 512),
            (".reloc", 0x6000, 512, 5632, 512),
        ]
        .into_iter()
        .enumerate()
        {
            let h = 376 + n * 40;
            b[h..h + name.len()].copy_from_slice(name.as_bytes());
            for (off, value) in [(8, virt), (12, rva), (16, size), (20, raw)] {
                put32(&mut b, h + off, value).unwrap();
            }
        }
        b[1024..1024 + code.len()].copy_from_slice(&code);
        for (at, v) in [
            (5120, 0x5030),
            (5132, 0x5060),
            (5136, 0x5040),
            (5168, 0x5070),
            (5184, 0x5070),
        ] {
            put32(&mut b, at, v).unwrap();
        }
        b[5216..5219].copy_from_slice(b"FA\0");
        b[5234..5246].copy_from_slice(b"_PLgearDown\0");
        put32(&mut b, 5632, 0x1000).unwrap();
        put32(&mut b, 5636, 16).unwrap();
        for (i, offset) in [5u16, 13, 18, 0xff2].into_iter().enumerate() {
            b[5640 + i * 2..5642 + i * 2].copy_from_slice(&(0x3000 | offset).to_le_bytes());
        }
        b
    }
    fn legacy() -> Vec<u8> {
        let mut b = fixture();
        let c = code(&b).unwrap();
        let m = Model::parse(&b).unwrap();
        let f = &m.faces[0];
        let mut record = b[f.offset..f.end].to_vec();
        record[1] = 4;
        record[2] |= 1;
        record.extend([0, 0, 63, 0, 0, 63]);
        let mut extra = vec![0xe2, 0];
        extra.extend(b"OLD.PIC");
        extra.resize(16, 0);
        extra.extend(record);
        extra.extend(&f.material_selector);
        extra.extend(jump(c.len + extra.len(), f.end - c.start).unwrap());
        let mut payload = b[c.start..c.start + c.len].to_vec();
        payload[f.offset - c.start..f.end - c.start].fill(0x1e);
        payload[f.offset - c.start..f.offset - c.start + 4]
            .copy_from_slice(&jump(f.offset - c.start, c.len).unwrap());
        payload[f.end - c.start - 4..f.end - c.start]
            .copy_from_slice(&jump(f.end - c.start - 4, f.end - c.start).unwrap());
        payload.extend(extra);
        let at = b.len();
        let raw = align(payload.len(), 512).unwrap();
        put32(&mut b, c.header + 8, payload.len()).unwrap();
        put32(&mut b, c.header + 16, raw).unwrap();
        put32(&mut b, c.header + 20, at).unwrap();
        b.extend(payload);
        b.resize(at + raw, 0);
        b
    }
    #[test]
    fn new_panel_precedes_marker_and_relocates_imports_across_a_page() {
        let b = fixture();
        let original = Model::parse(&b).unwrap();
        assert_eq!(
            original.state_words.iter().copied().collect::<Vec<_>>(),
            [0x1ff0]
        );
        let result = super::super::texture_panel(&b, 0, "NEW.PIC", 64).unwrap();
        let c = code(&result.shape).unwrap();
        let tail = tail(&result.shape, &c).unwrap().unwrap();
        let model = Model::parse(&result.shape).unwrap();
        assert_eq!(c.start, 1024);
        assert!(model.faces[0].offset < c.start + tail.start);
        assert_eq!(tail.end, c.len);
        assert_eq!(
            model.state_words.iter().copied().collect::<Vec<_>>(),
            [0x2030]
        );
        assert_eq!(
            crate::animation::symbols(&result.shape)
                .unwrap()
                .get(&0x2030)
                .map(|s| s.as_str()),
            Some("_PLgearDown")
        );
        assert_eq!(u32_at(&result.shape, 292).unwrap(), 28);
        assert!(repair(&result.shape).unwrap().is_none());
        let more = super::super::texture_panel(&result.shape, 1, "NEXT.PIC", 64).unwrap();
        assert_eq!(
            Model::parse(&more.shape)
                .unwrap()
                .state_words
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [0x2070]
        );
    }
    #[test]
    fn image_size_covers_relocation_padding_past_a_page() {
        // .reloc is the last section by RVA; a large table whose new file offset sits
        // late in a 4 KiB page is padded across a page boundary.
        let mut b = fixture();
        let (idata, reloc) = (376 + 40, 376 + 80);
        put32(&mut b, idata + 16, 2048).unwrap();
        let block = b[5632..5648].to_vec();
        b.resize(7168, 0);
        b.extend(block);
        let mut entries = 4;
        for k in 0..500usize {
            b.extend((0x3000u16 | (0x200 + k as u16 * 2)).to_le_bytes());
            entries += 1;
        }
        let size = 8 + entries * 2;
        put32(&mut b, 7168 + 4, size).unwrap();
        b.resize(7168 + 1536, 0);
        for (off, v) in [(8, 1536), (16, 1536), (20, 7168)] {
            put32(&mut b, reloc + off, v).unwrap();
        }
        put32(&mut b, 292, size).unwrap();
        assert_eq!(u32_at(&b, 208).unwrap(), 0x7000);
        let result = super::super::texture_panel(&b, 0, "NEW.PIC", 64).unwrap();
        let out = &result.shape;
        let image = u32_at(out, 208).unwrap();
        for n in 0..3 {
            let h = 376 + n * 40;
            let end = u32_at(out, h + 12).unwrap() + u32_at(out, h + 8).unwrap();
            assert!(
                end <= image,
                "Section {n} ends at {end:#x} past SizeOfImage {image:#x}"
            );
        }
        assert_eq!(image, 0x8000);
        assert!(Model::parse(out).is_ok());
    }
    #[test]
    fn legacy_repair_is_idempotent_and_preserves_all_face_data() {
        let mut source = legacy();
        let c = code(&source).unwrap();
        let original = fixture();
        let original_model = Model::parse(&original).unwrap();
        let face = &original_model.faces[0];
        let site = face.offset - code(&original).unwrap().start;
        // Reproduce the older 0.7 stub: FC data remained after its leading jump.
        source[c.start + site + 4..c.start + site + face.end - face.offset]
            .copy_from_slice(&original[face.offset + 4..face.end]);
        let (out, n) = repair(&source).unwrap().unwrap();
        assert_eq!(n, 1);
        assert_eq!(code(&out).unwrap().start, 1024);
        assert!(out.len() < source.len());
        assert!(repair(&out).unwrap().is_none());
        let model = Model::parse(&out).unwrap();
        assert_eq!(model.faces[0].texture, "OLD.PIC");
        assert_eq!(model.faces[0].uv, [[0, 0], [63, 0], [0, 63]]);
        let more = super::super::texture_panel(&source, 1, "NEXT.PIC", 64).unwrap();
        assert!(repair(&more.shape).unwrap().is_none());
        assert_eq!(
            Model::parse(&more.shape).unwrap().faces[0].texture,
            "OLD.PIC"
        );
    }
    #[test]
    fn malformed_or_unhandled_layouts_fail_without_repair() {
        let mut b = legacy();
        let c = code(&b).unwrap();
        b[c.start + c.len - 4] = 0;
        assert!(repair(&b).is_err());
        let mut b = fixture();
        put32(&mut b, 376 + 80 + 8, 16).unwrap();
        assert!(super::super::texture_panel(&b, 0, "NEW.PIC", 64)
            .unwrap_err()
            .contains("Relocation table"));
        let mut referenced = fixture();
        let face = Model::parse(&referenced).unwrap().faces[0].offset;
        put32(&mut referenced, 1024 + 5, 0x1000 + face - 1024 + 3).unwrap();
        assert!(super::super::texture_panel(&referenced, 0, "NEW.PIC", 64)
            .unwrap_err()
            .contains("inside this face"));
        let mut b = fixture();
        put32(&mut b, 152 + 96 + 6 * 8, 0x5000).unwrap();
        assert!(super::super::texture_panel(&b, 0, "NEW.PIC", 64).is_err());
        let b = legacy();
        for n in [0, 64, 128, 376, 1024, b.len() - 1] {
            assert!(repair(&b[..n]).is_err());
        }
    }
}
