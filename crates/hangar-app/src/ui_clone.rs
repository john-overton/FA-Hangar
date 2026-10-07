use super::view::{Action, Icon, Layout};
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
pub(super) fn index(path: &str) -> Result<Option<Vec<IndexedEntry>>> {
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
                "Export object from {}, step 1: new ID of 1 to 6 letters or digits",
                self.name()
            ),
            value: format!("{prefix}V1"),
            axis: 0,
        });
        self.status =
            "The selected object and its resource graph will be copied into a separate LIB".into();
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
            title: "Export object, step 2: display name".into(),
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
        for library in &self.libraries {
            for entry in &library.doc.archive.entries {
                catalog.insert(entry.name.clone());
            }
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
            &Default::default(),
            |name| {
                if let Some(i) = self.doc.archive.find(name) {
                    return self.doc.archive.entries[i].read();
                }
                // Explicit source choices win; an open document supplies its in-memory bytes.
                for source in sources.iter().filter(|s| s.explicit) {
                    if let Some(library) = self
                        .libraries
                        .iter()
                        .find(|l| normalized(&l.path) == normalized(&source.path))
                    {
                        if let Some(i) = library.doc.archive.find(name) {
                            return library.doc.archive.entries[i].read();
                        }
                    } else if let Some(e) = source.entries.iter().find(|e| e.name == name) {
                        return archive::decode_payload(
                            e.flag,
                            crate::platform::read_range(&source.path, e.offset, e.size)?,
                        );
                    }
                }
                let mut selected: Option<(Vec<u8>, &str)> = None;
                for library in &self.libraries {
                    if let Some(i) = library.doc.archive.find(name) {
                        let bytes = library.doc.archive.entries[i].read()?;
                        if let Some((old, path)) = &selected {
                            if old != &bytes {
                                return Err(format!("Conflicting open {name} in {path} and {}; choose an explicit source",library.path));
                            }
                        } else {
                            selected = Some((bytes, &library.path));
                        }
                    }
                }
                if let Some((bytes, _)) = selected.take() {
                    return Ok(bytes);
                }
                for source in &sources {
                    if self
                        .libraries
                        .iter()
                        .any(|l| normalized(&l.path) == normalized(&source.path))
                    {
                        continue;
                    }
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
        use theme::{metric as m, space};
        use widgets::{baseline, Btn};
        let Some(package) = &self.clone_draft else {
            return;
        };
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(660);
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        let title = format!(
            "Review export: {} \u{2192} {}.{}",
            package.donor,
            package.id,
            extension(&package.donor)
        );
        let [bx, by, bw, _] = self.dialog_frame(o, [x, y, w, h], &title);
        let d = &mut o.canvas;
        d.styled(
            bx,
            baseline(by, m::ROW_H, Style::Label),
            &fit(
                &format!(
                    "{} private resources. Source files are preserved; suggested output {}.LIB.",
                    package.archive.entries.len(),
                    package.id
                ),
                bw,
                Style::Label,
            ),
            c::INK,
            Style::Label,
        );
        let head = by + m::ROW_H + space::SPACE_2;
        d.rect(bx, head, bw, m::ROW_H, c::GM_900);
        let nx = bx + bw / 2;
        for (hx, label) in [(bx + 8, "FROM DONOR"), (nx, "IN NEW LIB")] {
            d.styled(
                hx,
                baseline(head, m::ROW_H, Style::Section),
                label,
                c::INK_MUTED,
                Style::Section,
            );
        }
        let notes = package.notes.len().min(4) as i32;
        let foot = y + h - space::SPACE_4 - m::BUTTON_H - space::SPACE_3 - (notes + 1) * m::ROW_H;
        let rows = ((foot - head - m::ROW_H) / m::ROW_H).max(1) as usize;
        for (row, (old, new)) in package
            .mapping
            .iter()
            .skip(self.clone_scroll)
            .take(rows)
            .enumerate()
        {
            let yy = head + m::ROW_H + row as i32 * m::ROW_H;
            d.rect(
                bx,
                yy,
                bw,
                m::ROW_H,
                if row % 2 == 0 { c::GM_950 } else { c::GM_900 },
            );
            let base = baseline(yy, m::ROW_H, Style::Value);
            d.styled(
                bx + 8,
                base,
                &fit(old, nx - bx - 16, Style::Value),
                c::INK_MUTED,
                Style::Value,
            );
            d.styled(
                nx,
                base,
                &fit(new, bx + bw - nx - 8, Style::Value),
                c::AMBER,
                Style::Value,
            );
        }
        d.styled(
            bx,
            baseline(foot, m::ROW_H, Style::Label),
            &fit(
                "Wheel scrolls the renames. No exported name replaces a scanned resource.",
                bw,
                Style::Label,
            ),
            c::INK_MUTED,
            Style::Label,
        );
        for (i, note) in package.notes.iter().take(4).enumerate() {
            let ny = foot + (i as i32 + 1) * m::ROW_H;
            let warn = i >= 3;
            d.icon(
                bx,
                ny + 2,
                if warn { Icon::Warning } else { Icon::Info },
                if warn { c::AMBER } else { c::INK_MUTED },
                c::GM_800,
            );
            d.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ny, m::ROW_H, Style::Label),
                &fit(note, bw - m::ICON - space::SPACE_1, Style::Label),
                if warn { c::INK } else { c::INK_MUTED },
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            [x, y, w, h],
            &[
                ("Add source LIB", Action::File(FileAction::CloneSource)),
                ("Back", Action::CloneBack),
            ],
            Some("Cancel"),
            Some(
                Btn::new("Export new LIB")
                    .with_icon(Icon::Package)
                    .primary(),
            ),
            Action::Apply,
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
        let at = self
            .doc
            .archive
            .find(donor)
            .ok_or("Source object not found")?;
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
            .find(&format!("{}.{}", id.to_ascii_uppercase(), extension(donor)))
            .is_none()
        {
            return Err("Exported object identity missing".into());
        }
        Ok(())
    }
}
