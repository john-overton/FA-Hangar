//! Resource relationships are available beside the model, not only in packaging.
use super::view::{icon, label_fit, text_fit, Action, Icon, Layout};
use super::*;
use hangar_core::validation::Level;

/// What a reference or validation row shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Section head: uppercase title and a count.
    Head,
    /// A resource present in an open LIB.
    Link,
    /// A referenced name no open LIB or catalog holds.
    Missing,
    /// A scan note or limitation.
    Note,
    Text,
}
pub(super) struct Row {
    text: String,
    /// Evidence or explanation after the name, `ink-muted`.
    detail: String,
    kind: Kind,
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
        let mut add = |kind: Kind, text: String, detail: String, name: Option<&str>| {
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
                detail,
                kind,
                color: c::INK,
                entry,
                library,
            });
        };
        if let Some(scan) = self.dependencies.get(self.name()) {
            add(
                Kind::Head,
                "References".into(),
                format!("{}", scan.links.len()),
                None,
            );
            for link in &scan.links {
                let present = self.doc.archive.find(&link.target).is_some();
                let (kind, detail) = if present {
                    (Kind::Link, link.evidence.to_string())
                } else if providers.contains_key(&link.target) {
                    (Kind::Link, "in another LIB; see Package".into())
                } else {
                    (Kind::Missing, "not in any open LIB or catalog".into())
                };
                add(kind, link.target.clone(), detail, Some(&link.target));
            }
            if let Some(error) = &scan.unavailable {
                add(
                    Kind::Note,
                    format!("Unverified: {error}"),
                    String::new(),
                    None,
                );
            }
            for note in &scan.notes {
                add(Kind::Note, note.clone(), String::new(), None);
            }
            add(
                Kind::Head,
                "Referenced by".into(),
                format!("{}", self.dependencies.incoming(self.name()).count()),
                None,
            );
            for source in self.dependencies.incoming(self.name()) {
                add(Kind::Link, source.clone(), String::new(), Some(source));
            }
            add(
                Kind::Head,
                "Aircraft users".into(),
                format!("{}", self.aircraft_users.len()),
                None,
            );
            for source in &self.aircraft_users {
                add(Kind::Link, source.clone(), String::new(), Some(source));
            }
        } else {
            add(
                Kind::Note,
                "Select a resource to see what it references and what uses it.".into(),
                String::new(),
                None,
            );
        }
        for library in &self.libraries {
            for source in library.dependencies.incoming(self.name()) {
                rows.push(Row {
                    text: source.clone(),
                    detail: format!(
                        "user in {}",
                        library
                            .path
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(&library.path)
                    ),
                    kind: Kind::Link,
                    color: c::INK,
                    entry: library.doc.archive.find(source),
                    library: Some(library.id),
                });
            }
        }
        rows
    }
    /// Reference rows that fit in the dock.
    pub(super) fn reference_visible(&self) -> usize {
        let h = self.height - theme::metric::STATUSBAR_H - self.dock_y();
        ((h - theme::metric::EDITOR_HEADER_H - 2 * theme::metric::ROW_H) / theme::metric::ROW_H)
            .max(1) as usize
    }
    pub(super) fn references_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        use theme::{metric as m, space};
        use widgets::baseline;
        o.canvas.rect(x, y, w, h, c::GM_800);
        let partial = self.dependencies.unavailable_count() > 0;
        let scope = format!(
            "Current LIB \u{b7} {}{}",
            super::view::count(self.aircraft_users.len(), "aircraft user", "aircraft users"),
            if partial { " \u{b7} partial scan" } else { "" }
        );
        o.canvas.styled(
            x + space::SPACE_3,
            baseline(y, m::ROW_H, Style::ValueSm),
            &fit(&scope, w - 2 * space::SPACE_3, Style::ValueSm),
            c::INK_MUTED,
            Style::ValueSm,
        );
        let rows = self.reference_rows();
        let visible = self.reference_visible();
        let top = y + m::ROW_H;
        for (i, row) in rows
            .iter()
            .skip(self.reference_scroll)
            .take(visible)
            .enumerate()
        {
            let yy = top + i as i32 * m::ROW_H;
            let rect = [x + space::SPACE_1, yy, w - 2 * space::SPACE_1 - 4, m::ROW_H];
            let link = row.entry.filter(|_| row.kind == Kind::Link);
            let hover = link.is_some() && o.over(rect);
            let fill = if hover { c::GM_700 } else { c::GM_800 };
            let d = &mut o.canvas;
            if hover {
                d.rect(rect[0], yy, rect[2], m::ROW_H, fill);
            }
            let mut tx = x + space::SPACE_3;
            let right = rect[0] + rect[2] - space::SPACE_2;
            match row.kind {
                Kind::Head => {
                    let head = row.text.to_ascii_uppercase();
                    d.styled(
                        tx,
                        baseline(yy, m::ROW_H, Style::Section),
                        &head,
                        c::INK_MUTED,
                        Style::Section,
                    );
                    d.styled(
                        tx + text_width(&head, Style::Section) + space::SPACE_2,
                        baseline(yy, m::ROW_H, Style::ValueSm),
                        &row.detail,
                        c::INK_MUTED,
                        Style::ValueSm,
                    );
                    continue;
                }
                Kind::Note | Kind::Text => {
                    d.icon(tx, yy + 2, Icon::Info, c::INK_MUTED, fill);
                    tx += m::ICON + space::SPACE_1;
                    d.styled(
                        tx,
                        baseline(yy, m::ROW_H, Style::Label),
                        &fit(&row.text, right - tx, Style::Label),
                        c::INK_MUTED,
                        Style::Label,
                    );
                    continue;
                }
                Kind::Missing => d.icon(tx, yy + 2, Icon::Warning, c::AMBER, fill),
                Kind::Link => d.icon(
                    tx,
                    yy + 2,
                    super::view::group_icon(&row.text),
                    c::INK_MUTED,
                    fill,
                ),
            }
            tx += m::ICON + space::SPACE_1;
            let name = fit(&row.text, (right - tx) * 3 / 5, Style::Value);
            d.styled(
                tx,
                baseline(yy, m::ROW_H, Style::Value),
                &name,
                if link.is_some() { c::STEEL } else { c::INK },
                Style::Value,
            );
            let dx = tx + text_width(&name, Style::Value) + space::SPACE_2;
            if !row.detail.is_empty() && dx < right {
                d.styled(
                    dx,
                    baseline(yy, m::ROW_H, Style::Label),
                    &fit(&row.detail, right - dx, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                );
            }
            if let Some(entry) = link {
                o.hit(
                    rect,
                    row.library
                        .map_or(Action::Related(entry), |id| Action::LibraryEntry(id, entry)),
                );
            }
        }
        if rows.len() > visible {
            let track = visible as i32 * m::ROW_H - 4;
            let thumb = (track * visible as i32 / rows.len() as i32).max(16);
            let span = (rows.len() - visible) as i32;
            let ty = top + 2 + (track - thumb) * (self.reference_scroll as i32).min(span) / span;
            o.canvas.rect(x + w - 5, ty, 3, thumb, c::GM_600);
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
                    detail: String::new(),
                    kind: Kind::Text,
                    library: None,
                    color: color(check.level),
                    entry: check.entry.as_ref().and_then(|n| self.doc.archive.find(n)),
                });
                for text in wrap(&check.message, columns) {
                    lines.push(Row {
                        text,
                        detail: String::new(),
                        kind: Kind::Text,
                        color: c::INK_MUTED,
                        entry: None,
                        library: None,
                    });
                }
                lines.push(Row {
                    text: String::new(),
                    detail: String::new(),
                    kind: Kind::Text,
                    color: c::INK_MUTED,
                    entry: None,
                    library: None,
                });
            }
            if report.omitted > 0 {
                lines.push(Row {
                    text: format!("{} further results omitted", report.omitted),
                    detail: String::new(),
                    kind: Kind::Text,
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
        // Editor backups for painted textures; distribution builds usually drop them.
        if self
            .doc
            .archive
            .entries
            .iter()
            .any(|e| hangar_core::originals::texture_of(&e.name).is_some())
        {
            o.button(
                [r + 12, h - 191, w - r - 24, 24],
                "Remove stored originals",
                Action::RemoveOriginals,
                false,
            );
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
