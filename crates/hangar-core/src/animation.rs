//! Inert animation inspection and byte-local part placement. No imported code executes.
use crate::{invalid, model::Model, slice, u16_at, u32_at, Result};
use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// Map imported data addresses to names, using bounded PE import tables.
pub fn symbols(bytes: &[u8]) -> Result<BTreeMap<usize, String>> {
    let pe = u32_at(bytes, 60)?;
    let opt = pe + 24;
    let size = u16_at(bytes, pe + 20)?;
    if size < 112 {
        return Ok(BTreeMap::new());
    }
    let count = u16_at(bytes, pe + 6)?;
    if count > 32 {
        return Err(invalid("Too many module sections"));
    }
    let base = u32_at(bytes, opt + 28)?;
    let offset = |rva: usize| -> Result<usize> {
        for n in 0..count {
            let s = slice(bytes, opt + size + n * 40, 40)?;
            let start = u32_at(s, 12)?;
            let len = u32_at(s, 8)?.min(u32_at(s, 16)?);
            if rva >= start && rva - start < len {
                return u32_at(s, 20)?
                    .checked_add(rva - start)
                    .ok_or(invalid("Import address overflow"));
            }
        }
        Err(invalid("Import address outside initialized sections"))
    };
    let rva = u32_at(bytes, opt + 104)?;
    if rva == 0 {
        return Ok(BTreeMap::new());
    }
    let mut out = BTreeMap::<usize, String>::new();
    for n in 0..64 {
        let d = slice(bytes, offset(rva + n * 20)?, 20)?;
        if d.iter().all(|b| *b == 0) {
            // FA guards refer to import trampoline aliases inside CODE.
            // Label only exact FF 25 slots resolving to an already reviewed IAT name.
            let mut aliases = Vec::new();
            for n in 0..count {
                let s = slice(bytes, opt + size + n * 40, 40)?;
                if &s[..4] != b"CODE" {
                    continue;
                }
                let code = slice(bytes, u32_at(s, 20)?, u32_at(s, 8)?.min(u32_at(s, 16)?))?;
                for (i, word) in code.windows(6).enumerate() {
                    if word[..2] == [0xff, 0x25] {
                        if let Some(name) = out.get(&u32_at(word, 2)?) {
                            aliases.push((base + u32_at(s, 12)? + i, name.clone()));
                        }
                    }
                }
            }
            out.extend(aliases);
            return Ok(out);
        }
        let lookup = u32_at(d, 0)?;
        let iat = u32_at(d, 16)?;
        for i in 0..4096 {
            let name = u32_at(
                bytes,
                offset(if lookup == 0 { iat } else { lookup } + i * 4)?,
            )?;
            if name == 0 {
                break;
            }
            if name & 0x80000000 != 0 {
                continue;
            }
            let at = offset(name)? + 2;
            let rest = bytes.get(at..).ok_or("Truncated import name")?;
            let len = rest
                .iter()
                .take(256)
                .position(|b| *b == 0)
                .ok_or("Unterminated import name")?;
            let name = core::str::from_utf8(&rest[..len]).map_err(|_| "Invalid import name")?;
            out.insert(base + iat + i * 4, String::from(name));
            if i == 4095 {
                return Err(invalid("Import table exceeds limit"));
            }
        }
    }
    Err(invalid("Unterminated import descriptors"))
}

/// Change a reached C4 placement word, preserving its rotation program and all addresses.
/// Positions use the vertex-view X/forward/up convention; C4 stores X/up/forward.
pub fn place_part(
    bytes: &[u8],
    state: &BTreeMap<usize, i32>,
    offset: usize,
    axis: usize,
    value: i32,
) -> Result<Vec<u8>> {
    if axis > 2 || !(-32768..=32767).contains(&value) {
        return Err(invalid("Part position must fit a signed source word"));
    }
    let model = Model::with_state(bytes, state)?;
    let part = model
        .parts
        .iter()
        .find(|p| p.offset == offset)
        .ok_or("Part is not reached in this state")?;
    if part.position[axis] == value {
        return Ok(bytes.to_vec());
    }
    let mut out = bytes.to_vec();
    let at = offset + [2, 6, 4][axis];
    out[at..at + 2].copy_from_slice(&(value as i16).to_le_bytes());
    let after = Model::with_state(&out, state)?;
    if after.faces.len() != model.faces.len() || after.vertices.len() != model.vertices.len() {
        return Err(invalid("Part placement changed record traversal"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn module(code: &[u8]) -> Vec<u8> {
        let mut b = crate::model::demo_shape();
        b.truncate(256);
        b.extend(code);
        for at in [128, 136] {
            b[at..at + 4].copy_from_slice(&(code.len() as u32).to_le_bytes());
        }
        b
    }
    #[test]
    fn part_placement_changes_only_one_word_and_noop_is_exact() {
        let demo = crate::model::demo_shape();
        let mut c = vec![0xc4, 0, 10, 0, 30, 0, 20, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0];
        c.extend(&demo[256..]);
        let b = module(&c);
        let m = Model::parse(&b).unwrap();
        assert!(!m.writable);
        assert_eq!(m.parts[0].position, [10, 20, 30]);
        let state = BTreeMap::new();
        assert_eq!(place_part(&b, &state, 256, 1, 20).unwrap(), b);
        let edited = place_part(&b, &state, 256, 1, 50).unwrap();
        assert_eq!(&b[..262], &edited[..262]);
        assert_eq!(&b[264..], &edited[264..]);
        let p = Model::parse(&edited).unwrap();
        for (a, b) in m.vertices.iter().zip(p.vertices) {
            assert_eq!(b.point, [a.point[0], a.point[1] + 30, a.point[2]]);
        }
        assert!(place_part(&b, &state, 257, 0, 0).is_err());
        assert!(place_part(&b, &state, 256, 0, 32768).is_err());
    }
    #[test]
    fn state_guard_changes_pose_without_changing_source() {
        let demo = crate::model::demo_shape();
        let a = &demo[256..];
        let second = 34 + a.len();
        let mut c = vec![0xf0, 0, 0x66, 0x83, 0x3d, 0, 0x80, 0, 0, 0, 0x75, 11];
        for target in [34, second] {
            c.push(0x68);
            c.extend((4096 + target as u32).to_le_bytes());
            c.extend([0x68, 0, 0, 0, 0, 0xc3]);
        }
        c.extend(a);
        let mut other = a.to_vec();
        other[6..8].copy_from_slice(&20i16.to_le_bytes());
        c.extend(other);
        let b = module(&c);
        let neutral = Model::parse(&b).unwrap();
        let state = BTreeMap::from([(0x8000, 1)]);
        let other = Model::with_state(&b, &state).unwrap();
        assert_eq!(
            neutral.state_words.into_iter().collect::<Vec<_>>(),
            vec![0x8000]
        );
        assert_ne!(neutral.vertices[0].point, other.vertices[0].point);
        assert_eq!(
            Model::parse(&b).unwrap().vertices[0].point,
            neutral.vertices[0].point
        );
    }

    #[test]
    fn loaded_launcher_envelope_is_bounded_and_read_only() {
        let demo = crate::model::demo_shape();
        let mut code = vec![0xeb, 5, 0xb8, 1, 0, 0, 0, 0x0b, 0xc0, 0x74, 0x11, 0x68];
        code.extend((4096u32 + 22).to_le_bytes());
        code.extend([0x68, 0, 0, 0, 0, 0xc3, 0x12, 0, 1, 0, 0]);
        code.extend(&demo[256..]);
        let shape = module(&code);
        let parsed = Model::parse(&shape).unwrap();
        assert!(!parsed.writable);
        assert_eq!(parsed.faces.len(), Model::parse(&demo).unwrap().faces.len());
        code[8] = 0xff;
        assert!(Model::parse(&module(&code)).is_err());
    }
}
