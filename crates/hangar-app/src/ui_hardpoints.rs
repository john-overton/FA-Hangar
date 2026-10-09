use super::view::{text_fit, Action, Icon, Layout};
use super::*;
use hangar_core::hardpoints::{self, Station};
use hangar_core::model::ViewFrame;

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
    pub(super) fn station_owner(&self) -> Option<usize> {
        if matches!(extension(self.name()), "PT" | "NT") {
            return Some(self.selected);
        }
        let shape = self.model_entry.or(self.context_entry)?;
        let name = &self.doc.archive.entries.get(shape)?.name;
        let mut owners = self
            .dependencies
            .incoming(name)
            .filter(|n| matches!(extension(n), "PT" | "NT"))
            .filter_map(|n| self.doc.archive.find(n));
        let owner = owners.next()?;
        if owners.next().is_some() {
            None
        } else {
            Some(owner)
        }
    }
    pub(super) fn refresh_hardpoints(&mut self) {
        self.hp_context = self.station_owner().and_then(|entry| {
            let bytes = self.doc.archive.entries.get(entry)?.read().ok()?;
            let ext = extension(&self.doc.archive.entries[entry].name);
            let brf = Brf::parse(&bytes, ext).ok()?;
            let stations = hardpoints::read(&brf).ok()?;
            let saved = self
                .doc
                .saved_entry(&self.doc.archive.entries[entry].name)
                .and_then(|e| e.read().ok())
                .and_then(|b| Brf::parse(&b, ext).ok());
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
        self.model.as_ref().or(self.context_model.as_deref())
    }
    /// Shared viewport framing: the committed model's bounds, never a preview's.
    pub(super) fn model_bounds(&self) -> Option<([i32; 3], i32)> {
        let model = self.hp_model()?;
        (!model.vertices.is_empty()).then(|| bounds(model))
    }
    /// The main viewport's size, px.
    pub(super) fn view_size(&self) -> [i32; 2] {
        [self.right() - self.left() - 2, self.dock_y() - 54]
    }
    /// The shared viewport projection for a view of `size` px: framed on
    /// the committed model (else the shown one), the camera, zoom and pan
    /// (scaled from the main viewport). The raster, the wireframe and every
    /// overlay project through this, so they agree to the pixel.
    pub(super) fn view_frame(&self, size: [i32; 2]) -> Option<ViewFrame> {
        let (center, span) = self
            .model_bounds()
            .or_else(|| self.model_for_paint().map(bounds))?;
        let [vw, vh] = self.view_size();
        Some(ViewFrame {
            yaw: self.yaw,
            pitch: self.pitch,
            zoom: self.zoom,
            perspective: self.perspective && !self.textured,
            center,
            span,
            size,
            pan: [
                self.pan[0] as i64 * 16 * size[0] as i64 / vw.max(1) as i64,
                self.pan[1] as i64 * 16 * size[1] as i64 / vh.max(1) as i64,
            ],
        })
    }
    /// A model point on screen in the main viewport (`view_frame`).
    pub(super) fn hp_project(&self, p: [i32; 3]) -> Option<[i32; 2]> {
        let [x, y] = self.view_frame(self.view_size())?.project(p);
        Some([self.left() + 1 + x, 54 + y])
    }
    pub(super) fn cursor_station(&self, x: i32, y: i32, reference: [i32; 3]) -> Result<[i32; 3]> {
        if self.perspective && !self.textured {
            return Err("Use orthographic view to place or drag stations".into());
        }
        let frame = self
            .view_frame(self.view_size())
            .ok_or("No model for station placement")?;
        let at = [
            (x - self.left() - 1) as i64 * 16 + 8,
            (y - 54) as i64 * 16 + 8,
        ];
        let world = frame.unproject16(at, reference);
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
            self.status = "Station moved. Ctrl+Z undoes it".into();
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
        self.status = "Station added in one undo step".into();
        Ok(())
    }
    pub(super) fn station_remove(&mut self) -> Result<()> {
        let c = self.hp_context.as_ref().ok_or("Select an aircraft")?;
        let entry = c.entry;
        let bytes = hardpoints::remove(&self.doc.archive.entries[entry].read()?, self.hp_selected)?;
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Station removed. Ctrl+Z undoes it".into();
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
                "Default store: a JT, SEE, ECM or GAS entry; empty clears it".into(),
                s.store.clone().unwrap_or_default(),
            )
        } else {
            let f = &c.brf.fields[s.fields[column]];
            (
                format!("HP{} {} in source units", self.hp_selected + 1, f.label),
                f.value.clone(),
            )
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::StationValue(column),
            title,
            value,
            axis: 0,
            caret: super::Caret::END,
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
            c.brf.edit(
                &old,
                station.fields[column],
                value,
                extension(&self.doc.archive.entries[entry].name),
            )?
        };
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Station updated. Ctrl+Z undoes it".into();
        Ok(())
    }
    /// Control for station column `column`: a NumberField, or a text field
    /// that opens the type prompt for `$hex` operands.
    fn station_control(&self, o: &mut Layout, rect: [i32; 4], column: usize, label: &str) {
        use super::widgets::{Number, NumberTarget};
        let t = NumberTarget::Station(column);
        match self.number_spec(t) {
            Some(spec) => o.number(
                rect,
                &Number {
                    target: t,
                    spec,
                    label,
                    unit: "",
                    locked: false,
                    axis: (1..=3).contains(&column).then(|| column - 1),
                },
            ),
            None => {
                let value = self
                    .hp_context
                    .as_ref()
                    .and_then(|c| {
                        let s = c.stations.get(self.hp_selected)?;
                        c.brf.fields.get(*s.fields.get(column)?)
                    })
                    .map_or(String::new(), |f| f.value.clone());
                o.text_field(
                    rect,
                    &format!("{label} {value}"),
                    false,
                    Action::StationField(column),
                );
            }
        }
    }
    pub(super) fn hardpoint_inspector(&self, o: &mut Layout) {
        use super::widgets::{pane, Btn, Check};
        use theme::{metric as m, space};
        self.inspector_header(o, None);
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0);
        let Some(c) = &self.hp_context else {
            if self.pane(o, &mut s, pane::STATION, "Stations", Icon::Hardpoint) {
                o.stack_notice(
                    &mut s,
                    super::widgets::Tone::Neutral,
                    "Select the PT or NT that owns this shape to edit its stations.",
                );
            }
            o.panel_end(&mut s);
            o.stack_end(s);
            return;
        };
        let station = c.stations.get(self.hp_selected);
        if self.pane(o, &mut s, pane::STATION, "Station", Icon::Hardpoint) {
            o.info(&mut s, "Owner", &self.doc.archive.entries[c.entry].name, "");
            if let Some([x, y, w, h]) = o.wide(&mut s, m::BUTTON_H) {
                o.button_ex(
                    [x, y, h, h],
                    Btn::icon(Icon::ChevronLeft),
                    Action::HardpointStep(-1),
                );
                o.button_ex(
                    [x + w - h, y, h, h],
                    Btn::icon(Icon::ChevronRight),
                    Action::HardpointStep(1),
                );
                let title = if c.stations.is_empty() {
                    "No stations".to_string()
                } else {
                    format!("HP{} of {}", self.hp_selected + 1, c.stations.len())
                };
                let tw = text_width(&title, Style::Strong);
                o.canvas.styled(
                    x + (w - tw) / 2,
                    super::widgets::baseline(y, h, Style::Strong),
                    &title,
                    c::AMBER,
                    Style::Strong,
                );
            }
            if let Some(st) = station {
                o.stack_subhead(&mut s, "Location");
                for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
                    // Joined vector: three fields with no gap between them.
                    if s.collapsed() {
                        break;
                    }
                    if let Some(rect) = s.take(m::FIELD_H) {
                        self.station_control(o, rect, k + 1, axis);
                    }
                }
                s.gap(space::SPACE_1);
                if let Some(rect) = o.prop(&mut s, "Store") {
                    o.select(
                        rect,
                        Some(Icon::Weapon),
                        st.store.as_deref().unwrap_or("None"),
                        Action::StationField(12),
                        false,
                    );
                }
            }
        }
        o.panel_end(&mut s);
        if station.is_some() {
            if self.pane(o, &mut s, pane::STATION_DATA, "Station data", Icon::Sliders) {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.segmented(
                        rect,
                        &[
                            (
                                Btn::new("Loadout").on(!self.hp_slew),
                                Action::StationSlew(false),
                            ),
                            (Btn::new("Slew").on(self.hp_slew), Action::StationSlew(true)),
                        ],
                    );
                }
                let rows = if self.hp_slew {
                    [
                        (4, "Heading"),
                        (5, "Pitch"),
                        (6, "Heading limit"),
                        (7, "Pitch limit"),
                    ]
                } else {
                    [
                        (9, "Weight class"),
                        (10, "Max items"),
                        (11, "Location code"),
                        (0, "Flags"),
                    ]
                };
                for (column, label) in rows {
                    if let Some(rect) = o.prop(&mut s, label) {
                        self.station_control(o, rect, column, "");
                    }
                }
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(rect, Btn::new("All station fields"), Action::StationFields);
                }
            }
            o.panel_end(&mut s);
        }
        if self.pane(o, &mut s, pane::STATIONS, "Stations", Icon::Hardpoint) {
            for (title, icon, action) in [
                ("Add station", Icon::Plus, Action::HardpointAdd(false)),
                ("Duplicate station", Icon::Plus, Action::HardpointAdd(true)),
            ] {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(rect, Btn::new(title).with_icon(icon), action);
                }
            }
            if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                o.button_ex(
                    rect,
                    Btn::new("Remove station")
                        .with_icon(Icon::Close)
                        .danger()
                        .enabled(station.is_some()),
                    Action::HardpointRemove,
                );
            }
            if let Some(rect) = o.wide(&mut s, m::ROW_H) {
                o.checkbox_row(
                    rect,
                    Some(Icon::Eye),
                    "Show markers",
                    "",
                    if self.hp_visible {
                        Check::On
                    } else {
                        Check::Off
                    },
                    Action::HardpointVisibility,
                );
            }
        }
        o.panel_end(&mut s);
        o.stack_end(s);
    }
}
/// Bounding-box centre and largest extent (at least 1) of a model's vertices.
pub(super) fn bounds(model: &Model) -> ([i32; 3], i32) {
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for v in &model.vertices {
        for k in 0..3 {
            min[k] = min[k].min(v.point[k]);
            max[k] = max[k].max(v.point[k]);
        }
    }
    if model.vertices.is_empty() {
        return ([0; 3], 1);
    }
    (
        core::array::from_fn(|i| ((min[i] as i64 + max[i] as i64) / 2) as i32),
        (0..3)
            .map(|i| max[i].saturating_sub(min[i]))
            .max()
            .unwrap_or(1)
            .max(1),
    )
}
