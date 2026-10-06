//! EALIB directory + EOF sentinel. Unmodified payloads keep their compression.
use crate::{dcl, invalid, slice, u16_at, u32_at, Result};
use alloc::{collections::BTreeSet, rc::Rc, string::String, vec::Vec};
pub const ARCHIVE_LIMIT: usize = 128 * 1024 * 1024;
pub const RESOURCE_LIMIT: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    directory: [u8; 14],
    source: Rc<Vec<u8>>,
    start: usize,
    len: usize,
}
impl Entry {
    pub fn new(name: &str, bytes: Vec<u8>) -> Result<Self> {
        let name = name.to_ascii_uppercase();
        validate_name(&name)?;
        if bytes.len() > RESOURCE_LIMIT {
            return Err(invalid("Resource exceeds 16 MiB limit"));
        }
        let mut directory = [0; 14];
        directory[..name.len()].copy_from_slice(name.as_bytes());
        Ok(Self {
            name,
            directory,
            start: 0,
            len: bytes.len(),
            source: Rc::new(bytes),
        })
    }
    pub fn renamed(&self, name: &str) -> Result<Self> {
        let name = name.to_ascii_uppercase();
        validate_name(&name)?;
        let mut entry = self.clone();
        entry.name = name;
        entry.directory[..13].fill(0);
        entry.directory[..entry.name.len()].copy_from_slice(entry.name.as_bytes());
        Ok(entry)
    }
    pub fn same_storage(&self, other: &Self) -> bool {
        self.directory == other.directory
            && self.start == other.start
            && self.len == other.len
            && Rc::ptr_eq(&self.source, &other.source)
    }
    pub fn source_offset(&self) -> usize {
        self.start
    }
    pub fn flag(&self) -> u8 {
        self.directory[13]
    }
    pub fn stored_len(&self) -> usize {
        self.len
    }
    pub fn stored(&self) -> &[u8] {
        &self.source[self.start..self.start + self.len]
    }
    pub fn read(&self) -> Result<Vec<u8>> {
        match self.flag() {
            0 if self.len <= RESOURCE_LIMIT => Ok(self.stored().to_vec()),
            4 => {
                let b = self.stored();
                dcl::explode(
                    slice(b, 4, b.len().saturating_sub(4))?,
                    u32_at(b, 0)?,
                    RESOURCE_LIMIT,
                )
            }
            0 => Err(invalid("Resource exceeds 16 MiB limit")),
            _ => Err(invalid(
                "Unknown compression, preserved but cannot edit or extract",
            )),
        }
    }
}
pub fn validate_name(name: &str) -> Result<()> {
    let (stem, ext) = name
        .split_once('.')
        .ok_or_else(|| invalid("Expected an ASCII 8.3 name"))?;
    if !(1..=8).contains(&stem.len())
        || !(1..=3).contains(&ext.len())
        || !stem
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~".contains(&c))
        || !ext
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~".contains(&c))
    {
        return Err(invalid("Expected an ASCII 8.3 name"));
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Archive {
    pub entries: Vec<Entry>,
    original: Option<Rc<Vec<u8>>>,
}
impl Archive {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            original: None,
        }
    }
    pub fn parse(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > ARCHIVE_LIMIT {
            return Err(invalid("Archive exceeds 128 MiB limit"));
        }
        if slice(&bytes, 0, 5)? != b"EALIB" {
            return Err(invalid("Not an EALIB archive"));
        }
        let count = u16_at(&bytes, 5)?;
        let end = 7 + (count + 1) * 18;
        slice(&bytes, 0, end)?;
        let sentinel = 7 + count * 18;
        if bytes[sentinel..sentinel + 14].iter().any(|b| *b != 0)
            || u32_at(&bytes, sentinel + 14)? != bytes.len()
        {
            return Err(invalid("Invalid EOF directory sentinel"));
        }
        let source = Rc::new(bytes);
        let mut entries = Vec::with_capacity(count);
        let mut names = BTreeSet::new();
        for i in 0..count {
            let at = 7 + i * 18;
            let directory: [u8; 14] = source[at..at + 14].try_into().unwrap();
            let raw = &directory[..13];
            let name = core::str::from_utf8(raw.split(|b| *b == 0).next().unwrap())
                .map_err(|_| invalid("Non-ASCII resource name"))?
                .to_ascii_uppercase();
            validate_name(&name).map_err(|reason| {
                format!("Invalid LIB entry {name:?} at directory offset 0x{at:X}: {reason}")
            })?;
            if !names.insert(name.clone()) {
                return Err(invalid("Duplicate resource name"));
            }
            let start = u32_at(&source, at + 14)?;
            let next = u32_at(&source, at + 32)?;
            if start < end || next < start || next > source.len() {
                return Err(invalid("Resource outside archive"));
            }
            entries.push(Entry {
                name,
                directory,
                source: source.clone(),
                start,
                len: next - start,
            });
        }
        Ok(Self {
            entries,
            original: Some(source),
        })
    }
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.name.eq_ignore_ascii_case(name))
    }
    pub fn changed(&mut self) {
        self.original = None;
    }
    pub fn bytes(&self) -> Result<Vec<u8>> {
        if let Some(original) = &self.original {
            return Ok(original.as_ref().clone());
        }
        if self.entries.is_empty() || self.entries.len() > 65535 {
            return Err(invalid("A LIB needs 1..65535 entries"));
        }
        let mut names = BTreeSet::new();
        let mut cursor = 7 + (self.entries.len() + 1) * 18;
        for e in &self.entries {
            validate_name(&e.name)?;
            if !names.insert(e.name.to_ascii_uppercase()) {
                return Err(invalid("Duplicate resource name"));
            }
            cursor = cursor
                .checked_add(e.len)
                .ok_or_else(|| invalid("Archive size overflow"))?;
            if cursor > ARCHIVE_LIMIT {
                return Err(invalid("Archive exceeds 128 MiB limit"));
            }
        }
        let mut out = Vec::with_capacity(cursor);
        out.extend(b"EALIB");
        out.extend((self.entries.len() as u16).to_le_bytes());
        cursor = 7 + (self.entries.len() + 1) * 18;
        for e in &self.entries {
            out.extend(e.directory);
            out.extend((cursor as u32).to_le_bytes());
            cursor += e.len;
        }
        out.extend([0; 14]);
        out.extend((cursor as u32).to_le_bytes());
        for e in &self.entries {
            out.extend(e.stored());
        }
        Ok(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_roundtrip_and_bounds() {
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("test.pt", b"hello".to_vec()).unwrap());
        let b = a.bytes().unwrap();
        assert_eq!(b.len(), 48);
        let a = Archive::parse(b.clone()).unwrap();
        assert_eq!(a.entries[0].read().unwrap(), b"hello");
        assert_eq!(a.bytes().unwrap(), b);
        for n in 0..b.len() {
            assert!(Archive::parse(b[..n].to_vec()).is_err());
        }
        let mut bad = b;
        bad[21..25].copy_from_slice(&1u32.to_le_bytes());
        assert!(Archive::parse(bad).is_err());
    }
    #[test]
    fn compressed_and_unknown_payloads_preserved() {
        let mut a = Archive::empty();
        let mut e = Entry::new(
            "TEST.PT",
            [
                13u32.to_le_bytes().as_slice(),
                &[0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f],
            ]
            .concat(),
        )
        .unwrap();
        e.directory[13] = 4;
        a.entries.push(e);
        let original = a.bytes().unwrap();
        let mut a = Archive::parse(original.clone()).unwrap();
        assert_eq!(a.entries[0].read().unwrap(), b"AIAIAIAIAIAIA");
        a.changed();
        assert_eq!(a.bytes().unwrap(), original);
        a.entries[0].directory[13] = 99;
        assert!(a.entries[0].read().is_err());
        assert_eq!(
            Archive::parse(a.bytes().unwrap()).unwrap().entries[0].flag(),
            99
        );
    }
    #[test]
    fn dos_punctuation_roundtrips_and_survives_repack() {
        // Synthetic EALIB: 13-byte name, flag, payload offset and EOF sentinel.
        let mut bytes = b"EALIB\x01\x00".to_vec();
        bytes.extend_from_slice(b"$ICON.PIC\0\0\0\0\0");
        bytes.extend_from_slice(&43_u32.to_le_bytes());
        bytes.extend_from_slice(&[0; 14]);
        bytes.extend_from_slice(&46_u32.to_le_bytes());
        bytes.extend_from_slice(b"pic");
        let mut archive = Archive::parse(bytes.clone()).unwrap();
        assert_eq!(archive.entries[0].name, "$ICON.PIC");
        assert_eq!(archive.bytes().unwrap(), bytes);
        archive.changed();
        archive
            .entries
            .push(Entry::new("EXTRA.TXT", b"new".to_vec()).unwrap());
        let repacked = Archive::parse(archive.bytes().unwrap()).unwrap();
        assert_eq!(repacked.entries[0].read().unwrap(), b"pic");
        for name in ["$NAME.PIC", "!TEST.PT", "X%Y.SH", "A@B.PIC", "TEST.$$$"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
        for name in [
            "A+B.PIC", "A,B.PIC", "A;B.PIC", "A=B.PIC", "A[B.PIC", "A?B.PIC",
        ] {
            assert!(validate_name(name).is_err(), "{name}");
        }
    }
    #[test]
    fn names_are_safe_and_unique() {
        assert!(Entry::new("$ICON.PIC", vec![]).is_ok());
        assert!(Entry::new("#CALL.5K", vec![]).is_ok());
        assert!(Entry::new("^CALL.11K", vec![]).is_ok());
        for name in ["../x", "A/B.PT", "TOOLONG99.PT", "A.B.C", "é.PT"] {
            assert!(Entry::new(name, vec![]).is_err());
        }
        let mut a = Archive::empty();
        a.entries.push(Entry::new("a.pt", vec![]).unwrap());
        a.entries.push(Entry::new("A.PT", vec![]).unwrap());
        assert!(a.bytes().is_err());
    }
}

/// Directory-only view for resolving dependencies in large sibling LIBs without
/// retaining their payloads. Offsets remain relative to the source file.
#[derive(Clone, Debug)]
pub struct IndexedEntry {
    pub name: String,
    pub flag: u8,
    pub offset: usize,
    pub size: usize,
}
pub fn directory(bytes: &[u8], file_size: usize) -> Result<Vec<IndexedEntry>> {
    if slice(bytes, 0, 5)? != b"EALIB" {
        return Err(invalid("Not an EALIB archive"));
    }
    let count = u16_at(bytes, 5)?;
    let end = 7 + (count + 1) * 18;
    slice(bytes, 0, end)?;
    let last = 7 + count * 18;
    if bytes[last..last + 14].iter().any(|b| *b != 0) || u32_at(bytes, last + 14)? != file_size {
        return Err(invalid("Invalid EOF directory sentinel"));
    }
    let mut out = Vec::with_capacity(count);
    let mut names = BTreeSet::new();
    for i in 0..count {
        let at = 7 + i * 18;
        let raw = &bytes[at..at + 13];
        let name = core::str::from_utf8(raw.split(|b| *b == 0).next().unwrap())
            .map_err(|_| invalid("Non-ASCII resource name"))?
            .to_ascii_uppercase();
        validate_name(&name)?;
        if !names.insert(name.clone()) {
            return Err(invalid("Duplicate resource name"));
        }
        let offset = u32_at(bytes, at + 14)?;
        let next = u32_at(bytes, at + 32)?;
        if offset < end || next < offset || next > file_size {
            return Err(invalid("Resource outside archive"));
        }
        out.push(IndexedEntry {
            name,
            flag: bytes[at + 13],
            offset,
            size: next - offset,
        });
    }
    Ok(out)
}
pub fn decode_payload(flag: u8, bytes: Vec<u8>) -> Result<Vec<u8>> {
    match flag {
        0 if bytes.len() <= RESOURCE_LIMIT => Ok(bytes),
        4 => dcl::explode(
            slice(&bytes, 4, bytes.len().saturating_sub(4))?,
            u32_at(&bytes, 0)?,
            RESOURCE_LIMIT,
        ),
        _ => Err(invalid(
            "Unsupported compression or resource exceeds 16 MiB",
        )),
    }
}
