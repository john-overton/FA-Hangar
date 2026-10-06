use super::view::{label_fit, text_fit, Action, Layout};
use super::*;
use hangar_core::hardpoints::{self, Station};

pub(super) struct Context {
    pub entry: usize,
    pub stations: Vec<Station>,
    pub brf: Brf,
    pub saved: Option<Brf>,
}
pub(super) struct Drag {
    pub original: Entry,
    pub entry: usize,
    pub station: usize,
    pub position: [i32; 3],
    pub reference: [i32; 3],
}
impl App {
    pub(super) fn aircraft_owner(&self) -> Option<usize> {
        if self.name().ends_with(".PT") {
            return Some(self.selected);
        }
        let shape = self.model_entry.or(self.context_entry)?;
        let name = &self.doc.archive.entries.get(shape)?.name;
        let mut owners = self
            .dependencies
            .incoming(name)
            .filter(|n| n.ends_with(".PT"))
            .filter_map(|n| self.doc.archive.find(n));
        let owner = owners.next()?;
        if owners.next().is_some() {
            None
        } else {
            Some(owner)
        }
    }
    pub(super) fn refresh_hardpoints(&mut self) {
        self.hp_context = self.aircraft_owner().and_then(|entry| {
            let bytes = self.doc.archive.entries.get(entry)?.read().ok()?;
            let brf = Brf::parse(&bytes, "PT").ok()?;
            let stations = hardpoints::read(&brf).ok()?;
            let saved = self
                .doc
                .saved_entry(&self.doc.archive.entries[entry].name)
                .and_then(|e| e.read().ok())
                .and_then(|b| Brf::parse(&b, "PT").ok());
            Some(Box::new(Context {
                entry,
                stations,
                brf,
                saved,
            }))
        });
        self.hp_selected = self.hp_selected.min(
            self.hp_context
                .as_ref()
                .map_or(0, |c| c.stations.len().saturating_sub(1)),
        );
    }
    fn hp_model(&self) -> Option<&Model> {
        self.model.as_ref().or(self.context_model.as_ref())
    }
    fn model_bounds(&self) -> Option<([i32; 3], i32)> {
        let model = self.hp_model()?;
        if model.vertices.is_empty() {
            return None;
        }
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for v in &model.vertices {
            for k in 0..3 {
                min[k] = min[k].min(v.point[k]);
                max[k] = max[k].max(v.point[k]);
            }
        }
        Some((
            core::array::from_fn(|i| ((min[i] as i64 + max[i] as i64) / 2) as i32),
            (0..3)
                .map(|i| max[i].saturating_sub(min[i]))
                .max()
                .unwrap_or(1)
                .max(1),
        ))
    }
    pub(super) fn hp_project(&self, p: [i32; 3]) -> Option<[i32; 2]> {
        let (center, span) = self.model_bounds()?;
        let p = [p[0] - center[0], p[2] - center[2], p[1] - center[1]];
        let p = model::rotate(model::rotate(p, 1, self.yaw), 0, self.pitch);
        let (w, h) = (self.right() - self.left() - 2, self.dock_y() - 54);
        let denom = if self.perspective && !self.textured {
            (span as i64 * 4 - p[2] as i64).max(span as i64)
        } else {
            span as i64 * 4
        };
        Some([
            self.left()
                + 1
                + w / 2
                + self.pan[0]
                + (p[0] as i64 * w.min(h) as i64 * self.zoom as i64 * 3 / (denom * 100)) as i32,
            54 + h / 2 + self.pan[1]
                - (p[1] as i64 * w.min(h) as i64 * self.zoom as i64 * 3 / (denom * 100)) as i32,
        ])
    }
    pub(super) fn cursor_station(&self, x: i32, y: i32, reference: [i32; 3]) -> Result<[i32; 3]> {
        if self.perspective && !self.textured {
            return Err("Use orthographic view to place or drag stations".into());
        }
        let (center, span) = self
            .model_bounds()
            .ok_or("No model for station placement")?;
        let (w, h) = (self.right() - self.left() - 2, self.dock_y() - 54);
        let p = [
            reference[0] - center[0],
            reference[2] - center[2],
            reference[1] - center[1],
        ];
        let mut p = model::rotate(model::rotate(p, 1, self.yaw), 0, self.pitch);
        let denom = w.min(h) as i64 * self.zoom as i64 * 3;
        p[0] =
            ((x - self.left() - 1 - w / 2 - self.pan[0]) as i64 * span as i64 * 400 / denom) as i32;
        p[1] = -((y - 54 - h / 2 - self.pan[1]) as i64 * span as i64 * 400 / denom) as i32;
        let p = model::rotate(model::rotate(p, 0, -self.pitch), 1, -self.yaw);
        let world = [p[0] + center[0], p[2] + center[1], p[1] + center[2]];
        if world.iter().any(|v| !(-32768..=32767).contains(v)) {
            return Err("Station outside signed source-coordinate range".into());
        }
        Ok(world)
    }
    pub(super) fn hardpoint_overlay(&self, o: &mut Layout) {
        if !self.hp_visible {
            return;
        }
        let Some(context) = &self.hp_context else {
            return;
        };
        for (i, station) in context.stations.iter().enumerate() {
            let position = self
                .hp_drag
                .as_ref()
                .filter(|d| d.station == i)
                .map_or(station.position, |d| d.position);
            let Some([x, y]) = self.hp_project(position) else {
                continue;
            };
            if x < self.left() + 10 || x > self.right() - 40 || y < 64 || y > self.dock_y() - 15 {
                continue;
            }
            let color = if self.hp_tool && i == self.hp_selected {
                c::AMBER
            } else {
                c::STEEL
            };
            for (a, b) in [
                ([x, y - 6], [x + 6, y]),
                ([x + 6, y], [x, y + 6]),
                ([x, y + 6], [x - 6, y]),
                ([x - 6, y], [x, y - 6]),
            ] {
                o.canvas.line(a[0], a[1], b[0], b[1], color);
            }
            text_fit(
                &mut o.canvas,
                x + 9,
                y + 4,
                self.right() - x - 12,
                &format!("HP{}", i + 1),
                color,
            );
            o.hit([x - 9, y - 9, 18, 18], Action::HardpointSelect(i));
        }
    }
    pub(super) fn start_station_drag(&mut self, index: usize) {
        self.finish_stroke();
        self.hp_selected = index;
        self.hp_tool = true;
        self.selected_face = None;
        self.decal_draft = None;
        self.decal_active = false;
        if let Some(c) = &self.hp_context {
            if let Some(station) = c.stations.get(index) {
                self.hp_drag = Some(Drag {
                    entry: c.entry,
                    station: index,
                    original: self.doc.archive.entries[c.entry].clone(),
                    position: station.position,
                    reference: station.position,
                });
            }
        }
    }
    pub(super) fn finish_station_drag(&mut self) -> Result<()> {
        if let Some(d) = self.hp_drag.take() {
            if d.position == d.reference {
                return Ok(());
            }
            if !self
                .doc
                .archive
                .entries
                .get(d.entry)
                .is_some_and(|e| e.same_storage(&d.original))
            {
                return Err("Station owner changed; drag cancelled".into());
            }
            let bytes = hardpoints::position(&d.original.read()?, d.station, d.position)?;
            self.doc.replace(d.entry, bytes)?;
            self.refresh();
            self.hp_selected = d.station;
            self.status = "Station moved / Ctrl+Z undo".into();
        }
        Ok(())
    }
    pub(super) fn station_add(&mut self, duplicate: bool, at_cursor: bool) -> Result<()> {
        self.finish_stroke();
        if at_cursor
            && (self.mode != Mode::Model
                || self.mouse[0] <= self.left()
                || self.mouse[0] >= self.right()
                || self.mouse[1] < 54
                || self.mouse[1] >= self.dock_y())
        {
            return Err("Place the cursor in the Model viewport before pressing H".into());
        }
        let c = self
            .hp_context
            .as_ref()
            .ok_or("Select a recognized aircraft PT")?;
        let entry = c.entry;
        let count = c.stations.len();
        let selected = c.stations.get(self.hp_selected);
        let reference = selected.map_or([0; 3], |s| s.position);
        let xyz = if at_cursor {
            self.cursor_station(self.mouse[0], self.mouse[1], reference)?
        } else {
            reference
        };
        let bytes = hardpoints::add(
            &self.doc.archive.entries[entry].read()?,
            if duplicate && selected.is_some() {
                Some(self.hp_selected)
            } else {
                None
            },
            xyz,
        )?;
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.hp_selected = count;
        self.hp_visible = true;
        self.hp_tool = true;
        self.status = "Station added / one Ctrl+Z undo step".into();
        Ok(())
    }
    pub(super) fn station_remove(&mut self) -> Result<()> {
        let c = self.hp_context.as_ref().ok_or("Select an aircraft")?;
        let entry = c.entry;
        let bytes = hardpoints::remove(&self.doc.archive.entries[entry].read()?, self.hp_selected)?;
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Station removed / Ctrl+Z undo".into();
        Ok(())
    }
    pub(super) fn station_prompt(&mut self, column: usize) {
        let Some(c) = &self.hp_context else {
            return;
        };
        let Some(s) = c.stations.get(self.hp_selected) else {
            return;
        };
        let (title, value) = if column == 12 {
            (
                "Default store / JT, SEE, ECM, GAS; empty clears".into(),
                s.store.clone().unwrap_or_default(),
            )
        } else {
            let f = &c.brf.fields[s.fields[column]];
            (
                format!("HP{} / {} / source value", self.hp_selected + 1, f.label),
                f.value.clone(),
            )
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::StationValue(column),
            title,
            value,
            axis: 0,
        });
    }
    pub(super) fn station_value(&mut self, column: usize, value: &str) -> Result<()> {
        let c = self.hp_context.as_ref().ok_or("No station owner")?;
        let entry = c.entry;
        let old = self.doc.archive.entries[entry].read()?;
        let station = c.stations.get(self.hp_selected).ok_or("No station")?;
        let bytes = if column == 12 {
            hardpoints::store(&old, self.hp_selected, value)?
        } else if (1..=3).contains(&column) {
            let mut xyz = station.position;
            xyz[column - 1] = value
                .parse()
                .map_err(|_| "Enter a signed source coordinate")?;
            hardpoints::position(&old, self.hp_selected, xyz)?
        } else {
            c.brf.edit(&old, station.fields[column], value, "PT")?
        };
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Station updated / Ctrl+Z undo".into();
        Ok(())
    }
    pub(super) fn hardpoint_inspector(&self, o: &mut Layout) {
        let (r, w, h) = (self.right(), self.width - self.right(), self.height);
        o.canvas
            .label(r + 12, 44, "HARDPOINTS / source coordinates", c::INK);
        let Some(c) = &self.hp_context else {
            label_fit(
                &mut o.canvas,
                r + 12,
                85,
                w - 24,
                "Select the owning aircraft PT.",
                c::INK_MUTED,
            );
            return;
        };
        text_fit(
            &mut o.canvas,
            r + 12,
            75,
            w - 24,
            &self.doc.archive.entries[c.entry].name,
            c::STEEL,
        );
        o.button([r + 12, 88, 50, 24], "<", Action::HardpointStep(-1), false);
        text_fit(
            &mut o.canvas,
            r + 72,
            105,
            w - 138,
            &format!("HP {} / {}", self.hp_selected + 1, c.stations.len()),
            c::AMBER,
        );
        o.button(
            [self.width - 62, 88, 50, 24],
            ">",
            Action::HardpointStep(1),
            false,
        );
        if let Some(s) = c.stations.get(self.hp_selected) {
            let changed = |column: usize| {
                let f = &c.brf.fields[s.fields[column]];
                c.saved
                    .as_ref()
                    .and_then(|b| b.fields.iter().find(|old| old.label == f.label))
                    .is_none_or(|old| old.value != f.value || old.kind != f.kind)
            };
            let pos = self.hp_drag.as_ref().map_or(s.position, |d| d.position);
            for (k, label) in ["X", "Y", "Z"].iter().enumerate() {
                o.button(
                    [r + 12, 124 + k as i32 * 28, w - 24, 24],
                    &format!("{label}  {}", pos[k]),
                    Action::StationField(k + 1),
                    changed(k + 1) || self.hp_drag.is_some(),
                );
            }
            for (row, (column, label)) in [
                (9, "Weight class"),
                (10, "Max items"),
                (11, "Location code"),
                (0, "Flags"),
            ]
            .iter()
            .enumerate()
            {
                o.button(
                    [r + 12, 212 + row as i32 * 28, w - 24, 24],
                    &format!("{label}  {}", c.brf.fields[s.fields[*column]].value),
                    Action::StationField(*column),
                    changed(*column),
                );
            }
            o.button(
                [r + 12, 330, w - 24, 26],
                &format!("Store: {}", s.store.as_deref().unwrap_or("(none)")),
                Action::StationField(12),
                changed(8),
            );
        }
        label_fit(
            &mut o.canvas,
            r + 12,
            382,
            w - 24,
            "Drag diamond in orthographic view.",
            c::INK_MUTED,
        );
        label_fit(
            &mut o.canvas,
            r + 12,
            404,
            w - 24,
            "H places a station at the cursor.",
            c::INK_MUTED,
        );
        o.button(
            [r + 12, 420, w - 24, 24],
            "All station fields / slew limits",
            Action::StationFields,
            false,
        );
        for (offset, title, action) in [
            (146, "Add station", Action::HardpointAdd(false)),
            (115, "Duplicate station", Action::HardpointAdd(true)),
            (84, "Remove station", Action::HardpointRemove),
            (53, "Hide / show markers", Action::HardpointVisibility),
        ] {
            o.button([r + 12, h - offset, w - 24, 24], title, action, false);
        }
    }
}
