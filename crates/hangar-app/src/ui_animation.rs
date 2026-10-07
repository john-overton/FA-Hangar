//! Parts: the stub-driven moving parts of a shape (gear, flaps, rudder, hook,
//! brakes, bays, afterburner, swing wings, canards) listed by role, a preview
//! pose keyed by import variable name, and each part's census-backed
//! settings applied through `shape_parts::apply_part_setting`.
use super::view::{Action, Icon, Layout};
use super::widgets::{baseline, pane, type_badge, Btn, Number, NumberSpec, NumberTarget, Tone};
use super::*;
use hangar_core::{
    animation,
    shape_parts::{self, Allowed, Axis, PartInfo, Role, Setting},
};
use theme::{metric as m, space};

/// Pose presets: label and the variables they set.
pub(super) const PRESETS: [(&str, &[(&str, i32)]); 4] = [
    ("Gear down", &[("_PLgearDown", 1), ("_PLgearPos", 0)]),
    ("Gear up", &[("_PLgearDown", 0), ("_PLgearPos", -8192)]),
    ("Flaps down", &[("_PLleftFlap", -1), ("_PLrightFlap", -1)]),
    ("Afterburner", &[("_PLafterBurner", 1)]),
];
/// Pose variables in panel order; others follow by name.
const ORDER: [&str; 15] = [
    "_PLgearDown",
    "_PLgearPos",
    "_PLleftFlap",
    "_PLrightFlap",
    "_PLrudder",
    "_PLbrake",
    "_PLhook",
    "_PLafterBurner",
    "_PLbayOpen",
    "_PLbayDoorPos",
    "_PLswingWing",
    "_PLcanardPos",
    "_PLvtOn",
    "_PLvtAngle",
    "_PLslats",
];
/// Variables driving a transform: NumberFields rather than toggles.
const CONTINUOUS: [&str; 5] = [
    "_PLgearPos",
    "_PLswingWing",
    "_PLcanardPos",
    "_PLbayDoorPos",
    "_PLvtAngle",
];
/// Gear position for the panel: 0 is down (100% extended), -8192 is up.
const GEAR_TRAVEL: i64 = 8192;
/// Role groups of the parts list.
const GROUPS: [(&str, &[Role]); 7] = [
    ("Gear", &[Role::Gear, Role::GearMesh]),
    (
        "Control surfaces",
        &[Role::Flap, Role::Rudder, Role::Canard, Role::Slats],
    ),
    ("Wings", &[Role::SwingWing]),
    ("Brakes and hook", &[Role::SpeedBrake, Role::Hook]),
    ("Bays", &[Role::BayDoor]),
    ("Engines", &[Role::Afterburner, Role::ThrustVector]),
    ("Other", &[Role::Other]),
];
fn role_icon(r: Role) -> Icon {
    match r {
        Role::Gear | Role::GearMesh => Icon::Gear,
        Role::Flap | Role::Rudder | Role::Canard | Role::Slats => Icon::Surface,
        Role::SwingWing => Icon::Wing,
        Role::SpeedBrake | Role::BayDoor => Icon::Door,
        Role::Hook => Icon::Hook,
        Role::Afterburner | Role::ThrustVector => Icon::Flame,
        Role::Other => Icon::Sliders,
    }
}
/// Options of a part control: (value, label, census form).
pub(super) fn options(c: &shape_parts::Control, gear: bool) -> Vec<(i32, String, bool)> {
    let unseen = |v: i32| !c.unseen.contains(&v);
    match (&c.allowed, &c.setting) {
        (Allowed::Values(v), Setting::Shift { .. }) => v
            .iter()
            .map(|n| (*n, shift_label(*n as u8, gear), unseen(*n)))
            .collect(),
        (Allowed::Values(v), _) => v.iter().map(|n| (*n, n.to_string(), true)).collect(),
        (Allowed::Either, Setting::Direction { .. }) => [0, 1]
            .into_iter()
            .map(|v| (v, direction_label(v != 0).into(), unseen(v)))
            .collect(),
        (Allowed::Either, _) => vec![(1, "Equal".into(), true), (0, "Not equal".into(), true)],
        (Allowed::Axes(a), _) => a
            .iter()
            .map(|x| (*x as i32, x.label().into(), unseen(*x as i32)))
            .collect(),
        _ => Vec::new(),
    }
}
fn rel_symbol(r: hangar_core::shape_code::Rel) -> &'static str {
    use hangar_core::shape_code::Rel::*;
    match r {
        Eq => "=",
        Ne => "!=",
        Lt | Below => "<",
        Ge | AboveEq => ">=",
        Le | BelowEq => "<=",
        Gt | Above => ">",
    }
}
fn shift_label(n: u8, gear: bool) -> String {
    if gear {
        // 8192 units per half turn; gearPos runs 0 to -8192 (OpenFA).
        format!("sar {n} \u{b7} {}\u{b0}", 180 >> n)
    } else {
        format!("sar {n}")
    }
}
/// Both forms occur in the retail census (D1 alone and D1 + NEG), so the
/// labels name the encoding rather than a "standard" direction.
fn direction_label(negated: bool) -> &'static str {
    if negated {
        "NEG"
    } else {
        "No NEG"
    }
}
/// Panel label of a pose variable; unknown names fall back to the raw name.
pub(super) fn pose_label(var: &str) -> &str {
    let name = var.trim_start_matches("_PL");
    match name {
        "gearDown" => "Gear",
        "gearPos" => "Gear position",
        "leftFlap" => "Flap left",
        "rightFlap" => "Flap right",
        "brake" => "Speed brake",
        "hook" => "Hook",
        "afterBurner" => "Afterburner",
        "bayOpen" => "Bay doors",
        "swingWing" => "Swing wing",
        "canardPos" => "Canards",
        "bayDoorPos" => "Bay door position",
        "vtOn" => "Vectored thrust",
        "vtAngle" => "Thrust angle",
        "slats" => "Slats",
        _ => name,
    }
}
/// The current value of a control as an option value, and its label.
fn current(c: &shape_parts::Control, gear: bool) -> (i32, String) {
    match &c.setting {
        Setting::Compare { value, .. } => (*value, value.to_string()),
        Setting::Branch { equal, .. } => (
            i32::from(*equal),
            if *equal { "Equal" } else { "Not equal" }.into(),
        ),
        Setting::Shift { amount, .. } => (*amount as i32, shift_label(*amount, gear)),
        Setting::Direction { negated, .. } => {
            (i32::from(*negated), direction_label(*negated).into())
        }
        Setting::Axis { axis, .. } => (*axis as i32, axis.label().into()),
        Setting::Pivot(p) => (0, format!("{} {} {}", p[0], p[1], p[2])),
    }
}
impl App {
    pub(super) fn open_animation(&mut self) {
        self.finish_stroke();
        self.mesh_edit = false;
        self.mesh_drag = None;
        self.hp_tool = false;
        self.hp_visible = false;
        self.hp_drag = None;
        self.model_paint = false;
        self.paint_enabled = false;
        self.decal_active = false;
        self.decal_draft = None;
        self.selected_face = None;
        self.mode = Mode::Model;
        self.media_tab = 0;
        self.animation_tool = true;
        self.preview = None;
        self.status = if self.ed.parts.is_empty() {
            "Parts: this shape has no stub-driven parts".into()
        } else {
            format!(
                "Parts: {}. The pose preview is never saved.",
                view::count(self.ed.parts.len(), "part", "parts")
            )
        };
    }
    /// The shown SH's bytes: the local entry, or the linked shape in another LIB.
    fn shape_bytes(&self) -> Option<Vec<u8>> {
        if let Some(i) = self.model_entry {
            return self.doc.archive.entries.get(i)?.read().ok();
        }
        let (id, i) = self.external_model?;
        let l = self.libraries.iter().find(|l| l.id == id)?;
        l.doc.archive.entries.get(i)?.read().ok()
    }
    /// Recompute the parts list after the shape changed.
    pub(super) fn refresh_parts(&mut self) {
        self.ed.parts.clear();
        self.ed.parts_note.clear();
        if self.model.is_some() {
            if let Some(bytes) = self.shape_bytes() {
                match shape_parts::parts(&bytes) {
                    Ok(list) => self.ed.parts = list,
                    Err(e) => self.ed.parts_note = e,
                }
            }
        }
        if self
            .ed
            .part_selected
            .is_some_and(|i| i >= self.ed.parts.len() + self.static_parts().len())
        {
            self.ed.part_selected = None;
        }
    }
    /// C4/C6 parts of the shown model that no stub drives (indices into
    /// `Model::parts`); their pivots move with `animation::place_part`.
    pub(super) fn static_parts(&self) -> Vec<usize> {
        let Some(model) = &self.model else {
            return Vec::new();
        };
        (0..model.parts.len())
            .filter(|i| {
                !self
                    .ed
                    .parts
                    .iter()
                    .any(|p| p.id.target == model.parts[*i].offset)
            })
            .collect()
    }
    /// Pose variables of this shape's parts, in panel order.
    pub(super) fn pose_vars(&self) -> Vec<String> {
        let mut vars: Vec<String> = Vec::new();
        for p in &self.ed.parts {
            for v in &p.variables {
                if !vars.contains(v) {
                    vars.push(v.clone());
                }
            }
        }
        vars.sort_unstable_by_key(|v| (ORDER.iter().position(|o| o == v).unwrap_or(99), v.clone()));
        vars
    }
    /// Values a toggle variable is compared against, with 0 and the values
    /// the retail census shows for it.
    fn toggle_values(&self, var: &str) -> Vec<i32> {
        let mut out = vec![0];
        for v in shape_parts::census_values(var) {
            if !out.contains(v) {
                out.push(*v);
            }
        }
        for p in &self.ed.parts {
            for c in p.when.iter().flatten().filter(|c| c.variable == var) {
                if !out.contains(&c.value) {
                    out.push(c.value);
                }
            }
        }
        out.sort_unstable();
        out
    }
    /// Rebuild the shown model for the current pose (not an edit).
    fn pose_changed(&mut self) {
        self.refresh();
        let set: Vec<String> = self
            .ed
            .pose
            .iter()
            .map(|(k, v)| format!("{}={v}", pose_label(k)))
            .collect();
        self.status = if set.is_empty() {
            "Pose reset to the neutral preview.".into()
        } else {
            format!("Pose preview {}. Never saved.", set.join(" "))
        };
    }
    pub(super) fn pose_preset(&mut self, n: u8) {
        if let Some((_, vars)) = PRESETS.get(n as usize) {
            for (k, v) in vars.iter() {
                self.ed.pose.insert((*k).into(), *v);
            }
            self.pose_changed();
        }
    }
    pub(super) fn pose_reset(&mut self) {
        self.ed.pose.clear();
        self.pose_changed();
    }
    pub(super) fn pose_set(&mut self, var: u8, value: i32) {
        if let Some(name) = self.pose_vars().get(var as usize) {
            self.ed.pose.insert(name.clone(), value);
            self.pose_changed();
        }
    }
    /// Pose NumberField: gear position in percent extended, others raw.
    pub(super) fn pose_spec(&self, var: u8) -> Option<NumberSpec> {
        let name = self.pose_vars().get(var as usize)?.clone();
        let raw = self.ed.pose.get(&name).copied().unwrap_or(0) as i64;
        Some(if name == "_PLgearPos" {
            NumberSpec {
                value: ((GEAR_TRAVEL + raw.clamp(-GEAR_TRAVEL, 0)) * 100 / GEAR_TRAVEL),
                disk: None,
                step: 10,
                min: 0,
                max: 100,
                decimals: 0,
                bounded: true,
                group: false,
            }
        } else {
            NumberSpec {
                value: raw,
                disk: None,
                step: 256,
                min: -32768,
                max: 32767,
                decimals: 0,
                bounded: false,
                group: false,
            }
        })
    }
    pub(super) fn pose_commit(&mut self, var: u8, value: i64) {
        if let Some(name) = self.pose_vars().get(var as usize).cloned() {
            let raw = if name == "_PLgearPos" {
                value.clamp(0, 100) * GEAR_TRAVEL / 100 - GEAR_TRAVEL
            } else {
                value
            };
            self.ed.pose.insert(name, raw as i32);
            self.pose_changed();
        }
    }
    pub(super) fn pose_clear(&mut self, var: u8) {
        if let Some(name) = self.pose_vars().get(var as usize).cloned() {
            self.ed.pose.remove(&name);
            self.pose_changed();
        }
    }
    /// Selecting a part selects its geometry for Edit Mesh and highlights it.
    pub(super) fn pick_part(&mut self, i: usize) {
        self.ed.part_selected = Some(i);
        self.ed.part_menu = None;
        let Some(model) = &self.model else {
            return;
        };
        let target = match self.ed.parts.get(i) {
            Some(p) => p.id.target,
            None => match self.static_parts().get(i - self.ed.parts.len()) {
                Some(j) => model.parts[*j].offset,
                None => return,
            },
        };
        let group = model.groups.iter().position(|g| g.offset == target);
        let faces: Vec<usize> = match group {
            Some(g) => model
                .faces
                .iter()
                .filter(|f| {
                    let mut x = f.group;
                    for _ in 0..64 {
                        match x {
                            Some(y) if y == g => return true,
                            Some(y) => x = model.groups[y].parent,
                            None => return false,
                        }
                    }
                    false
                })
                .map(|f| f.offset)
                .collect(),
            None => Vec::new(),
        };
        self.ed.face_select = true;
        self.ed.mesh_faces = faces;
        self.sync_face_vertices();
        let name = self.part_title(i);
        self.status = if self.ed.mesh_faces.is_empty() {
            format!("{name} is not drawn in this pose; set its state in Pose preview")
        } else {
            format!(
                "{name}: {} selected for Edit Mesh",
                view::count(self.ed.mesh_faces.len(), "face", "faces")
            )
        };
    }
    /// Click in the viewport in Parts mode picks the part drawing that face.
    pub(super) fn parts_click(&mut self, x: i32, y: i32) {
        let Some(model) = &self.model else {
            return;
        };
        let Some(f) = self.pick_face(x, y) else {
            return;
        };
        let mut g = model.faces[f].group;
        for _ in 0..64 {
            let Some(group) = g.and_then(|i| model.groups.get(i)) else {
                break;
            };
            if let Some(i) = self
                .ed
                .parts
                .iter()
                .position(|p| p.id.target == group.offset)
            {
                self.pick_part(i);
                return;
            }
            if let Some(k) = self
                .static_parts()
                .iter()
                .position(|j| model.parts[*j].offset == group.offset)
            {
                self.pick_part(self.ed.parts.len() + k);
                return;
            }
            g = group.parent;
        }
        self.status = "That face belongs to the body, not a moving part".into();
    }
    fn part_title(&self, i: usize) -> String {
        match self.ed.parts.get(i) {
            Some(p) => p.name.clone(),
            None => format!("Static part {}", i - self.ed.parts.len() + 1),
        }
    }
    /// Model-space pivot of the selected part, for the viewport marker.
    pub(super) fn part_origin(&self) -> Option<[i32; 3]> {
        let model = self.model.as_ref()?;
        let i = self.ed.part_selected?;
        let target = match self.ed.parts.get(i) {
            Some(p) => p.id.target,
            None => model.parts[*self.static_parts().get(i - self.ed.parts.len())?].offset,
        };
        let mut g = Some(model.groups.iter().position(|g| g.offset == target)?);
        let mut origin = [0; 3];
        for _ in 0..64 {
            let Some(group) = g.and_then(|x| model.groups.get(x)) else {
                break;
            };
            if matches!(group.opcode, 0xc4 | 0xc6) {
                if let Some(p) = model.parts.iter().find(|p| p.offset == group.offset) {
                    for (o, v) in origin.iter_mut().zip(p.posed_position) {
                        *o += v;
                    }
                }
            }
            g = group.parent;
        }
        Some(origin)
    }
    /// Pivot NumberField of the selected part (model order right, forward, up).
    pub(super) fn pivot_spec(&self, axis: u8) -> Option<NumberSpec> {
        let i = self.ed.part_selected?;
        let (value, disk) = match self.ed.parts.get(i) {
            Some(p) => {
                let now = p.pivot?[axis as usize];
                let saved = self
                    .saved_pivot(p.id.target)
                    .map(|v| v[axis as usize] as i64);
                (now, saved)
            }
            None => {
                let model = self.model.as_ref()?;
                let j = *self.static_parts().get(i - self.ed.parts.len())?;
                (model.parts[j].position[axis as usize], None)
            }
        };
        Some(NumberSpec {
            value: value as i64,
            disk,
            step: 1,
            min: -32768,
            max: 32767,
            decimals: 0,
            bounded: false,
            group: true,
        })
    }
    /// The pivot of a part in the saved copy of the shape, when it differs.
    fn saved_pivot(&self, target: usize) -> Option<[i32; 3]> {
        let i = self.model_entry?;
        let entry = &self.doc.archive.entries[i];
        let saved = self.doc.saved_entry(&entry.name)?;
        if saved.same_storage(entry) {
            return self.ed.parts.iter().find(|p| p.id.target == target)?.pivot;
        }
        let list = shape_parts::parts(&saved.read().ok()?).ok()?;
        list.into_iter().find(|p| p.id.target == target)?.pivot
    }
    /// Write one pivot component of the selected part as one undo step.
    pub(super) fn pivot_commit(&mut self, axis: u8, value: i64) -> Result<()> {
        let i = self.ed.part_selected.ok_or("Select a part")?;
        let entry = self
            .model_entry
            .ok_or("Open the shape owner's LIB to change part settings")?;
        let source = self.doc.archive.entries[entry].read()?;
        let bytes = match self.ed.parts.get(i) {
            Some(p) => {
                let mut pivot = p.pivot.ok_or("This part has no pivot")?;
                pivot[axis as usize] = value as i32;
                shape_parts::apply_part_setting(&source, p.id, &Setting::Pivot(pivot))?
            }
            None => {
                let model = self.model.as_ref().ok_or("No model")?;
                let j = *self
                    .static_parts()
                    .get(i - self.ed.parts.len())
                    .ok_or("No part")?;
                // Inserted one by one (`state_from_pose` collects, which sorts
                // on a 4 KiB stack buffer the CRT-free x86_64 build cannot probe).
                let mut state = BTreeMap::new();
                for (address, name) in &model.state_names {
                    if let Some(v) = self.ed.pose.get(name) {
                        state.insert(*address, *v);
                    }
                }
                animation::place_part(
                    &source,
                    &state,
                    model.parts[j].offset,
                    axis as usize,
                    value as i32,
                )?
            }
        };
        if bytes != source {
            self.doc.replace(entry, bytes)?;
            self.refresh();
        }
        self.status = format!(
            "{} pivot {} = {value}. One undo step.",
            self.part_title(i),
            ["X", "Y", "Z"][axis as usize % 3]
        );
        Ok(())
    }
    /// Open the options Select of control `k`, anchored under its field.
    pub(super) fn open_part_menu(&mut self, k: usize) {
        let rect = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::PartMenu(n) if n == k))
            .map(|h| h.rect);
        if let Some(rect) = rect {
            self.ed.part_menu = Some((k, rect));
            self.menu = Some(chrome::MENU_PART);
        }
    }
    /// Labels and actions of the open part-control Select.
    pub(super) fn part_menu_items(&self) -> Vec<(String, Action, bool, bool)> {
        let Some((k, _)) = self.ed.part_menu else {
            return Vec::new();
        };
        let Some(p) = self.ed.part_selected.and_then(|i| self.ed.parts.get(i)) else {
            return Vec::new();
        };
        let Some(c) = p.controls.get(k) else {
            return Vec::new();
        };
        let gear = p.role == Role::Gear;
        let now = current(c, gear).0;
        options(c, gear)
            .into_iter()
            .map(|(v, label, census)| (label, Action::PartValue(k, v), v == now, !census))
            .collect()
    }
    /// Apply option `value` to control `k` of the selected part: one undo step.
    pub(super) fn part_value(&mut self, k: usize, value: i32) {
        let result = (|| -> Result<()> {
            let i = self.ed.part_selected.ok_or("Select a part")?;
            let p = self
                .ed
                .parts
                .get(i)
                .ok_or("Select a stub-driven part")?
                .clone();
            let c = p.controls.get(k).ok_or("No such setting")?;
            let setting = match c.setting {
                Setting::Compare { index, .. } => Setting::Compare { index, value },
                Setting::Branch { index, .. } => Setting::Branch {
                    index,
                    equal: value != 0,
                },
                Setting::Shift { index, .. } => Setting::Shift {
                    index,
                    amount: value.clamp(0, 31) as u8,
                },
                Setting::Direction { index, .. } => Setting::Direction {
                    index,
                    negated: value != 0,
                },
                Setting::Axis { index, .. } => Setting::Axis {
                    index,
                    axis: *Axis::ALL.get(value as usize).ok_or("No such axis")?,
                },
                Setting::Pivot(_) => return Err("Edit the pivot in its fields".into()),
            };
            let entry = self
                .model_entry
                .ok_or("Open the shape owner's LIB to change part settings")?;
            let source = self.doc.archive.entries[entry].read()?;
            let bytes = shape_parts::apply_part_setting(&source, p.id, &setting)?;
            if bytes == source {
                return Ok(());
            }
            self.doc.replace(entry, bytes)?;
            self.refresh();
            let now = self
                .ed
                .parts
                .iter()
                .find(|x| x.id == p.id)
                .and_then(|x| x.controls.get(k));
            let gear = p.role == Role::Gear;
            self.status = format!(
                "{}: {} set to {}. One undo step.{}",
                p.name,
                c.label,
                now.map_or(String::new(), |c| current(c, gear).1),
                if now.is_some_and(|c| !c.retail) {
                    " Not seen in retail; verify in the game."
                } else {
                    ""
                }
            );
            Ok(())
        })();
        self.ed.part_menu = None;
        self.result(result);
    }
    /// One wrapped hint line group in `ink-muted` (reasons under fixed controls).
    fn stack_hint(&self, o: &mut Layout, s: &mut widgets::Stack, text: &str) {
        let lines = widgets::wrap(text, s.w - 3 * space::SPACE_2, Style::Hint);
        for line in lines {
            if let Some([x, y, _, _]) = o.wide(s, 14) {
                o.canvas
                    .styled(x + 2, y + 11, &line, c::INK_MUTED, Style::Hint);
            }
        }
    }
    /// The Parts inspector: list, pose preview, settings.
    pub(super) fn animation_inspector(&self, o: &mut Layout) {
        self.inspector_header(o, None);
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0);
        if self.model.is_none() {
            o.stack_end(s);
            return;
        }
        let statics = self.static_parts();
        if self.pane(o, &mut s, pane::PARTS, "Parts", Icon::Sliders) {
            if self.ed.parts.is_empty() && statics.is_empty() {
                let note = if self.ed.parts_note.is_empty() {
                    "No stub-driven parts: this shape has no gear, flap or other moving-part code."
                        .to_string()
                } else {
                    format!("Parts are unavailable: {}", self.ed.parts_note)
                };
                o.stack_notice(&mut s, Tone::Neutral, &note);
            }
            for (title, roles) in GROUPS {
                let members: Vec<usize> = (0..self.ed.parts.len())
                    .filter(|i| roles.contains(&self.ed.parts[*i].role))
                    .collect();
                if members.is_empty() {
                    continue;
                }
                o.stack_subhead(&mut s, title);
                for i in members {
                    let p = &self.ed.parts[i];
                    self.part_row(o, &mut s, i, role_icon(p.role), &p.name, p);
                }
            }
            if !statics.is_empty() {
                o.stack_subhead(&mut s, "Static parts");
                for k in 0..statics.len() {
                    let i = self.ed.parts.len() + k;
                    if let Some(rect) = o.wide(&mut s, m::ROW_H) {
                        self.list_row(o, rect, i, Icon::Shape, &self.part_title(i), &[]);
                    }
                }
            }
        }
        o.panel_end(&mut s);
        if let Some(i) = self.ed.part_selected {
            let title = self.part_title(i);
            if self.pane(o, &mut s, pane::PART_SETTINGS, &title, Icon::Sliders) {
                self.settings_rows(o, &mut s, i);
            }
            o.panel_end(&mut s);
        }
        let vars = self.pose_vars();
        if self.pane(o, &mut s, pane::POSE, "Pose preview", Icon::Play) {
            for row in PRESETS.chunks(2) {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    let half = (rect[2] - space::SPACE_1) / 2;
                    for (k, (label, _)) in row.iter().enumerate() {
                        let n = PRESETS.iter().position(|(l, _)| l == label).unwrap_or(0);
                        o.button_ex(
                            [
                                rect[0] + k as i32 * (half + space::SPACE_1),
                                rect[1],
                                half,
                                rect[3],
                            ],
                            Btn::new(label),
                            Action::PosePreset(n as u8),
                        );
                    }
                }
            }
            for (v, name) in vars.iter().enumerate() {
                let label = pose_label(name);
                let Some(rect) = o.prop(&mut s, label) else {
                    continue;
                };
                if CONTINUOUS.contains(&name.as_str()) {
                    if let Some(spec) = self.pose_spec(v as u8) {
                        o.number(
                            rect,
                            &Number {
                                target: NumberTarget::Pose(v as u8),
                                spec,
                                label: "",
                                unit: if name == "_PLgearPos" { "% down" } else { "" },
                                locked: false,
                                axis: None,
                            },
                        );
                    }
                    continue;
                }
                let now = self.ed.pose.get(name).copied();
                let values = self.toggle_values(name);
                let labels: Vec<String> = values
                    .iter()
                    .map(|x| match (name.as_str(), *x) {
                        ("_PLgearDown", 0) => "Up".into(),
                        ("_PLgearDown", 1) => "Down".into(),
                        (_, 0) if values.len() == 2 && values[1] == 1 => "Off".into(),
                        (_, 1) if values.len() == 2 && values[0] == 0 => "On".into(),
                        _ => x.to_string(),
                    })
                    .collect();
                let items: Vec<(Btn, Action)> = values
                    .iter()
                    .zip(&labels)
                    .map(|(x, l)| {
                        (
                            Btn::new(l).on(now == Some(*x)),
                            Action::PoseSet(v as u8, *x),
                        )
                    })
                    .collect();
                o.segmented(rect, &items);
            }
            if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                o.button_ex(
                    rect,
                    Btn::new("Reset pose")
                        .ghost()
                        .enabled(!self.ed.pose.is_empty()),
                    Action::PoseReset,
                );
            }
            self.stack_hint(
                o,
                &mut s,
                "Preview only: the pose is never saved and never in undo.",
            );
        }
        o.panel_end(&mut s);
        o.stack_end(s);
    }
    fn part_row(
        &self,
        o: &mut Layout,
        s: &mut widgets::Stack,
        i: usize,
        icon: Icon,
        name: &str,
        p: &PartInfo,
    ) {
        let mut badges: Vec<(&str, Tone)> = Vec::new();
        if p.locked.is_some() {
            badges.push(("LOCKED", Tone::Neutral));
        } else if !p.retail() {
            badges.push(("NOT RETAIL", Tone::Warn));
        }
        if let Some(rect) = o.wide(s, m::ROW_H) {
            self.list_row(o, rect, i, icon, name, &badges);
        }
    }
    /// A selectable list row: icon, name, right-aligned badges.
    fn list_row(
        &self,
        o: &mut Layout,
        rect: [i32; 4],
        i: usize,
        icon: Icon,
        name: &str,
        badges: &[(&str, Tone)],
    ) {
        let [x, y, w, h] = rect;
        let selected = self.ed.part_selected == Some(i);
        let ground = if selected {
            c::AMBER_DEEP
        } else if o.over(rect) {
            c::GM_700
        } else {
            c::GM_800
        };
        if ground != c::GM_800 {
            widgets::notched(&mut o.canvas, rect, Some(ground), None);
        }
        let ink = if selected { c::AMBER } else { c::INK_MUTED };
        o.canvas
            .icon(x + 2, y + (h - m::ICON) / 2, icon, ink, ground);
        let mut right = x + w - 2;
        for (text, tone) in badges.iter().rev() {
            right -= widgets::badge_width(text);
            type_badge(&mut o.canvas, right, y + (h - m::BADGE_H) / 2, text, *tone);
            right -= space::SPACE_1;
        }
        let tx = x + 2 + m::ICON + space::SPACE_1;
        o.canvas.styled(
            tx,
            baseline(y, h, Style::Label),
            &fit(name, right - space::SPACE_1 - tx, Style::Label),
            if selected { c::AMBER } else { c::INK },
            Style::Label,
        );
        o.hit(rect, Action::PartPick(i));
    }
    fn settings_rows(&self, o: &mut Layout, s: &mut widgets::Stack, i: usize) {
        let Some(p) = self.ed.parts.get(i) else {
            // A C4 part no stub drives: its pivot only.
            self.pivot_rows(o, s);
            self.stack_hint(
                o,
                s,
                "No stub drives this part; its placement moves the C4 translation in place.",
            );
            return;
        };
        o.info(s, "Driven by", &p.variables.join(", "), "");
        let when: Vec<String> = p
            .when
            .first()
            .into_iter()
            .flatten()
            .map(|c| {
                format!(
                    "{} {} {}",
                    c.variable.trim_start_matches("_PL"),
                    rel_symbol(c.rel),
                    c.value
                )
            })
            .collect();
        if !when.is_empty() {
            o.info(s, "Drawn when", &when.join(" & "), "");
        }
        if let Some(why) = &p.locked {
            o.stack_notice(s, Tone::Warn, why);
            return;
        }
        o.stack_notice(
            s,
            Tone::Neutral,
            "Settings change existing parameters of this part's native code in place, at the same size. In-game behaviour of edited settings is not verified yet.",
        );
        let gear = p.role == Role::Gear;
        for (k, c) in p.controls.iter().enumerate() {
            let label = match (&c.setting, gear) {
                (Setting::Compare { .. }, _) => "Gate value",
                (Setting::Branch { .. }, _) => "Gate test",
                (Setting::Shift { .. }, true) => "Swing range",
                (Setting::Shift { .. }, false) => "Shift",
                (Setting::Direction { .. }, _) => "Direction",
                (Setting::Axis { .. }, _) => "Rotation axis",
                (Setting::Pivot(_), _) => {
                    o.stack_subhead(s, "Pivot");
                    self.pivot_rows(o, s);
                    if let Allowed::Fixed(why) = &c.allowed {
                        self.stack_hint(o, s, why);
                    }
                    continue;
                }
            };
            let (_, now) = current(c, gear);
            match (&c.allowed, &c.setting) {
                (Allowed::Fixed(why), _) => {
                    o.info(s, label, &now, "");
                    self.stack_hint(o, s, why);
                }
                (Allowed::Values(v), Setting::Compare { value, .. }) => {
                    if let Some(rect) = o.prop(s, label) {
                        let labels: Vec<String> = v.iter().map(|x| x.to_string()).collect();
                        let items: Vec<(Btn, Action)> = v
                            .iter()
                            .zip(&labels)
                            .map(|(x, l)| (Btn::new(l).on(x == value), Action::PartValue(k, *x)))
                            .collect();
                        o.segmented(rect, &items);
                    }
                }
                (Allowed::Either, Setting::Branch { equal, .. }) => {
                    if let Some(rect) = o.prop(s, label) {
                        o.segmented(
                            rect,
                            &[
                                (Btn::new("Equal").on(*equal), Action::PartValue(k, 1)),
                                (Btn::new("Not equal").on(!*equal), Action::PartValue(k, 0)),
                            ],
                        );
                    }
                }
                _ => {
                    if let Some(rect) = o.prop(s, label) {
                        o.select(
                            rect,
                            None,
                            &now,
                            Action::PartMenu(k),
                            self.menu == Some(chrome::MENU_PART)
                                && self.ed.part_menu.is_some_and(|(n, _)| n == k),
                        );
                    }
                }
            }
            if !c.retail {
                if let Some([x, y, _, h]) = o.wide(s, m::BADGE_H + 4) {
                    let bx = x + (s.w * m::PROP_LABEL_PCT / 100) + space::SPACE_1;
                    type_badge(
                        &mut o.canvas,
                        bx,
                        y + (h - m::BADGE_H) / 2,
                        "Not seen in retail",
                        Tone::Warn,
                    );
                }
            }
        }
        if gear {
            self.stack_hint(o, s, "Degrees assume gearPos runs from 0 down to -8192 up.");
            self.stack_hint(o, s, "NEG and No NEG fold the part in opposite directions.");
        }
    }
    /// Joined X/Y/Z pivot NumberFields of the selected part.
    fn pivot_rows(&self, o: &mut Layout, s: &mut widgets::Stack) {
        for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
            if s.collapsed() {
                break;
            }
            let Some(spec) = self.pivot_spec(k as u8) else {
                break;
            };
            let locked = self
                .ed
                .part_selected
                .and_then(|i| self.ed.parts.get(i))
                .is_some_and(|p| {
                    p.controls.iter().any(|c| {
                        matches!(c.setting, Setting::Pivot(_))
                            && matches!(c.allowed, Allowed::Fixed(_))
                    })
                });
            if let Some(rect) = s.take(m::FIELD_H) {
                o.number(
                    rect,
                    &Number {
                        target: NumberTarget::Pivot(k as u8),
                        spec,
                        label: axis,
                        unit: "",
                        locked,
                        axis: Some(k),
                    },
                );
            }
        }
        s.gap(space::SPACE_1);
    }
}

#[inline(never)]
fn boxed_app() -> Box<App> {
    Box::new(App::new())
}
impl App {
    #[inline(never)]
    pub(super) fn smoke_advanced_tools(&mut self) {
        let mut a = boxed_app();
        a.demo();
        let nt = hangar_core::brf::demo_npc();
        a.doc
            .transaction(vec![Entry::new("DEMO.NT", nt).unwrap()], &[])
            .unwrap();
        a.select_entry(a.doc.archive.find("DEMO.NT").unwrap());
        assert_eq!(a.hp_context.as_ref().unwrap().stations.len(), 2);
        a.act(Action::Hardpoints);
        let find = |a: &App, predicate: &dyn Fn(Action) -> bool| {
            a.chrome_hit(predicate).expect("Station control missing")
        };
        let press = |a: &mut App, r: [i32; 4], dx: i32| a.smoke_drag(r, dx);
        let slew = find(&a, &|x| matches!(x, Action::StationSlew(true)));
        press(&mut a, slew, 0);
        assert!(a.hp_slew);
        // The heading limit is a NumberField: a click types the exact value.
        let limit = find(&a, &|x| {
            matches!(x, Action::Number(NumberTarget::Station(6)))
        });
        press(&mut a, limit, 0);
        assert!(a.prompt.is_some());
        a.key(Key::Char('a'), true, false);
        for ch in "15000".chars() {
            a.key(Key::Char(ch), false, false);
        }
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        assert_eq!(
            a.hp_context
                .as_ref()
                .unwrap()
                .brf
                .fields
                .iter()
                .find(|f| f.label == "hardpoint[0].slewLimitH")
                .unwrap()
                .value,
            "15000"
        );
        a.act(Action::Undo);
        // The X location is an axis-coloured vector NumberField: scrub, then
        // Backspace returns the operand on disk; undo restores each step.
        a.doc.mark_saved();
        a.refresh();
        let before = a.doc.archive.bytes().unwrap();
        let t = NumberTarget::Station(1);
        let x0 = a.number_spec(t).unwrap().value;
        let field = find(&a, &|x| matches!(x, Action::Number(n) if n == t));
        assert!(a.layout().canvas.commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, color, _) if s == "X" && *color == c::AXIS_X.0)));
        press(&mut a, field, 12);
        assert_eq!(a.number_spec(t).unwrap().value, x0 + 6);
        assert_eq!(
            a.hp_context.as_ref().unwrap().stations[0].position[0] as i64,
            x0 + 6
        );
        a.motion(field[0] + field[2] / 2, field[1] + field[3] / 2, false);
        a.key(Key::Backspace, false, false);
        assert_eq!(a.number_spec(t).unwrap().value, x0);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
        a.act(Action::Undo);
        assert_eq!(a.number_spec(t).unwrap().value, x0 + 6);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
        let next = find(&a, &|x| matches!(x, Action::HardpointStep(1)));
        press(&mut a, next, 0);
        assert_eq!(a.hp_selected, 1);
        // A static C4 part (no stub): its pivot moves through place_part.
        let mut shape = model::demo_shape();
        let mut code = vec![0xc4, 0, 10, 0, 30, 0, 20, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0];
        code.extend(&shape[256..]);
        shape.truncate(256);
        shape.extend(&code);
        for at in [128, 136] {
            shape[at..at + 4].copy_from_slice(&(code.len() as u32).to_le_bytes());
        }
        a.doc.replace(0, shape).unwrap();
        a.doc.mark_saved();
        a.select_entry(0);
        let original = a.doc.archive.bytes().unwrap();
        a.open_animation();
        assert!(a.animation_tool);
        assert!(!a.doc.dirty());
        assert_eq!(a.model.as_ref().unwrap().parts[0].position, [10, 20, 30]);
        let row = find(&a, &|x| matches!(x, Action::PartPick(0)));
        press(&mut a, row, 0);
        assert_eq!(a.ed.part_selected, Some(0));
        a.pivot_commit(1, 40).unwrap();
        assert_eq!(a.model.as_ref().unwrap().parts[0].position, [10, 40, 30]);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert_eq!(a.model.as_ref().unwrap().parts[0].position, [10, 20, 30]);
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Advanced tool bounds"
                );
            }
        }
        a.pivot_commit(0, 12).unwrap();
        a.close_library().unwrap();
        assert!(a.prompt.is_some());
        a.act(Action::Cancel);
        assert!(a.doc.dirty());
        a.close_library().unwrap();
        let hit = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::DiscardChanges))
            .unwrap();
        a.click(hit.rect[0] + 5, hit.rect[1] + 5, 1, true);
        assert!(a.doc.archive.entries.is_empty());
        assert!(!a.quit);
        a.demo();
        a.doc.replace(0, model::demo_shape()).unwrap();
        a.close();
        a.act(Action::Cancel);
        assert!(!a.quit);
        a.close();
        a.act(Action::DiscardChanges);
        assert!(a.quit);
    }
}
impl App {
    /// The Parts panel through its hit regions: list, pose presets, toggles
    /// and the gear NumberField (never an edit), then settings as one undo
    /// step each, including a non-retail direction flip.
    #[inline(never)]
    pub(super) fn smoke_parts_panel(&mut self) {
        let mut a = super::edit_ui::parts_app();
        let original = a.doc.archive.bytes().unwrap();
        let hit = |a: &mut App, p: &dyn Fn(Action) -> bool| a.smoke_find(p);
        let select = hit(&mut a, &|x| matches!(x, Action::Menu(chrome::MENU_MODE)));
        a.chrome_click(select);
        let item = hit(&mut a, &|x| matches!(x, Action::ViewportMode(3)));
        a.chrome_click(item);
        assert!(a.animation_tool);
        let names: Vec<&str> = a.ed.parts.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"Gear left") && names.contains(&"Hook (state 1)"));
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, _, _) if s == "CONTROL SURFACES")));
        // Pose: presets, toggles and the gear field change the preview only.
        let drawn = |a: &App| -> Vec<usize> {
            a.model
                .as_ref()
                .unwrap()
                .faces
                .iter()
                .map(|f| f.offset)
                .collect()
        };
        let neutral = drawn(&a);
        let flaps = hit(&mut a, &|x| matches!(x, Action::PosePreset(2)));
        a.chrome_click(flaps);
        assert_eq!(a.ed.pose.get("_PLleftFlap"), Some(&-1));
        assert_ne!(
            drawn(&a),
            neutral,
            "Flaps down draws the lowered flap meshes"
        );
        let vars = a.pose_vars();
        let hook = vars.iter().position(|v| v == "_PLhook").unwrap() as u8;
        let on = hit(&mut a, &|x| matches!(x, Action::PoseSet(v, 1) if v == hook));
        a.chrome_click(on);
        assert_eq!(a.ed.pose.get("_PLhook"), Some(&1));
        let gear = vars.iter().position(|v| v == "_PLgearPos").unwrap() as u8;
        let field = hit(
            &mut a,
            &|x| matches!(x, Action::Number(NumberTarget::Pose(v)) if v == gear),
        );
        a.smoke_drag(field, -4);
        assert_eq!(a.ed.pose.get("_PLgearPos"), Some(&(80 * 8192 / 100 - 8192)));
        assert!(!a.doc.dirty(), "The pose is never an edit");
        let reset = hit(&mut a, &|x| matches!(x, Action::PoseReset));
        a.chrome_click(reset);
        assert!(a.ed.pose.is_empty());
        let down = hit(&mut a, &|x| matches!(x, Action::PosePreset(0)));
        a.chrome_click(down);
        assert_eq!(a.ed.pose.get("_PLgearDown"), Some(&1));
        // Pick the left gear: its geometry is selected for Edit Mesh.
        let left =
            a.ed.parts
                .iter()
                .position(|p| p.name == "Gear left")
                .unwrap();
        let row = hit(&mut a, &|x| matches!(x, Action::PartPick(i) if i == left));
        a.chrome_click(row);
        assert_eq!(a.ed.part_selected, Some(left));
        assert!(!a.ed.mesh_faces.is_empty() && a.ed.face_select);
        assert!(a.part_origin().is_some());
        // Direction: Select, then the non-retail option. One undo step.
        let k = a.ed.parts[left]
            .controls
            .iter()
            .position(|c| matches!(c.setting, shape_parts::Setting::Direction { .. }))
            .unwrap();
        let field = hit(&mut a, &|x| matches!(x, Action::PartMenu(n) if n == k));
        a.chrome_click(field);
        assert_eq!(a.menu, Some(chrome::MENU_PART));
        let [mx, my, mw, mh] = a.open_menu_rect().unwrap();
        assert!(mx >= 0 && my >= 0 && mx + mw <= a.width && my + mh <= a.height);
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, _, _) if s == "Not seen in retail")));
        let plain = hit(&mut a, &|x| matches!(x, Action::PartValue(n, 0) if n == k));
        a.chrome_click(plain);
        assert!(a.menu.is_none());
        assert_eq!(a.doc.changed_count(), 1, "{}", a.status);
        assert!(a.status.contains("Not seen in retail"), "{}", a.status);
        assert!(!a.ed.parts[left].retail());
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, _, _) if s == "Not seen in retail")));
        assert_eq!(
            a.ed.pose.get("_PLgearDown"),
            Some(&1),
            "The pose survives edits"
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert!(a.ed.parts[left].retail());
        // Pivot X: scrub, Backspace back to the file, undo.
        let pivot = hit(&mut a, &|x| {
            matches!(x, Action::Number(NumberTarget::Pivot(0)))
        });
        let x0 = a.ed.parts[left].pivot.unwrap()[0];
        a.smoke_drag(pivot, 8);
        assert_eq!(a.ed.parts[left].pivot.unwrap()[0], x0 + 4);
        a.motion(pivot[0] + pivot[2] / 2, pivot[1] + pivot[3] / 2, false);
        a.key(Key::Backspace, false, false);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // A gate value on the hook: segmented, one step.
        let hook =
            a.ed.parts
                .iter()
                .position(|p| p.role == shape_parts::Role::Hook)
                .unwrap();
        a.pick_part(hook);
        let gate = hit(&mut a, &|x| matches!(x, Action::PartValue(0, 0)));
        a.chrome_click(gate);
        assert!(
            a.ed.parts.iter().any(|p| p.name == "Hook (state 0)"),
            "{}",
            a.status
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Locked controls show their reason.
        let right =
            a.ed.parts
                .iter()
                .position(|p| p.name == "Gear right")
                .unwrap();
        a.pick_part(right);
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, _, _) if s.starts_with("Reversing needs"))));
        // A face click in the viewport picks its part.
        let (f, p) = a.smoke_pickable(false);
        let g = a.model.as_ref().unwrap().faces[f].group.unwrap();
        let target = a.model.as_ref().unwrap().groups[g].offset;
        a.smoke_press(p[0], p[1], false);
        assert_eq!(
            a.ed.part_selected.map(|i| a.ed.parts[i].id.target),
            Some(target),
            "{}",
            a.status
        );
        assert!(!a.doc.dirty());
    }
}
