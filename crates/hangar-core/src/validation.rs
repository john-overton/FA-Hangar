//! Advisory package checks. Archive integrity is distinct from runtime behavior.
use crate::{
    archive::{Archive, ARCHIVE_LIMIT},
    audio::Pcm,
    brf::Brf,
    dependencies::Index,
    document::Document,
    model::Model,
    picture::{self, Pic},
    Result,
};
use alloc::{string::String, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Pass,
    Warning,
    Error,
    Info,
}
impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
            Self::Info => "INFO",
        }
    }
}
#[derive(Debug)]
pub struct Check {
    pub level: Level,
    pub entry: Option<String>,
    pub message: String,
}
#[derive(Default, Debug)]
pub struct Report {
    pub checks: Vec<Check>,
    pub errors: usize,
    pub warnings: usize,
    pub omitted: usize,
}
impl Report {
    pub fn add(&mut self, level: Level, entry: Option<&str>, message: String) {
        self.errors += usize::from(level == Level::Error);
        self.warnings += usize::from(level == Level::Warning);
        // Bound report memory even for archives with thousands of broken links.
        if self.checks.len() < 2048 {
            self.checks.push(Check {
                level,
                entry: entry.map(String::from),
                message,
            });
        } else {
            self.omitted += 1;
        }
    }
    pub fn summary(&self) -> String {
        format!(
            "{}: {} errors / {} warnings",
            if self.errors > 0 {
                "ERROR"
            } else if self.warnings > 0 {
                "WARN"
            } else {
                "PASS"
            },
            self.errors,
            self.warnings
        )
    }
}
pub fn text(doc: &Document, report: &Report) -> String {
    let mut out = format!(
        "T.O.R.E Hangar package report\n{}\n\nChanges\n",
        report.summary()
    );
    for (name, kind) in doc.changes() {
        out.push_str(&format!("{} {name}\n", kind.label()));
    }
    out.push_str("\nChecks\n");
    for check in &report.checks {
        out.push_str(&format!(
            "{} {}: {}\n",
            check.level.label(),
            check.entry.as_deref().unwrap_or("Package"),
            check.message
        ));
    }
    if report.omitted > 0 {
        out.push_str(&format!("{} further results omitted\n", report.omitted));
    }
    out
}
fn payload(name: &str, bytes: &[u8]) -> Option<Result<()>> {
    if bytes.starts_with(b"[brent's_relocatable_format]") {
        return Some(
            Brf::parse(bytes, name.rsplit('.').next().unwrap_or("")).and_then(|b| {
                if b.issues.is_empty() {
                    Ok(())
                } else {
                    Err(b.issues.join("; "))
                }
            }),
        );
    }
    match name.rsplit('.').next().unwrap_or("") {
        "SH" => Some(Model::parse(bytes).map(|_| ())),
        "PIC" | "ORG" => Some(Pic::parse(bytes).map(|_| ())),
        "PAL" => Some(picture::palette(bytes).map(|_| ())),
        "5K" | "11K" | "WAV" => Some(Pcm::parse(name, bytes).map(|_| ())),
        _ => None,
    }
}
/// Stored originals (`X.ORG`) are editor backups; FA does not load them by name.
fn originals(doc: &Document, report: &mut Report) {
    let mut decoded = 0usize;
    let mut count = 0usize;
    for e in &doc.archive.entries {
        let Some(pic) = crate::originals::texture_of(&e.name) else {
            continue;
        };
        if decoded >= ARCHIVE_LIMIT {
            report.add(
                Level::Warning,
                Some(&e.name),
                "Stored original check budget reached; unverified".into(),
            );
            break;
        }
        let Some(original) = e.read().ok().and_then(|b| {
            decoded = decoded.saturating_add(b.len());
            Pic::parse(&b).ok()
        }) else {
            report.add(
                Level::Info,
                Some(&e.name),
                "Not a PIC payload; not treated as a stored original".into(),
            );
            continue;
        };
        count += 1;
        let Some(at) = doc.archive.find(&pic) else {
            report.add(
                Level::Warning,
                Some(&e.name),
                format!("Stored original without {pic}; restore the texture or remove it"),
            );
            continue;
        };
        let current = doc.archive.entries[at].read().ok().and_then(|b| {
            decoded = decoded.saturating_add(b.len());
            Pic::parse(&b).ok()
        });
        if current.is_some_and(|p| !p.same_layout(&original)) {
            report.add(
                Level::Info,
                Some(&e.name),
                format!("Raster layout differs from {pic}; Eraser unavailable, Restore texture replaces the whole entry"),
            );
        }
    }
    if count > 0 {
        report.add(
            Level::Info,
            None,
            format!(
                "{count} stored original texture{} (.ORG); remove them for distribution builds",
                if count == 1 { "" } else { "s" }
            ),
        );
    }
}
/// Message prefix of the Package error for a texture FA cannot map.
pub const TEXTURE_LAYOUT_ERROR: &str = "Would crash FA's texture mapper";
/// A local PIC that FA's polygon texture mapper would read, but that is not
/// in the retail texture layout (`picture::is_texture_layout`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextureProblem {
    pub pic: String,
    /// The SH entries whose textured faces draw it.
    pub shapes: Vec<String>,
    pub reason: String,
}
/// Local PICs named by SH texture records (the dependency index confirms E2
/// operands against the record inventory) that are not in the retail texture
/// layout and that a textured face of that SH draws. A PIC named only by
/// other records, such as MOON.SH's sprite, is not read by the texture
/// mapper; when the SH does not decode, the texture record alone counts.
/// `index` must be up to date for `archive`.
pub fn texture_layout_problems(archive: &Archive, index: &Index) -> Vec<TextureProblem> {
    let mut layouts: alloc::collections::BTreeMap<String, Option<String>> = Default::default();
    let mut found: alloc::collections::BTreeMap<String, TextureProblem> = Default::default();
    let mut decoded = 0usize;
    for e in archive.entries.iter().filter(|e| e.name.ends_with(".SH")) {
        let Some(scan) = index.get(&e.name) else {
            continue;
        };
        let mut model: Option<Option<Vec<String>>> = None;
        for link in scan.links.iter().filter(|l| l.evidence == "Texture record") {
            let Some(at) = archive.find(&link.target) else {
                continue;
            };
            if !link.target.ends_with(".PIC") {
                continue;
            }
            let reason = layouts
                .entry(link.target.clone())
                .or_insert_with(|| {
                    if decoded >= ARCHIVE_LIMIT {
                        return None;
                    }
                    let bytes = archive.entries[at].read().ok()?;
                    decoded = decoded.saturating_add(bytes.len());
                    picture::retail_texture_check(&bytes).err()
                })
                .clone();
            let Some(reason) = reason else {
                continue;
            };
            let faces = model.get_or_insert_with(|| textured_faces(e));
            let drawn = faces.as_ref().is_none_or(|names| {
                names
                    .iter()
                    .any(|n| n.is_empty() || n.eq_ignore_ascii_case(&link.target))
            });
            if drawn {
                found
                    .entry(link.target.clone())
                    .or_insert_with(|| TextureProblem {
                        pic: link.target.clone(),
                        shapes: Vec::new(),
                        reason,
                    })
                    .shapes
                    .push(e.name.clone());
            }
        }
    }
    found.into_values().collect()
}
/// Texture names of an SH's textured polygons. When the model reader cannot
/// decode it, a textured FC record in the record inventory counts as an
/// unnamed one (empty name: any texture record may feed it); `None` when
/// neither reader understands the shape.
fn textured_faces(e: &crate::archive::Entry) -> Option<Vec<String>> {
    let bytes = e.read().ok()?;
    if let Ok(m) = Model::parse(&bytes) {
        return Some(
            m.faces
                .iter()
                .filter(|f| f.sub & 4 != 0)
                .map(|f| f.texture.clone())
                .collect(),
        );
    }
    let inventory = crate::shape_code::Inventory::parse(&bytes).ok()?;
    let textured = inventory.records.iter().any(|r| {
        r.kind == crate::shape_code::Kind::Sh(0xfc)
            && bytes
                .get(inventory.code_start + r.offset + 1)
                .is_some_and(|sub| sub & 4 != 0)
    });
    Some(if textured {
        vec![String::new()]
    } else {
        Vec::new()
    })
}
/// Retail-layout replacements for every `texture_layout_problems` PIC, plus
/// its stored original (`X.ORG`) when that is not in the layout either, so
/// Restore texture and the Eraser never bring the crashing layout back. No SH
/// changes: every texel keeps its (u, v).
#[derive(Debug, Default)]
pub struct TextureRepair {
    pub entries: Vec<crate::archive::Entry>,
    /// Repaired PICs and what happened to their embedded palette.
    pub repaired: Vec<(String, picture::PaletteCheck)>,
    /// Stored originals converted alongside their PIC.
    pub originals: Vec<String>,
    /// PICs that cannot keep their UVs in a retail texture, with the reason.
    pub refused: Vec<(String, String)>,
}
pub fn repair_textures(
    archive: &Archive,
    index: &mut Index,
    game: Option<&[[u8; 3]; 256]>,
) -> Result<TextureRepair> {
    index.update(archive);
    let mut out = TextureRepair::default();
    for problem in texture_layout_problems(archive, index) {
        let at = archive
            .find(&problem.pic)
            .ok_or_else(|| format!("{} is missing", problem.pic))?;
        let converted = match picture::to_retail_texture(&archive.entries[at].read()?, game) {
            Ok(c) => c,
            Err(e) => {
                out.refused.push((problem.pic, e));
                continue;
            }
        };
        out.entries
            .push(crate::archive::Entry::new(&problem.pic, converted.bytes)?);
        if let Some(org) = crate::originals::backup(archive, &problem.pic) {
            let bytes = org.read()?;
            if picture::retail_texture_check(&bytes).is_err() {
                if let Ok(c) = picture::to_retail_texture(&bytes, game) {
                    out.entries
                        .push(crate::archive::Entry::new(&org.name, c.bytes)?);
                    out.originals.push(org.name.clone());
                }
            }
        }
        out.repaired.push((problem.pic, converted.palette));
    }
    Ok(out)
}
/// Package errors for `texture_layout_problems`.
fn texture_layouts(doc: &Document, index: &Index, report: &mut Report) {
    for p in texture_layout_problems(&doc.archive, index) {
        report.add(
            Level::Error,
            Some(&p.pic),
            format!(
                "{TEXTURE_LAYOUT_ERROR}: {}. Drawn by {}. Repair textures for FA rewrites it as a retail texture (kind 0, 256 wide, row table, no palette); no SH changes",
                p.reason,
                p.shapes.join(", ")
            ),
        );
    }
}
/// Updates the supplied index. Checks do not modify source data or prohibit
/// packages which intentionally use external resources.
pub fn inspect(doc: &Document, index: &mut Index) -> Report {
    inspect_with(doc, index, &alloc::collections::BTreeMap::new())
}
/// External catalogs identify providers; they do not prove game load order.
pub fn inspect_with(
    doc: &Document,
    index: &mut Index,
    providers: &alloc::collections::BTreeMap<String, Vec<String>>,
) -> Report {
    let mut catalog = alloc::collections::BTreeSet::new();
    for name in providers.keys() {
        catalog.insert(name.clone());
    }
    index.update_with(&doc.archive, &catalog);

    let mut report = Report::default();
    match doc.archive.bytes().and_then(Archive::parse) {
        Ok(output) => {
            report.add(
                Level::Pass,
                None,
                format!(
                    "Directory offsets and EOF verified / {} entries",
                    output.entries.len()
                ),
            );
            let mut preserved = 0;
            // Serialization retains entry order, so no quadratic name lookup is needed.
            for (source, written) in doc.archive.entries.iter().zip(&output.entries) {
                if !doc.entry_changed(source) {
                    if source.name != written.name
                        || source.flag() != written.flag()
                        || source.stored() != written.stored()
                    {
                        report.add(
                            Level::Error,
                            Some(&source.name),
                            "Untouched payload or compression changed".into(),
                        );
                    } else {
                        preserved += 1;
                    }
                }
            }
            report.add(
                Level::Pass,
                None,
                format!("{preserved} untouched payloads retain bytes and compression"),
            );
        }
        Err(error) => report.add(Level::Error, None, error),
    }
    let mut decoded = 0usize;
    let mut checked = 0usize;
    let mut local = 0usize;
    let mut names = alloc::collections::BTreeSet::new();
    for e in &doc.archive.entries {
        names.insert(e.name.as_str());
    }
    for e in &doc.archive.entries {
        if let Some(scan) = index.get(&e.name) {
            if let Some(error) = &scan.unavailable {
                report.add(
                    Level::Warning,
                    Some(&e.name),
                    format!("Dependencies unverified: {error}"),
                );
            }
            for note in &scan.notes {
                report.add(Level::Warning, Some(&e.name), note.clone());
            }
            for link in &scan.links {
                if names.contains(link.target.as_str()) {
                    local += 1;
                } else if let Some(sources) = providers.get(&link.target) {
                    report.add(
                        if sources.len() == 1 {
                            Level::Info
                        } else {
                            Level::Warning
                        },
                        Some(&e.name),
                        format!(
                            "{} / {}: {} / catalog only, payload unverified",
                            link.target,
                            if sources.len() == 1 {
                                "external provider"
                            } else {
                                "ambiguous providers"
                            },
                            sources.join(", ")
                        ),
                    );
                } else {
                    report.add(
                        Level::Warning,
                        Some(&e.name),
                        format!(
                            "{} not in this LIB or source catalogs / {}",
                            link.target, link.evidence
                        ),
                    );
                }
            }
        }
        if doc.entry_changed(e) {
            if decoded >= ARCHIVE_LIMIT {
                report.add(
                    Level::Warning,
                    Some(&e.name),
                    "Payload check budget reached; unverified".into(),
                );
                continue;
            }
            match e.read() {
                Ok(bytes) => {
                    decoded = decoded.saturating_add(bytes.len());
                    if decoded > ARCHIVE_LIMIT {
                        report.add(
                            Level::Warning,
                            Some(&e.name),
                            "Payload checks exceed 128 MiB; unverified".into(),
                        );
                        continue;
                    }
                    match payload(&e.name, &bytes) {
                        Some(Ok(())) => checked += 1,
                        Some(Err(error)) => report.add(
                            Level::Error,
                            Some(&e.name),
                            format!("Supported payload check failed: {error}"),
                        ),
                        None => report.add(
                            Level::Warning,
                            Some(&e.name),
                            "No payload validator for this encoding; preserved as supplied".into(),
                        ),
                    }
                }
                Err(error) => report.add(Level::Error, Some(&e.name), error),
            }
        }
    }
    report.add(
        Level::Pass,
        None,
        format!("{checked} changed payloads pass supported format checks"),
    );
    texture_layouts(doc, index, &mut report);
    originals(doc, &mut report);
    report.add(
        Level::Info,
        None,
        format!("{local} observed references resolve inside this LIB"),
    );
    report.add(Level::Info, None, "Scope: stored names and reviewed damage/HUD/store conventions; other runtime lookups and game behavior remain unverified".into());
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{archive::Entry, document::ChangeKind, model};
    fn document() -> Document {
        let mut archive = Archive::empty();
        archive.entries = vec![
            Entry::new("BODY.SH", model::demo_textured()).unwrap(),
            Entry::new("DEMO.PIC", texture()).unwrap(),
        ];
        Document::new(archive)
    }
    #[test]
    fn removed_texture_is_reported_and_undo_restores_validation() {
        let mut doc = document();
        let mut index = Index::default();
        let before = doc.archive.bytes().unwrap();
        let report = inspect(&doc, &mut index);
        assert_eq!((report.errors, report.warnings), (0, 0));
        assert_eq!(doc.archive.bytes().unwrap(), before);
        doc.remove(1).unwrap();
        assert_eq!(
            doc.changes(),
            vec![("DEMO.PIC".into(), ChangeKind::Removed)]
        );
        let report = inspect(&doc, &mut index);
        assert_eq!(report.errors, 0);
        assert!(report
            .checks
            .iter()
            .any(|c| c.entry.as_deref() == Some("BODY.SH")
                && c.message.contains("DEMO.PIC not in this LIB")));
        doc.undo();
        let report = inspect(&doc, &mut index);
        assert_eq!((report.errors, report.warnings), (0, 0));
        assert!(doc.changes().is_empty());
        assert_eq!(doc.archive.bytes().unwrap(), before);
    }
    #[test]
    fn invalid_changed_payloads_and_unverified_formats_are_distinct() {
        let mut doc = document();
        doc.replace(1, b"bad PIC".to_vec()).unwrap();
        doc.import("UNKNOWN.BIN", b"opaque".to_vec()).unwrap();
        assert_eq!(
            doc.changes(),
            vec![
                ("DEMO.PIC".into(), ChangeKind::Modified),
                ("UNKNOWN.BIN".into(), ChangeKind::Added)
            ]
        );
        let report = inspect(&doc, &mut Index::default());
        // The payload check and the texture mapper check both fail.
        assert_eq!(report.errors, 2);
        assert!(report.warnings > 0);
        assert!(report
            .checks
            .iter()
            .any(|c| c.entry.as_deref() == Some("DEMO.PIC") && c.level == Level::Error));
        assert!(report
            .checks
            .iter()
            .any(|c| c.entry.as_deref() == Some("UNKNOWN.BIN") && c.level == Level::Warning));
        doc.mark_saved();
        assert!(doc.changes().is_empty());
        doc.undo();
        assert_eq!(
            doc.changes(),
            vec![("UNKNOWN.BIN".into(), ChangeKind::Removed)]
        );
    }
    #[test]
    fn stored_originals_are_checked_as_pictures_and_orphans_warn() {
        let mut doc = document();
        let backup = doc.archive.entries[1].renamed("DEMO.ORG").unwrap();
        doc.transaction(vec![backup], &[]).unwrap();
        let report = inspect(&doc, &mut Index::default());
        assert_eq!((report.errors, report.warnings), (0, 0));
        assert!(report
            .checks
            .iter()
            .any(|c| c.message.contains("1 stored original texture (.ORG)")));
        let mut small = vec![0; 128];
        for (at, n) in [(2, 8u32), (6, 8), (10, 64), (14, 64)] {
            small[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        doc.transaction(vec![Entry::new("DEMO.PIC", small).unwrap()], &[])
            .unwrap();
        let report = inspect(&doc, &mut Index::default());
        assert!(report.checks.iter().any(|c| c.level == Level::Info
            && c.entry.as_deref() == Some("DEMO.ORG")
            && c.message.contains("layout differs")));
        doc.transaction(Vec::new(), &["DEMO.PIC".into()]).unwrap();
        let report = inspect(&doc, &mut Index::default());
        assert!(report.checks.iter().any(|c| c.level == Level::Warning
            && c.entry.as_deref() == Some("DEMO.ORG")
            && c.message.contains("without DEMO.PIC")));
        // A changed ORG that is not a PIC fails its payload check but is never a backup.
        doc.transaction(vec![Entry::new("DEMO.ORG", b"text".to_vec()).unwrap()], &[])
            .unwrap();
        let report = inspect(&doc, &mut Index::default());
        assert_eq!(report.errors, 1);
        assert!(!report
            .checks
            .iter()
            .any(|c| c.message.contains("without DEMO.PIC")));
    }
    #[test]
    fn report_limit_preserves_error_counts() {
        let mut doc = document();
        for i in 0..2050 {
            doc.archive
                .entries
                .push(Entry::new(&format!("P{i}.PIC"), vec![0]).unwrap());
        }
        doc.archive.changed();
        let report = inspect(&doc, &mut Index::default());
        assert_eq!(report.errors, 2050);
        assert_eq!(report.checks.len(), 2048);
        assert!(report.omitted > 0);
    }
    /// DEMO.PIC in the retail texture layout.
    fn texture() -> Vec<u8> {
        picture::to_retail_texture(&picture::demo(), None)
            .unwrap()
            .bytes
    }
    /// An SH whose only texture record names SKY.PIC while every face stays
    /// flat, like a sprite record's texture.
    fn sprite_shape() -> Vec<u8> {
        let original = model::demo_shape();
        let mut code = vec![0xe2, 0];
        code.extend(b"SKY.PIC\0\0\0\0\0\0\0");
        code.extend(&original[256..]);
        let mut out = original[..256].to_vec();
        for at in [128, 136] {
            out[at..at + 4].copy_from_slice(&(code.len() as u32).to_le_bytes());
        }
        out.extend(code);
        out
    }
    #[test]
    fn legacy_textures_are_errors_and_repair_keeps_every_shape() {
        let legacy = picture::demo();
        let mut archive = Archive::empty();
        archive.entries = vec![
            Entry::new("BODY.SH", model::demo_textured()).unwrap(),
            Entry::new("DEMO.PIC", legacy.clone()).unwrap(),
            Entry::new("DEMO.ORG", legacy.clone()).unwrap(),
            Entry::new("SKY.SH", sprite_shape()).unwrap(),
            Entry::new("SKY.PIC", legacy.clone()).unwrap(),
        ];
        let mut doc = Document::new(archive);
        let before = doc.archive.bytes().unwrap();
        let mut index = Index::default();
        let report = inspect(&doc, &mut index);
        assert_eq!(report.errors, 1, "{}", text(&doc, &report));
        let check = report
            .checks
            .iter()
            .find(|c| c.level == Level::Error)
            .unwrap();
        assert_eq!(check.entry.as_deref(), Some("DEMO.PIC"));
        for part in [
            "Would crash FA's texture mapper",
            "32 pixels wide, not 256",
            "no row-offset table",
            "embedded 768-byte palette",
            "Drawn by BODY.SH",
        ] {
            assert!(check.message.contains(part), "{}", check.message);
        }
        // The game palette is the one the legacy sheet embedded.
        let game = Pic::parse(&legacy).unwrap().colors(&[[0; 3]; 256]);
        let repair = repair_textures(&doc.archive, &mut index, Some(&game)).unwrap();
        assert_eq!(
            repair.repaired,
            vec![("DEMO.PIC".into(), picture::PaletteCheck::Same)]
        );
        assert_eq!(repair.originals, vec![String::from("DEMO.ORG")]);
        assert!(repair.refused.is_empty());
        doc.transaction(repair.entries, &[]).unwrap();
        let report = inspect(&doc, &mut index);
        assert_eq!(report.errors, 0, "{}", text(&doc, &report));
        // No SH changed; the texture keeps every texel at its UV.
        let changed: Vec<_> = doc.changes().into_iter().map(|(n, _)| n).collect();
        assert_eq!(changed, ["DEMO.ORG", "DEMO.PIC"]);
        let read = |doc: &Document, name: &str| {
            doc.archive.entries[doc.archive.find(name).unwrap()]
                .read()
                .unwrap()
        };
        let pic = read(&doc, "DEMO.PIC");
        assert!(picture::retail_texture_check(&pic).is_ok());
        let (old, new) = (Pic::parse(&legacy).unwrap(), Pic::parse(&pic).unwrap());
        for y in 0..32 {
            assert_eq!(
                &new.pixels[y * 256..y * 256 + 32],
                &old.pixels[y * 32..y * 32 + 32]
            );
        }
        assert!(new.same_layout(&Pic::parse(&read(&doc, "DEMO.ORG")).unwrap()));
        assert_eq!(read(&doc, "SKY.PIC"), legacy);
        // Nothing left to repair; one undo restores the archive.
        let again = repair_textures(&doc.archive, &mut index, Some(&game)).unwrap();
        assert!(again.entries.is_empty() && again.repaired.is_empty());
        doc.undo();
        assert_eq!(doc.archive.bytes().unwrap(), before);
        // Without a game palette indices are kept and flagged; a PIC wider
        // than 256 cannot keep its UVs and is refused.
        let repair = repair_textures(&doc.archive, &mut index, None).unwrap();
        assert_eq!(repair.repaired[0].1, picture::PaletteCheck::Unverified);
        let mut wide = vec![0; 64 + 300 * 2];
        for (at, n) in [(2, 300u32), (6, 2), (10, 64), (14, 600)] {
            wide[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        doc.transaction(vec![Entry::new("DEMO.PIC", wide).unwrap()], &[])
            .unwrap();
        let repair = repair_textures(&doc.archive, &mut index, None).unwrap();
        assert!(repair.entries.is_empty());
        assert!(repair.refused[0].1.contains("300 pixels wide"));
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;
    use crate::archive::Entry;
    use alloc::collections::BTreeMap;
    #[test]
    fn external_providers_and_conventions_remain_distinct_from_local_payload_checks() {
        let mut archive = Archive::empty();
        archive
            .entries
            .push(Entry::new("DEMO.PT", crate::brf::demo()).unwrap());
        let doc = Document::new(archive);
        let mut index = Index::default();
        let initial = inspect(&doc, &mut index);
        assert_eq!(initial.warnings, 6); // main, shadow and four damage companions
        let mut sources = BTreeMap::new();
        for name in [
            "DEMO.SH",
            "DEMO_S.SH",
            "DEMO_A.SH",
            "DEMO_B.SH",
            "DEMO_C.SH",
            "DEMO_D.SH",
            "DEMO.HUD",
        ] {
            sources.insert(name.into(), vec!["SOURCE.LIB".into()]);
        }
        let report = inspect_with(&doc, &mut index, &sources);
        assert_eq!(report.warnings, 0);
        assert!(index
            .get("DEMO.PT")
            .unwrap()
            .links
            .iter()
            .any(|l| l.target == "DEMO.HUD" && l.evidence == "Default HUD convention"));
        sources.get_mut("DEMO.SH").unwrap().push("OTHER.LIB".into());
        let report = inspect_with(&doc, &mut index, &sources);
        assert_eq!(report.warnings, 1);
        assert!(text(&doc, &report).contains("ambiguous providers"));
        let report = inspect(&doc, &mut index);
        assert_eq!(report.warnings, 6);
        assert!(!index
            .get("DEMO.PT")
            .unwrap()
            .links
            .iter()
            .any(|l| l.target == "DEMO.HUD"));
    }
}
