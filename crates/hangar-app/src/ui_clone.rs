use super::view::{border, label_fit, Action, Layout};
use super::*;
use alloc::collections::BTreeSet;
use hangar_core::{
    archive::{self, IndexedEntry},
    clone_aircraft,
};
struct Source {
    path: String,
    entries: Vec<IndexedEntry>,
    explicit: bool,
}
fn normalized(path: &str) -> String {
    let p = path.replace('\\', "/");
    if cfg!(windows) {
        p.to_ascii_uppercase()
    } else {
        p
    }
}
fn index(path: &str) -> Result<Option<Vec<IndexedEntry>>> {
    let size = crate::platform::file_size(path)?;
    if size < 7 {
        return Ok(None);
    }
    let head = crate::platform::read_range(path, 0, 7)?;
    if &head[..5] != b"EALIB" {
        return Ok(None);
    }
    let count = u16::from_le_bytes([head[5], head[6]]) as usize;
    let len = 7 + (count + 1) * 18;
    if len > size {
        return Err(format!("{path}: truncated LIB directory"));
    }
    let bytes = crate::platform::read_range(path, 0, len)?;
    archive::directory(&bytes, size)
        .map(Some)
        .map_err(|e| format!("{path}: {e}"))
}
impl App {
    pub(super) fn begin_clone(&mut self) {
        self.clone_draft = None;
        self.clone_scroll = 0;
        self.clone_sources.clear();
        self.browser = None;
        let stem = self.name().split('.').next().unwrap_or("NEW");
        let prefix: String = stem.chars().take(4).collect();
        let display = self
            .brf
            .as_ref()
            .and_then(|b| b.fields.iter().find(|f| f.label == "object.ot_names"))
            .and_then(|p| {
                self.brf
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .find(|f| f.block == p.value && f.kind == "string")
            })
            .map(|f| f.value.trim_matches('"'))
            .unwrap_or(stem);
        self.clone_title = format!("{} variant", display.chars().take(30).collect::<String>());
        self.prompt = Some(Prompt {
            kind: PromptKind::CloneId,
            title: format!(
                "New aircraft from {} / step 1: new ID (1..6 letters/digits)",
                self.name()
            ),
            value: format!("{prefix}V1"),
            axis: 0,
        });
        self.status =
            "The selected PT and its resource graph will be copied into a separate LIB".into();
    }
    pub(super) fn add_clone_source(&mut self, path: &str) -> Result<()> {
        if index(path)?.is_none() {
            return Err("Choose an EALIB source library".into());
        }
        self.clone_sources
            .retain(|p| normalized(p) != normalized(path));
        self.clone_sources.push(path.into());
        self.prompt = Some(Prompt {
            kind: PromptKind::CloneTitle,
            title: "New aircraft / step 2: display name".into(),
            value: self.clone_title.clone(),
            axis: 0,
        });
        self.status = "Source LIB added; press Enter to rebuild the review".into();
        Ok(())
    }
    pub(super) fn build_clone(&self) -> Result<clone_aircraft::Package> {
        let mut catalog = BTreeSet::new();
        for e in &self.doc.archive.entries {
            catalog.insert(e.name.clone());
        }
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        seen.insert(normalized(&self.path));
        let mut candidates: Vec<_> = self
            .clone_sources
            .iter()
            .rev()
            .map(|p| (p.clone(), true))
            .collect();
        if crate::platform::file_size(&self.path).is_ok() {
            let parent = Self::parent_path(&self.path);
            for item in crate::platform::list_dir(&parent)? {
                if !item.directory && item.name.to_ascii_uppercase().ends_with(".LIB") {
                    candidates.push((item.path, false));
                }
            }
        }
        for (path, explicit) in candidates {
            if !seen.insert(normalized(&path)) {
                continue;
            }
            if sources.len() >= 64 {
                return Err("More than 64 source LIBs; use a smaller source folder".into());
            }
            if let Some(entries) = index(&path)? {
                for e in &entries {
                    catalog.insert(e.name.clone());
                }
                if catalog.len() > 131072 {
                    return Err(
                        "Source catalog exceeds 131072 names; use a smaller LIB folder".into(),
                    );
                }
                sources.push(Source {
                    path,
                    entries,
                    explicit,
                });
            }
        }
        clone_aircraft::build_with(
            &catalog,
            self.name(),
            &self.variant_id,
            &self.clone_title,
            |name| {
                if let Some(i) = self.doc.archive.find(name) {
                    return self.doc.archive.entries[i].read();
                }
                let mut selected: Option<(Vec<u8>, &str)> = None;
                for source in &sources {
                    if let Some(e) = source.entries.iter().find(|e| e.name == name) {
                        let stored = crate::platform::read_range(&source.path, e.offset, e.size)?;
                        let bytes = archive::decode_payload(e.flag, stored)?;
                        if source.explicit {
                            return Ok(bytes);
                        }
                        if let Some((old, path)) = &selected {
                            if old != &bytes {
                                return Err(format!("Conflicting {name} in {path} and {}. Use Add source LIB to select the intended source",source.path));
                            }
                        } else {
                            selected = Some((bytes, &source.path));
                        }
                    }
                }
                selected
                    .map(|(bytes, _)| bytes)
                    .ok_or_else(|| format!("Missing {name}; use Add source LIB to locate it"))
            },
        )
    }
    pub(super) fn clone_review(&self, o: &mut Layout) {
        let Some(package) = &self.clone_draft else {
            return;
        };
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(660);
        let x = (self.width - w) / 2;
        let y = (self.height - h) / 2;
        o.hits.clear();
        let d = &mut o.canvas;
        d.rect(x + 4, y + 5, w, h, c::GM_1000);
        d.rect(x, y, w, h, c::GM_800);
        border(d, x, y, w, h, c::LINE_STRONG);
        d.rect(x + 1, y + 1, w - 2, 32, c::GM_700);
        label_fit(
            d,
            x + 14,
            y + 22,
            w - 28,
            &format!(
                "New aircraft / review: {} -> {}.PT",
                package.donor, package.id
            ),
            c::INK,
        );
        label_fit(
            d,
            x + 14,
            y + 55,
            w - 28,
            &format!(
                "{} private resources / source files preserved / output suggested: {}.LIB",
                package.archive.entries.len(),
                package.id
            ),
            c::STEEL,
        );
        d.rect(x + 12, y + 69, w - 24, 24, c::GM_900);
        d.label(x + 22, y + 85, "FROM DONOR", c::INK_MUTED);
        d.label(x + w / 2, y + 85, "IN NEW LIB", c::INK_MUTED);
        let rows = ((h - 266) / 22).max(1) as usize;
        for (row, (old, new)) in package
            .mapping
            .iter()
            .skip(self.clone_scroll)
            .take(rows)
            .enumerate()
        {
            let yy = y + 93 + row as i32 * 22;
            d.rect(
                x + 12,
                yy,
                w - 24,
                22,
                if row % 2 == 0 { c::GM_950 } else { c::GM_900 },
            );
            d.text(x + 22, yy + 16, old, c::INK_MUTED);
            d.text(x + w / 2, yy + 16, new, c::AMBER);
        }
        label_fit(
            d,
            x + 14,
            y + h - 157,
            w - 28,
            "Wheel scrolls the rename list. No exported filename replaces a scanned resource.",
            c::INK_FAINT,
        );
        for (i, note) in package.notes.iter().take(4).enumerate() {
            label_fit(
                d,
                x + 14,
                y + h - 132 + i as i32 * 20,
                w - 28,
                note,
                if i >= 3 { c::AMBER } else { c::INK_MUTED },
            );
        }
        o.button(
            [x + 12, y + h - 45, 146, 28],
            "Add source LIB",
            Action::File(FileAction::CloneSource),
            false,
        );
        o.button(
            [x + 172, y + h - 45, 84, 28],
            "Back",
            Action::CloneBack,
            false,
        );
        o.button(
            [x + w - 263, y + h - 45, 94, 28],
            "Cancel",
            Action::Cancel,
            false,
        );
        o.button(
            [x + w - 158, y + h - 45, 146, 28],
            "Export new LIB",
            Action::Apply,
            true,
        );
    }
}
impl App {
    pub fn smoke_clone(&mut self) {
        self.doc = Document::new(Archive::empty());
        self.demo();
        self.select_entry(1);
        let before = self.doc.archive.bytes().unwrap();
        crate::platform::write_new("HGC_SRC.LIB", &before).unwrap();
        let directory = index("HGC_SRC.LIB").unwrap().unwrap();
        let entry = directory.iter().find(|e| e.name == "DEMO.PIC").unwrap();
        let bytes = crate::platform::read_range("HGC_SRC.LIB", entry.offset, entry.size).unwrap();
        assert_eq!(
            archive::decode_payload(entry.flag, bytes).unwrap(),
            self.doc.archive.entries[self.doc.archive.find("DEMO.PIC").unwrap()]
                .read()
                .unwrap()
        );
        crate::platform::remove_file("HGC_SRC.LIB").unwrap();
        self.file_prompt(FileAction::Variant);
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::CloneId
        ));
        assert!(self.browser.is_none());
        for value in ["NEWJET", "New aircraft"] {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
        }
        assert!(
            matches!(self.prompt.as_ref().unwrap().kind, PromptKind::CloneReview),
            "{}",
            self.status
        );
        assert_eq!(self.doc.archive.bytes().unwrap(), before);
        let p = self.clone_draft.as_ref().unwrap();
        assert_eq!(p.archive.entries.len(), 8);
        assert!(p
            .mapping
            .iter()
            .all(|(_, new)| self.doc.archive.find(new).is_none()));
        self.key(Key::Enter, false, false);
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::File(FileAction::Save)
        ));
        assert!(self.prompt.as_ref().unwrap().value.ends_with("NEWJET.LIB"));
        assert!(self.doc.dirty());
        assert_eq!(self.doc.archive.entries[0].name, "NEWJET.PT");
        let reopened = Archive::parse(self.doc.archive.bytes().unwrap()).unwrap();
        assert_eq!(reopened.entries.len(), 8);
        self.key(Key::Escape, false, false);
        // Directory-only platform reader is exercised against an existing LIB by Linux CLI QA.
    }
}

impl App {
    #[cfg(not(windows))]
    pub fn clone_export_check(
        &mut self,
        donor: &str,
        id: &str,
        title: &str,
        output: &str,
    ) -> Result<()> {
        let at = self.doc.archive.find(donor).ok_or("Donor PT not found")?;
        self.select_entry(at);
        let before = self.doc.archive.bytes()?;
        self.file_prompt(FileAction::Variant);
        for value in [id, title] {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
            if self.status.starts_with("Error:") {
                return Err(self.status.clone());
            }
        }
        if !matches!(
            self.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::CloneReview)
        ) {
            return Err("Wizard did not reach review".into());
        }
        if self.doc.archive.bytes()? != before {
            return Err("Source document changed before export".into());
        }
        self.key(Key::Enter, false, false);
        self.key(Key::Char('a'), true, false);
        for c in output.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        if self.doc.dirty() {
            return Err(self.status.clone());
        }
        self.open(output)?;
        if self
            .doc
            .archive
            .find(&format!("{}.PT", id.to_ascii_uppercase()))
            .is_none()
        {
            return Err("Exported aircraft identity missing".into());
        }
        Ok(())
    }
}
