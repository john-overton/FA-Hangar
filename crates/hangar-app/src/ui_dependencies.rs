//! Resource relationships are available beside the model, not only in packaging.
use super::view::{icon, label_fit, text_fit, Action, Icon, Layout};
use super::*;
use hangar_core::validation::Level;

pub(super) struct Row {
    text: String,
    color: Rgb,
    entry: Option<usize>,
    library: Option<u64>,
}
fn color(level: Level) -> Rgb {
    match level {
        Level::Pass => c::OK,
        Level::Warning => c::AMBER,
        Level::Error => c::DANGER,
        Level::Info => c::INK_MUTED,
    }
}
pub(super) fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + word.len() + 1 > columns {
            lines.push(core::mem::take(&mut line));
        }
        for ch in word.chars() {
            if line.len() >= columns {
                lines.push(core::mem::take(&mut line));
            }
            line.push(ch);
        }
        line.push(' ');
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
impl App {
    pub(super) fn dependency_providers(&self) -> BTreeMap<String, Vec<String>> {
        let mut providers = BTreeMap::<String, Vec<String>>::new();
        for (path, names) in &self.dependency_catalogs {
            for name in names {
                providers
                    .entry(name.clone())
                    .or_default()
                    .push(path.clone());
            }
        }
        for library in &self.libraries {
            for entry in &library.doc.archive.entries {
                let paths = providers.entry(entry.name.clone()).or_default();
                if !paths.contains(&library.path) {
                    paths.push(library.path.clone());
                }
            }
        }
        providers
    }
    pub(super) fn package_report(&mut self) -> hangar_core::validation::Report {
        let providers = self.dependency_providers();
        hangar_core::validation::inspect_with(&self.doc, &mut self.dependencies, &providers)
    }

    pub(super) fn reference_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let providers = self.dependency_providers();
        let mut add = |text: String, name: Option<&str>, color| {
            let local = name.and_then(|n| self.doc.archive.find(n));
            let external: Vec<_> = name
                .map(|n| {
                    self.libraries
                        .iter()
                        .filter_map(|l| l.doc.archive.find(n).map(|i| (l.id, i)))
                        .collect()
                })
                .unwrap_or_default();
            let (library, entry) = if local.is_none() && external.len() == 1 {
                (Some(external[0].0), Some(external[0].1))
            } else {
                (None, local)
            };
            rows.push(Row {
                text,
                color,
                entry,
                library,
            });
        };
        if let Some(scan) = self.dependencies.get(self.name()) {
            add(format!("REFERENCES / {}", scan.links.len()), None, c::INK);
            for link in &scan.links {
                let present = self.doc.archive.find(&link.target).is_some();
                add(
                    format!(
                        "{} / {}",
                        link.target,
                        if present {
                            link.evidence
                        } else if providers.contains_key(&link.target) {
                            "external / see Package"
                        } else {
                            "not in source catalogs"
                        }
                    ),
                    Some(&link.target),
                    if present { c::STEEL } else { c::AMBER },
                );
            }
            if let Some(error) = &scan.unavailable {
                add(format!("Unverified: {error}"), None, c::AMBER);
            }
            for note in &scan.notes {
                add(note.clone(), None, c::AMBER);
            }
            add(
                format!(
                    "REFERENCED BY / {}",
                    self.dependencies.incoming(self.name()).count()
                ),
                None,
                c::INK,
            );
            for source in self.dependencies.incoming(self.name()) {
                add(source.clone(), Some(source), c::STEEL);
            }
            add(
                format!("AIRCRAFT USERS / {}", self.aircraft_users.len()),
                None,
                c::INK,
            );
            for source in &self.aircraft_users {
                add(source.clone(), Some(source), c::STEEL);
            }
        } else {
            add(
                "Select a resource to inspect its relationships".into(),
                None,
                c::INK_MUTED,
            );
        }
        for library in &self.libraries {
            for source in library.dependencies.incoming(self.name()) {
                rows.push(Row {
                    text: format!(
                        "{} / user in {}",
                        source,
                        library
                            .path
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(&library.path)
                    ),
                    color: c::STEEL,
                    entry: library.doc.archive.find(source),
                    library: Some(library.id),
                });
            }
        }
        rows
    }
    pub(super) fn references_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        o.canvas.rect(x, y, w, h, c::GM_950);
        let count = self.dependencies.unavailable_count();
        let scope = if count > 0 {
            format!(
                "Current LIB / {} aircraft / partial scan",
                self.aircraft_users.len()
            )
        } else {
            format!("Current LIB / {} aircraft users", self.aircraft_users.len())
        };
        label_fit(&mut o.canvas, x + 12, y + 16, w - 24, &scope, c::INK_FAINT);
        let rows = self.reference_rows();
        let visible = ((h - 32) / 22).max(0) as usize;
        for (i, row) in rows
            .iter()
            .skip(self.reference_scroll)
            .take(visible)
            .enumerate()
        {
            let yy = y + 25 + i as i32 * 22;
            if row.entry.is_some() {
                o.canvas.rect(x + 8, yy, w - 16, 21, c::GM_900);
            }
            text_fit(&mut o.canvas, x + 14, yy + 15, w - 28, &row.text, row.color);
            if let Some(entry) = row.entry {
                o.hit(
                    [x + 8, yy, w - 16, 21],
                    row.library
                        .map_or(Action::Related(entry), |id| Action::LibraryEntry(id, entry)),
                );
            }
        }
        if rows.len() > visible {
            label_fit(
                &mut o.canvas,
                x + 12,
                y + h - 4,
                w - 24,
                "Wheel scroll / click a resource to select",
                c::INK_FAINT,
            );
        }
    }
    pub(super) fn validation_lines(&self) -> Vec<Row> {
        let mut lines = Vec::new();
        let columns = ((self.right() - self.left() - 30) / 7).max(16) as usize;
        if let Some(report) = &self.validation {
            for check in &report.checks {
                lines.push(Row {
                    text: format!(
                        "{} {}",
                        check.level.label(),
                        check.entry.as_deref().unwrap_or("Package")
                    ),
                    library: None,
                    color: color(check.level),
                    entry: check.entry.as_ref().and_then(|n| self.doc.archive.find(n)),
                });
                for text in wrap(&check.message, columns) {
                    lines.push(Row {
                        text,
                        color: c::INK_MUTED,
                        entry: None,
                        library: None,
                    });
                }
                lines.push(Row {
                    text: String::new(),
                    color: c::INK_MUTED,
                    entry: None,
                    library: None,
                });
            }
            if report.omitted > 0 {
                lines.push(Row {
                    text: format!("{} further results omitted", report.omitted),
                    color: c::AMBER,
                    entry: None,
                    library: None,
                });
            }
        }
        lines
    }
    pub(super) fn package_layout(&self, o: &mut Layout) {
        let (l, r, w, h) = (self.left(), self.right(), self.width, self.height);
        let mid = r - l;
        let d = &mut o.canvas;
        d.rect(0, 26, l, h - 48, c::GM_800);
        d.rect(l + 1, 26, mid - 2, h - 48, c::GM_900);
        d.rect(r + 1, 26, w - r - 1, h - 48, c::GM_800);
        d.label(12, 44, "Build list", c::INK);
        d.label(l + 12, 44, "Package checks", c::INK);
        d.label(r + 12, 44, "Output", c::INK);
        text_fit(
            d,
            12,
            77,
            l - 24,
            self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path),
            c::INK,
        );
        let changes = self.doc.changes();
        let visible = ((h - 220) / 24).max(1) as usize;
        if changes.is_empty() {
            d.label(14, 110, "No changed entries", c::INK_MUTED);
        }
        for (i, (name, kind)) in changes
            .iter()
            .skip(self.changes_scroll)
            .take(visible)
            .enumerate()
        {
            let y = 100 + i as i32 * 24;
            text_fit(d, 14, y + 15, l - 94, name, c::AMBER);
            d.label(l - 80, y + 15, kind.label(), c::INK_MUTED);
        }
        label_fit(
            d,
            14,
            h - 107,
            l - 28,
            &format!("{} changes / wheel scroll", changes.len()),
            c::INK_MUTED,
        );
        d.label(14, h - 80, "OUTPUT MODE", c::INK_FAINT);
        label_fit(
            d,
            14,
            h - 54,
            l - 28,
            if hangar_core::save::protected_name(&self.path).is_some() {
                "Protected / save a copy"
            } else {
                "Custom / save with backup"
            },
            c::INK_MUTED,
        );
        let summary = self
            .validation
            .as_ref()
            .map(|r| r.summary())
            .unwrap_or_else(|| "Checks have not been run".into());
        label_fit(
            d,
            l + 12,
            76,
            mid - 24,
            &summary,
            if self.validation.as_ref().is_some_and(|r| r.errors > 0) {
                c::DANGER
            } else {
                c::INK
            },
        );
        o.button(
            [l + 12, 88, mid - 24, 24],
            "Re-run package checks",
            Action::Validate,
            false,
        );
        let d = &mut o.canvas;
        label_fit(
            d,
            l + 12,
            133,
            mid - 24,
            "Current LIB / external resources may be required",
            c::INK_FAINT,
        );
        if self
            .validation
            .as_ref()
            .is_some_and(|report| report.errors == 0 && report.warnings == 0)
        {
            icon(&mut o.canvas, r + 18, 380, Icon::Check, c::OK);
            label_fit(
                &mut o.canvas,
                r + 42,
                395,
                w - r - 54,
                "Available checks passed",
                c::OK,
            );
        }
        let lines = self.validation_lines();
        let visible = ((h - 192) / 20).max(1) as usize;
        for (i, row) in lines
            .iter()
            .skip(self.validation_scroll)
            .take(visible)
            .enumerate()
        {
            let y = 146 + i as i32 * 20;
            text_fit(
                &mut o.canvas,
                l + 12,
                y + 15,
                mid - 24,
                &row.text,
                row.color,
            );
            if let Some(entry) = row.entry {
                o.hit([l + 8, y, mid - 16, 19], Action::Related(entry));
            }
        }
        label_fit(
            &mut o.canvas,
            l + 12,
            h - 29,
            mid - 24,
            "Wheel scroll / click entry to inspect",
            c::INK_FAINT,
        );
        let d = &mut o.canvas;
        d.label(r + 18, 80, "Destination", c::INK);
        d.label(r + 18, 108, "Choose a custom LIB filename.", c::INK_MUTED);
        d.label(r + 18, 143, "Retail names stay protected.", c::INK_MUTED);
        d.label(r + 18, 169, "Custom saves keep backups.", c::INK_MUTED);
        d.label(r + 18, 216, "Checks are advisory.", c::INK);
        for (i, line) in wrap("References outside this LIB can be supplied by other game libraries. Runtime-generated names and implicit families are not fully checked. Test the result in Fighters Anthology.", ((w - r - 36) / 7) as usize).iter().enumerate() {
            text_fit(d, r + 18, 242 + i as i32 * 20, w - r - 36, line, c::INK_MUTED);
        }
        for (offset, title, action) in [
            (
                161,
                "Add source LIB catalog",
                Action::File(FileAction::ReferenceSource),
            ),
            (131, "Clear source catalogs", Action::ClearSources),
            (101, "Export report", Action::File(FileAction::Report)),
        ] {
            o.button([r + 12, h - offset, w - r - 24, 24], title, action, false);
        }
        o.button(
            [r + 12, h - 63, w - r - 24, 28],
            "Package LIB",
            Action::File(FileAction::Save),
            true,
        );
    }
}

impl App {
    pub(super) fn smoke_dependencies(&mut self) {
        self.libraries.clear();
        self.demo();
        self.width = 1280;
        self.height = 800;
        let original = self.doc.archive.bytes().unwrap();
        self.dock = 4;
        let picture = self.doc.archive.find("DEMO.PIC").unwrap();
        let hit = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::Related(i) if i == picture))
            .expect("Texture link missing");
        self.filter = "SH".into();
        self.click(hit.rect[0] + 4, hit.rect[1] + 4, 1, true);
        assert_eq!(self.selected, picture);
        assert!(self.mode == Mode::Media);
        assert!(self.context_model.is_some());
        assert!(self.filter.is_empty());
        assert_eq!(self.aircraft_users, vec!["DEMO.PT"]);
        self.height = 600;
        self.mouse = [self.left() + 40, self.dock_y() + 65];
        self.wheel(-10);
        assert!(self.reference_scroll > 0);
        let aircraft = self.doc.archive.find("DEMO.PT").unwrap();
        let hit = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::Related(i) if i == aircraft))
            .expect("Aircraft user link missing");
        self.click(hit.rect[0] + 4, hit.rect[1] + 4, 1, true);
        assert_eq!(self.selected, aircraft);
        assert!(self.mode == Mode::Model);
        assert_eq!(self.model_entry, Some(0));
        assert_eq!(self.doc.archive.bytes().unwrap(), original);
        self.doc.remove(picture).unwrap();
        self.refresh();
        self.width = 800;
        self.act(Action::Validate);
        assert!(self.validation.as_ref().unwrap().warnings > 0);
        assert!(self
            .doc
            .changes()
            .iter()
            .any(|(name, kind)| name == "DEMO.PIC"
                && *kind == hangar_core::document::ChangeKind::Removed));
        self.mouse = [self.left() + 40, 200];
        self.wheel(-100);
        assert!(self.validation_scroll > 0);
        self.act(Action::Undo);
        assert!(self.validation.is_none());
        self.act(Action::Validate);
        assert_eq!(self.validation.as_ref().unwrap().warnings, 0);
        assert_eq!(self.doc.archive.bytes().unwrap(), original);
        // Exercise the full compact dock, including the live preview tab.
        self.select_entry(0);
        self.open_texture(picture);
        for (width, height) in [(800, 600), (1280, 800)] {
            self.width = width;
            self.height = height;
            for mode in [
                Mode::Browse,
                Mode::Model,
                Mode::Properties,
                Mode::Media,
                Mode::Package,
            ] {
                self.mode = mode;
                for dock in [0, 1, 2, 3, 4] {
                    self.dock = dock;
                    for hit in self.layout().hits {
                        assert!(
                            hit.rect[0] >= 0
                                && hit.rect[1] >= 0
                                && hit.rect[0] + hit.rect[2] <= width
                                && hit.rect[1] + hit.rect[3] <= height,
                            "Reference/package control outside window"
                        );
                        if matches!(hit.action, Action::Dock(_))
                            && hit.rect[1] >= self.dock_y()
                            && hit.rect[1] < self.dock_y() + 30
                        {
                            assert!(
                                hit.rect[0] + hit.rect[2] <= self.right(),
                                "Dock tab overlaps inspector"
                            );
                        }
                    }
                }
            }
        }
        self.width = 1280;
        self.height = 800;
        self.dock = 0;
        self.demo();
    }
}
