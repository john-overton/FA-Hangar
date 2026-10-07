//! Identity panel (short/long names, reference ID) and the in-place aircraft
//! rename and duplicate reviews.
use super::view::{Action, Icon, Layout};
use super::*;
use alloc::collections::BTreeSet;
use hangar_core::{
    clone_aircraft::Names,
    identity::{self, Duplicate, Ownership, Rename},
};

/// Rename and duplicate drafts, boxed to keep `App` small.
#[derive(Default)]
pub(super) struct State {
    pub rename: Option<Rename>,
    pub own: Option<Ownership>,
    pub share: BTreeSet<String>,
    pub duplicate: Option<Duplicate>,
    /// First list row shown in either review.
    pub scroll: usize,
    /// The export wizard is duplicating into this LIB.
    pub in_place: bool,
    /// Wizard short name; the long name is `App::clone_title`.
    pub short: String,
}
/// One review list row: a section head, or a resource with its detail and,
/// for duplicates, the index of its Copy/Share toggle and whether it is shared.
pub(super) enum Row {
    Head(String),
    Item {
        name: String,
        detail: String,
        amber: bool,
        toggle: Option<(usize, bool)>,
    },
}
fn stem(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}
const ROW: i32 = theme::metric::BUTTON_H + theme::space::SPACE_1;
impl App {
    /// Identity panel: short and long names (editable in place), the
    /// reference ID with Rename, and Duplicate aircraft. PT/NT/JT/OT entries
    /// whose names block is recognized.
    pub(super) fn identity_panel(&self, o: &mut Layout, s: &mut widgets::Stack) {
        use theme::{metric as m, space};
        use widgets::{baseline, pane, Btn};
        let Some(id) = self.brf.as_ref().and_then(identity::read) else {
            return;
        };
        if !matches!(extension(self.name()), "PT" | "NT" | "JT" | "OT") {
            return;
        }
        let saved = self.original_brf.as_ref().and_then(identity::read);
        if self.pane(o, s, pane::IDENTITY, "Identity", Icon::Aircraft) {
            for (long, label, value) in [
                (false, "Short name", &id.short),
                (true, "Long name", &id.long),
            ] {
                let old = saved
                    .as_ref()
                    .map(|s| if long { &s.long } else { &s.short });
                let changed = old.is_none_or(|o| o != value);
                if let Some(rect) = o.prop(s, label) {
                    o.text_field(rect, value, changed, Action::IdentityEdit(long));
                }
                if let Some(old) = old.filter(|o| *o != value) {
                    if let Some([x, y, w, h]) = o.prop(s, "Saved") {
                        let rw = m::ICON_BUTTON;
                        o.canvas.styled(
                            x + 6,
                            baseline(y, h, Style::Value),
                            &fit(old, w - rw - 12, Style::Value),
                            c::INK_MUTED,
                            Style::Value,
                        );
                        o.button_on(
                            [x + w - rw, y, rw, h],
                            Btn::icon(Icon::Rotate).ghost(),
                            Action::IdentityReset(long),
                            c::GM_800,
                        );
                    }
                }
            }
            let aircraft = extension(self.name()) == "PT";
            if let Some([x, y, w, h]) = o.prop(s, "Reference ID") {
                let b = Btn::new("Rename\u{2026}");
                let bw = if aircraft { b.width() } else { 0 };
                o.canvas.styled(
                    x + 6,
                    baseline(y, h, Style::Value),
                    &fit(stem(self.name()), w - bw - 12, Style::Value),
                    c::INK,
                    Style::Value,
                );
                if aircraft {
                    o.button_on([x + w - bw, y, bw, h], b, Action::RenameAircraft, c::GM_800);
                }
            }
            if !id.reference.eq_ignore_ascii_case(self.name()) {
                o.stack_notice(
                    s,
                    widgets::Tone::Warn,
                    &format!("The stored self reference is {}.", id.reference),
                );
            }
            if aircraft {
                if let Some(rect) = o.wide(s, m::BUTTON_H) {
                    o.button_ex(
                        rect,
                        Btn::new("Duplicate aircraft\u{2026}").with_icon(Icon::Plus),
                        Action::DuplicateAircraft,
                    );
                }
            }
            s.gap(space::SPACE_1);
        }
        o.panel_end(s);
    }
    pub(super) fn identity_edit(&mut self, long: bool) {
        let Some(id) = self.brf.as_ref().and_then(identity::read) else {
            self.status = "This entry has no recognized names block".into();
            return;
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::IdentityName(long),
            title: format!(
                "{} of {}: 1 to {} plain ASCII characters",
                if long { "Long name" } else { "Short name" },
                self.name(),
                identity::NAME_LIMIT
            ),
            value: if long { id.long } else { id.short },
            axis: 0,
        });
    }
    /// One undo step; only the name operand's bytes change.
    pub(super) fn identity_commit(&mut self, long: bool, value: &str) -> Result<()> {
        let at = self.selected;
        let e = self
            .doc
            .archive
            .entries
            .get(at)
            .ok_or("No selected entry")?;
        let bytes = e.read()?;
        let value = Some(value);
        let (short, long_name) = if long { (None, value) } else { (value, None) };
        let out = identity::set_names(&bytes, extension(&e.name), short, long_name)?;
        let label = if long { "Long name" } else { "Short name" };
        if out == bytes {
            self.status = format!("{label} unchanged");
            return Ok(());
        }
        self.doc.replace(at, out)?;
        self.refresh();
        self.status = format!("{label} changed. Ctrl+Z undoes it.");
        Ok(())
    }
    pub(super) fn identity_reset(&mut self, long: bool) {
        let saved = self.original_brf.as_ref().and_then(identity::read);
        let r = match saved {
            Some(id) => self.identity_commit(long, if long { &id.long } else { &id.short }),
            None => Err("No saved value".into()),
        };
        self.result(r);
    }

    // ---------------------------------------------------------- rename

    pub(super) fn rename_aircraft_prompt(&mut self) {
        if extension(self.name()) != "PT" {
            self.status = "Select an aircraft PT to rename its reference ID".into();
            return;
        }
        self.identity.rename = None;
        self.prompt = Some(Prompt {
            kind: PromptKind::RenameId,
            title: format!(
                "Rename reference ID of {}: 1 to 6 letters, digits or underscore",
                self.name()
            ),
            value: stem(self.name()).into(),
            axis: 0,
        });
    }
    pub(super) fn rename_review(&mut self, id: &str) -> Result<()> {
        let mut plan = identity::rename(&self.doc.archive, self.name(), id)?;
        // Other open LIBs that name a renamed resource without holding it.
        let mut users = Vec::new();
        for l in &self.libraries {
            if plan.renames.iter().any(|(old, _)| {
                l.doc.archive.find(old).is_none() && l.dependencies.incoming(old).next().is_some()
            }) {
                users.push(
                    l.path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&l.path)
                        .to_string(),
                );
            }
        }
        if !users.is_empty() {
            plan.notes.insert(
                0,
                format!("Open LIBs that name these resources: {}", users.join(", ")),
            );
        }
        let title = format!("Rename reference ID: {} \u{2192} {}", plan.old, plan.new);
        self.identity.rename = Some(plan);
        self.identity.scroll = 0;
        self.prompt = Some(Prompt {
            kind: PromptKind::RenameReview,
            title,
            value: id.trim().to_ascii_uppercase(),
            axis: 0,
        });
        Ok(())
    }
    pub(super) fn rename_apply(&mut self) -> Result<()> {
        let plan = self.identity.rename.as_ref().ok_or("No rename review")?;
        if !plan.ready() {
            return Err(plan.refusals[0].clone());
        }
        if plan.unchanged() {
            self.identity.rename = None;
            self.status = "Same reference ID; nothing changed".into();
            return Ok(());
        }
        plan.apply(&mut self.doc)?;
        let plan = self.identity.rename.take().unwrap();
        if let Some(i) = self.doc.archive.find(&plan.new) {
            self.select_entry(i);
        }
        self.mode = Mode::Model;
        self.refresh();
        self.status = format!(
            "Renamed {} to {}: {}, {} rewritten; one undo step. Missions and other LIBs still name {}",
            plan.old,
            plan.new,
            view::count(plan.renames.len(), "resource", "resources"),
            plan.rewritten.len(),
            plan.old
        );
        Ok(())
    }
    pub(super) fn rename_rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let Some(p) = &self.identity.rename else {
            return rows;
        };
        let item = |name: &str, detail: String, amber: bool| Row::Item {
            name: name.into(),
            detail,
            amber,
            toggle: None,
        };
        if !p.renames.is_empty() {
            rows.push(Row::Head(format!("Renamed \u{b7} {}", p.renames.len())));
            for (old, new) in &p.renames {
                rows.push(item(old, format!("\u{2192} {new}"), true));
            }
        }
        if !p.rewritten.is_empty() {
            rows.push(Row::Head(format!(
                "References rewritten in place \u{b7} {}",
                p.rewritten.len()
            )));
            for name in &p.rewritten {
                rows.push(item(name, "Stored names updated".into(), false));
            }
        }
        if !p.kept.is_empty() {
            rows.push(Row::Head(format!(
                "Private, name kept \u{b7} {}",
                p.kept.len()
            )));
            for name in &p.kept {
                rows.push(item(
                    name,
                    format!("Name does not start with {}", stem(&p.old)),
                    false,
                ));
            }
        }
        if !p.shared.is_empty() {
            rows.push(Row::Head(format!(
                "Shared, left alone \u{b7} {}",
                p.shared.len()
            )));
            for (name, why) in &p.shared {
                rows.push(item(name, why.clone(), false));
            }
        }
        rows
    }
    pub(super) fn rename_review_layout(&self, o: &mut Layout) {
        let Some(p) = &self.identity.rename else {
            return;
        };
        let summary = if p.unchanged() {
            "Same reference ID; nothing changes.".to_string()
        } else {
            format!(
                "{}, {} rewritten in place, {} left alone. One undo step.",
                view::count(p.renames.len(), "resource renamed", "resources renamed"),
                p.rewritten.len(),
                view::count(p.shared.len(), "shared resource", "shared resources"),
            )
        };
        let mut notices = Vec::new();
        if !p.refusals.is_empty() {
            notices.push((
                widgets::Tone::Danger,
                format!("Refused: {}", p.refusals.join("; ")),
            ));
        }
        if !p.unchanged() {
            let mut warning = p.warning();
            if !p.mentions.is_empty() {
                warning.push_str(&format!(
                    " {} in this LIB name it and are not rewritten.",
                    view::count(
                        p.mentions.len(),
                        "mission or unparsed resource",
                        "missions or unparsed resources"
                    )
                ));
            }
            notices.push((widgets::Tone::Warn, warning));
        }
        let ready = p.ready() && !p.unchanged();
        self.identity_review(
            o,
            &summary,
            &notices,
            &self.rename_rows(),
            &p.notes,
            widgets::Btn::new("Rename").primary().enabled(ready),
        );
    }

    // ------------------------------------------------------- duplicate

    pub(super) fn begin_duplicate(&mut self) {
        if extension(self.name()) != "PT" {
            self.status = "Select an aircraft PT to duplicate".into();
            return;
        }
        self.begin_clone();
        self.identity.in_place = true;
        self.identity.own = None;
        self.identity.duplicate = None;
        self.clone_step(1);
        self.status = format!(
            "{} will be duplicated in this LIB; shared resources stay shared",
            self.name()
        );
    }
    /// Build or rebuild the duplicate from the current share choices.
    pub(super) fn build_duplicate(&mut self) -> Result<()> {
        let donor = self.name().to_string();
        if self
            .identity
            .own
            .as_ref()
            .is_none_or(|o| o.graph.donor != donor)
        {
            let own = identity::ownership(&self.doc.archive, &donor)?;
            self.identity.share = own.default_share();
            self.identity.own = Some(own);
        }
        let own = self.identity.own.as_ref().unwrap();
        let names = Names {
            short: &self.identity.short,
            long: &self.clone_title,
        };
        let dup = identity::duplicate(
            &self.doc.archive,
            own,
            &self.variant_id,
            names,
            &self.identity.share,
        )?;
        self.identity.duplicate = Some(dup);
        Ok(())
    }
    pub(super) fn duplicate_review_prompt(&mut self) -> Result<()> {
        self.build_duplicate()?;
        self.identity.scroll = 0;
        self.prompt = Some(Prompt {
            kind: PromptKind::DuplicateReview,
            title: format!(
                "Duplicate aircraft: {} \u{2192} {}.PT",
                self.name(),
                self.variant_id
            ),
            value: String::new(),
            axis: 0,
        });
        Ok(())
    }
    /// Copy (`share` false) or share one toggle row; a failed rebuild keeps
    /// the previous choice.
    pub(super) fn duplicate_toggle(&mut self, row: usize, share: bool) -> Result<()> {
        let (_, names) = self.duplicate_rows();
        let name = names.get(row).ok_or("No resource in this row")?.clone();
        if self.identity.share.contains(&name) == share {
            return Ok(());
        }
        let previous = self.identity.share.clone();
        let own = self.identity.own.as_ref().ok_or("No duplicate review")?;
        let mut next = previous.clone();
        own.toggle(&mut next, &name)?;
        let moved = own.group(&name).len();
        self.identity.share = next;
        if let Err(e) = self.build_duplicate() {
            self.identity.share = previous;
            self.build_duplicate()?;
            return Err(e);
        }
        self.status = format!(
            "{name}{} now {}",
            if moved > 1 {
                format!(
                    " and {} moving with it",
                    view::count(moved - 1, "resource", "resources")
                )
            } else {
                String::new()
            },
            if share {
                "shared: the copy references the existing names"
            } else {
                "copied under new private names"
            }
        );
        Ok(())
    }
    pub(super) fn duplicate_apply(&mut self) -> Result<()> {
        let dup = self
            .identity
            .duplicate
            .as_ref()
            .ok_or("No duplicate review")?;
        dup.apply(&mut self.doc)?;
        let root = dup.root();
        let copied = dup.package.archive.entries.len();
        let shared = dup.package.shared.len();
        self.identity.duplicate = None;
        self.identity.own = None;
        self.identity.in_place = false;
        if let Some(i) = self.doc.archive.find(&root) {
            self.select_entry(i);
        }
        self.mode = Mode::Model;
        self.refresh();
        self.status = format!(
            "Duplicated as {root}: {} copied, {} shared; one undo step",
            copied, shared
        );
        Ok(())
    }
    /// Review rows and, in toggle order, the resource each toggle switches.
    pub(super) fn duplicate_rows(&self) -> (Vec<Row>, Vec<String>) {
        let mut rows = Vec::new();
        let mut names = Vec::new();
        let (Some(dup), Some(own)) = (&self.identity.duplicate, &self.identity.own) else {
            return (rows, names);
        };
        let p = &dup.package;
        rows.push(Row::Head(format!(
            "Copied \u{b7} {}",
            p.archive.entries.len()
        )));
        for (old, new) in &p.mapping {
            let fixed = own.always_copied(old);
            let toggle = (!fixed && own.graph.resources.contains(old)).then(|| {
                names.push(old.clone());
                (names.len() - 1, false)
            });
            rows.push(Row::Item {
                name: old.clone(),
                detail: if fixed {
                    format!("\u{2192} {new} \u{b7} always copied")
                } else {
                    format!("\u{2192} {new}")
                },
                amber: true,
                toggle,
            });
        }
        let originals = p.archive.entries.len() - p.mapping.len();
        if originals > 0 {
            rows.push(Row::Item {
                name: view::count(originals, "stored original", "stored originals"),
                detail: "Follow their copied PICs".into(),
                amber: false,
                toggle: None,
            });
        }
        if !p.shared.is_empty() {
            rows.push(Row::Head(format!("Shared \u{b7} {}", p.shared.len())));
            for name in &p.shared {
                names.push(name.clone());
                rows.push(Row::Item {
                    name: name.clone(),
                    detail: own.reason(name).into(),
                    amber: false,
                    toggle: Some((names.len() - 1, true)),
                });
            }
        }
        (rows, names)
    }
    pub(super) fn duplicate_review_layout(&self, o: &mut Layout) {
        let Some(dup) = &self.identity.duplicate else {
            return;
        };
        let p = &dup.package;
        let summary = format!(
            "{}, {} referenced as they are. One undo step; {} is selected afterwards.",
            view::count(
                p.archive.entries.len(),
                "resource copied",
                "resources copied"
            ),
            view::count(p.shared.len(), "shared resource", "shared resources"),
            dup.root()
        );
        let notices = [(
            widgets::Tone::Neutral,
            "Copy gives the new aircraft its own file under a new name; Share keeps the existing name. Damage families, skins and store icons switch together.".to_string(),
        )];
        let notes: Vec<String> = p
            .notes
            .iter()
            .filter(|n| !n.starts_with("Referenced files are copied"))
            .cloned()
            .collect();
        self.identity_review(
            o,
            &summary,
            &notices,
            &self.duplicate_rows().0,
            &notes,
            widgets::Btn::new("Duplicate aircraft").primary(),
        );
    }
    /// Shared review dialog: summary, notices, a scrolling list, notes and
    /// Back / Cancel / main action.
    fn identity_review(
        &self,
        o: &mut Layout,
        summary: &str,
        notices: &[(widgets::Tone, String)],
        rows: &[Row],
        notes: &[String],
        main: widgets::Btn,
    ) {
        use theme::{metric as m, space};
        use widgets::{baseline, notice, subhead, Btn};
        let (x, y, w, h) = self.identity_dialog_rect();
        o.hits.clear();
        let title = self.prompt.as_ref().map_or("Review", |p| p.title.as_str());
        let [bx, by, bw, _] = self.dialog_frame(o, [x, y, w, h], title);
        o.canvas.styled(
            bx,
            baseline(by, m::ROW_H, Style::Label),
            &fit(summary, bw, Style::Label),
            c::INK,
            Style::Label,
        );
        let mut top = by + m::ROW_H + space::SPACE_1;
        for (tone, text) in notices {
            top += notice(&mut o.canvas, bx, top, bw, *tone, text) + space::SPACE_1;
        }
        let shown_notes: Vec<&String> = notes.iter().take(3).collect();
        let foot = y + h - space::SPACE_4 - m::BUTTON_H - space::SPACE_3;
        let notes_y = foot - shown_notes.len() as i32 * m::ROW_H;
        let visible = ((notes_y - space::SPACE_1 - top) / ROW).max(1) as usize;
        let first = self.identity.scroll.min(rows.len().saturating_sub(visible));
        for (i, row) in rows.iter().enumerate().skip(first).take(visible) {
            let ry = top + (i - first) as i32 * ROW;
            match row {
                Row::Head(text) => subhead(&mut o.canvas, bx, ry + ROW - m::ROW_H, bw, text),
                Row::Item {
                    name,
                    detail,
                    amber,
                    toggle,
                } => {
                    let fill = if i % 2 == 0 { c::GM_950 } else { c::GM_900 };
                    o.canvas.rect(bx, ry, bw, ROW, fill);
                    let items = toggle.map(|(t, shared)| {
                        [
                            (
                                Btn::new("Copy").on(!shared),
                                Action::DuplicateToggle(t, false),
                            ),
                            (
                                Btn::new("Share").on(shared),
                                Action::DuplicateToggle(t, true),
                            ),
                        ]
                    });
                    let tw = items.as_ref().map_or(0, |i| Layout::segmented_width(i));
                    let nx = bx + space::SPACE_2;
                    let name_w = (bw / 3).min(140);
                    let base = baseline(ry, ROW, Style::Value);
                    o.canvas.styled(
                        nx,
                        base,
                        &fit(name, name_w, Style::Value),
                        c::INK,
                        Style::Value,
                    );
                    let dx = nx + name_w + space::SPACE_2;
                    let right = bx + bw - space::SPACE_1 - tw;
                    o.canvas.styled(
                        dx,
                        base,
                        &fit(detail, right - space::SPACE_2 - dx, Style::Value),
                        if *amber { c::AMBER } else { c::INK_MUTED },
                        Style::Value,
                    );
                    if let Some(items) = items {
                        o.segmented(
                            [right, ry + (ROW - m::BUTTON_H) / 2, tw, m::BUTTON_H],
                            &items,
                        );
                    }
                }
            }
        }
        if rows.len() > visible {
            let range = format!(
                "{}\u{2013}{} of {}",
                first + 1,
                (first + visible).min(rows.len()),
                rows.len()
            );
            let rw = text_width(&range, Style::ValueSm);
            o.canvas.styled(
                bx + bw - rw,
                baseline(by, m::ROW_H, Style::ValueSm),
                &range,
                c::INK_MUTED,
                Style::ValueSm,
            );
        }
        for (i, note) in shown_notes.iter().enumerate() {
            let ny = notes_y + i as i32 * m::ROW_H;
            o.canvas
                .icon(bx, ny + 2, Icon::Info, c::INK_MUTED, c::GM_800);
            o.canvas.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ny, m::ROW_H, Style::Label),
                &fit(note, bw - m::ICON - space::SPACE_1, Style::Label),
                c::INK_MUTED,
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            [x, y, w, h],
            &[("Back", Action::IdentityBack)],
            Some("Cancel"),
            Some(main),
            Action::Apply,
        );
    }
    fn identity_dialog_rect(&self) -> (i32, i32, i32, i32) {
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(660);
        ((self.width - w) / 2, (self.height - h) / 2, w, h)
    }
    pub(super) fn identity_wheel(&mut self, delta: i32) {
        let len = if self.identity.rename.is_some() {
            self.rename_rows().len()
        } else {
            self.duplicate_rows().0.len()
        };
        self.identity.scroll = (self.identity.scroll as i32 - delta * 3)
            .clamp(0, len.saturating_sub(1) as i32) as usize;
    }
    /// Back from a review: the rename returns to its ID, the duplicate to
    /// the long name.
    pub(super) fn identity_back(&mut self) {
        match self.prompt.as_ref().map(|p| &p.kind) {
            Some(PromptKind::RenameReview) => {
                let value = self.prompt.as_ref().unwrap().value.clone();
                self.rename_aircraft_prompt();
                if let Some(p) = &mut self.prompt {
                    p.value = value;
                }
            }
            Some(PromptKind::DuplicateReview) => {
                self.identity.duplicate = None;
                self.clone_step(3);
            }
            _ => {}
        }
    }
    /// Esc from any identity prompt drops its drafts.
    pub(super) fn identity_cancel(&mut self) {
        self.identity.rename = None;
        self.identity.duplicate = None;
        self.identity.own = None;
        self.identity.in_place = false;
    }
}
impl App {
    /// The demo with a weapon (SHOT.JT and its icon) and a stored original
    /// of DEMO.PIC; DEMO.PT is selected in Model.
    fn identity_fixture(&mut self) {
        self.doc = Document::new(Archive::empty());
        self.demo();
        let mut a = self.doc.archive.clone();
        let at = a.find("DEMO.PT").unwrap();
        let mut text = String::from_utf8(a.entries[at].read().unwrap()).unwrap();
        text.truncate(text.rfind("end\r\n").unwrap());
        text.push_str(":stores\r\nstring \"SHOT.JT\"\r\nend\r\n");
        a.entries[at] = Entry::new("DEMO.PT", text.into_bytes()).unwrap();
        a.entries.push(
            Entry::new(
                "SHOT.JT",
                b"[brent's_relocatable_format]\nstring \"SHOT.JT\"\nend\n".to_vec(),
            )
            .unwrap(),
        );
        for name in ["$SHOT.PIC", "DEMO.ORG"] {
            a.entries.push(Entry::new(name, picture::demo()).unwrap());
        }
        self.doc = Document::new(a);
        self.refresh();
        self.select_entry(self.doc.archive.find("DEMO.PT").unwrap());
        self.mode = Mode::Model;
        self.panels = 0;
    }
    fn smoke_click(&mut self, predicate: &dyn Fn(Action) -> bool) {
        let rect = self.smoke_find(predicate);
        self.chrome_click(rect);
    }
    fn smoke_type(&mut self, value: &str) {
        self.key(Key::Char('a'), true, false);
        for c in value.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
    }
    fn smoke_names(&self) -> identity::Identity {
        identity::read(self.brf.as_ref().unwrap()).unwrap()
    }
    /// Identity panel, Rename reference ID and Duplicate aircraft through
    /// their hit regions, at the minimum and the default window size.
    #[inline(never)]
    pub(super) fn smoke_identity(&mut self) {
        for (w, h) in [(800, 600), (1280, 800)] {
            self.width = w;
            self.height = h;
            self.identity_fixture();
            let original = self.doc.archive.bytes().unwrap();
            // Short name: typed in place, amber with its saved value; reset.
            assert_eq!(self.smoke_names().short, "Demo");
            self.smoke_click(&|a| matches!(a, Action::IdentityEdit(false)));
            assert!(matches!(
                self.prompt.as_ref().unwrap().kind,
                PromptKind::IdentityName(false)
            ));
            self.smoke_type("Bad\"name");
            assert!(self.status.starts_with("Error: Short name must be"));
            self.smoke_type("Twin");
            assert!(self.prompt.is_none(), "{}", self.status);
            assert_eq!(self.smoke_names().short, "Twin");
            assert_eq!(self.doc.changed_count(), 1);
            assert!(self.draw().commands.iter().any(
                |d| matches!(d, Draw::Text(_, _, s, color, _) if s == "Twin" && *color == c::AMBER.0)
            ));
            assert!(self.draw().commands.iter().any(
                |d| matches!(d, Draw::Text(_, _, s, color, _) if s == "Demo" && *color == c::INK_MUTED.0)
            ));
            self.smoke_click(&|a| matches!(a, Action::IdentityReset(false)));
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
            assert!(self
                .chrome_hit(&|a| matches!(a, Action::IdentityReset(_)))
                .is_none());
            // Long name, then undo each step back to the file's bytes.
            self.smoke_click(&|a| matches!(a, Action::IdentityEdit(true)));
            self.smoke_type("Twin test aircraft");
            assert_eq!(self.smoke_names().long, "Twin test aircraft");
            assert_eq!(self.smoke_names().short, "Demo");
            self.key(Key::Char('z'), true, false);
            assert_eq!(self.smoke_names().long, "Synthetic aircraft");
            self.key(Key::Char('z'), true, false);
            self.key(Key::Char('z'), true, false);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);

            // Rename reference ID: review, Back, apply, undo.
            self.smoke_click(&|a| matches!(a, Action::RenameAircraft));
            assert_eq!(self.prompt.as_ref().unwrap().value, "DEMO");
            self.smoke_type("NEO");
            assert!(
                matches!(self.prompt.as_ref().unwrap().kind, PromptKind::RenameReview),
                "{}",
                self.status
            );
            let plan = self.identity.rename.as_ref().unwrap();
            assert_eq!(plan.renames[0], ("DEMO.PT".into(), "NEO.PT".into()));
            assert!(plan
                .renames
                .contains(&("DEMO.ORG".into(), "NEO.ORG".into())));
            assert!(plan.shared.iter().any(|(n, _)| n == "SHOT.JT"));
            assert!(self.draw().commands.iter().any(|d| matches!(d,
                Draw::Text(_, _, s, _, _) if s.starts_with("Missions and other LIBs that refer to DEMO.PT"))));
            self.smoke_click(&|a| matches!(a, Action::IdentityBack));
            assert!(matches!(
                self.prompt.as_ref().unwrap().kind,
                PromptKind::RenameId
            ));
            assert_eq!(self.prompt.as_ref().unwrap().value, "NEO");
            self.key(Key::Enter, false, false);
            self.smoke_click(&|a| matches!(a, Action::Apply));
            assert!(self.prompt.is_none(), "{}", self.status);
            assert_eq!(self.name(), "NEO.PT");
            assert!(self.doc.archive.find("DEMO.PT").is_none());
            assert_eq!(self.smoke_names().reference, "NEO.PT");
            let report = hangar_core::validation::inspect(&self.doc, &mut Default::default());
            assert_eq!(report.errors, 0, "{}", report.summary());
            self.key(Key::Char('z'), true, false);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
            // The same ID changes nothing.
            self.select_entry(self.doc.archive.find("DEMO.PT").unwrap());
            self.smoke_click(&|a| matches!(a, Action::RenameAircraft));
            self.key(Key::Enter, false, false);
            assert!(self.chrome_hit(&|a| matches!(a, Action::Apply)).is_none());
            self.key(Key::Escape, false, false);
            assert!(self.identity.rename.is_none());

            // Duplicate aircraft: wizard, review, toggles, apply, undo.
            self.smoke_click(&|a| matches!(a, Action::DuplicateAircraft));
            assert!(self.identity.in_place);
            for value in ["TWIN", "Twin", "Twin aircraft"] {
                assert!(self
                    .chrome_hit(&|a| matches!(a, Action::File(FileAction::CloneSource)))
                    .is_none());
                self.smoke_type(value);
            }
            assert!(
                matches!(
                    self.prompt.as_ref().unwrap().kind,
                    PromptKind::DuplicateReview
                ),
                "{}",
                self.status
            );
            let (_, names) = self.duplicate_rows();
            let shot = names.iter().position(|n| n == "SHOT.JT").unwrap();
            assert!(self.identity.share.contains("SHOT.JT"));
            let originals = |app: &App| {
                let p = &app.identity.duplicate.as_ref().unwrap().package;
                p.archive
                    .entries
                    .iter()
                    .filter(|e| e.name.ends_with(".ORG"))
                    .count()
            };
            // The copied skin's stored original follows it.
            assert_eq!(originals(self), 1);
            // Copy the weapon: its icon follows; then share it again.
            self.smoke_click(&|a| matches!(a, Action::DuplicateToggle(i, false) if i == shot));
            let copied = |app: &App, n: &str| {
                app.identity
                    .duplicate
                    .as_ref()
                    .unwrap()
                    .package
                    .mapping
                    .iter()
                    .any(|(o, _)| o == n)
            };
            assert!(copied(self, "SHOT.JT") && copied(self, "$SHOT.PIC"));
            let (_, names) = self.duplicate_rows();
            let shot = names.iter().position(|n| n == "SHOT.JT").unwrap();
            self.smoke_click(&|a| matches!(a, Action::DuplicateToggle(i, true) if i == shot));
            assert!(!copied(self, "SHOT.JT"));
            // Share the main shape: the copy references DEMO.SH.
            // Share the main shape: the copy references DEMO.SH, so its skin
            // (and the skin's original) is no longer copied.
            let (_, names) = self.duplicate_rows();
            let shape = names.iter().position(|n| n == "DEMO.SH").unwrap();
            self.smoke_click(&|a| matches!(a, Action::DuplicateToggle(i, true) if i == shape));
            assert!(!copied(self, "DEMO.SH") && !copied(self, "DEMO.PIC"));
            assert!(copied(self, "DEMO_S.SH"));
            assert_eq!(originals(self), 0);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
            self.smoke_click(&|a| matches!(a, Action::Apply));
            assert!(self.prompt.is_none(), "{}", self.status);
            assert_eq!(self.name(), "TWIN.PT");
            let names = self.smoke_names();
            assert_eq!(
                (
                    names.short.as_str(),
                    names.long.as_str(),
                    names.reference.as_str()
                ),
                ("Twin", "Twin aircraft", "TWIN.PT")
            );
            let text = String::from_utf8(self.data.clone()).unwrap();
            assert!(text.contains("\"DEMO.SH\"") && text.contains("\"SHOT.JT\""));
            assert!(text.contains("\"TWIN_S.SH\""));
            let report = hangar_core::validation::inspect(&self.doc, &mut Default::default());
            assert_eq!(report.errors, 0, "{}", report.summary());
            self.key(Key::Char('z'), true, false);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);

            // Context menu Duplicate on a PT opens the aircraft duplicate; a
            // shape keeps the plain resource duplicate.
            let pt = self.doc.archive.find("DEMO.PT").unwrap();
            for (entry, aircraft) in [(pt, true), (0, false)] {
                // Selecting first expands the entry's category in the outliner.
                self.select_entry(entry);
                let row = self
                    .chrome_hit(&|a| matches!(a, Action::Entry(i) if i == entry))
                    .unwrap();
                self.motion(row[0] + 70, row[1] + 8, false);
                self.pointer(row[0] + 70, row[1] + 8, 3, true, false);
                self.pointer(row[0] + 70, row[1] + 8, 3, false, false);
                assert_eq!(self.menu, Some(super::chrome::MENU_CONTEXT));
                let item = self.chrome_hit(&|a| {
                    matches!(a, Action::DuplicateAircraft | Action::RenameResource(true))
                });
                self.chrome_click(item.unwrap());
                if aircraft {
                    assert!(self.identity.in_place);
                    assert!(matches!(
                        self.prompt.as_ref().unwrap().kind,
                        PromptKind::CloneId
                    ));
                    assert!(self
                        .prompt
                        .as_ref()
                        .unwrap()
                        .title
                        .starts_with("Duplicate aircraft DEMO.PT"));
                } else {
                    assert!(matches!(
                        self.prompt.as_ref().unwrap().kind,
                        PromptKind::ResourceName(true)
                    ));
                }
                self.key(Key::Escape, false, false);
                assert!(!self.identity.in_place);
            }
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
        }
        self.width = 1280;
        self.height = 800;
        self.doc = Document::new(Archive::empty());
        self.demo();
    }
}
impl App {
    /// Manual real-data check: rename `rename`'s reference ID to `new_id`,
    /// duplicate `dup` as `dup_id` (default names and choices) in the same
    /// LIB, validate, and save create-new to `output` through the app.
    #[cfg(not(windows))]
    pub fn identity_check(
        &mut self,
        rename: &str,
        new_id: &str,
        dup: &str,
        dup_id: &str,
        output: &str,
    ) -> Result<String> {
        let mut out = String::new();
        let at = self
            .doc
            .archive
            .find(rename)
            .ok_or("Aircraft to rename not found")?;
        self.select_entry(at);
        self.mode = Mode::Model;
        self.act(Action::RenameAircraft);
        self.key(Key::Char('a'), true, false);
        for c in new_id.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        let plan = self
            .identity
            .rename
            .as_ref()
            .ok_or_else(|| self.status.clone())?;
        out.push_str(&format!(
            "Rename {} -> {}: {} renamed, {} rewritten in place, {} shared left alone, {} private kept, {} refusals, {} unparsed resources name {}\n",
            plan.old,
            plan.new,
            plan.renames.len(),
            plan.rewritten.len(),
            plan.shared.len(),
            plan.kept.len(),
            plan.refusals.len(),
            plan.mentions.len(),
            plan.old
        ));
        for (old, new) in &plan.renames {
            out.push_str(&format!("  {old:13} -> {new}\n"));
        }
        for (name, why) in &plan.shared {
            out.push_str(&format!("  shared {name:13} {why}\n"));
        }
        for r in plan.refusals.iter().chain(&plan.notes) {
            out.push_str(&format!("  {r}\n"));
        }
        let before = self.doc.archive.bytes()?;
        self.key(Key::Enter, false, false);
        if self.status.starts_with("Error:") || self.prompt.is_some() {
            return Err(self.status.clone());
        }
        if self
            .doc
            .archive
            .find(&format!("{}.PT", new_id.to_ascii_uppercase()))
            .is_none()
        {
            return Err("Renamed aircraft missing".into());
        }
        let renamed = self.doc.archive.bytes()?;
        self.key(Key::Char('z'), true, false);
        if self.doc.archive.bytes()? != before {
            return Err("Undo did not restore the exact bytes".into());
        }
        self.doc.redo();
        self.refresh();
        if self.doc.archive.bytes()? != renamed {
            return Err("Redo did not restore the rename".into());
        }
        out.push_str("  undo restores the exact LIB bytes; redo restores the rename\n");

        let at = self
            .doc
            .archive
            .find(dup)
            .ok_or("Aircraft to duplicate not found")?;
        self.select_entry(at);
        self.mode = Mode::Model;
        self.act(Action::DuplicateAircraft);
        self.key(Key::Char('a'), true, false);
        for c in dup_id.chars() {
            self.key(Key::Char(c), false, false);
        }
        // Default short and long names.
        for _ in 0..3 {
            self.key(Key::Enter, false, false);
        }
        let d = self
            .identity
            .duplicate
            .as_ref()
            .ok_or_else(|| self.status.clone())?;
        let p = &d.package;
        out.push_str(&format!(
            "Duplicate {} -> {}: {} copied ({} stored originals), {} shared\n",
            p.donor,
            d.root(),
            p.archive.entries.len(),
            p.archive.entries.len() - p.mapping.len(),
            p.shared.len()
        ));
        for (old, new) in &p.mapping {
            out.push_str(&format!("  copy   {old:13} -> {new}\n"));
        }
        for name in &p.shared {
            out.push_str(&format!("  shared {name}\n"));
        }
        self.key(Key::Enter, false, false);
        if self.status.starts_with("Error:") || self.prompt.is_some() {
            return Err(self.status.clone());
        }
        if self.name() != format!("{}.PT", dup_id.to_ascii_uppercase()) {
            return Err("The new aircraft is not selected".into());
        }
        let report = hangar_core::validation::inspect(&self.doc, &mut Default::default());
        out.push_str(&format!("In-memory validation: {}\n", report.summary()));
        self.file_prompt(FileAction::Save);
        self.key(Key::Char('a'), true, false);
        for c in output.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        if self.doc.dirty() {
            return Err(self.status.clone());
        }
        out.push_str(&format!("Saved {output}\n"));
        Ok(out)
    }
}
