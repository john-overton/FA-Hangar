use super::view::{border, label_fit, text_fit, Action, Icon, Layout};
use super::*;
use hangar_core::definition::{self, ASPECTS};

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
    pub(super) fn graft_layout(&self, o: &mut Layout) {
        let (l, r, bottom) = (self.left(), self.right(), self.dock_y());
        let w = r - l;
        o.canvas
            .label(l + 12, 44, "Graft / characteristics", c::INK);
        let cw = (w - 36) / 2;
        for (x, title, name, path, col) in [
            (
                l + 12,
                "SOURCE SNAPSHOT",
                self.graft_donor
                    .as_ref()
                    .map_or("Choose donor", |d| d.name.as_str()),
                self.graft_donor.as_ref().map_or("", |d| d.path.as_str()),
                c::STEEL,
            ),
            (
                l + 24 + cw,
                "TARGET",
                self.name(),
                self.path.as_str(),
                c::AMBER,
            ),
        ] {
            o.canvas.rect(x, 60, cw, 84, c::GM_900);
            border(&mut o.canvas, x, 60, cw, 84, c::LINE_STRONG);
            label_fit(&mut o.canvas, x + 8, 80, cw - 16, title, col);
            text_fit(&mut o.canvas, x + 8, 105, cw - 16, name, c::INK);
            text_fit(
                &mut o.canvas,
                x + 8,
                130,
                cw - 16,
                path.rsplit(['/', '\\']).next().unwrap_or(path),
                c::INK_MUTED,
            );
        }
        o.button(
            [l + 12, 154, w - 24, 24],
            "Choose donor LIB",
            Action::File(FileAction::GraftLibrary),
            false,
        );
        if let Some((_, archive)) = &self.graft_library {
            label_fit(
                &mut o.canvas,
                l + 12,
                199,
                w - 24,
                "Select a compatible donor entry / wheel scroll",
                c::STEEL,
            );
            for (row, (i, e)) in archive
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| extension(&e.name) == extension(self.name()))
                .skip(self.graft_scroll)
                .take(((bottom - 228) / 26).max(0) as usize)
                .enumerate()
            {
                o.button(
                    [l + 12, 210 + row as i32 * 26, w - 24, 24],
                    &e.name,
                    Action::GraftDonor(i),
                    false,
                );
            }
            return;
        }
        label_fit(
            &mut o.canvas,
            l + 12,
            199,
            w - 24,
            "Field differences / target -> donor / source units",
            c::INK_MUTED,
        );
        let mut rows: Vec<(String, String, Rgb)> = self
            .graft_preview
            .conflicts
            .iter()
            .map(|s| {
                (
                    "CONFLICT / keep target or deselect aspect".into(),
                    s.clone(),
                    c::DANGER,
                )
            })
            .collect();
        rows.extend(self.graft_preview.edits.iter().map(|e| {
            (
                e.label.clone(),
                format!("{} -> {}", e.before, e.after),
                c::AMBER,
            )
        }));
        if rows.is_empty() {
            label_fit(
                &mut o.canvas,
                l + 12,
                234,
                w - 24,
                "Select aspects on the right to preview changes",
                c::INK_FAINT,
            );
        }
        for (i, (label, value, color)) in rows
            .iter()
            .skip(self.graft_scroll)
            .take(((bottom - 230) / 44).max(0) as usize)
            .enumerate()
        {
            let y = 211 + i as i32 * 44;
            o.canvas.rect(l + 10, y, w - 20, 42, c::GM_900);
            text_fit(&mut o.canvas, l + 18, y + 15, w - 36, label, c::INK_MUTED);
            text_fit(&mut o.canvas, l + 18, y + 34, w - 36, value, *color);
        }
        label_fit(
            &mut o.canvas,
            l + 12,
            bottom - 10,
            w - 24,
            "Wheel scroll / unselected aspects keep target values",
            c::INK_FAINT,
        );
    }
    pub(super) fn graft_inspector(&self, o: &mut Layout) {
        let (r, w, h) = (self.right(), self.width - self.right(), self.height);
        o.canvas.label(r + 12, 44, "ASPECTS TO CARRY OVER", c::INK);
        let mut y = 65;
        for (i, group) in ASPECTS.iter().enumerate() {
            if !self.brf.as_ref().is_some_and(|b| {
                b.fields
                    .iter()
                    .any(|f| definition::aspect(&f.label) == Some(*group))
            }) {
                continue;
            }
            let count = self
                .graft_preview
                .edits
                .iter()
                .filter(|e| definition::aspect(&e.label) == Some(*group))
                .count();
            let on = self.graft_mask & group.bit() != 0;
            o.button(
                [r + 10, y, w - 20, 24],
                &format!(
                    "[{}] {} / {count}",
                    if on { "x" } else { " " },
                    group.label()
                ),
                Action::GraftGroup(i),
                on,
            );
            y += 29;
        }
        for (i, line) in [
            "Hardpoints: numeric station values.",
            "Store references stay with target.",
            "Geometry and animation are separate.",
            "Matching field types/scaling required.",
        ]
        .iter()
        .enumerate()
        {
            label_fit(
                &mut o.canvas,
                r + 12,
                h - 176 + i as i32 * 22,
                w - 24,
                line,
                c::INK_FAINT,
            );
        }
        if self.graft_preview.conflicts.is_empty() && !self.graft_preview.edits.is_empty() {
            o.button(
                [r + 12, h - 64, w - 24, 28],
                "Apply graft",
                Action::ApplyGraft,
                true,
            );
        } else {
            label_fit(
                &mut o.canvas,
                r + 12,
                h - 44,
                w - 24,
                "Choose compatible aspects to apply",
                c::INK_MUTED,
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
        self.act(Action::GraftGroup(3));
        assert_eq!(self.graft_preview.edits.len(), 1);
        assert!(self.graft_preview.conflicts.is_empty());
        let hit = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::ApplyGraft))
            .unwrap();
        self.click(hit.rect[0] + 4, hit.rect[1] + 4, 1, true);
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
