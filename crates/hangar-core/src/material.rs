//! Fixed-size palette/UV patches and complete stored SH texture-name rewrites.
use crate::{
    archive::{Archive, Entry},
    dependencies::{self, Location},
    invalid,
    model::Model,
    picture::Pic,
    u32_at, Result,
};
use alloc::{collections::BTreeSet, vec::Vec};
pub fn palette_color(source: &[u8], pic: bool, index: usize, color: [u8; 3]) -> Result<Vec<u8>> {
    if index >= 256 || color.iter().any(|v| *v > 63) {
        return Err(invalid("Palette index 0..255 / RGB components 0..63"));
    }
    let at = if pic {
        Pic::parse(source)?;
        let at = u32_at(source, 18)?;
        let len = u32_at(source, 22)?;
        if (index + 1) * 3 > len {
            return Err(invalid("This palette index comes from the base PAL"));
        }
        if at < 64 {
            return Err(invalid("PIC palette overlaps header"));
        }
        let mut sections = vec![
            (64, u32_at(source, 14)?),
            (u32_at(source, 34)?, u32_at(source, 38)?),
        ];
        if u32_at(source, 26)? != 0 {
            sections.push((u32_at(source, 26)?, u32_at(source, 30)?));
        }
        let glyph = u32_at(source, 42)?;
        if glyph != 0 {
            sections.push((glyph, 256 * 6));
        }
        if sections
            .iter()
            .any(|(p, n)| *n > 0 && at < *p + *n && *p < at + len)
        {
            return Err(invalid("PIC palette aliases another data section"));
        }
        at + index * 3
    } else {
        crate::picture::palette(source)?;
        index * 3
    };
    let mut out = source.to_vec();
    out.get_mut(at..at + 3)
        .ok_or("Palette color outside resource")?
        .copy_from_slice(&color);
    if pic {
        Pic::parse(&out)?;
    } else {
        crate::picture::palette(&out)?;
    }
    Ok(out)
}
#[derive(Clone, Copy, Debug)]
pub struct UvTransform {
    pub shift: [i32; 2],
    pub scale: [i32; 2],
    pub degrees: i32,
}
pub fn face_uv(source: &[u8], face: usize, transform: UvTransform) -> Result<Vec<u8>> {
    if transform.scale.iter().any(|v| !(-1600..=1600).contains(v))
        || transform
            .shift
            .iter()
            .any(|v| !(-65535..=65535).contains(v))
    {
        return Err(invalid("UV transform exceeds limits"));
    }
    let model = Model::parse(source)?;
    let face = model.faces.get(face).ok_or("Select a textured face")?;
    if face.uv.is_empty() || face.uv.len() != face.uv_offsets.len() {
        return Err(invalid("Selected face has no editable UV record"));
    }
    let count = face.uv.len() as i64;
    let center = [
        face.uv.iter().map(|p| p[0] as i64).sum::<i64>() / count,
        face.uv.iter().map(|p| p[1] as i64).sum::<i64>() / count,
    ];
    let (s, c) = crate::model::sin_cos(transform.degrees);
    let width = if face.flags & 1 != 0 { 1 } else { 2 };
    let max = if width == 1 { 255 } else { 65535 };
    let mut out = source.to_vec();
    for (uv, at) in face.uv.iter().zip(&face.uv_offsets) {
        let p = [
            (uv[0] as i64 - center[0]) * transform.scale[0] as i64 / 100,
            (uv[1] as i64 - center[1]) * transform.scale[1] as i64 / 100,
        ];
        let q = [
            center[0] + (p[0] * c as i64 - p[1] * s as i64) / 1024 + transform.shift[0] as i64,
            center[1] + (p[0] * s as i64 + p[1] * c as i64) / 1024 + transform.shift[1] as i64,
        ];
        if q.iter().any(|v| !(0..=max).contains(v)) {
            return Err(invalid("UV values exceed this record's coordinate width"));
        }
        for k in 0..2 {
            if width == 1 {
                out[*at + k] = q[k] as u8;
            } else {
                out[*at + k * 2..*at + k * 2 + 2].copy_from_slice(&(q[k] as u16).to_le_bytes());
            }
        }
    }
    Model::parse(&out)?;
    Ok(out)
}
/// Clone a texture and retarget all stored literals in shapes reachable from the
/// selected aircraft, including non-displayed LOD/animation branches.
pub fn clone_family_texture(
    archive: &Archive,
    aircraft: &str,
    old: &str,
    new: &str,
) -> Result<Vec<Entry>> {
    crate::archive::validate_name(new)?;
    if !new.to_ascii_uppercase().ends_with(".PIC") || archive.find(new).is_some() {
        return Err(invalid("Choose an unused PIC name"));
    }
    let mut index = dependencies::Index::default();
    index.update(archive);
    let root = archive
        .find(aircraft)
        .ok_or("Aircraft definition missing")?;
    if !archive.entries[root].name.ends_with(".PT") {
        return Err(invalid("Select an aircraft PT for family texture cloning"));
    }
    let mut pending = vec![aircraft.to_ascii_uppercase()];
    let mut seen = BTreeSet::new();
    let mut shapes = Vec::new();
    let mut catalog = BTreeSet::new();
    for e in &archive.entries {
        catalog.insert(e.name.clone());
    }
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if seen.len() > 4096 {
            return Err(invalid("Aircraft graph exceeds 4096 resources"));
        }
        if let Some(scan) = index.get(&name) {
            pending.extend(
                scan.links
                    .iter()
                    .filter(|l| archive.find(&l.target).is_some())
                    .map(|l| l.target.clone()),
            );
        }
        if name.ends_with(".SH") {
            shapes.push(name);
        }
    }
    let mut entries = Vec::new();
    let mut total = 0;
    let new = new.to_ascii_uppercase();
    let mut matches = 0usize;
    for name in shapes {
        let e = &archive.entries[archive.find(&name).unwrap()];
        let bytes = e.read()?;
        total += bytes.len();
        if total > crate::archive::ARCHIVE_LIMIT {
            return Err(invalid("Shape scan exceeds 128 MiB"));
        }
        let refs = dependencies::references(&name, &bytes, &catalog, &mut BTreeSet::new())?;
        let mut out = bytes.clone();
        for r in refs.iter().filter(|r| r.target.eq_ignore_ascii_case(old)) {
            if let Location::Literal { at, len, stem_only } = r.location {
                let text = if stem_only {
                    new.split('.').next().unwrap()
                } else {
                    new.as_str()
                };
                if text.len() > len {
                    return Err(format!("{name}: new texture name exceeds a compiled slot"));
                }
                out[at..at + len + 1].fill(0);
                out[at..at + text.len()].copy_from_slice(text.as_bytes());
                matches += 1;
            }
        }
        if out != bytes {
            entries.push(Entry::new(&name, out)?);
        }
    }
    if matches == 0 {
        return Err(invalid("No stored family texture references matched"));
    }
    let original = archive
        .find(old)
        .ok_or("Original PIC is not in the active LIB")?;
    entries.push(archive.entries[original].renamed(&new)?);
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_and_uv_writes_are_byte_local() {
        let pic = crate::picture::demo();
        let output = palette_color(&pic, true, 17, [63, 2, 0]).unwrap();
        let at = u32_at(&pic, 18).unwrap() + 17 * 3;
        assert_eq!(&output[at..at + 3], &[63, 2, 0]);
        for i in 0..pic.len() {
            if !(at..at + 3).contains(&i) {
                assert_eq!(pic[i], output[i]);
            }
        }
        assert!(palette_color(&pic, true, 17, [64, 0, 0]).is_err());
        let pal = vec![0; 768];
        let output = palette_color(&pal, false, 255, [1, 2, 3]).unwrap();
        assert_eq!(&output[765..], &[1, 2, 3]);
        assert_eq!(&output[..765], &pal[..765]);
        let shape = crate::model::demo_textured();
        let model = Model::parse(&shape).unwrap();
        let fi = model.faces.iter().position(|f| !f.uv.is_empty()).unwrap();
        let output = face_uv(
            &shape,
            fi,
            UvTransform {
                shift: [1, 2],
                scale: [100, 100],
                degrees: 0,
            },
        )
        .unwrap();
        let after = Model::parse(&output).unwrap();
        assert_eq!(shape.len(), output.len());
        assert_eq!(
            model.vertices.iter().map(|v| v.point).collect::<Vec<_>>(),
            after.vertices.iter().map(|v| v.point).collect::<Vec<_>>()
        );
        let face = &model.faces[fi];
        let width = if face.flags & 1 != 0 { 1 } else { 2 };
        for i in 0..shape.len() {
            if !face
                .uv_offsets
                .iter()
                .any(|at| (*at..*at + 2 * width).contains(&i))
            {
                assert_eq!(shape[i], output[i]);
            }
        }
        assert!(face_uv(
            &shape,
            fi,
            UvTransform {
                shift: [65535, 0],
                scale: [100, 100],
                degrees: 0
            }
        )
        .is_err());
    }
    #[test]
    fn family_texture_clone_includes_hidden_module_references_and_undo() {
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("DEMO.PT", crate::brf::demo()).unwrap());
        a.entries
            .push(Entry::new("DEMO.SH", crate::model::demo_textured()).unwrap());
        for suffix in ["A", "B", "C", "D", "S"] {
            let mut bytes = crate::model::demo_shape();
            if suffix == "B" {
                bytes.extend([0xe2, 0]);
                bytes.extend(b"DEMO.PIC\0\0\0\0\0\0");
                let n = (bytes.len() - 256) as u32;
                bytes[128..132].copy_from_slice(&n.to_le_bytes());
                bytes[136..140].copy_from_slice(&n.to_le_bytes());
            }
            a.entries
                .push(Entry::new(&format!("DEMO_{suffix}.SH"), bytes).unwrap());
        }
        a.entries
            .push(Entry::new("DEMO.PIC", crate::picture::demo()).unwrap());
        let mut doc = crate::document::Document::new(a);
        let before = doc.archive.bytes().unwrap();
        let entries = clone_family_texture(&doc.archive, "DEMO.PT", "DEMO.PIC", "NEW.PIC").unwrap();
        assert_eq!(entries.len(), 3);
        doc.transaction(entries, &[]).unwrap();
        let mut idx = dependencies::Index::default();
        idx.update(&doc.archive);
        assert_eq!(idx.incoming("NEW.PIC").count(), 2);
        doc.undo();
        assert_eq!(doc.archive.bytes().unwrap(), before);
    }
}
