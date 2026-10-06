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
    fn add(&mut self, level: Level, entry: Option<&str>, message: String) {
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
fn payload(name: &str, bytes: &[u8]) -> Option<Result<()>> {
    if bytes.starts_with(b"[brent's_relocatable_format]") {
        return Some(Brf::parse(bytes, name.rsplit('.').next().unwrap_or("")).map(|_| ()));
    }
    match name.rsplit('.').next().unwrap_or("") {
        "SH" => Some(Model::parse(bytes).map(|_| ())),
        "PIC" => Some(Pic::parse(bytes).map(|_| ())),
        "PAL" => Some(picture::palette(bytes).map(|_| ())),
        "5K" | "11K" | "WAV" => Some(Pcm::parse(name, bytes).map(|_| ())),
        _ => None,
    }
}
/// Updates the supplied index. Checks do not modify source data or prohibit
/// packages which intentionally use external resources.
pub fn inspect(doc: &Document, index: &mut Index) -> Report {
    index.update(&doc.archive);
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
                } else {
                    report.add(
                        Level::Warning,
                        Some(&e.name),
                        format!("{} not in this LIB / {}", link.target, link.evidence),
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
    report.add(
        Level::Info,
        None,
        format!("{local} observed references resolve inside this LIB"),
    );
    report.add(Level::Info, None, "Scope: stored names in this LIB; game-generated lookups, implicit families and runtime behavior are unverified".into());
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
