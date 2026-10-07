//! Resource relationships are available beside the model, not only in packaging.
use super::view::{count, Action, Icon, Layout};
use super::*;

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
    entry: Option<usize>,
    library: Option<u64>,
}
/// Package columns: top of the content below the 28px headers.
const PACKAGE_TOP: i32 =
    theme::metric::MENUBAR_H + theme::metric::EDITOR_HEADER_H + theme::space::SPACE_2;
/// Build list rows start below the LIB name and the Changes sub-head.
const PACKAGE_LIST_TOP: i32 = PACKAGE_TOP + 2 * theme::metric::ROW_H + theme::space::SPACE_2;
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
        let mut report =
            hangar_core::validation::inspect_with(&self.doc, &mut self.dependencies, &providers);
        self.palette_checks(&mut report);
        report
    }
    /// Package errors for textures FA's texture mapper cannot read.
    pub(super) fn texture_layout_errors(&self) -> usize {
        self.validation.as_ref().map_or(0, |r| {
            r.checks
                .iter()
                .filter(|c| {
                    c.message
                        .starts_with(hangar_core::validation::TEXTURE_LAYOUT_ERROR)
                })
                .count()
        })
    }
    /// Repair textures for FA: every PIC a textured SH face draws that is not
    /// a retail texture (and its stored original) is rewritten in that layout
    /// in one undo step. No SH changes; texels keep their UVs. The loaded base
    /// PAL is the game palette the embedded palette is compared with.
    pub(super) fn repair_fa_textures(&mut self) -> Result<()> {
        use hangar_core::picture::PaletteCheck;
        self.finish_stroke();
        let game = self.palette_loaded.then_some(&*self.base_palette);
        let mut index = hangar_core::dependencies::Index::default();
        let repair = hangar_core::validation::repair_textures(&self.doc.archive, &mut index, game)?;
        let refused = repair
            .refused
            .iter()
            .map(|(n, why)| format!("{n}: {why}"))
            .collect::<Vec<_>>()
            .join("; ");
        if repair.entries.is_empty() {
            self.status = if refused.is_empty() {
                "Every texture SH faces draw is already in the FA layout. Nothing changed.".into()
            } else {
                format!("Nothing repaired. {refused}")
            };
            return Ok(());
        }
        let selected = self.name().to_string();
        let context = self.context_name();
        self.doc.transaction(repair.entries, &[])?;
        self.reselect(&selected, context);
        let mut notes = Vec::new();
        let remapped: Vec<_> = repair
            .repaired
            .iter()
            .filter_map(|(n, c)| matches!(c, PaletteCheck::Remapped(_)).then_some(n.as_str()))
            .collect();
        if !remapped.is_empty() {
            notes.push(format!(
                "Embedded palette differed from the base PAL in {}; colors mapped to the nearest base color",
                remapped.join(", ")
            ));
        }
        if repair
            .repaired
            .iter()
            .any(|(_, c)| *c == PaletteCheck::Unverified)
        {
            notes.push("No base PAL loaded: palette indices kept unverified".into());
        }
        if !repair.originals.is_empty() {
            notes.push(format!("{} converted too", repair.originals.join(", ")));
        }
        if !refused.is_empty() {
            notes.push(format!("Not repaired: {refused}"));
        }
        let n = repair.repaired.len();
        self.status = format!(
            "Repaired {} for FA: 256 wide, row table, no palette; no SH changed. One undo step.{}",
            view::count(n, "texture", "textures"),
            notes.iter().map(|s| format!(" {s}.")).collect::<String>()
        );
        if self.validation.is_some() {
            self.validation = Some(self.package_report());
            self.validation_scroll = 0;
        }
        Ok(())
    }
    /// `--texture-repair-check`: Repair textures for FA on a real LIB keeps
    /// every SH byte and the textured viewport pixels of `shape`, leaves no
    /// texture-layout error, and undoes and redoes as one step.
    #[cfg(not(windows))]
    pub fn check_texture_repair(&mut self, shape: &str, palette: Option<&str>) -> Result<String> {
        if let Some(path) = palette {
            self.perform_file(FileAction::Palette, path)?;
        }
        let at = self.doc.archive.find(shape).ok_or("SH not found")?;
        self.select_entry(at);
        self.mode = Mode::Model;
        self.textured = true;
        self.perspective = false;
        let images = |app: &App| {
            app.draw()
                .commands
                .into_iter()
                .filter_map(|d| match d {
                    Draw::Bitmap(_, _, _, _, p) => Some(p),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let before = self.doc.archive.bytes()?;
        let shapes: Vec<_> = self
            .doc
            .archive
            .entries
            .iter()
            .filter(|e| e.name.ends_with(".SH"))
            .map(|e| (e.name.clone(), e.read()))
            .collect();
        let pixels = images(self);
        self.validation = Some(self.package_report());
        let errors = self.texture_layout_errors();
        let mut out = format!("{errors} texture-layout errors before repair\n");
        self.repair_fa_textures()?;
        out.push_str(&format!("{}\n", self.status));
        if !self.doc.dirty() {
            return Err("Repair changed nothing".into());
        }
        for (name, bytes) in &shapes {
            let at = self.doc.archive.find(name).ok_or("SH missing")?;
            if self.doc.archive.entries[at].read() != *bytes {
                return Err(format!("Repair changed {name}"));
            }
        }
        if self.texture_layout_errors() != 0 {
            return Err("Texture-layout errors remain after repair".into());
        }
        let after = images(self);
        if after.is_empty() || after != pixels {
            return Err("Repair changed textured viewport pixels".into());
        }
        out.push_str("PASS textured viewport pixels unchanged; every SH byte unchanged\n");
        let repaired = self.doc.archive.bytes()?;
        self.act(Action::Undo);
        if self.doc.archive.bytes()? != before {
            return Err("Undo did not restore the archive".into());
        }
        self.act(Action::Redo);
        if self.doc.archive.bytes()? != repaired {
            return Err("Redo differs".into());
        }
        out.push_str("PASS one undo step restores the archive; redo restores the repair\n");
        Ok(out)
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

    /// Validation report rows: one head row per check (badge, entry) and its
    /// wrapped message.
    pub(super) fn validation_lines(&self) -> Vec<Row> {
        let mut lines = Vec::new();
        let width = self.right() - self.left() - 1 - 2 * theme::space::SPACE_3;
        if let Some(report) = &self.validation {
            for check in &report.checks {
                lines.push(Row {
                    text: check.entry.as_deref().unwrap_or("Package").into(),
                    detail: check.level.label().into(),
                    kind: Kind::Head,
                    library: None,
                    entry: check.entry.as_ref().and_then(|n| self.doc.archive.find(n)),
                });
                for text in widgets::wrap(&check.message, width, Style::Label) {
                    lines.push(Row {
                        text,
                        detail: String::new(),
                        kind: Kind::Text,
                        entry: None,
                        library: None,
                    });
                }
            }
            if report.omitted > 0 {
                lines.push(Row {
                    text: format!("{} further results omitted", report.omitted),
                    detail: String::new(),
                    kind: Kind::Note,
                    entry: None,
                    library: None,
                });
            }
        }
        lines
    }
    /// The package checks summary Notice.
    pub(super) fn package_summary(&self) -> (widgets::Tone, String) {
        use widgets::Tone;
        match &self.validation {
            None => (
                Tone::Neutral,
                "Checks have not been run. They cover this LIB; external resources may be required.".into(),
            ),
            Some(r) if r.errors > 0 => (
                Tone::Danger,
                format!(
                    "{} and {}. Fix errors before packaging.",
                    count(r.errors, "error", "errors"),
                    count(r.warnings, "warning", "warnings")
                ),
            ),
            Some(r) if r.warnings > 0 => (
                Tone::Warn,
                format!("No errors, {}. Review them before packaging.", count(r.warnings, "warning", "warnings")),
            ),
            Some(_) => (Tone::Ok, "Available checks passed: no errors or warnings.".into()),
        }
    }
    /// Top of the validation results, below the summary Notice.
    fn validation_top(&self) -> i32 {
        let cw = self.right() - self.left() - 1 - 2 * theme::space::SPACE_3;
        PACKAGE_TOP + widgets::notice_height(cw, &self.package_summary().1) + theme::space::SPACE_2
    }
    /// Validation rows that fit below the summary.
    pub(super) fn validation_visible(&self) -> usize {
        ((self.height - theme::metric::STATUSBAR_H - self.validation_top() - theme::space::SPACE_2)
            / theme::metric::ROW_H)
            .max(1) as usize
    }
    /// Changed entries that fit in the build list.
    pub(super) fn changes_visible(&self) -> usize {
        let markings = i32::from(self.package_markings().is_some());
        ((self.height
            - theme::metric::STATUSBAR_H
            - PACKAGE_LIST_TOP
            - (3 + markings) * theme::metric::ROW_H)
            / theme::metric::ROW_H)
            .max(1) as usize
    }
    /// Package workspace: build list, package checks and output columns.
    pub(super) fn package_layout(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, dot, notice, subhead, type_badge, Btn, Tone};
        let (l, r, w, h) = (self.left(), self.right(), self.width, self.height);
        let top = m::MENUBAR_H;
        let bottom = h - m::STATUSBAR_H;
        let d = &mut o.canvas;
        d.rect(0, top, w, bottom - top, c::GM_800);
        for (x, title) in [
            (0, "Build list"),
            (l + 1, "Package checks"),
            (r + 1, "Output"),
        ] {
            d.rect(x, top + m::EDITOR_HEADER_H - 1, w, 1, c::GM_1000);
            d.styled(
                x + space::SPACE_3,
                baseline(top, m::EDITOR_HEADER_H, Style::Strong),
                title,
                c::INK,
                Style::Strong,
            );
        }
        d.rect(l, top, 1, bottom - top, c::GM_1000);
        d.rect(r, top, 1, bottom - top, c::GM_1000);
        // Build list: the LIB and its changed entries.
        let lib = self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path);
        let ly = top + m::EDITOR_HEADER_H + space::SPACE_2;
        d.icon(space::SPACE_3, ly + 2, Icon::Lib, c::INK_MUTED, c::GM_800);
        d.styled(
            space::SPACE_3 + m::ICON + space::SPACE_1,
            baseline(ly, m::ROW_H, Style::Value),
            &fit(lib, l - 2 * space::SPACE_3 - m::ICON, Style::Value),
            c::INK,
            Style::Value,
        );
        let changes = self.doc.changes();
        subhead(
            d,
            space::SPACE_3,
            PACKAGE_LIST_TOP - m::ROW_H,
            l - 24,
            &format!("Changes \u{b7} {}", changes.len()),
        );
        if changes.is_empty() {
            d.styled(
                space::SPACE_3,
                baseline(PACKAGE_LIST_TOP, m::ROW_H, Style::Label),
                "No changed entries",
                c::INK_MUTED,
                Style::Label,
            );
        }
        for (i, (name, kind)) in changes
            .iter()
            .skip(self.changes_scroll)
            .take(self.changes_visible())
            .enumerate()
        {
            let y = PACKAGE_LIST_TOP + i as i32 * m::ROW_H;
            d.rect(
                1,
                y,
                l - 2,
                m::ROW_H,
                if i % 2 == 0 { c::GM_800 } else { c::GM_900 },
            );
            dot(
                d,
                space::SPACE_3,
                y + (m::ROW_H - m::DIRTY_DOT) / 2,
                c::AMBER,
            );
            let label = kind.label();
            let kx = l - space::SPACE_3 - text_width(label, Style::Label);
            d.styled(
                kx,
                baseline(y, m::ROW_H, Style::Label),
                label,
                c::INK_MUTED,
                Style::Label,
            );
            let tx = space::SPACE_3 + m::DIRTY_DOT + space::SPACE_2;
            d.styled(
                tx,
                baseline(y, m::ROW_H, Style::Value),
                &fit(name, kx - space::SPACE_2 - tx, Style::Value),
                c::INK,
                Style::Value,
            );
        }
        let my = bottom - 2 * m::ROW_H - space::SPACE_2;
        if let Some(line) = self.package_markings() {
            d.styled(
                space::SPACE_3,
                baseline(my - m::ROW_H, m::ROW_H, Style::Label),
                &fit(&line, l - 24, Style::Label),
                c::INK_MUTED,
                Style::Label,
            );
        }
        subhead(d, space::SPACE_3, my, l - 24, "Output mode");
        d.styled(
            space::SPACE_3,
            baseline(my + m::ROW_H, m::ROW_H, Style::Label),
            &fit(
                if hangar_core::save::protected_name(&self.path).is_some() {
                    "Protected name: save a copy"
                } else {
                    "Custom LIB: saved with a backup"
                },
                l - 24,
                Style::Label,
            ),
            c::INK,
            Style::Label,
        );
        // Package checks: summary Notice, Run checks, then each result.
        let (cx, cw) = (l + 1 + space::SPACE_3, r - l - 1 - 2 * space::SPACE_3);
        let run = Btn::new("Run checks").with_icon(Icon::Check);
        let rw = run.width();
        o.button_ex(
            [
                r - space::SPACE_1 - rw,
                top + (m::EDITOR_HEADER_H - m::BUTTON_H) / 2,
                rw,
                m::BUTTON_H,
            ],
            run,
            Action::Validate,
        );
        let (tone, summary) = self.package_summary();
        let ny = PACKAGE_TOP;
        notice(&mut o.canvas, cx, ny, cw, tone, &summary);
        let lines = self.validation_lines();
        let list = self.validation_top();
        for (i, row) in lines
            .iter()
            .skip(self.validation_scroll)
            .take(self.validation_visible())
            .enumerate()
        {
            let y = list + i as i32 * m::ROW_H;
            let rect = [cx - 4, y, cw + 8, m::ROW_H];
            let hover = row.entry.is_some() && o.over(rect);
            let d = &mut o.canvas;
            if hover {
                d.rect(rect[0], y, rect[2], m::ROW_H, c::GM_700);
            }
            match row.kind {
                Kind::Head => {
                    let tone = match row.detail.as_str() {
                        "PASS" => Tone::Ok,
                        "WARN" => Tone::Warn,
                        "ERROR" => Tone::Danger,
                        _ => Tone::Neutral,
                    };
                    let bw = type_badge(d, cx, y + (m::ROW_H - m::BADGE_H) / 2, &row.detail, tone);
                    d.styled(
                        cx + bw + space::SPACE_2,
                        baseline(y, m::ROW_H, Style::Value),
                        &fit(&row.text, cw - bw - space::SPACE_2, Style::Value),
                        if row.entry.is_some() {
                            c::STEEL
                        } else {
                            c::INK
                        },
                        Style::Value,
                    );
                }
                _ => d.styled(
                    cx,
                    baseline(y, m::ROW_H, Style::Label),
                    &fit(&row.text, cw, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                ),
            }
            if let Some(entry) = row.entry {
                o.hit(rect, Action::Related(entry));
            }
        }
        // Output: what packaging writes, catalogs and the primary action.
        let (ox, ow) = (r + 1 + space::SPACE_3, w - r - 1 - 2 * space::SPACE_3);
        let mut y = top + m::EDITOR_HEADER_H + space::SPACE_2;
        y += notice(
            &mut o.canvas,
            ox,
            y,
            ow,
            Tone::Neutral,
            "Package writes a new custom LIB. Retail names stay protected and custom saves keep backups.",
        ) + space::SPACE_2;
        notice(
            &mut o.canvas,
            ox,
            y,
            ow,
            Tone::Neutral,
            "Checks are advisory. Other game LIBs can supply outside references; runtime names and implicit families are not fully checked. Test in Fighters Anthology.",
        );
        let mut buttons = vec![
            (
                "Add source LIB catalog",
                Action::File(FileAction::ReferenceSource),
            ),
            ("Clear source catalogs", Action::ClearSources),
            ("Export report", Action::File(FileAction::Report)),
        ];
        // Editor backups for painted textures; distribution builds usually drop them.
        if self
            .doc
            .archive
            .entries
            .iter()
            .any(|e| hangar_core::originals::texture_of(&e.name).is_some())
        {
            buttons.insert(0, ("Remove stored originals", Action::RemoveOriginals));
        }
        // Shown while the last checks found textures FA's mapper would crash on.
        if self.texture_layout_errors() > 0 {
            buttons.insert(0, ("Repair textures for FA", Action::RepairTextures));
        }
        let pitch = m::BUTTON_H + space::SPACE_1;
        let package_y = bottom - space::SPACE_2 - m::BUTTON_H;
        let mut by = package_y - space::SPACE_3 - buttons.len() as i32 * pitch;
        for (title, action) in buttons {
            o.button_ex([ox, by, ow, m::BUTTON_H], Btn::new(title), action);
            by += pitch;
        }
        o.button_ex(
            [ox, package_y, ow, m::BUTTON_H],
            Btn::new("Package LIB").with_icon(Icon::Package).primary(),
            Action::File(FileAction::Save),
        );
    }
}

impl App {
    /// Package checks flag the demo's legacy DEMO.PIC (32 wide, palette, no
    /// row table) as a texture FA would crash on; the Repair textures for FA
    /// button rewrites it with the textured viewport and every SH unchanged,
    /// as one undo step.
    pub fn smoke_texture_repair(&mut self) {
        let press = |a: &mut App, want: fn(&Action) -> bool| {
            let hit = a
                .layout()
                .hits
                .into_iter()
                .find(|h| want(&h.action))
                .expect("Package control missing");
            a.click(hit.rect[0] + 4, hit.rect[1] + 4, 1, true);
        };
        self.libraries.clear();
        self.demo();
        let at = self.doc.archive.find("DEMO.PIC").unwrap();
        let legacy = self.doc.archive.entries[at].read().unwrap();
        // The base PAL is the palette the legacy sheet embedded.
        let game = Pic::parse(&legacy).unwrap().colors(&[[0; 3]; 256]);
        self.palette_override = Some(Box::new(game));
        let shape = self.doc.archive.find("DEMO.SH").unwrap();
        self.select_entry(shape);
        self.mode = Mode::Model;
        self.textured = true;
        self.perspective = false;
        let images = |app: &App| {
            app.draw()
                .commands
                .into_iter()
                .filter_map(|d| match d {
                    Draw::Bitmap(_, _, _, _, p) => Some(p),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let original = self.doc.archive.bytes().unwrap();
        let sh = self.doc.archive.entries[shape].read().unwrap();
        for (width, height) in [(800, 600), (1280, 800)] {
            self.width = width;
            self.height = height;
            self.mode = Mode::Model;
            let pixels = images(self);
            assert!(!pixels.is_empty());
            self.mode = Mode::Package;
            self.validation = None;
            assert!(!self
                .layout()
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::RepairTextures)));
            press(self, |a| matches!(a, Action::Validate));
            assert_eq!(self.texture_layout_errors(), 1);
            let report = self.validation.as_ref().unwrap();
            let error = report
                .checks
                .iter()
                .find(|c| {
                    c.message
                        .starts_with(hangar_core::validation::TEXTURE_LAYOUT_ERROR)
                })
                .unwrap();
            assert_eq!(error.entry.as_deref(), Some("DEMO.PIC"));
            assert!(error.message.contains("32 pixels wide, not 256"));
            assert!(self
                .validation_lines()
                .iter()
                .any(|r| r.entry == Some(at) && r.detail == "ERROR"));
            for hit in self.layout().hits {
                assert!(hit.rect[0] + hit.rect[2] <= width && hit.rect[1] + hit.rect[3] <= height);
            }
            press(self, |a| matches!(a, Action::RepairTextures));
            assert!(
                self.status.starts_with("Repaired 1 texture for FA"),
                "{}",
                self.status
            );
            assert_eq!(self.texture_layout_errors(), 0);
            assert!(!self
                .layout()
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::RepairTextures)));
            let at = self.doc.archive.find("DEMO.PIC").unwrap();
            let fixed = self.doc.archive.entries[at].read().unwrap();
            assert!(picture::is_retail_texture(&fixed));
            let shape = self.doc.archive.find("DEMO.SH").unwrap();
            assert_eq!(self.doc.archive.entries[shape].read().unwrap(), sh);
            self.mode = Mode::Model;
            assert_eq!(images(self), pixels, "repair keeps the textured view");
            self.act(Action::Undo);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
        }
        self.palette_override = None;
        self.width = 1280;
        self.height = 800;
        self.demo();
    }
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
        // A long report scrolls; results open their entry through the row.
        let report = self.validation.as_mut().unwrap();
        for n in 0..12 {
            report.add(
                hangar_core::validation::Level::Info,
                Some("DEMO.SH"),
                format!("Synthetic note {n} for the scroll check"),
            );
        }
        self.mouse = [self.left() + 40, 200];
        self.wheel(-100);
        assert!(self.validation_scroll > 0);
        assert_eq!(
            self.validation_scroll,
            self.validation_lines().len() - self.validation_visible()
        );
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
