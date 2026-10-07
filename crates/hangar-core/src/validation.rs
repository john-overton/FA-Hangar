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
            Entry::new("DEMO.PIC", picture::demo()).unwrap(),
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
        assert_eq!(report.errors, 1);
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
