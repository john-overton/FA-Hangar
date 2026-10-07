use super::view::{count, Action, Icon, Layout};
use super::*;
use hangar_core::definition::{self, ASPECTS};

/// Source and target slot height.
const GRAFT_SLOT_H: i32 = 58;
/// Top of the donor list or differences below the slots.
const GRAFT_LIST_TOP: i32 = theme::metric::MENUBAR_H
    + theme::metric::EDITOR_HEADER_H
    + theme::space::SPACE_2
    + GRAFT_SLOT_H
    + theme::space::SPACE_3;
/// UI name and icon of a definition aspect (noun labels, no slashes).
pub(super) fn aspect_view(a: definition::Aspect) -> (&'static str, Icon) {
    use definition::Aspect as A;
    match a {
        A::Envelope => ("Flight envelope", Icon::Flight),
        A::Propulsion => ("Propulsion", Icon::Engine),
        A::Handling => ("Handling", Icon::Sliders),
        A::Weights => ("Weights", Icon::Measure),
        A::Damage => ("Damage", Icon::Damage),
        A::Hardpoints => ("Hardpoint values", Icon::Hardpoint),
        A::Systems => ("Systems", Icon::Info),
        A::Seeker => ("Seeker", Icon::Eye),
        A::Motor => ("Motor", Icon::Engine),
        A::Warhead => ("Warhead", Icon::Weapon),
        A::Movement => ("Movement", Icon::Move),
        A::Engagement => ("Engagement and firing", Icon::Weapon),
    }
}
pub(super) struct Donor {
    pub path: String,
    pub name: String,
    pub brf: Brf,
}
impl App {
    pub(super) fn pin_donor(&mut self) -> Result<()> {
        let brf = self
            .brf
            .clone()
            .ok_or("Select a textual definition to use as donor")?;
        self.graft_donor = Some(Donor {
            path: self.path.clone(),
            name: self.name().into(),
            brf,
        });
        self.graft_library = None;
        self.refresh_graft();
        self.mode = Mode::Graft;
        self.status = "Donor snapshot pinned; select a target entry in the outliner".into();
        Ok(())
    }
    pub(super) fn choose_graft_donor(&mut self, i: usize) -> Result<()> {
        let (path, archive) = self.graft_library.as_ref().ok_or("No donor library")?;
        let e = archive.entries.get(i).ok_or("Donor missing")?;
        let brf = Brf::parse(&e.read()?, extension(&e.name))?;
        self.graft_donor = Some(Donor {
            path: path.clone(),
            name: e.name.clone(),
            brf,
        });
        self.graft_library = None;
        self.refresh_graft();
        self.status = "Select aspects and review the differences before Apply graft".into();
        Ok(())
    }
    pub(super) fn refresh_graft(&mut self) {
        self.graft_scroll = 0;
        self.graft_preview = Default::default();
        if let (Some(target), Some(donor)) = (&self.brf, &self.graft_donor) {
            if extension(&donor.name) != extension(self.name()) {
                self.graft_preview
                    .conflicts
                    .push("Source and target must have the same definition type".into());
            } else {
                self.graft_preview = definition::Graft::review(target, &donor.brf, self.graft_mask);
            }
        }
    }
    pub(super) fn apply_graft(&mut self) -> Result<()> {
        if self.graft_donor.is_none() || self.graft_preview.edits.is_empty() {
            return Err("Choose a donor and aspects with changed values".into());
        }
        let count = self.graft_preview.edits.len();
        let bytes = self
            .graft_preview
            .apply(&self.data, extension(self.name()))?;
        self.doc.replace(self.selected, bytes)?;
        self.refresh();
        self.status = format!("Grafted {count} values / one Ctrl+Z undo step");
        Ok(())
    }
    pub(super) fn graft_row_count(&self) -> usize {
        self.graft_library.as_ref().map_or(
            self.graft_preview.conflicts.len() + self.graft_preview.edits.len(),
            |(_, a)| {
                a.entries
                    .iter()
                    .filter(|e| extension(&e.name) == extension(self.name()))
                    .count()
            },
        )
    }
    /// Rows of the graft list (donor entries or differences) that fit.
    pub(super) fn graft_visible(&self) -> usize {
        ((self.dock_y() - GRAFT_LIST_TOP - 2 * theme::metric::ROW_H) / theme::metric::ROW_H).max(1)
            as usize
    }
    /// Graft editor: source and target slots, then the donor list or the
    /// field differences the selected aspects would apply.
    pub(super) fn graft_layout(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, notched, subhead, Btn};
        let (l, r) = (self.left(), self.right());
        let (x, w) = (l + 1, r - l - 2);
        let top = m::MENUBAR_H;
        o.canvas.rect(x, top, w, self.dock_y() - top, c::GM_800);
        o.canvas
            .rect(x, top + m::EDITOR_HEADER_H - 1, w, 1, c::GM_1000);
        o.canvas.styled(
            x + space::SPACE_3,
            baseline(top, m::EDITOR_HEADER_H, Style::Strong),
            "Graft",
            c::INK,
            Style::Strong,
        );
        let choose = Btn::new("Choose donor LIB").with_icon(Icon::Folder);
        let cw = choose.width();
        o.button_ex(
            [
                x + w - space::SPACE_1 - cw,
                top + (m::EDITOR_HEADER_H - m::BUTTON_H) / 2,
                cw,
                m::BUTTON_H,
            ],
            choose,
            Action::File(FileAction::GraftLibrary),
        );
        // Source -> target slots.
        let sy = top + m::EDITOR_HEADER_H + space::SPACE_2;
        let arrow = m::ICON + 2 * space::SPACE_2;
        let slot_w = (w - 2 * space::SPACE_3 - arrow) / 2;
        let file = |p: &str| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string();
        let slots = [
            (
                "Source",
                self.graft_donor
                    .as_ref()
                    .map_or("No donor".to_string(), |d| d.name.clone()),
                self.graft_donor
                    .as_ref()
                    .map_or("Use as graft donor or choose a LIB".into(), |d| {
                        file(&d.path)
                    }),
            ),
            ("Target", self.name().to_string(), file(self.lib_name())),
        ];
        for (n, (role, name, lib)) in slots.iter().enumerate() {
            let sx = x + space::SPACE_3 + n as i32 * (slot_w + arrow);
            let d = &mut o.canvas;
            notched(
                d,
                [sx, sy, slot_w, GRAFT_SLOT_H],
                Some(c::GM_950),
                Some(c::LINE_STRONG),
            );
            subhead(d, sx + space::SPACE_2, sy + 2, slot_w - 16, role);
            d.styled(
                sx + space::SPACE_2,
                baseline(sy + 20, 20, Style::Title),
                &fit(name, slot_w - 16, Style::Title),
                c::INK,
                Style::Title,
            );
            d.styled(
                sx + space::SPACE_2,
                baseline(sy + 38, 16, Style::ValueSm),
                &fit(lib, slot_w - 16, Style::ValueSm),
                c::INK_MUTED,
                Style::ValueSm,
            );
        }
        o.canvas.icon(
            x + space::SPACE_3 + slot_w + space::SPACE_2,
            sy + (GRAFT_SLOT_H - m::ICON) / 2,
            Icon::ChevronRight,
            c::STEEL,
            c::GM_800,
        );
        let list = GRAFT_LIST_TOP;
        let visible = self.graft_visible();
        let row_rect = |i: usize| {
            [
                x + space::SPACE_2,
                list + m::ROW_H + i as i32 * m::ROW_H,
                w - 2 * space::SPACE_2,
                m::ROW_H,
            ]
        };
        if let Some((_, archive)) = &self.graft_library {
            subhead(
                &mut o.canvas,
                x + space::SPACE_3,
                list,
                w - 24,
                "Donor entries",
            );
            for (row, (i, e)) in archive
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| extension(&e.name) == extension(self.name()))
                .skip(self.graft_scroll)
                .take(visible)
                .enumerate()
            {
                let rect = row_rect(row);
                let fill = if o.over(rect) { c::GM_700 } else { c::GM_800 };
                let d = &mut o.canvas;
                d.rect(rect[0], rect[1], rect[2], rect[3], fill);
                d.icon(
                    rect[0] + 4,
                    rect[1] + 2,
                    super::view::group_icon(&e.name),
                    c::INK_MUTED,
                    fill,
                );
                d.styled(
                    rect[0] + 4 + m::ICON + space::SPACE_2,
                    baseline(rect[1], m::ROW_H, Style::Value),
                    &fit(&e.name, rect[2] - 32, Style::Value),
                    c::INK,
                    Style::Value,
                );
                o.hit(rect, Action::GraftDonor(i));
            }
            return;
        }
        let edits = &self.graft_preview.edits;
        let conflicts = &self.graft_preview.conflicts;
        subhead(
            &mut o.canvas,
            x + space::SPACE_3,
            list,
            w - 24,
            &format!(
                "Changes \u{b7} {}",
                count(edits.len() + conflicts.len(), "field", "fields")
            ),
        );
        if edits.is_empty() && conflicts.is_empty() {
            o.canvas.styled(
                x + space::SPACE_3,
                baseline(list + m::ROW_H, m::ROW_H, Style::Label),
                &fit(
                    if self.graft_donor.is_some() {
                        "Select aspects to preview the values they change."
                    } else {
                        "Pin a donor to compare its values with this entry."
                    },
                    w - 24,
                    Style::Label,
                ),
                c::INK_MUTED,
                Style::Label,
            );
        }
        let vx = x + w * 55 / 100;
        for (row, i) in (0..conflicts.len() + edits.len())
            .skip(self.graft_scroll)
            .take(visible)
            .enumerate()
        {
            let [rx, ry, rw, rh] = row_rect(row);
            let d = &mut o.canvas;
            d.rect(
                rx,
                ry,
                rw,
                rh,
                if row % 2 == 0 { c::GM_800 } else { c::GM_900 },
            );
            let base = baseline(ry, rh, Style::Value);
            if let Some(conflict) = conflicts.get(i) {
                d.icon(rx + 4, ry + 2, Icon::Warning, c::AMBER, c::GM_800);
                d.styled(
                    rx + 4 + m::ICON + space::SPACE_2,
                    baseline(ry, rh, Style::Label),
                    &fit(conflict, rw - 32, Style::Label),
                    c::INK,
                    Style::Label,
                );
                continue;
            }
            let e = &edits[i - conflicts.len()];
            d.styled(
                rx + 4,
                base,
                &fit(&e.label, vx - rx - 12, Style::Value),
                c::INK_MUTED,
                Style::Value,
            );
            let before = fit(&e.before, (rx + rw - vx) / 2 - 12, Style::Value);
            d.styled(vx, base, &before, c::INK_MUTED, Style::Value);
            let ax = vx + text_width(&before, Style::Value) + space::SPACE_1;
            d.styled(ax, base, "\u{2192}", c::INK_MUTED, Style::Value);
            let tx = ax + text_width("\u{2192}", Style::Value) + space::SPACE_1;
            d.styled(
                tx,
                base,
                &fit(&e.after, rx + rw - 4 - tx, Style::Value),
                c::AMBER,
                Style::Value,
            );
        }
        o.canvas.styled(
            x + space::SPACE_3,
            baseline(
                self.dock_y() - m::ROW_H - space::SPACE_1,
                m::ROW_H,
                Style::Label,
            ),
            &fit(
                "Unselected aspects keep the target's values.",
                w - 24,
                Style::Label,
            ),
            c::INK_MUTED,
            Style::Label,
        );
    }
    /// Graft panel: one checkbox row per aspect with its icon and a diff
    /// summary, conflicts as a Notice, Apply graft as the only primary button.
    pub(super) fn graft_inspector(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{pane, Btn, Check, Tone};
        self.inspector_header(o, None);
        let foot = m::BUTTON_H + 2 * space::SPACE_2;
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, foot);
        if self.pane(o, &mut s, pane::GRAFT, "Graft", Icon::Graft) {
            o.stack_subhead(&mut s, "Aspects to carry over");
            for (i, group) in ASPECTS.iter().enumerate() {
                let total = self.brf.as_ref().map_or(0, |b| {
                    b.fields
                        .iter()
                        .filter(|f| definition::aspect(&f.label) == Some(*group))
                        .count()
                });
                if total == 0 {
                    continue;
                }
                let on = self.graft_mask & group.bit() != 0;
                let changed = self
                    .graft_preview
                    .edits
                    .iter()
                    .filter(|e| definition::aspect(&e.label) == Some(*group))
                    .count();
                let summary = if on && self.graft_donor.is_some() {
                    format!("{changed} of {total} differ")
                } else {
                    count(total, "field", "fields")
                };
                let (label, icon) = aspect_view(*group);
                if let Some(rect) = o.wide(&mut s, m::ROW_H) {
                    o.checkbox_row(
                        rect,
                        Some(icon),
                        label,
                        &summary,
                        if on { Check::On } else { Check::Off },
                        Action::GraftGroup(i),
                    );
                }
            }
            if !self.graft_preview.conflicts.is_empty() {
                o.stack_notice(
                    &mut s,
                    Tone::Warn,
                    &format!(
                        "{} not compatible. Keep the target's values by clearing the aspect.",
                        count(self.graft_preview.conflicts.len(), "field is", "fields are")
                    ),
                );
            }
            o.stack_notice(
                &mut s,
                Tone::Neutral,
                "Hardpoint values copy numeric station data; store references stay with the target. Geometry and animation are not grafted. Fields must match in type and scaling.",
            );
        }
        o.panel_end(&mut s);
        o.stack_end(s);
        // Footer: Apply graft, the panel's one primary action.
        let r = self.right();
        let y = self.height - m::STATUSBAR_H - foot;
        o.canvas.rect(r + 1, y, self.width - r - 1, 1, c::GM_1000);
        let ready = self.graft_preview.conflicts.is_empty() && !self.graft_preview.edits.is_empty();
        let apply = Btn::new("Apply graft").primary().enabled(ready);
        let aw = apply.width();
        o.button_ex(
            [
                self.width - space::SPACE_2 - aw,
                y + space::SPACE_2,
                aw,
                m::BUTTON_H,
            ],
            apply,
            Action::ApplyGraft,
        );
        if !ready {
            o.canvas.styled(
                r + 1 + space::SPACE_3,
                widgets::baseline(y + space::SPACE_2, m::BUTTON_H, Style::Label),
                &fit(
                    "Select aspects with changes",
                    self.width - space::SPACE_2 - aw - space::SPACE_2 - (r + 1 + space::SPACE_3),
                    Style::Label,
                ),
                c::INK_MUTED,
                Style::Label,
            );
        }
    }

    /// Flight inspector: field groups as a list, BRF issues as notices, and
    /// the graft entry points.
    pub(super) fn definition_inspector(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, pane, Btn, Tone};
        self.inspector_header(o, None);
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0);
        if self.pane(o, &mut s, pane::GROUPS, "Field groups", Icon::Sliders) {
            let mut groups: Vec<(Option<definition::Aspect>, &str, Icon, usize)> = vec![(
                None,
                "All fields",
                Icon::Sliders,
                self.brf.as_ref().map_or(0, |b| b.fields.len()),
            )];
            for group in ASPECTS {
                let n = self.brf.as_ref().map_or(0, |b| {
                    b.fields
                        .iter()
                        .filter(|f| definition::aspect(&f.label) == Some(group))
                        .count()
                });
                if n > 0 {
                    let (label, icon) = aspect_view(group);
                    groups.push((Some(group), label, icon, n));
                }
            }
            for (group, label, icon, n) in groups {
                let Some(rect) = o.wide(&mut s, m::ROW_H) else {
                    continue;
                };
                let [x, y, w, h] = rect;
                let on = self.field_group == group;
                let fill = if on {
                    c::AMBER_DEEP
                } else if o.over(rect) {
                    c::GM_700
                } else {
                    c::GM_800
                };
                let d = &mut o.canvas;
                widgets::notched(d, rect, Some(fill), None);
                d.icon(
                    x + 4,
                    y + 2,
                    icon,
                    if on { c::AMBER } else { c::INK_MUTED },
                    fill,
                );
                let count = format!("{n}");
                let cx = x + w - space::SPACE_2 - text_width(&count, Style::ValueSm);
                d.styled(
                    cx,
                    baseline(y, h, Style::ValueSm),
                    &count,
                    c::INK_MUTED,
                    Style::ValueSm,
                );
                let tx = x + 4 + m::ICON + space::SPACE_2;
                d.styled(
                    tx,
                    baseline(y, h, Style::Label),
                    &fit(label, cx - space::SPACE_2 - tx, Style::Label),
                    if on { c::AMBER_BRIGHT } else { c::INK },
                    Style::Label,
                );
                o.hit(
                    rect,
                    Action::FieldGroup(group.map_or(usize::MAX, |g| {
                        ASPECTS.iter().position(|a| *a == g).unwrap()
                    })),
                );
            }
        }
        o.panel_end(&mut s);
        if let Some(b) = self.brf.as_ref().filter(|b| !b.issues.is_empty()) {
            if self.pane(o, &mut s, pane::ISSUES, "Diagnostics", Icon::Warning) {
                for issue in b.issues.iter().take(3) {
                    o.stack_notice(&mut s, Tone::Warn, issue);
                }
            }
            o.panel_end(&mut s);
        }
        if self.pane(o, &mut s, pane::GRAFT, "Graft", Icon::Graft) {
            for (title, action) in [
                ("Use as graft donor", Action::PinDonor),
                ("Graft characteristics", Action::Mode(Mode::Graft)),
            ] {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(rect, Btn::new(title), action);
                }
            }
        }
        o.panel_end(&mut s);
        o.stack_end(s);
    }
}

impl App {
    pub(super) fn smoke_graft(&mut self) {
        self.demo();
        self.select_entry(1);
        self.pin_donor().unwrap();
        let b = self.brf.as_ref().unwrap();
        let i = b
            .fields
            .iter()
            .position(|f| f.label == "object.weight")
            .unwrap();
        let target = b.edit(&self.data, i, "23456", "PT").unwrap();
        self.doc.import("TARGET.PT", target.clone()).unwrap();
        self.doc.mark_saved();
        self.select_entry(self.doc.archive.find("TARGET.PT").unwrap());
        self.mode = Mode::Graft;
        let hit = |app: &App, predicate: &dyn Fn(Action) -> bool| {
            app.layout()
                .hits
                .into_iter()
                .find(|h| predicate(h.action))
                .map(|h| h.rect)
        };
        // Apply graft is disabled (no hit region) until an aspect has changes.
        assert!(hit(self, &|a| matches!(a, Action::ApplyGraft)).is_none());
        let weights = hit(self, &|a| matches!(a, Action::GraftGroup(3))).expect("Weights row");
        assert_eq!(weights[3], theme::metric::ROW_H);
        self.click(weights[0] + weights[2] - 4, weights[1] + 4, 1, true);
        assert_eq!(self.graft_mask, definition::Aspect::Weights.bit());
        assert_eq!(self.graft_preview.edits.len(), 1);
        assert!(self.graft_preview.conflicts.is_empty());
        assert!(self.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, color, _) if s == "1 of 2 differ" && *color == c::INK_MUTED.0)));
        let apply = hit(self, &|a| matches!(a, Action::ApplyGraft)).expect("Apply graft");
        // The primary button: amber fill.
        assert!(self.draw().commands.iter().any(|d| matches!(d,
            Draw::Rect(x, y, _, _, color) if *x == apply[0] + 1 && *y == apply[1] + 1 && *color == c::AMBER.0)));
        self.click(apply[0] + 4, apply[1] + 4, 1, true);
        assert!(self.doc.dirty());
        assert_eq!(self.brf.as_ref().unwrap().fields[i].value, "10000");
        self.act(Action::Undo);
        assert_eq!(self.data, target);
        assert!(!self.doc.dirty());
        for (w, h) in [(800, 600), (1280, 800)] {
            self.width = w;
            self.height = h;
            for mode in [Mode::Graft, Mode::Properties] {
                self.mode = mode;
                for hit in self.layout().hits {
                    assert!(
                        hit.rect[0] >= 0
                            && hit.rect[1] >= 0
                            && hit.rect[0] + hit.rect[2] <= w
                            && hit.rect[1] + hit.rect[3] <= h
                    );
                }
            }
        }
        self.width = 1280;
        self.height = 800;
        self.field_group = None;
        self.graft_donor = None;
        self.graft_mask = 0;
        self.demo();
    }
}
