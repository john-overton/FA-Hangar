//! Stored texture originals: one `X.ORG` companion per edited `X.PIC`, holding
//! the pre-edit entry exactly (shared storage and compression flag). A name that
//! is not a valid PIC payload is never adopted, overwritten or removed as a backup.
use crate::{
    archive::{Archive, Entry},
    document::Document,
    invalid,
    picture::Pic,
    u16_at, u32_at, Result,
};
use alloc::{string::String, vec::Vec};

fn swap(name: &str, from: &str, to: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    ext.eq_ignore_ascii_case(from)
        .then(|| format!("{}.{to}", stem.to_ascii_uppercase()))
}
/// `X.PIC` -> `X.ORG`; the stem, including any `_`, `~` or `$` prefix, is kept.
pub fn companion(pic: &str) -> Option<String> {
    swap(pic, "PIC", "ORG")
}
/// `X.ORG` -> `X.PIC`.
pub fn texture_of(org: &str) -> Option<String> {
    swap(org, "ORG", "PIC")
}
/// A stored original must decode as a PIC; anything else is someone else's file.
pub fn valid(entry: &Entry) -> bool {
    entry
        .read()
        .ok()
        .is_some_and(|bytes| Pic::parse(&bytes).is_ok())
}
/// The valid stored original of `pic`, if any.
pub fn backup<'a>(archive: &'a Archive, pic: &str) -> Option<&'a Entry> {
    let name = companion(pic)?;
    archive
        .find(&name)
        .map(|i| &archive.entries[i])
        .filter(|e| valid(e))
}
/// Append a backup for every existing PIC these entries replace when it has no
/// stored original yet. An existing `X.ORG` of any kind is left alone, so a
/// first edit is captured once and later edits never touch it. The backup is
/// the saved entry when this session already changed the PIC (for example
/// after its original was removed), otherwise the current entry. `skip` names
/// textures whose original is rebuilt elsewhere (new generated panel sheets).
pub fn with_originals(doc: &Document, mut entries: Vec<Entry>, skip: &[String]) -> Vec<Entry> {
    let archive = &doc.archive;
    let mut extra = Vec::new();
    for entry in &entries {
        let Some(name) = companion(&entry.name) else {
            continue;
        };
        if skip.iter().any(|s| s.eq_ignore_ascii_case(&entry.name))
            || archive.find(&name).is_some()
            || entries.iter().chain(&extra).any(|e| e.name == name)
        {
            continue;
        }
        let Some(current) = archive.find(&entry.name).map(|i| &archive.entries[i]) else {
            continue;
        };
        if current.same_storage(entry) {
            continue;
        }
        let source = doc
            .saved_entry(&current.name)
            .filter(|s| !s.same_storage(current) && valid(s))
            .or_else(|| valid(current).then_some(current));
        if let Some(Ok(backup)) = source.map(|e| e.renamed(&name)) {
            extra.push(backup);
        }
    }
    entries.extend(extra);
    entries
}
/// Restore transaction: `X.PIC` takes the stored original's exact bytes and
/// flag, and `X.ORG` is removed. When the original is the saved entry's own
/// payload, the saved entry itself is used so the texture reads as unchanged.
pub fn restore(
    archive: &Archive,
    pic: &str,
    saved: Option<&Entry>,
) -> Result<(Vec<Entry>, Vec<String>)> {
    let pic = pic.to_ascii_uppercase();
    let org = backup(archive, &pic).ok_or_else(|| format!("No stored original for {pic}"))?;
    let entry = match saved {
        Some(s) if s.name == pic && s.same_payload(org) => s.clone(),
        _ => org.renamed(&pic)?,
    };
    Ok((vec![entry], vec![org.name.clone()]))
}
/// Names removed with `name`: a PIC takes its valid stored original along.
pub fn removals(archive: &Archive, name: &str) -> Vec<String> {
    let mut out = vec![name.to_ascii_uppercase()];
    if let Some(org) = backup(archive, name) {
        out.push(org.name.clone());
    }
    out
}
/// Every valid stored original, for distribution builds.
pub fn stored(archive: &Archive) -> Vec<String> {
    archive
        .entries
        .iter()
        .filter(|e| texture_of(&e.name).is_some() && valid(e))
        .map(|e| e.name.clone())
        .collect()
}
/// Copy `from`'s stored original to `to`'s companion name for a texture clone.
pub fn cloned(archive: &Archive, from: &str, to: &str) -> Result<Option<Entry>> {
    let Some(org) = backup(archive, from) else {
        return Ok(None);
    };
    let name = companion(to).ok_or_else(|| invalid("Texture clone must be a PIC"))?;
    if archive.find(&name).is_some() {
        return Err(format!(
            "{name} already exists; choose another texture name"
        ));
    }
    Ok(Some(org.renamed(&name)?))
}
/// Raw square sheet exactly as Hangar generates for flat-color panels: header,
/// one raster and a full 256-color palette, nothing else.
pub fn panel_sheet(bytes: &[u8]) -> bool {
    let field = |at| u32_at(bytes, at).ok();
    let Some(size) = field(2) else {
        return false;
    };
    (8..=256).contains(&size)
        && size.is_power_of_two()
        && u16_at(bytes, 0) == Ok(0)
        && bytes.len() == 64 + size * size + 768
        && [
            (6, size),
            (10, 64),
            (14, size * size),
            (18, 64 + size * size),
            (22, 768),
        ]
        .iter()
        .all(|(at, n)| field(*at) == Some(*n))
        && bytes[26..64].iter().all(|b| *b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture;
    fn texture() -> Vec<u8> {
        picture::demo()
    }
    fn painted(bytes: &[u8], color: u8) -> Vec<u8> {
        let mut out = bytes.to_vec();
        Pic::parse(bytes)
            .unwrap()
            .paint(&mut out, 4, 4, 1, color)
            .unwrap();
        out
    }
    fn document() -> Document {
        let mut a = Archive::empty();
        a.entries = vec![
            Entry::new("DEMO.SH", crate::model::demo_textured()).unwrap(),
            Entry::new("_F18.PIC", texture()).unwrap(),
            Entry::new("OTHER.PIC", texture()).unwrap(),
        ];
        Document::new(Archive::parse(a.bytes().unwrap()).unwrap())
    }
    fn paint(doc: &mut Document, name: &str, color: u8) {
        let at = doc.archive.find(name).unwrap();
        let bytes = painted(&doc.archive.entries[at].read().unwrap(), color);
        let entries = with_originals(doc, vec![Entry::new(name, bytes).unwrap()], &[]);
        doc.transaction(entries, &[]).unwrap();
    }
    #[test]
    fn names_keep_prefixes_and_pass_8_3() {
        assert_eq!(companion("_F18.PIC").unwrap(), "_F18.ORG");
        assert_eq!(companion("~f18h.pic").unwrap(), "~F18H.ORG");
        assert_eq!(companion("$AIM9.PIC").unwrap(), "$AIM9.ORG");
        assert_eq!(texture_of("$AIM9.ORG").unwrap(), "$AIM9.PIC");
        assert!(companion("DEMO.SH").is_none());
        assert!(crate::archive::validate_name("~F18H.ORG").is_ok());
        assert!(crate::dependencies::leaf("_F18.ORG"));
    }
    #[test]
    fn first_edit_backs_up_once_and_undo_removes_both() {
        let mut doc = document();
        let before = doc.archive.bytes().unwrap();
        let original = doc.archive.entries[1].clone();
        paint(&mut doc, "_F18.PIC", 7);
        let org = doc.archive.find("_F18.ORG").unwrap();
        assert_eq!(doc.archive.entries.len(), 4);
        assert!(doc.archive.entries[org].same_payload(&original));
        paint(&mut doc, "_F18.PIC", 9);
        assert_eq!(doc.archive.entries.len(), 4);
        assert!(doc.archive.entries[org].same_payload(&original));
        assert!(doc.undo());
        assert!(doc.archive.find("_F18.ORG").is_some());
        assert!(doc.undo());
        assert!(doc.archive.find("_F18.ORG").is_none());
        assert_eq!(doc.archive.bytes().unwrap(), before);
        // Unchanged, new or non-PIC entries never produce a backup.
        let same = with_originals(&doc, vec![original.clone()], &[]);
        assert_eq!(same.len(), 1);
        let new = Entry::new("NEW.PIC", texture()).unwrap();
        assert_eq!(with_originals(&doc, vec![new], &[]).len(), 1);
        let shape = Entry::new("DEMO.SH", vec![1]).unwrap();
        assert_eq!(with_originals(&doc, vec![shape], &[]).len(), 1);
        let skipped = Entry::new("_F18.PIC", painted(&texture(), 3)).unwrap();
        assert_eq!(
            with_originals(&doc, vec![skipped], &["_F18.PIC".into()]).len(),
            1
        );
    }
    #[test]
    fn a_removed_original_is_recreated_from_the_saved_entry_not_the_painted_one() {
        let mut doc = document();
        let saved = doc.archive.entries[1].clone();
        paint(&mut doc, "_F18.PIC", 7);
        doc.transaction(Vec::new(), &stored(&doc.archive)).unwrap();
        assert!(doc.archive.find("_F18.ORG").is_none());
        paint(&mut doc, "_F18.PIC", 9);
        let org = backup(&doc.archive, "_F18.PIC").unwrap();
        assert!(org.same_payload(&saved));
        // A PIC added this session has no saved entry; its current state is kept.
        doc.transaction(vec![Entry::new("NEW.PIC", texture()).unwrap()], &[])
            .unwrap();
        let added = doc.archive.entries[doc.archive.find("NEW.PIC").unwrap()].clone();
        paint(&mut doc, "NEW.PIC", 9);
        assert!(backup(&doc.archive, "NEW.PIC")
            .unwrap()
            .same_payload(&added));
    }
    #[test]
    fn existing_junk_org_is_never_overwritten_adopted_or_removed() {
        let mut doc = document();
        doc.transaction(
            vec![Entry::new("_F18.ORG", b"notes".to_vec()).unwrap()],
            &[],
        )
        .unwrap();
        let junk = doc.archive.entries[doc.archive.find("_F18.ORG").unwrap()].clone();
        paint(&mut doc, "_F18.PIC", 7);
        let now = &doc.archive.entries[doc.archive.find("_F18.ORG").unwrap()];
        assert!(now.same_storage(&junk));
        assert!(backup(&doc.archive, "_F18.PIC").is_none());
        assert!(restore(&doc.archive, "_F18.PIC", None).is_err());
        assert_eq!(removals(&doc.archive, "_F18.PIC"), vec!["_F18.PIC"]);
        assert!(stored(&doc.archive).is_empty());
    }
    #[test]
    fn restore_is_byte_exact_one_step_and_clean_against_saved() {
        let mut doc = document();
        let before = doc.archive.bytes().unwrap();
        paint(&mut doc, "_F18.PIC", 7);
        paint(&mut doc, "OTHER.PIC", 7);
        assert_eq!(stored(&doc.archive), vec!["_F18.ORG", "OTHER.ORG"]);
        let saved = doc.saved_entry("_F18.PIC").cloned();
        let (entries, removals) = restore(&doc.archive, "_F18.PIC", saved.as_ref()).unwrap();
        assert_eq!(removals, vec!["_F18.ORG"]);
        doc.transaction(entries, &removals).unwrap();
        let at = doc.archive.find("_F18.PIC").unwrap();
        assert!(!doc.entry_changed(&doc.archive.entries[at]));
        assert_eq!(doc.archive.entries[at].read().unwrap(), texture());
        assert!(doc.archive.find("_F18.ORG").is_none());
        assert_eq!(doc.changed_count(), 2);
        // Without a saved entry the stored original still restores exact bytes and flag.
        let (entries, removals) = restore(&doc.archive, "OTHER.PIC", None).unwrap();
        doc.transaction(entries, &removals).unwrap();
        assert_eq!(doc.archive.bytes().unwrap(), before);
        assert!(doc.undo());
        assert!(doc.archive.find("OTHER.ORG").is_some());
        assert!(doc.redo());
        assert!(doc.archive.find("OTHER.ORG").is_none());
    }
    /// Literal-only DCL stream; the end code is found by asking the decoder.
    fn implode_literals(data: &[u8]) -> Vec<u8> {
        let mut prefix = Vec::new();
        for b in data {
            prefix.push(0u8);
            prefix.extend((0..8).map(|i| (b >> i) & 1));
        }
        for len in 1..=13 {
            for code in 0..1u32 << len {
                let mut bits = prefix.clone();
                bits.push(1);
                bits.extend((0..len).rev().map(|i| ((code >> i) & 1) as u8));
                bits.extend([1u8; 8]);
                let mut out = (data.len() as u32).to_le_bytes().to_vec();
                out.extend([0, 4]);
                out.extend(
                    bits.chunks(8)
                        .map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | (b << i))),
                );
                if crate::dcl::explode(&out[4..], data.len(), 1 << 20)
                    .ok()
                    .as_deref()
                    == Some(data)
                {
                    return out;
                }
            }
        }
        panic!("no DCL end code");
    }
    #[test]
    fn compressed_original_keeps_its_flag_and_unknown_codecs_are_not_adopted() {
        let mut small = vec![0; 128];
        for (at, n) in [(2, 8u32), (6, 8), (10, 64), (14, 64)] {
            small[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        small[64..].fill(3);
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("X.PIC", implode_literals(&small)).unwrap());
        a.entries.push(Entry::new("Y.PIC", small.clone()).unwrap());
        let mut bytes = a.bytes().unwrap();
        bytes[7 + 13] = 4;
        bytes[7 + 18 + 13] = 9; // unknown codec: preserved, not decodable
        let mut doc = Document::new(Archive::parse(bytes).unwrap());
        assert_eq!(doc.archive.entries[0].read().unwrap(), small);
        let edits = vec![
            Entry::new("X.PIC", painted(&small, 7)).unwrap(),
            Entry::new("Y.PIC", painted(&small, 7)).unwrap(),
        ];
        let out = with_originals(&doc, edits, &[]);
        assert_eq!(out.len(), 3);
        assert_eq!(out[2].name, "X.ORG");
        assert_eq!(out[2].flag(), 4);
        assert!(out[2].same_payload(&doc.archive.entries[0]));
        doc.transaction(out, &[]).unwrap();
        let saved = doc.saved_entry("X.PIC").cloned();
        let (entries, removals) = restore(&doc.archive, "X.PIC", saved.as_ref()).unwrap();
        doc.transaction(entries, &removals).unwrap();
        assert_eq!(doc.archive.entries[0].flag(), 4);
        assert!(!doc.entry_changed(&doc.archive.entries[0]));
    }
    #[test]
    fn clone_copies_backup_and_panel_sheets_are_recognized() {
        let mut doc = document();
        paint(&mut doc, "_F18.PIC", 7);
        let copy = cloned(&doc.archive, "_F18.PIC", "LIVERY.PIC")
            .unwrap()
            .unwrap();
        assert_eq!(copy.name, "LIVERY.ORG");
        assert!(copy.same_payload(backup(&doc.archive, "_F18.PIC").unwrap()));
        assert!(cloned(&doc.archive, "OTHER.PIC", "COPY.PIC")
            .unwrap()
            .is_none());
        doc.transaction(vec![Entry::new("LIVERY.ORG", vec![1]).unwrap()], &[])
            .unwrap();
        assert!(cloned(&doc.archive, "_F18.PIC", "LIVERY.PIC").is_err());
        let mut sheet = vec![0; 64 + 64 * 64 + 768];
        for (at, n) in [
            (2, 64u32),
            (6, 64),
            (10, 64),
            (14, 4096),
            (18, 4160),
            (22, 768),
        ] {
            sheet[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        assert!(Pic::parse(&sheet).is_ok());
        assert!(panel_sheet(&sheet));
        assert!(!panel_sheet(&texture()[..100]));
        sheet[40] = 1;
        assert!(!panel_sheet(&sheet));
    }
}
