//! Stored resource references shared by cloning, navigation and package reports.
//! Modules are inspected as inert bytes; this is not game-runtime dependency closure.
use crate::{
    archive::{validate_name, Archive, Entry, ARCHIVE_LIMIT},
    brf::Brf,
    invalid, slice, u16_at, u32_at, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

#[derive(Clone, Debug)]
pub(crate) enum Location {
    Text(usize),
    Literal {
        at: usize,
        len: usize,
        stem_only: bool,
    },
}
/// How a stored name was found; used for review labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Evidence {
    Brf,
    /// An E2 texture record operand. `Some(false)` when no SH traversal reaches it.
    Texture(Option<bool>),
    Hud,
    Module,
    /// A damage-family member derived from the shadow name, not a stored name.
    Convention,
}
impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Brf => "BRF string",
            Self::Texture(_) => "Texture record",
            Self::Hud => "HUD texture name",
            Self::Module => "Module filename",
            Self::Convention => "Damage-family convention",
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Reference {
    pub(crate) target: String,
    pub(crate) location: Location,
    pub(crate) evidence: Evidence,
}
fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}
fn ext(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or("")
}
fn resource_extension(s: &str) -> bool {
    matches!(
        ext(s),
        "PT" | "SH"
            | "HUD"
            | "PTS"
            | "BI"
            | "JT"
            | "OT"
            | "NT"
            | "SEE"
            | "ECM"
            | "GAS"
            | "PIC"
            | "PAL"
            | "FNT"
            | "5K"
            | "8K"
            | "11K"
            | "22K"
            | "WAV"
    )
}
pub(crate) fn leaf(s: &str) -> bool {
    matches!(
        ext(s),
        "PIC" | "ORG" | "PAL" | "FNT" | "5K" | "8K" | "11K" | "22K" | "WAV"
    )
}
fn name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~.".contains(&b)
}
fn sections(bytes: &[u8]) -> Result<Vec<(usize, usize, bool)>> {
    if slice(bytes, 0, 2)? != b"MZ" {
        return Err(invalid("Expected an inert PL/PE module"));
    }
    let p = u32_at(bytes, 60)?;
    if !matches!(slice(bytes, p, 4)?, b"PL\0\0" | b"PE\0\0") || u16_at(bytes, p + 4)? != 0x14c {
        return Err(invalid("Unsupported module signature/machine"));
    }
    let count = u16_at(bytes, p + 6)?;
    let optional = u16_at(bytes, p + 20)?;
    if count == 0 || count > 32 || optional < 32 {
        return Err(invalid("Invalid module section table"));
    }
    let table = p
        .checked_add(24 + optional)
        .ok_or_else(|| invalid("Section offset overflow"))?;
    let mut ranges = Vec::new();
    let mut occupied: Vec<(usize, usize)> = Vec::new();
    for i in 0..count {
        let s = slice(bytes, table + i * 40, 40)?;
        let section_name =
            core::str::from_utf8(s[..8].split(|b| *b == 0).next().unwrap()).unwrap_or("");
        let size = u32_at(s, 16)?;
        let at = u32_at(s, 20)?;
        if size == 0 {
            continue;
        }
        slice(bytes, at, size)?;
        if at < table + count * 40 {
            return Err(invalid("Module section overlaps its headers"));
        }
        if occupied.iter().any(|(a, n)| at < *a + *n && *a < at + size) {
            return Err(invalid("Overlapping module sections"));
        }
        occupied.push((at, size));
        if matches!(section_name, "CODE" | "DATA" | ".data" | ".rdata" | ".text") {
            ranges.push((at, size.min(u32_at(s, 8)?), section_name == "CODE"));
        }
    }
    Ok(ranges)
}
/// Recognize complete null-terminated filenames in module sections, never byte
/// substrings in images/audio. Extensionless picture names require a named E2
/// operand or the established ~/underscore picture prefix and a catalog match.
pub(crate) fn references(
    name: &str,
    bytes: &[u8],
    catalog: &BTreeSet<String>,
    unresolved: &mut BTreeSet<String>,
) -> Result<Vec<Reference>> {
    if leaf(name) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    if bytes.starts_with(b"[brent's_relocatable_format]") {
        let brf = Brf::parse(bytes, ext(name))?;
        for (i, f) in brf
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == "string")
        {
            if out.len() > 4096 {
                return Err(invalid("Reference scan exceeds 4096 operands"));
            }
            let target = unquote(&f.value).to_ascii_uppercase();
            if validate_name(&target).is_ok()
                && (catalog.contains(&target) || resource_extension(&target))
            {
                out.push(Reference {
                    target,
                    location: Location::Text(i),
                    evidence: Evidence::Brf,
                });
            }
        }
    } else if bytes.starts_with(b"MZ") {
        // SH texture operands are confirmed against the record inventory, so
        // E2-like bytes inside face or vertex data are not taken for names.
        let mut shape: Option<Option<crate::shape_code::Inventory>> = None;
        for (at, len, code) in sections(bytes)? {
            let b = &bytes[at..at + len];
            let mut start = 0;
            for (i, value) in b.iter().enumerate() {
                if out.len() > 4096 {
                    return Err(invalid("Reference scan exceeds 4096 operands"));
                }
                if name_byte(*value) {
                    continue;
                }
                if *value == 0 && (1..=12).contains(&(i - start)) {
                    let token = core::str::from_utf8(&b[start..i])
                        .unwrap()
                        .to_ascii_uppercase();
                    let mut e2 = code && start >= 2 && b[start - 2..start] == [0xe2, 0];
                    let mut reached = None;
                    if e2 && ext(name) == "SH" {
                        let inventory = shape.get_or_insert_with(|| {
                            crate::shape_code::Inventory::parse(bytes)
                                .ok()
                                .filter(|v| v.code_start == at)
                        });
                        if let Some(r) = inventory
                            .as_ref()
                            .and_then(|v| v.record_index(start - 2).map(|i| &v.records[i]))
                        {
                            use crate::shape_code::Kind;
                            if r.kind == Kind::Sh(0xe2) && r.offset == start - 2 {
                                reached = Some(r.reached);
                            } else if r.kind != Kind::Opaque {
                                e2 = false;
                            }
                        }
                    }
                    const HUD_NAMES: &[usize] = &[
                        1, 0xe, 0x1b, 0x13c, 0x149, 0x156, 0x163, 0x170, 0x17d, 0x18a, 0x197,
                        0x1a4, 0x275, 0x282,
                    ];
                    let hud = ext(name) == "HUD" && code && HUD_NAMES.contains(&start);
                    let width = if e2 {
                        14
                    } else if hud {
                        13
                    } else {
                        0
                    };
                    let capacity = if width > 0
                        && start + width <= b.len()
                        && b[i..start + width].iter().all(|b| *b == 0)
                    {
                        width - 1
                    } else {
                        i - start
                    };
                    let evidence = if e2 {
                        Evidence::Texture(reached)
                    } else if hud {
                        Evidence::Hud
                    } else {
                        Evidence::Module
                    };
                    if token.contains('.') {
                        if validate_name(&token).is_ok()
                            && (catalog.contains(&token) || resource_extension(&token))
                        {
                            out.push(Reference {
                                target: token,
                                location: Location::Literal {
                                    at: at + start,
                                    len: capacity,
                                    stem_only: false,
                                },
                                evidence,
                            });
                        }
                    } else {
                        let candidate = format!("{token}.PIC");
                        if hud
                            && token.starts_with('~')
                            && token.len() >= 3
                            && !catalog.contains(&candidate)
                            && unresolved.len() < 128
                        {
                            unresolved.insert(format!("{name}: unresolved module name {token}; no matching PIC, left unchanged."));
                        }
                        if validate_name(&candidate).is_ok()
                            && (e2
                                || ((hud || token.starts_with('~') || token.starts_with('_'))
                                    && catalog.contains(&candidate)))
                        {
                            out.push(Reference {
                                target: candidate,
                                location: Location::Literal {
                                    at: at + start,
                                    len: capacity,
                                    stem_only: true,
                                },
                                evidence,
                            });
                        }
                    }
                }
                start = i + 1;
            }
        }
    } else {
        return Err(format!(
            "Cannot inspect dependencies of {name}: unsupported resource encoding"
        ));
    }
    if out.len() > 4096 {
        return Err(invalid("Reference scan exceeds 4096 operands"));
    }
    Ok(out)
}

/// Reviewed filename conventions, distinct from literal references. Optional
/// companions are included only when catalogued; a PT's damage family is required.
pub(crate) fn conventional(name: &str, bytes: &[u8], catalog: &BTreeSet<String>) -> Vec<Link> {
    let mut links = Vec::new();
    let stem = name.split('.').next().unwrap_or(name);
    if name.ends_with(".PT") {
        if let Ok(brf) = Brf::parse(bytes, "PT") {
            if let Some(pointer) = brf
                .fields
                .iter()
                .find(|f| f.label == "object.shadowShape" && f.kind == "ptr")
            {
                if let Some(field) = brf
                    .fields
                    .iter()
                    .find(|f| f.block == pointer.value && f.kind == "string")
                {
                    let shadow = unquote(&field.value).to_ascii_uppercase();
                    if let Some(base) = shadow.strip_suffix("_S.SH") {
                        for suffix in ["A", "B", "C", "D"] {
                            links.push(Link {
                                target: format!("{base}_{suffix}.SH"),
                                evidence: "Damage-family convention",
                            });
                        }
                    }
                }
            }
            let hud = format!("{stem}.HUD");
            if brf
                .fields
                .iter()
                .any(|f| f.label == "object.hudName" && f.kind == "dword" && f.value == "0")
                && catalog.contains(&hud)
            {
                links.push(Link {
                    target: hud,
                    evidence: "Default HUD convention",
                });
            }
        }
    }
    if matches!(ext(name), "JT" | "SEE" | "ECM" | "GAS") {
        let icon = format!("${stem}.PIC");
        if catalog.contains(&icon) {
            links.push(Link {
                target: icon,
                evidence: "Store-icon convention",
            });
        }
    }
    links
}

/// A unique observed filename. A stored literal is evidence of a reference,
/// not proof that the original executable will visit it in every state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub target: String,
    pub evidence: &'static str,
}
#[derive(Clone, Debug, Default)]
pub struct Inspection {
    pub links: Vec<Link>,
    pub notes: Vec<String>,
    pub unavailable: Option<String>,
}
struct Cached {
    source: Entry,
    inspection: Inspection,
    decoded_len: usize,
}
/// Keeps only filenames, diagnostics and shared source handles, not decoded
/// models or textures. Selection changes reuse the index; edits rescan changed
/// payloads. Catalog changes invalidate scans of extensionless module names.
#[derive(Default)]
pub struct Index {
    entries: BTreeMap<String, Cached>,
    incoming: BTreeMap<String, BTreeSet<String>>,
    catalog: BTreeSet<String>,
}
impl Index {
    pub fn update(&mut self, archive: &Archive) -> bool {
        self.update_with(archive, &BTreeSet::new())
    }
    pub fn update_with(&mut self, archive: &Archive, external: &BTreeSet<String>) -> bool {
        let mut catalog = external.clone();
        for e in &archive.entries {
            catalog.insert(e.name.clone());
        }
        let catalog_changed = catalog != self.catalog;
        if !catalog_changed
            && self.entries.len() == archive.entries.len()
            && archive.entries.iter().all(|e| {
                self.entries
                    .get(&e.name)
                    .is_some_and(|old| old.source.same_storage(e))
            })
        {
            return false;
        }
        self.catalog = catalog.clone();
        let mut entries = BTreeMap::new();
        let mut decoded = 0usize;
        let mut links = 0usize;
        for e in &archive.entries {
            let cached = self
                .entries
                .remove(&e.name)
                .filter(|old| !catalog_changed && old.source.same_storage(e));
            let mut node = if let Some(old) = cached {
                old
            } else {
                let mut inspection = Inspection::default();
                let mut decoded_len = 0;
                if leaf(&e.name) {
                    // Raster/sample data cannot contain outgoing resource names.
                } else if decoded >= ARCHIVE_LIMIT || links >= 65536 {
                    inspection.unavailable =
                        Some("Reference scan budget reached; dependencies not inspected".into());
                } else {
                    match e.read() {
                        Ok(bytes) => {
                            decoded_len = bytes.len();
                            if decoded + decoded_len > ARCHIVE_LIMIT {
                                inspection.unavailable = Some(
                                    "Reference scan exceeds 128 MiB; dependencies not inspected"
                                        .into(),
                                );
                            } else {
                                let mut notes = BTreeSet::new();
                                match references(&e.name, &bytes, &catalog, &mut notes) {
                                    Ok(refs) => {
                                        let mut unique = BTreeMap::new();
                                        for r in refs {
                                            if r.target != e.name {
                                                unique.insert(r.target, r.evidence.label());
                                            }
                                        }
                                        for link in conventional(&e.name, &bytes, &catalog) {
                                            unique.entry(link.target).or_insert(link.evidence);
                                        }
                                        if links + unique.len() > 65536 {
                                            inspection.unavailable = Some("Reference scan exceeds 65536 links; dependencies not inspected".into());
                                        } else {
                                            inspection.links = unique
                                                .into_iter()
                                                .map(|(target, evidence)| Link { target, evidence })
                                                .collect();
                                        }
                                    }
                                    Err(error) => inspection.unavailable = Some(error),
                                }
                                inspection.notes = notes.into_iter().collect();
                            }
                        }
                        Err(error) => inspection.unavailable = Some(error),
                    }
                }
                Cached {
                    source: e.clone(),
                    inspection,
                    decoded_len,
                }
            };
            if decoded.saturating_add(node.decoded_len) > ARCHIVE_LIMIT
                || links + node.inspection.links.len() > 65536
            {
                node.inspection.links.clear();
                node.inspection.unavailable =
                    Some("Reference scan budget reached; dependencies not inspected".into());
            }
            decoded = decoded.saturating_add(node.decoded_len);
            links += node.inspection.links.len();
            entries.insert(e.name.clone(), node);
        }
        self.entries = entries;
        self.incoming.clear();
        for (source, node) in &self.entries {
            for link in &node.inspection.links {
                self.incoming
                    .entry(link.target.clone())
                    .or_default()
                    .insert(source.clone());
            }
        }
        true
    }
    pub fn get(&self, name: &str) -> Option<&Inspection> {
        self.entries
            .get(&name.to_ascii_uppercase())
            .map(|c| &c.inspection)
    }
    pub fn incoming(&self, name: &str) -> impl Iterator<Item = &String> {
        self.incoming
            .get(&name.to_ascii_uppercase())
            .into_iter()
            .flatten()
    }
    /// Transitive aircraft users, with cycle protection. Excludes the selected
    /// aircraft itself. Results cover this LIB's observed stored references only.
    pub fn aircraft_users(&self, name: &str) -> Vec<String> {
        let name = name.to_ascii_uppercase();
        let mut visited = BTreeSet::new();
        visited.insert(name.clone());
        let mut pending = vec![name];
        let mut users = BTreeSet::new();
        while let Some(n) = pending.pop() {
            for source in self.incoming(&n) {
                if visited.insert(source.clone()) {
                    if source.ends_with(".PT") {
                        users.insert(source.clone());
                    }
                    pending.push(source.clone());
                }
            }
        }
        users.into_iter().collect()
    }
    pub fn unavailable_count(&self) -> usize {
        self.entries
            .values()
            .filter(|n| n.inspection.unavailable.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::Document, model, picture};
    fn brf(name: &str, references: &[&str]) -> Entry {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for target in references {
            text.push_str(&format!("string \"{target}\"\n"));
        }
        text.push_str("end\n");
        Entry::new(name, text.into_bytes()).unwrap()
    }
    #[test]
    fn reverse_users_follow_cycles_and_invalidate_on_edit_and_undo() {
        let mut archive = Archive::empty();
        archive.entries = vec![
            brf("ONE.PT", &["ONE.PT", "BODY.SH", "CYCLE.OT"]),
            brf("TWO.PT", &["BODY.SH"]),
            brf("CYCLE.OT", &["ONE.PT"]),
            Entry::new("BODY.SH", model::demo_textured()).unwrap(),
            Entry::new("DEMO.PIC", picture::demo()).unwrap(),
            Entry::new("SOUND.11K", b"GHOST.SH\0".to_vec()).unwrap(),
        ];
        let before = archive.bytes().unwrap();
        let mut doc = Document::new(archive);
        let mut index = Index::default();
        assert!(index.update(&doc.archive));
        assert!(!index.update(&doc.archive));
        assert_eq!(index.aircraft_users("demo.pic"), vec!["ONE.PT", "TWO.PT"]);
        assert_eq!(
            index.incoming("DEMO.PIC").cloned().collect::<Vec<_>>(),
            vec!["BODY.SH"]
        );
        assert!(!index
            .get("ONE.PT")
            .unwrap()
            .links
            .iter()
            .any(|r| r.target == "ONE.PT"));
        assert!(index.get("SOUND.11K").unwrap().links.is_empty());
        assert_eq!(doc.archive.bytes().unwrap(), before);
        doc.replace(3, model::demo_shape()).unwrap();
        assert!(index.update(&doc.archive));
        assert!(index.aircraft_users("DEMO.PIC").is_empty());
        assert!(doc.undo());
        index.update(&doc.archive);
        assert_eq!(index.aircraft_users("DEMO.PIC"), vec!["ONE.PT", "TWO.PT"]);
        assert_eq!(doc.archive.bytes().unwrap(), before);
    }
    #[test]
    fn catalog_changes_rescan_extensionless_names_and_missing_names_stay_visible() {
        let mut bytes = model::demo_textured();
        bytes.extend(b"\0_EXTRA\0");
        let size = (bytes.len() - 256) as u32;
        bytes[128..132].copy_from_slice(&size.to_le_bytes());
        bytes[136..140].copy_from_slice(&size.to_le_bytes());
        let mut archive = Archive::empty();
        archive.entries.push(Entry::new("BODY.SH", bytes).unwrap());
        let mut index = Index::default();
        index.update(&archive);
        assert_eq!(index.get("BODY.SH").unwrap().links[0].target, "DEMO.PIC");
        assert!(!index
            .get("BODY.SH")
            .unwrap()
            .links
            .iter()
            .any(|r| r.target == "_EXTRA.PIC"));
        archive
            .entries
            .push(Entry::new("_EXTRA.PIC", picture::demo()).unwrap());
        index.update(&archive);
        assert!(index
            .get("BODY.SH")
            .unwrap()
            .links
            .iter()
            .any(|r| r.target == "_EXTRA.PIC"));
        archive.entries.pop();
        index.update(&archive);
        assert!(!index
            .get("BODY.SH")
            .unwrap()
            .links
            .iter()
            .any(|r| r.target == "_EXTRA.PIC"));
        archive.entries[0] = Entry::new("BODY.SH", b"MZ".to_vec()).unwrap();
        index.update(&archive);
        assert!(index.get("BODY.SH").unwrap().unavailable.is_some());
        assert_eq!(index.incoming("DEMO.PIC").count(), 0);
    }
    #[test]
    fn e2_like_bytes_inside_face_data_are_not_texture_names() {
        let mut bytes = model::demo_textured();
        // First face's texture coordinates spell E2 00 'B' 00, as F14_C.SH does.
        let uv = 256 + 16 + 42 + 9;
        bytes[uv..uv + 4].copy_from_slice(&[0xe2, 0, b'B', 0]);
        let inventory = crate::shape_code::Inventory::parse(&bytes).unwrap();
        let face = &inventory.records[inventory.record_index(uv - 256).unwrap()];
        assert_eq!(face.kind, crate::shape_code::Kind::Sh(0xfc));
        let mut notes = BTreeSet::new();
        let refs = references("BODY.SH", &bytes, &BTreeSet::new(), &mut notes).unwrap();
        let targets: Vec<_> = refs.iter().map(|r| r.target.as_str()).collect();
        assert_eq!(targets, ["DEMO.PIC"]);
        assert_eq!(refs[0].evidence, Evidence::Texture(Some(true)));
        // Without an SH record inventory the bounded literal heuristic still applies.
        let refs = references("BODY.HUD", &bytes, &BTreeSet::new(), &mut notes).unwrap();
        assert!(refs.iter().any(|r| r.target == "B.PIC"));
    }
    #[test]
    fn texture_records_carry_static_reachability() {
        let mut a = crate::shape_testkit::Asm::default();
        a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
        a.b(&[0xf2, 0]).rel16("end", 2);
        a.b(&[0xe2, 0]).b(b"SKIN.PIC\0\0\0\0\0\0");
        a.verts(0, &[[0, 0, 0], [10, 0, 0], [0, 10, 0]]);
        a.face(0x23, 32, None, &[0, 1, 2], &[]);
        a.jump("end");
        a.b(&[0xe2, 0]).b(b"SPARE.PIC\0\0\0\0\0");
        a.label("end")
            .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
        let bytes = a.finish();
        let refs = references("BODY.SH", &bytes, &BTreeSet::new(), &mut BTreeSet::new()).unwrap();
        let found: Vec<_> = refs
            .iter()
            .map(|r| (r.target.as_str(), r.evidence))
            .collect();
        assert_eq!(
            found,
            [
                ("SKIN.PIC", Evidence::Texture(Some(true))),
                ("SPARE.PIC", Evidence::Texture(Some(false)))
            ]
        );
    }
    #[test]
    fn too_many_reference_operands_fail_explicitly() {
        let targets = vec!["BODY.SH"; 4097];
        let entry = brf("ONE.PT", &targets);
        let mut archive = Archive::empty();
        archive.entries.push(entry);
        let mut index = Index::default();
        index.update(&archive);
        assert!(index
            .get("ONE.PT")
            .unwrap()
            .unavailable
            .as_ref()
            .unwrap()
            .contains("4096"));
    }
}
