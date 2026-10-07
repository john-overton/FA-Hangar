//! Per-face textures: clone the selected faces' PIC for those faces only,
//! assign another PIC with Keep, Scale or Project UVs, and return faces to
//! the shape's texture. Each is one undo step through `shape_texture`.
use super::view::{Action, Icon, Layout};
use super::widgets::{pane, Btn, Tone};
use super::*;
use hangar_core::shape_remap as remap;
use hangar_core::shape_texture::{self as tex, Plane, UvMode};
use theme::{metric as m, space};

pub(super) const TEX_CLONE: u8 = 0;
pub(super) const TEX_ASSIGN: u8 = 1;
pub(super) const TEX_RESTORE: u8 = 2;
pub(super) const TEX_REMAP: u8 = 3;
/// Remap dialog fills.
const FILLS: [&str; 2] = ["Bake current look", "Blank"];
/// Why the per-face actions are disabled without a selection.
pub(super) const NO_FACES: &str = "Select faces in Edit Mesh or pick a face to paint";
/// List rows shown in the Assign texture dialog.
const ROWS: usize = 8;
const MODES: [&str; 3] = ["Keep", "Scale", "Project"];
const PLANES: [(&str, Plane); 4] = [
    ("Auto", Plane::Auto),
    ("Top", Plane::Top),
    ("Side", Plane::Side),
    ("Front", Plane::Front),
];
/// The Assign texture dialog.
#[derive(Default)]
pub(super) struct AssignDraft {
    /// Shape entry and the faces (file offsets) the dialog acts on.
    pub entry: usize,
    pub faces: Vec<usize>,
    /// Every PIC in the LIB with its size, read once when the dialog opens.
    pub pics: Vec<(String, [u32; 2])>,
    pub picked: Option<usize>,
    /// 0 Keep, 1 Scale, 2 Project.
    pub mode: u8,
    pub plane: u8,
    /// Size of the faces' current PIC, when they share one.
    pub from: Option<[u32; 2]>,
    /// Whether every face carries UVs (Keep and Scale need them).
    pub textured: bool,
}
/// The Remap from view dialog: the faces, the camera they were seen with
/// and the planned sheet.
#[derive(Default)]
pub(super) struct RemapDraft {
    pub entry: usize,
    pub faces: Vec<usize>,
    pub yaw: i32,
    pub pitch: i32,
    pub pose: model::Pose,
    /// Requested density (Q16 texels per unit) and the planned sheet.
    pub density: u32,
    pub size: [u32; 2],
    /// Texels the panels span inside the sheet.
    pub span: [u32; 2],
    pub used: u32,
    /// 0 Bake current look, 1 Blank.
    pub fill: u8,
    /// Where the faces draw from now, for the status.
    pub from: String,
}
/// Hangar assignment faces of one shape entry, cached by storage.
#[derive(Default)]
pub(super) struct AssignedCache(core::cell::RefCell<Option<(Entry, Vec<usize>)>>);
pub(super) fn full(name: &str) -> String {
    if name.contains('.') {
        name.to_ascii_uppercase()
    } else {
        format!("{}.PIC", name.to_ascii_uppercase())
    }
}
impl App {
    /// File offsets of the faces the texture actions act on: the Edit Mesh
    /// selection, else the selected panels, else the face picked for painting.
    pub(super) fn texture_faces(&self) -> Vec<usize> {
        let Some(model) = self.model_for_paint() else {
            return Vec::new();
        };
        if self.mesh_edit && self.mode == Mode::Model {
            return self
                .selected_faces()
                .iter()
                .filter_map(|f| model.faces.get(*f).map(|f| f.offset))
                .collect();
        }
        let panels = self.panel_offsets();
        if !panels.is_empty() {
            return panels;
        }
        self.selected_face
            .and_then(|i| model.faces.get(i))
            .map(|f| vec![f.offset])
            .unwrap_or_default()
    }
    /// The selected panels outside Edit Mesh: the shared face selection
    /// (`EditState::mesh_faces`), as offsets the shown model draws.
    pub(super) fn panel_offsets(&self) -> Vec<usize> {
        let Some(model) = self.model_for_paint() else {
            return Vec::new();
        };
        self.ed
            .mesh_faces
            .iter()
            .filter(|o| model.faces.iter().any(|f| f.offset == **o))
            .copied()
            .collect()
    }
    /// Model face indices of the selected panels, for drawing.
    pub(super) fn panel_indices(&self) -> Vec<usize> {
        let Some(model) = self.model_for_paint() else {
            return Vec::new();
        };
        let mut picked = self.ed.mesh_faces.clone();
        picked.sort_unstable();
        (0..model.faces.len())
            .filter(|f| picked.binary_search(&model.faces[*f].offset).is_ok())
            .collect()
    }
    /// A panel click outside Edit Mesh: a face replaces the selection, with
    /// Shift it is added or removed; empty space clears (Shift keeps it).
    pub(super) fn panel_click(&mut self, face: Option<usize>, shift: bool) {
        let Some(offset) =
            face.and_then(|i| self.model_for_paint()?.faces.get(i).map(|f| f.offset))
        else {
            if !shift {
                self.clear_panels();
            }
            return;
        };
        if !shift {
            self.ed.mesh_faces = vec![offset];
            self.selected_face = face;
        } else if let Some(at) = self.ed.mesh_faces.iter().position(|o| *o == offset) {
            self.ed.mesh_faces.remove(at);
            let last = self.ed.mesh_faces.last().copied();
            self.selected_face = last.and_then(|o| {
                self.model_for_paint()?
                    .faces
                    .iter()
                    .position(|f| f.offset == o)
            });
        } else {
            self.ed.mesh_faces.push(offset);
            self.selected_face = face;
        }
        self.status = format!(
            "{} selected | Shift+click adds or removes, Esc clears",
            view::count(self.panel_offsets().len(), "panel", "panels")
        );
    }
    /// Esc or a click on empty space: no panel selected.
    pub(super) fn clear_panels(&mut self) {
        let had = !self.ed.mesh_faces.is_empty() || self.selected_face.is_some();
        self.ed.mesh_faces.clear();
        self.mesh_vertices.clear();
        self.selected_face = None;
        if had {
            self.status = "Selection cleared".into();
        }
    }
    /// Texture name (as `X.PIC`, empty when untextured) of each face offset
    /// in the shown model, with how many faces use it.
    pub(super) fn face_texture_names(&self, faces: &[usize]) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        let Some(model) = self.model_for_paint() else {
            return out;
        };
        for o in faces {
            let Some(f) = model.faces.iter().find(|f| f.offset == *o) else {
                continue;
            };
            let name = if f.sub & 4 != 0 && !f.texture.is_empty() {
                full(&f.texture)
            } else {
                String::new()
            };
            match out.iter_mut().find(|(n, _)| *n == name) {
                Some((_, k)) => *k += 1,
                None => out.push((name, 1)),
            }
        }
        out
    }
    /// The shape entry the texture actions write.
    fn texture_shape(&self) -> Result<usize> {
        self.model_entry
            .or(self.context_entry)
            .ok_or_else(|| "Select the SH entry or its owner to change face textures".into())
    }
    /// Offsets of faces drawn from Hangar texture assignments in the shown
    /// shape, recomputed only when its entry changes.
    pub(super) fn assigned_offsets(&self) -> Vec<usize> {
        let Some(entry) = self
            .model_entry
            .or(self.context_entry)
            .and_then(|i| self.doc.archive.entries.get(i))
        else {
            return Vec::new();
        };
        let mut cache = self.ed.assigned.0.borrow_mut();
        if let Some((e, list)) = cache.as_ref() {
            if e.name == entry.name && e.same_storage(entry) {
                return list.clone();
            }
        }
        let list: Vec<usize> = entry
            .read()
            .ok()
            .and_then(|b| tex::assigned_faces(&b).ok())
            .map(|m| m.into_keys().collect())
            .unwrap_or_default();
        *cache = Some((entry.clone(), list.clone()));
        list
    }
    fn texture_name_free(&self, name: &str) -> bool {
        self.doc.archive.find(name).is_none()
            && self.stroke.as_ref().is_none_or(|s| s.name != name)
            && !self.stroke_parked.iter().any(|s| s.name == name)
            && !self
                .libraries
                .iter()
                .any(|l| l.doc.archive.find(name).is_some())
            && !self
                .dependency_catalogs
                .iter()
                .any(|(_, names)| names.iter().any(|n| n == name))
    }
    /// A free private 8.3 name for a copy of `from`: its first six stem
    /// characters, `T` and one more character (never two hex digits, which
    /// generated panel sheets use).
    pub(super) fn private_texture_name(&self, from: &str) -> String {
        let stem: String = from
            .split('.')
            .next()
            .unwrap_or("TEX")
            .chars()
            .take(6)
            .collect();
        "123456789ABCDEFGHJKLMNPQRSTUVWXYZ"
            .chars()
            .map(|c| format!("{stem}T{c}.PIC"))
            .find(|n| self.texture_name_free(n))
            .unwrap_or_else(|| "LIVERY.PIC".into())
    }
    /// The one PIC every selected face draws from.
    fn shared_texture(&self, faces: &[usize]) -> Result<String> {
        let names = self.face_texture_names(faces);
        match names.as_slice() {
            [] => Err(NO_FACES.into()),
            [(n, _)] if n.is_empty() => Err(
                "The selected faces are untextured; use Assign texture with Project, or paint them"
                    .into(),
            ),
            [(n, _)] => Ok(n.clone()),
            _ => Err("The selected faces use several textures; clone one texture at a time".into()),
        }
    }
    /// Mesh menu, Edit Mesh inspector and Paint inspector actions.
    pub(super) fn face_texture_action(&mut self, op: u8) {
        let r = self.face_texture_inner(op);
        if let Err(e) = r {
            self.status = format!("Error: {e}");
            if self.mesh_edit {
                self.ed.mesh_refusal = Some(e);
            }
        }
    }
    fn face_texture_inner(&mut self, op: u8) -> Result<()> {
        self.finish_stroke();
        let faces = self.texture_faces();
        if faces.is_empty() {
            return Err(NO_FACES.into());
        }
        let entry = self.texture_shape()?;
        match op {
            TEX_CLONE => {
                let from = self.shared_texture(&faces)?;
                if self.doc.archive.find(&from).is_none() {
                    return Err(format!(
                        "{from} is outside the active LIB; copy or open its owner first"
                    ));
                }
                let suggestion = self.private_texture_name(&from);
                self.prompt = Some(Prompt {
                    kind: PromptKind::FaceClone,
                    title: format!(
                        "Clone {from} for {}: new 8.3 PIC name",
                        view::count(faces.len(), "selected face", "selected faces")
                    ),
                    value: suggestion,
                    axis: 0,
                });
            }
            TEX_ASSIGN => {
                let names = self.face_texture_names(&faces);
                let textured = names.iter().all(|(n, _)| !n.is_empty());
                let from = match names.as_slice() {
                    [(n, _)] if !n.is_empty() => self
                        .doc
                        .archive
                        .find(n)
                        .and_then(|i| self.doc.archive.entries[i].read().ok())
                        .and_then(|b| Pic::parse(&b).ok())
                        .map(|p| [p.width as u32, p.height as u32]),
                    _ => None,
                };
                let mut pics = Vec::new();
                for e in &self.doc.archive.entries {
                    if extension(&e.name) != "PIC" || pics.len() >= 4096 {
                        continue;
                    }
                    if let Some(p) = e.read().ok().and_then(|b| Pic::parse(&b).ok()) {
                        pics.push((e.name.clone(), [p.width as u32, p.height as u32]));
                    }
                }
                pics.sort_unstable();
                if pics.is_empty() {
                    return Err("This LIB has no PIC to assign; add or clone one first".into());
                }
                self.ed.assign = AssignDraft {
                    entry,
                    faces: faces.clone(),
                    pics,
                    picked: None,
                    mode: if textured { 0 } else { 2 },
                    plane: 0,
                    from,
                    textured,
                };
                self.prompt = Some(Prompt {
                    kind: PromptKind::AssignTexture,
                    title: format!(
                        "Assign texture to {}",
                        view::count(faces.len(), "face", "faces")
                    ),
                    value: String::new(),
                    axis: 0,
                });
            }
            TEX_REMAP => self.open_remap(entry, faces)?,
            _ => {
                let source = self.doc.archive.entries[entry].read()?;
                let restored = tex::restore_texture_assignment(&source, &faces)?;
                let names = self.face_texture_names(&faces);
                self.doc.replace(entry, restored.shape)?;
                self.after_texture_edit(&restored.faces);
                let now = self.face_texture_names(&restored.faces);
                self.status = format!(
                    "{} back on {} from {}. One undo step.",
                    view::count(faces.len(), "face", "faces"),
                    describe(&now),
                    describe(&names)
                );
            }
        }
        Ok(())
    }
    /// Reselect faces by their new offsets after a texture edit.
    fn after_texture_edit(&mut self, faces: &[usize]) {
        let panels = !self.ed.mesh_faces.is_empty();
        self.refresh();
        if self.mesh_edit && self.mode == Mode::Model {
            self.ed.face_select = true;
            self.ed.mesh_faces = faces.to_vec();
            self.sync_face_vertices();
        } else if let Some(model) = self.model_for_paint() {
            self.selected_face = faces
                .first()
                .and_then(|o| model.faces.iter().position(|f| f.offset == *o));
            // The selected panels follow their faces to the new offsets.
            if panels {
                self.ed.mesh_faces = faces.to_vec();
            }
        }
    }
    /// Clone the selected faces' PIC to `to` and draw those faces from it,
    /// with their UVs kept: the SH, the new PIC and a copied `.ORG` (when the
    /// source has one) in one undo step. Other faces keep the old PIC.
    pub(super) fn clone_face_texture(&mut self, to: &str) -> Result<()> {
        let to = to.trim().to_ascii_uppercase();
        let faces = self.texture_faces();
        if faces.is_empty() {
            return Err(NO_FACES.into());
        }
        let entry = self.texture_shape()?;
        let from = self.shared_texture(&faces)?;
        let texture = self
            .doc
            .archive
            .find(&from)
            .ok_or_else(|| format!("{from} is not in the active LIB"))?;
        if !to.ends_with(".PIC") || !self.texture_name_free(&to) {
            return Err("Choose an unused PIC name".into());
        }
        hangar_core::archive::validate_name(&to)?;
        let source = self.doc.archive.entries[entry].read()?;
        let assigned = tex::assign_texture(&source, &faces, &to, UvMode::Keep)?;
        // Stored bytes and compression are copied as-is, with any stored original.
        let mut entries = vec![
            Entry::new(
                &self.doc.archive.entries[entry].name.clone(),
                assigned.shape,
            )?,
            self.doc.archive.entries[texture].renamed(&to)?,
        ];
        entries.extend(hangar_core::originals::cloned(
            &self.doc.archive,
            &from,
            &to,
        )?);
        self.doc.transaction(entries, &[])?;
        self.after_texture_edit(&assigned.faces);
        self.status = format!(
            "Cloned {from} to {to}; {} now draw from {to}, every other face keeps {from}. One undo step.",
            view::count(faces.len(), "selected face", "selected faces")
        );
        Ok(())
    }
    /// Filtered rows of the Assign texture dialog: indices into `pics`.
    fn assign_rows(&self) -> Vec<usize> {
        let filter = self
            .prompt
            .as_ref()
            .map(|p| p.value.trim().to_ascii_uppercase())
            .unwrap_or_default();
        self.ed
            .assign
            .pics
            .iter()
            .enumerate()
            .filter(|(_, (n, _))| n.contains(&filter))
            .map(|(i, _)| i)
            .collect()
    }
    fn assign_size(&self) -> Option<[u32; 2]> {
        let d = &self.ed.assign;
        d.picked.and_then(|i| d.pics.get(i)).map(|(_, s)| *s)
    }
    /// Why each UV mode is unavailable for the picked PIC.
    fn assign_refusals(&self) -> [Option<String>; 3] {
        let d = &self.ed.assign;
        let Some(to) = self.assign_size() else {
            return [None, None, None];
        };
        let keep = if !d.textured {
            Some("Keep needs textured faces; untextured faces take Project".into())
        } else {
            tex::keep_refusal(d.from, to)
        };
        let scale = if !d.textured {
            Some("Scale needs textured faces; untextured faces take Project".into())
        } else if d.from.is_none() {
            Some("Scale needs faces on one PIC of known size".into())
        } else {
            None
        };
        [keep, scale, None]
    }
    pub(super) fn assign_pick(&mut self, i: usize) {
        if i < self.ed.assign.pics.len() {
            self.ed.assign.picked = Some(i);
            // Keep when it fits, else the next mode that does.
            let refusals = self.assign_refusals();
            if refusals[self.ed.assign.mode as usize].is_some() {
                self.ed.assign.mode = (0..3).find(|m| refusals[*m].is_none()).unwrap_or(2) as u8;
            }
        }
    }
    /// Up/Down in the dialog: the previous or next filtered PIC.
    pub(super) fn assign_step(&mut self, delta: i32) {
        let rows = self.assign_rows();
        if rows.is_empty() {
            return;
        }
        let at = self
            .ed
            .assign
            .picked
            .and_then(|p| rows.iter().position(|r| *r == p));
        let next = match at {
            None => 0,
            Some(k) => (k as i32 + delta).clamp(0, rows.len() as i32 - 1) as usize,
        };
        self.assign_pick(rows[next]);
    }
    pub(super) fn assign_mode(&mut self, mode: u8) {
        if (mode as usize) < 3 && self.assign_refusals()[mode as usize].is_none() {
            self.ed.assign.mode = mode;
        }
    }
    /// Enter in the Assign texture dialog.
    pub(super) fn apply_assign(&mut self) -> Result<()> {
        let d = &self.ed.assign;
        let (name, to) = d
            .picked
            .and_then(|i| d.pics.get(i))
            .cloned()
            .ok_or("Choose a PIC from the list")?;
        if let Some(why) = &self.assign_refusals()[d.mode as usize] {
            return Err(why.clone());
        }
        let mode = match d.mode {
            0 => UvMode::Keep,
            1 => UvMode::Scale {
                from: d.from.ok_or("Scale needs faces on one PIC of known size")?,
                to,
            },
            _ => UvMode::Project {
                plane: PLANES[(d.plane as usize).min(3)].1,
                size: to,
            },
        };
        let (entry, faces) = (d.entry, d.faces.clone());
        let source = self.doc.archive.entries[entry].read()?;
        let assigned = tex::assign_texture(&source, &faces, &name, mode)?;
        if assigned.shape == source {
            self.status = format!("The selected faces already draw from {name}; nothing changed");
            return Ok(());
        }
        let before = self.face_texture_names(&faces);
        self.doc.replace(entry, assigned.shape)?;
        self.after_texture_edit(&assigned.faces);
        self.status = format!(
            "{} now draw from {name} ({} UVs), before {}. One undo step.",
            view::count(faces.len(), "face", "faces"),
            MODES[d_mode(mode)].to_ascii_lowercase(),
            describe(&before)
        );
        // Assign creates no PIC; warn when the chosen one is not an FA texture.
        if let Some(Err(why)) = self
            .doc
            .archive
            .find(&name)
            .and_then(|i| self.doc.archive.entries[i].read().ok())
            .map(|b| picture::retail_texture_check(&b))
        {
            self.status += &format!(
                " {name} is not an FA texture ({why}); FA would crash drawing it. Package \u{b7} Repair textures for FA converts it."
            );
        }
        Ok(())
    }
    /// The Assign texture dialog: filter, PIC list, UV mode and plane.
    pub(super) fn assign_dialog(&self, o: &mut Layout) {
        use widgets::{baseline, notice};
        let Some(p) = self.prompt.as_ref() else {
            return;
        };
        let d = &self.ed.assign;
        let w = (self.width - 48).min(560);
        let h = 2 * m::ROW_H
            + 26
            + ROWS as i32 * m::ROW_H
            + 3 * (m::BUTTON_H + space::SPACE_2)
            + 2 * m::ROW_H
            + super::view::DIALOG_HEAD
            + 3 * space::SPACE_4;
        let h = h.min(self.height - 24);
        let rect = [(self.width - w) / 2, (self.height - h) / 2, w, h];
        o.hits.clear();
        let [bx, mut y, bw, _] = self.dialog_frame(o, rect, &p.title);
        let label = |o: &mut Layout, y: i32, text: &str| {
            o.canvas.styled(
                bx,
                baseline(y, m::ROW_H, Style::Label),
                text,
                c::INK_MUTED,
                Style::Label,
            );
        };
        label(o, y, "Filter PIC names");
        y += m::ROW_H;
        self.dialog_input(o, [bx, y, bw, 26], &p.value);
        y += 26 + space::SPACE_2;
        let rows = self.assign_rows();
        let picked = d.picked.and_then(|i| rows.iter().position(|r| *r == i));
        let first = picked.map_or(0, |k| k.saturating_sub(ROWS - 1));
        let list = [bx, y, bw, ROWS as i32 * m::ROW_H];
        widgets::notched(&mut o.canvas, list, Some(c::GM_950), Some(c::GM_1000));
        for (k, i) in rows.iter().enumerate().skip(first).take(ROWS) {
            let ry = y + (k - first) as i32 * m::ROW_H;
            let (name, size) = &d.pics[*i];
            let on = d.picked == Some(*i);
            if on {
                o.canvas
                    .rect(bx + 1, ry + 1, bw - 2, m::ROW_H - 2, c::AMBER_DEEP);
            }
            let ink = if on { c::AMBER } else { c::INK };
            o.canvas.icon_sm(
                bx + space::SPACE_2,
                ry + (m::ROW_H - m::ICON_SM) / 2,
                Icon::Image,
                c::INK_MUTED,
                if on { c::AMBER_DEEP } else { c::GM_950 },
            );
            o.canvas.styled(
                bx + space::SPACE_2 + m::ICON_SM + space::SPACE_1,
                baseline(ry, m::ROW_H, Style::Value),
                &fit(name, bw / 2, Style::Value),
                ink,
                Style::Value,
            );
            let dims = format!("{} \u{d7} {}", size[0], size[1]);
            let dw = text_width(&dims, Style::ValueSm);
            o.canvas.styled(
                bx + bw - space::SPACE_2 - dw,
                baseline(ry, m::ROW_H, Style::ValueSm),
                &dims,
                c::INK_MUTED,
                Style::ValueSm,
            );
            o.hit([bx + 1, ry, bw - 2, m::ROW_H], Action::AssignPick(*i));
        }
        if rows.is_empty() {
            o.canvas.styled(
                bx + space::SPACE_2,
                baseline(y, m::ROW_H, Style::Label),
                "No PIC matches the filter",
                c::INK_FAINT,
                Style::Label,
            );
        }
        y += list[3] + space::SPACE_2;
        let refusals = self.assign_refusals();
        let half = bw / 3;
        label(o, y, "UV mapping");
        let modes: Vec<(Btn, Action)> = MODES
            .iter()
            .enumerate()
            .map(|(k, t)| {
                (
                    Btn::new(t)
                        .on(d.mode as usize == k)
                        .enabled(refusals[k].is_none() && d.picked.is_some()),
                    Action::AssignMode(k as u8),
                )
            })
            .collect();
        o.segmented([bx + half, y, bw - half, m::BUTTON_H], &modes);
        y += m::BUTTON_H + space::SPACE_2;
        if d.mode == 2 {
            label(o, y, "Plane");
            let planes: Vec<(Btn, Action)> = PLANES
                .iter()
                .enumerate()
                .map(|(k, (t, _))| {
                    (
                        Btn::new(t).on(d.plane as usize == k),
                        Action::AssignPlane(k as u8),
                    )
                })
                .collect();
            o.segmented([bx + half, y, bw - half, m::BUTTON_H], &planes);
        }
        y += m::BUTTON_H + space::SPACE_2;
        let to = self.assign_size();
        let (tone, text) = match (to, refusals[0].as_ref()) {
            (None, _) => (
                Tone::Neutral,
                "Choose the PIC these faces draw from.".to_string(),
            ),
            (Some(_), Some(why)) if d.mode != 0 => (Tone::Neutral, format!("{why}.")),
            (Some(t), _) => (
                Tone::Neutral,
                match d.mode {
                    0 => "Keep: stored UVs unchanged.".to_string(),
                    1 => format!(
                        "Scale: UVs from {} to {} \u{d7} {}.",
                        d.from
                            .map_or("?".into(), |f| format!("{} \u{d7} {}", f[0], f[1])),
                        t[0],
                        t[1]
                    ),
                    _ => format!(
                        "Project: planar UVs fitted into {} \u{d7} {}, square texels.",
                        t[0], t[1]
                    ),
                },
            ),
        };
        notice(
            &mut o.canvas,
            bx,
            y,
            bw,
            tone,
            &fit(&text, bw - 32, Style::Label),
        );
        if let Some(error) = self.status.strip_prefix("Error: ") {
            let ey = y + m::ROW_H + space::SPACE_2;
            o.canvas
                .icon(bx, ey + 2, Icon::Warning, c::DANGER, c::GM_800);
            o.canvas.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ey, m::ROW_H, Style::Label),
                &fit(error, bw - m::ICON - space::SPACE_1, Style::Label),
                c::DANGER,
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            rect,
            &[],
            Some("Cancel"),
            Some(
                Btn::new("Assign texture")
                    .primary()
                    .enabled(d.picked.is_some() && refusals[d.mode as usize].is_none()),
            ),
            Action::Apply,
        );
    }
    /// Face texture rows and actions for an inspector stack: the texture of
    /// the selected faces, then Clone for selected faces (primary), Assign
    /// texture and Use shape texture, each disabled with its reason.
    pub(super) fn face_texture_pane(&self, o: &mut Layout, s: &mut widgets::Stack, whole: bool) {
        let faces = self.texture_faces();
        if self.pane(o, s, pane::MESH_TEXTURE, "Face textures", Icon::Textured) {
            if !faces.is_empty() {
                o.info(
                    s,
                    "Selected",
                    &view::count(faces.len(), "panel", "panels"),
                    "",
                );
            }
            let names = self.face_texture_names(&faces);
            for (name, n) in names.iter().take(4) {
                let shown = if name.is_empty() { "Untextured" } else { name };
                let value = format!("{shown} \u{b7} {}", view::count(*n, "face", "faces"));
                o.info(s, "Texture", &value, "");
            }
            if names.len() > 4 {
                o.info(s, "Texture", &format!("{} more", names.len() - 4), "");
            }
            let assigned = self.assigned_offsets();
            let any_assigned = faces.iter().any(|f| assigned.contains(f));
            let clone_ok = !faces.is_empty() && self.shared_texture(&faces).is_ok();
            let items: [(&str, Action, bool, bool); 4] = [
                (
                    "Clone texture for selected faces",
                    Action::FaceTexture(TEX_CLONE),
                    clone_ok,
                    true,
                ),
                (
                    "Assign texture\u{2026}",
                    Action::FaceTexture(TEX_ASSIGN),
                    !faces.is_empty(),
                    false,
                ),
                (
                    "Remap selected panels from view\u{2026}",
                    Action::FaceTexture(TEX_REMAP),
                    !faces.is_empty(),
                    false,
                ),
                (
                    "Use shape texture",
                    Action::FaceTexture(TEX_RESTORE),
                    any_assigned,
                    false,
                ),
            ];
            for (label, action, enabled, primary) in items {
                if let Some(rect) = o.wide(s, m::BUTTON_H) {
                    let b = Btn::new(label).enabled(enabled);
                    o.button_ex(rect, if primary { b.primary() } else { b }, action);
                }
            }
            if whole {
                if let Some(rect) = o.wide(s, m::BUTTON_H) {
                    o.button_ex(
                        rect,
                        Btn::new("Clone texture for whole shape"),
                        Action::Isolate,
                    );
                }
            }
            if let Some(rect) = o.wide(s, m::BUTTON_H) {
                o.button_ex(
                    rect,
                    Btn::new("Replace color\u{2026}").enabled(!faces.is_empty()),
                    Action::ReplaceDialog,
                );
            }
            let note = if faces.is_empty() {
                NO_FACES.to_string()
            } else if !clone_ok {
                self.shared_texture(&faces).err().unwrap_or_default()
            } else if whole {
                "Selected faces: only those faces get the copy. Whole shape: every face on the PIC."
                    .into()
            } else if !any_assigned {
                "Use shape texture applies to faces with a Hangar texture assignment.".into()
            } else {
                "Each action is one undo step.".into()
            };
            o.stack_notice(s, Tone::Neutral, &note);
        }
        o.panel_end(s);
    }
}
/// "3.1" for a Q16 density.
fn density_text(q16: u32) -> String {
    let t = (q16 as u64 * 10 + 32768) >> 16;
    format!("{}.{}", t / 10, t % 10)
}
impl App {
    /// The pose the shown model is drawn in: the preview pose in the Model
    /// workspace, neutral for a Paint workspace context model.
    fn shown_pose(&self) -> model::Pose {
        if self.model.is_some() {
            self.ed.pose.clone()
        } else {
            model::Pose::new()
        }
    }
    /// Plan the layout from the current view and open the Remap dialog.
    fn open_remap(&mut self, entry: usize, faces: Vec<usize>) -> Result<()> {
        let model = self.model_for_paint().ok_or(NO_FACES)?;
        let density = tex::atlas_density(model).unwrap_or(tex::DEFAULT_DENSITY);
        let pose = self.shown_pose();
        let source = self.doc.archive.entries[entry].read()?;
        let view = remap::View {
            yaw: self.yaw,
            pitch: self.pitch,
            pose: &pose,
        };
        let plan = remap::remap_plan(&source, &faces, &view, tex::Fit::Density(density))?;
        let names = self.face_texture_names(&plan.faces);
        let stem = names
            .iter()
            .find(|(n, _)| !n.is_empty())
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| self.doc.archive.entries[entry].name.clone());
        let suggestion = self.private_texture_name(&stem);
        let n = plan.faces.len();
        let span = [0, 1].map(|k| {
            let all = plan.uv.iter().flatten().map(|p| p[k]);
            (all.clone().max().unwrap_or(0) - all.min().unwrap_or(0)).max(0) as u32
        });
        self.ed.remap = RemapDraft {
            span,
            entry,
            faces: plan.faces,
            yaw: self.yaw,
            pitch: self.pitch,
            pose,
            density,
            size: plan.size,
            used: plan.density,
            fill: 0,
            from: describe(&names),
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::RemapView,
            title: format!(
                "Remap {} from view",
                view::count(n, "selected panel", "selected panels")
            ),
            value: suggestion,
            axis: 0,
        });
        Ok(())
    }
    pub(super) fn remap_fill(&mut self, fill: u8) {
        self.ed.remap.fill = fill.min(1);
    }
    /// Enter in the Remap dialog: the SH and the new PIC in one undo step.
    pub(super) fn apply_remap(&mut self, name: &str) -> Result<()> {
        let to = name.trim().to_ascii_uppercase();
        if !to.ends_with(".PIC") || !self.texture_name_free(&to) {
            return Err("Choose an unused PIC name".into());
        }
        hangar_core::archive::validate_name(&to)?;
        let d = &self.ed.remap;
        let model = self.model_for_paint().ok_or(NO_FACES)?;
        // What the faces draw now, painted strokes included.
        // The new texture holds game-palette indices, like retail skins.
        if !self.palette_loaded {
            return Err("Load the game PALETTE.PAL before remapping panels".into());
        }
        let mut textures = BTreeMap::new();
        for f in model.faces.iter().filter(|f| d.faces.contains(&f.offset)) {
            if f.sub & 4 == 0 || f.texture.is_empty() {
                continue;
            }
            let key = remap::texture_key(&f.texture);
            let pic = self.texture_for(&f.texture).ok_or_else(|| {
                format!("{key} is outside the active LIB; copy or open its owner first")
            })?;
            textures.insert(key, pic.clone());
        }
        let view = remap::View {
            yaw: d.yaw,
            pitch: d.pitch,
            pose: &d.pose,
        };
        let fill = if d.fill == 1 {
            remap::Fill::Blank
        } else {
            remap::Fill::Bake
        };
        let source = self.doc.archive.entries[d.entry].read()?;
        let r = remap::remap_from_view(
            &source,
            &d.faces,
            &to,
            &view,
            tex::Fit::Density(d.density),
            fill,
            &remap::Sources {
                textures: &textures,
                palette: &self.base_palette,
            },
        )?;
        let (n, from, entry) = (d.faces.len(), d.from.clone(), d.entry);
        let shape = self.doc.archive.entries[entry].name.clone();
        self.doc.transaction(
            vec![Entry::new(&shape, r.shape)?, Entry::new(&to, r.picture)?],
            &[],
        )?;
        self.after_texture_edit(&r.faces);
        self.status = format!(
            "Remapped {} from the view to {to}, {} \u{d7} {} at {} texels per unit, {} {from}. One undo step.",
            view::count(n, "panel", "panels"),
            r.size[0],
            r.size[1],
            density_text(r.density),
            if fill == remap::Fill::Bake {
                "baked from"
            } else {
                "blank, was"
            }
        );
        Ok(())
    }
    /// The Remap from view dialog: name, planned size, fill and Remap.
    pub(super) fn remap_dialog(&self, o: &mut Layout) {
        use widgets::{baseline, notice};
        let Some(p) = self.prompt.as_ref() else {
            return;
        };
        let d = &self.ed.remap;
        let w = (self.width - 48).min(520);
        let h = super::view::DIALOG_HEAD
            + space::SPACE_3
            + m::ROW_H
            + 26
            + space::SPACE_2
            + 2 * m::ROW_H
            + space::SPACE_2
            + m::BUTTON_H
            + space::SPACE_3
            + 2 * m::ROW_H
            + m::ROW_H
            + space::SPACE_2
            + m::BUTTON_H
            + 2 * space::SPACE_4;
        let h = h.min(self.height - 24);
        let rect = [(self.width - w) / 2, (self.height - h) / 2, w, h];
        o.hits.clear();
        let [bx, mut y, bw, _] = self.dialog_frame(o, rect, &p.title);
        let label = |o: &mut Layout, y: i32, text: &str| {
            o.canvas.styled(
                bx,
                baseline(y, m::ROW_H, Style::Label),
                text,
                c::INK_MUTED,
                Style::Label,
            );
        };
        let half = bw / 3;
        label(o, y, "New PIC name");
        y += m::ROW_H;
        self.dialog_input(o, [bx, y, bw, 26], &p.value);
        y += 26 + space::SPACE_2;
        let rows = [
            (
                "Size",
                format!(
                    "{} \u{d7} {} px, panels {} \u{d7} {}",
                    d.size[0], d.size[1], d.span[0], d.span[1]
                ),
            ),
            (
                "Density",
                format!("{} texels per unit, square", density_text(d.used)),
            ),
        ];
        for (k, v) in rows {
            label(o, y, k);
            o.canvas.styled(
                bx + half,
                baseline(y, m::ROW_H, Style::Value),
                &fit(&v, bw - half, Style::Value),
                c::INK,
                Style::Value,
            );
            y += m::ROW_H;
        }
        y += space::SPACE_2;
        label(o, y, "Fill");
        let fills: Vec<(Btn, Action)> = FILLS
            .iter()
            .enumerate()
            .map(|(k, t)| {
                (
                    Btn::new(t).on(d.fill as usize == k),
                    Action::RemapFill(k as u8),
                )
            })
            .collect();
        o.segmented([bx + half, y, bw - half, m::BUTTON_H], &fills);
        y += m::BUTTON_H + space::SPACE_3;
        notice(
            &mut o.canvas,
            bx,
            y,
            bw,
            Tone::Neutral,
            &fit(
                "Laid out as this view shows them. Orbit first so the panels face you.",
                bw - 32,
                Style::Label,
            ),
        );
        if let Some(error) = self.status.strip_prefix("Error: ") {
            let ey = y + 2 * m::ROW_H;
            o.canvas
                .icon(bx, ey + 2, Icon::Warning, c::DANGER, c::GM_800);
            o.canvas.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ey, m::ROW_H, Style::Label),
                &fit(error, bw - m::ICON - space::SPACE_1, Style::Label),
                c::DANGER,
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            rect,
            &[],
            Some("Cancel"),
            Some(Btn::new("Remap").primary()),
            Action::Apply,
        );
    }
}
fn d_mode(mode: UvMode) -> usize {
    match mode {
        UvMode::Keep => 0,
        UvMode::Scale { .. } => 1,
        UvMode::Project { .. } => 2,
    }
}
/// "_F18.PIC", "_F18.PIC and 2 more", "untextured".
fn describe(names: &[(String, usize)]) -> String {
    let first = names
        .first()
        .map(|(n, _)| {
            if n.is_empty() {
                "untextured".into()
            } else {
                n.clone()
            }
        })
        .unwrap_or_else(|| "nothing".into());
    if names.len() > 1 {
        format!("{first} and {} more", names.len() - 1)
    } else {
        first
    }
}

#[cfg(not(windows))]
impl App {
    /// Snapshot workspace `assign-texture`: Edit Mesh on the selected SH (the
    /// synthetic kit in the demo) with its first textured face selected and
    /// the Assign texture dialog open on the first other PIC.
    pub fn snapshot_assign(&mut self) -> Result<()> {
        if self.path.starts_with("Synthetic") {
            self.doc.transaction(
                vec![
                    Entry::new("KIT.SH", hangar_core::shape_testkit::demo_textured_kit())?,
                    Entry::new("KIT.PIC", picture::demo())?,
                    Entry::new("WIDE.PIC", raw_pic(64, 32, 3))?,
                ],
                &[],
            )?;
            self.doc.mark_saved();
            let at = self.doc.archive.find("KIT.SH").ok_or("Kit missing")?;
            self.select_entry(at);
        }
        self.mode = Mode::Model;
        self.textured = true;
        if !self.mesh_edit {
            self.act(Action::MeshMode);
        }
        self.select_mode(true);
        let model = self.model.as_ref().ok_or("Select an SH")?;
        let face = model
            .faces
            .iter()
            .find(|f| f.sub & 4 != 0 && !f.texture.is_empty())
            .ok_or("No textured face")?;
        let current = full(&face.texture);
        self.ed.mesh_faces = vec![face.offset];
        self.sync_face_vertices();
        self.act(Action::FaceTexture(TEX_ASSIGN));
        let pick = self
            .ed
            .assign
            .pics
            .iter()
            .position(|(n, _)| *n != current)
            .ok_or("No other PIC")?;
        self.assign_pick(pick);
        Ok(())
    }
    /// Manual real-data check (`--face-texture-check`): on each SH, pick up
    /// to four textured faces of the vertical tail (normal along the right
    /// axis, highest centres), clone their PIC through the Mesh menu action,
    /// paint the copy, and check coverage, bindings, face count, the moved
    /// and the unmoved faces, then Use shape texture and its undo. The
    /// painted edits stay and the LIB is written create-new to `output`.
    pub fn check_face_textures(&mut self, names: &[String], output: &str) -> Result<String> {
        use hangar_core::shape_geometry::Geometry;
        let mut out = String::new();
        for name in names {
            let entry = self.doc.archive.find(name).ok_or("SH not found")?;
            self.select_entry(entry);
            self.mode = Mode::Model;
            self.textured = true;
            self.ed.pose.clear();
            self.refresh();
            if !self.mesh_edit {
                self.act(Action::MeshMode);
            }
            self.select_mode(true);
            let before = self.doc.archive.entries[entry].read()?;
            let g0 = Geometry::parse(&before)?;
            let m0 = self.model.clone().ok_or("No model")?;
            // Tail: the rearmost 30% of the length; fins reach highest.
            let (aft, fore) = m0.vertices.iter().fold((i32::MAX, i32::MIN), |(a, b), v| {
                (a.min(v.point[1]), b.max(v.point[1]))
            });
            let tail = aft + (fore - aft) * 3 / 10;
            let top = |f: &model::Face| -> i32 {
                f.indices
                    .iter()
                    .map(|i| m0.vertices[*i].point[2])
                    .max()
                    .unwrap_or(0)
            };
            let centre = |f: &model::Face, k: usize| -> i32 {
                f.indices
                    .iter()
                    .map(|i| m0.vertices[*i].point[k])
                    .sum::<i32>()
                    / f.indices.len().max(1) as i32
            };
            let mut fins: Vec<(i32, usize)> = m0
                .faces
                .iter()
                .filter(|f| f.sub & 4 != 0 && !f.texture.is_empty())
                .filter(|f| f.normal.is_some_and(|n| n[0].abs() >= 30000))
                .filter(|f| centre(f, 1) <= tail)
                .map(|f| (top(f), f.offset))
                .collect();
            fins.sort_unstable_by(|a, b| b.cmp(a));
            let source = m0
                .faces
                .iter()
                .find(|f| fins.first().is_some_and(|(_, o)| *o == f.offset))
                .map(|f| full(&f.texture))
                .ok_or("No textured tail face")?;
            let picked: Vec<usize> = fins
                .iter()
                .map(|(_, o)| *o)
                .filter(|o| self.face_texture_names(&[*o]).first().map(|x| &x.0) == Some(&source))
                .take(4)
                .collect();
            self.ed.mesh_faces = picked.clone();
            self.sync_face_vertices();
            let source_entry = self.doc.archive.find(&source).ok_or("Source PIC missing")?;
            let source_bytes = self.doc.archive.entries[source_entry].read()?;
            self.face_texture_action(TEX_CLONE);
            let to = self
                .prompt
                .as_ref()
                .filter(|p| matches!(p.kind, PromptKind::FaceClone))
                .map(|p| p.value.clone())
                .ok_or_else(|| format!("{name}: {}", self.status))?;
            self.key(Key::Enter, false, false);
            if !self.status.starts_with("Cloned") {
                return Err(format!("{name}: {}", self.status));
            }
            let assigned = self.ed.mesh_faces.clone();
            let after = self.doc.archive.entries[entry].read()?;
            let g1 = Geometry::parse(&after)?;
            let m1 = self.model.clone().ok_or("No model")?;
            let moved = m1
                .faces
                .iter()
                .filter(|f| assigned.contains(&f.offset) && f.texture == to)
                .count();
            let others = m0
                .faces
                .iter()
                .filter(|f| !picked.contains(&f.offset))
                .all(|f| {
                    m1.faces.iter().any(|g| {
                        g.offset - g1.inventory.code_start == f.offset - g0.inventory.code_start
                            && g.texture == f.texture
                            && g.uv == f.uv
                    })
                });
            let runs = hangar_core::shape_texture::assignments(&g1).len();
            out += &format!(
                "{name}: cloned {source} to {to} for {} tail faces ({moved} draw from it, {runs} continuation{}); SH {} -> {} bytes\n",
                picked.len(),
                if runs == 1 { "" } else { "s" },
                before.len(),
                after.len()
            );
            let coverage = g1.inventory.contiguous()
                && g1.inventory.opaque_bytes() == g0.inventory.opaque_bytes();
            let bindings = g1.inventory.bindings == g0.inventory.bindings
                && g1.inventory.stubs.len() == g0.inventory.stubs.len();
            out += &format!(
                "  inventory {} ({} opaque bytes), bindings {} ({}), faces {} -> {}, other faces unchanged: {others}\n",
                if coverage { "complete" } else { "CHANGED" },
                g1.inventory.opaque_bytes(),
                if bindings { "unchanged" } else { "CHANGED" },
                g1.inventory.bindings.len(),
                m0.faces.len(),
                m1.faces.len()
            );
            if !coverage
                || !bindings
                || !others
                || moved != picked.len()
                || m1.faces.len() != m0.faces.len()
            {
                return Err(format!("{name}: assignment check failed\n{out}"));
            }
            // Paint the copy at the UV centre of each assigned face, Panel
            // lock on: one stroke, one undo step.
            self.act(Action::ModelPaint);
            self.paint_lock = true;
            let copy = self.doc.archive.find(&to).ok_or("Copy missing")?;
            let pic = Pic::parse(&self.doc.archive.entries[copy].read()?)?;
            self.brush = 249;
            self.brush_radius = 7;
            self.mouse = [0, 0];
            for offset in &assigned {
                let model = self.model_for_paint().ok_or("No model")?;
                let face = model
                    .faces
                    .iter()
                    .position(|f| f.offset == *offset)
                    .ok_or("Assigned face lost")?;
                let uv = &model.faces[face].uv;
                let n = uv.len().max(1) as i32;
                let centre = [
                    uv.iter().map(|p| p[0]).sum::<i32>() / n,
                    pic.height as i32 - 1 - uv.iter().map(|p| p[1]).sum::<i32>() / n,
                ];
                // A new face starts a new dab rather than a line from the last.
                if let Some(s) = &mut self.stroke {
                    s.last = None;
                }
                self.paint_model_hit(face, centre);
            }
            self.finish_stroke();
            let painted = self.doc.archive.find(&to).ok_or("Copy missing")?;
            let changed = self.doc.archive.entries[painted].read()?;
            let src_now = self.doc.archive.entries
                [self.doc.archive.find(&source).ok_or("Source lost")?]
            .read()?;
            out += &format!(
                "  painted {to}: {} pixels changed; {source} {}\n",
                Pic::parse(&changed)?
                    .pixels
                    .iter()
                    .zip(&pic.pixels)
                    .filter(|(a, b)| a != b)
                    .count(),
                if src_now == source_bytes {
                    "unchanged"
                } else {
                    "CHANGED"
                }
            );
            if src_now != source_bytes {
                return Err(format!("{name}: the source PIC changed\n{out}"));
            }
            // Use shape texture, compare every drawn face, then undo it.
            self.act(Action::ModelPaint);
            self.act(Action::MeshMode);
            self.select_mode(true);
            self.ed.mesh_faces = assigned.clone();
            self.sync_face_vertices();
            let kept = self.doc.archive.bytes()?;
            self.face_texture_action(TEX_RESTORE);
            let back = self.doc.archive.entries[entry].read()?;
            let drawn = |b: &[u8]| -> Result<Vec<DrawnFace>> {
                let cs = Geometry::parse(b)?.inventory.code_start;
                let mut v: Vec<_> = Model::parse(b)?
                    .faces
                    .iter()
                    .map(|f| (f.offset - cs, f.texture.clone(), f.uv.clone(), f.sub))
                    .collect();
                v.sort_unstable();
                Ok(v)
            };
            let same = drawn(&back)? == drawn(&before)?;
            let sites = picked.iter().all(|o| {
                let (a, b) = (o - g0.inventory.code_start, o);
                let gb = Geometry::parse(&back).ok();
                gb.is_some_and(|g| {
                    let cs = g.inventory.code_start;
                    g.face_at(cs + a)
                        .map(|i| &back[cs + a..cs + a + g.faces[i].len])
                        == g0.face_at(*b).map(|i| &before[*b..*b + g0.faces[i].len])
                })
            });
            self.act(Action::Undo);
            let undone = self.doc.archive.bytes()? == kept;
            out += &format!(
                "  Use shape texture: drawn faces identical to the original {same}, original records back in place {sites}; undo restores the assigned bytes {undone}\n"
            );
            if !same || !sites || !undone {
                return Err(format!("{name}: reverse round trip failed\n{out}"));
            }
            if self.mesh_edit {
                self.act(Action::MeshMode);
            }
        }
        crate::platform::write_new(output, &self.doc.archive.bytes()?)?;
        let reopened = Archive::parse(crate::platform::read(output)?)?;
        out += &format!(
            "Wrote {output}: {} entries; reopened and every SH re-parses: {}\n",
            reopened.entries.len(),
            names.iter().all(|n| reopened
                .find(n)
                .and_then(|i| reopened.entries[i].read().ok())
                .is_some_and(|b| Model::parse(&b).is_ok()))
        );
        Ok(out)
    }
}
/// A drawn face by CODE offset: texture, UVs and content flags.
#[cfg(not(windows))]
type DrawnFace = (usize, String, Vec<[i32; 2]>, u8);
/// A raw indexed PIC with a full 6-bit palette, for smoke fixtures.
fn raw_pic(w: usize, h: usize, seed: u8) -> Vec<u8> {
    let mut b = vec![0; 64 + w * h + 768];
    for (at, n) in [
        (2, w),
        (6, h),
        (10, 64),
        (14, w * h),
        (18, 64 + w * h),
        (22, 768),
    ] {
        b[at..at + 4].copy_from_slice(&(n as u32).to_le_bytes());
    }
    for i in 0..w * h {
        b[64 + i] = (i as u8).wrapping_mul(7).wrapping_add(seed);
    }
    for i in 0..768 {
        b[64 + w * h + i] = (i % 64) as u8;
    }
    b
}
/// The demo LIB plus the synthetic textured kit KIT.SH, its 32 x 32
/// KIT.PIC, a 64 x 32 WIDE.PIC and a 32 x 32 SAME.PIC, KIT.SH selected in
/// the Model workspace.
#[inline(never)]
fn boxed_app() -> Box<App> {
    Box::new(App::new())
}
#[inline(never)]
pub(super) fn texture_app() -> Box<App> {
    let mut a = boxed_app();
    a.demo();
    a.doc
        .transaction(
            vec![
                Entry::new("KIT.SH", hangar_core::shape_testkit::demo_textured_kit()).unwrap(),
                Entry::new("KIT.PIC", picture::demo()).unwrap(),
                Entry::new("WIDE.PIC", raw_pic(64, 32, 3)).unwrap(),
                Entry::new("SAME.PIC", raw_pic(32, 32, 9)).unwrap(),
            ],
            &[],
        )
        .unwrap();
    a.doc.mark_saved();
    a.width = 1280;
    a.height = 800;
    a.select_entry(a.doc.archive.find("KIT.SH").unwrap());
    a.mode = Mode::Model;
    a.textured = true;
    a.yaw = 35;
    a.pitch = 20;
    a
}
impl App {
    /// A root face of the shown model with (or without) a texture, picked by
    /// the raster at its projected centre.
    pub(super) fn smoke_texture_face(&self, textured: bool) -> (usize, [i32; 2]) {
        let m = self.model_for_paint().unwrap();
        (0..m.faces.len())
            .filter(|f| m.faces[*f].group.is_none() && (m.faces[*f].sub & 4 != 0) == textured)
            .find_map(|f| {
                let face = &m.faces[f];
                let n = face.indices.len() as i32;
                let c: [i32; 3] = core::array::from_fn(|k| {
                    face.indices
                        .iter()
                        .map(|i| m.vertices[*i].point[k])
                        .sum::<i32>()
                        / n
                });
                let p = self.hp_project(c)?;
                (self.in_viewport(p[0], p[1])
                    && p[1] > 130
                    && self.model_hit(p[0], p[1]).map(|(g, _)| g) == Some(f))
                .then_some((f, p))
            })
            .expect("A pickable face")
    }
    pub(super) fn smoke_menu_pick(&mut self, menu: usize, action: &dyn Fn(Action) -> bool) {
        let name = self.smoke_find(&|x| matches!(x, Action::Menu(n) if n == menu));
        self.chrome_click(name);
        let item = self.smoke_find(action);
        self.chrome_click(item);
    }
    fn smoke_texture_of(&self, offset: usize) -> String {
        self.model_for_paint()
            .unwrap()
            .faces
            .iter()
            .find(|f| f.offset == offset)
            .map(|f| f.texture.clone())
            .expect("face drawn")
    }
    /// Open the Assign texture dialog for the selection, filter to `name`
    /// and pick it through the rendered list.
    fn smoke_assign_open(&mut self, name: &str) {
        self.smoke_menu_pick(chrome::MENU_MESH, &|x| {
            matches!(x, Action::FaceTexture(TEX_ASSIGN))
        });
        assert!(
            matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::AssignTexture)
            ),
            "{}",
            self.status
        );
        for ch in name.chars() {
            self.key(Key::Char(ch), false, false);
        }
        let i = self
            .ed
            .assign
            .pics
            .iter()
            .position(|(n, _)| n == name)
            .unwrap();
        let rows: Vec<Action> = self.layout().hits.iter().map(|h| h.action).collect();
        assert_eq!(
            rows.iter()
                .filter(|a| matches!(a, Action::AssignPick(_)))
                .count(),
            1,
            "the filter leaves one PIC"
        );
        let row = self.smoke_find(&|x| matches!(x, Action::AssignPick(j) if j == i));
        self.chrome_click(row);
        assert_eq!(self.ed.assign.picked, Some(i));
    }
    /// Per-face textures end to end through rendered controls: clone for the
    /// selected faces, paint them under Panel lock, undo to the exact bytes,
    /// Assign texture with Keep, Scale and Project, Use shape texture, the
    /// refusals, and the dialog at both window sizes.
    #[inline(never)]
    pub(super) fn smoke_face_textures(&mut self) {
        let mut a = texture_app();
        let original = a.doc.archive.bytes().unwrap();
        let kit = a.doc.archive.find("KIT.PIC").unwrap();
        let kit_bytes = a.doc.archive.entries[kit].read().unwrap();
        // Edit Mesh face select; without a selection the actions are off.
        a.key(Key::Tab, false, false);
        a.key(Key::Char('3'), false, false);
        assert!(a.mesh_edit && a.ed.face_select);
        assert!(a
            .chrome_hit(&|x| matches!(x, Action::FaceTexture(TEX_CLONE)))
            .is_none());
        // (The notice wraps; its first line names the way to select.)
        assert!(super::media::shows_text(
            &mut a,
            "Select faces in Edit Mesh"
        ));
        // Pick a textured face and clone its PIC for that face only.
        let (f, p) = a.smoke_texture_face(true);
        let offset = a.model.as_ref().unwrap().faces[f].offset;
        a.smoke_press(p[0], p[1], false);
        assert_eq!(a.ed.mesh_faces, vec![offset]);
        assert!(super::media::shows_text(&mut a, "KIT.PIC"));
        a.smoke_menu_pick(chrome::MENU_MESH, &|x| {
            matches!(x, Action::FaceTexture(TEX_CLONE))
        });
        assert_eq!(
            a.prompt.as_ref().map(|p| p.value.as_str()),
            Some("KITT1.PIC"),
            "{}",
            a.status
        );
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        assert!(
            a.status.starts_with("Cloned KIT.PIC to KITT1.PIC"),
            "{}",
            a.status
        );
        assert!(a.doc.archive.find("KITT1.PIC").is_some());
        assert_eq!(a.doc.changed_count(), 2, "the SH and the new PIC");
        let new_offset = a.ed.mesh_faces[0];
        assert_eq!(a.smoke_texture_of(new_offset), "KITT1.PIC");
        let m = a.model.as_ref().unwrap();
        assert_eq!(
            m.faces.iter().filter(|x| x.texture == "KITT1.PIC").count(),
            1
        );
        assert!(m
            .faces
            .iter()
            .filter(|x| x.offset != new_offset && x.sub & 4 != 0 && x.group.is_none())
            .all(|x| x.texture == "KIT.PIC"));
        assert!(
            a.textures.contains_key("KITT1.PIC"),
            "renders from the copy"
        );
        assert!(super::media::shows_text(&mut a, "KITT1.PIC"));
        // Paint the assigned face with Panel lock: only KITT1.PIC changes.
        a.act(Action::ModelPaint);
        a.act(Action::PaintLock);
        assert!(a.model_paint && a.paint_lock);
        a.brush = 5;
        a.brush_radius = 3;
        let (_, uv) = a.model_hit(p[0], p[1]).unwrap();
        a.motion(p[0], p[1], false);
        a.pointer(p[0], p[1], 1, true, false);
        for dx in [4, 8, 60, 120] {
            a.motion(p[0] + dx, p[1], false);
        }
        a.pointer(p[0] + 120, p[1], 1, false, false);
        assert!(a.status.starts_with("Paint stroke applied"), "{}", a.status);
        let copy = a.doc.archive.find("KITT1.PIC").unwrap();
        let painted = Pic::parse(&a.doc.archive.entries[copy].read().unwrap()).unwrap();
        assert_eq!(
            painted.pixels[uv[1] as usize * painted.width + uv[0] as usize],
            5
        );
        assert_eq!(
            a.doc.archive.entries[a.doc.archive.find("KIT.PIC").unwrap()]
                .read()
                .unwrap(),
            kit_bytes,
            "the shape's own PIC is untouched"
        );
        assert!(a.doc.archive.find("KIT.ORG").is_none());
        // Undo the stroke and the clone: the exact original bytes.
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // The same clone from a face picked in the Model workspace (a click
        // with the brush off selects without painting).
        a.act(Action::ModelPaint);
        assert!(!a.model_paint);
        let (f2, p2) = a.smoke_texture_face(true);
        let before = a.model_for_paint().unwrap().faces[f2].offset;
        a.smoke_press(p2[0], p2[1], false);
        assert!(a.selected_face.is_some() && !a.doc.dirty());
        // Both clone actions side by side, with the hint saying which faces.
        a.mouse = [a.right() + 20, 300];
        a.wheel(-100);
        assert!(super::media::shows_text(&mut a, "Selected faces: only"));
        a.smoke_find(&|x| matches!(x, Action::Isolate));
        let primary = a.smoke_find(&|x| matches!(x, Action::FaceTexture(TEX_CLONE)));
        a.chrome_click(primary);
        a.key(Key::Enter, false, false);
        assert!(a.status.starts_with("Cloned KIT.PIC"), "{}", a.status);
        let face = a.selected_face.unwrap();
        let m = a.model_for_paint().unwrap();
        assert_eq!(m.faces[face].texture, "KITT1.PIC");
        assert_eq!(face, f2, "the clone keeps the face's draw position");
        assert_ne!(
            m.faces[face].offset, before,
            "now drawn from the continuation"
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.selected_face = None;
        assert!(a
            .chrome_hit(&|x| matches!(x, Action::FaceTexture(TEX_CLONE)))
            .is_none());
        // Assign texture: Keep is refused for a PIC of another size; Scale.
        a.act(Action::MeshMode);
        a.key(Key::Char('3'), false, false);
        a.smoke_press(p[0], p[1], false);
        assert_eq!(a.ed.mesh_faces, vec![offset]);
        let uv0 = a.model.as_ref().unwrap().faces[f].uv.clone();
        a.smoke_assign_open("WIDE.PIC");
        assert!(a
            .chrome_hit(&|x| matches!(x, Action::AssignMode(0)))
            .is_none());
        assert!(super::media::shows_text(&mut a, "Keep needs the same size"));
        assert_eq!(a.ed.assign.mode, 1, "Scale is the first mode that fits");
        let apply = a.smoke_find(&|x| matches!(x, Action::Apply));
        a.chrome_click(apply);
        assert!(a.prompt.is_none(), "{}", a.status);
        // WIDE.PIC is 64 wide with a palette: the status says FA cannot draw it.
        assert!(
            a.status
                .contains("WIDE.PIC is not an FA texture (64 pixels wide, not 256"),
            "{}",
            a.status
        );
        let o = a.ed.mesh_faces[0];
        assert_eq!(a.smoke_texture_of(o), "WIDE.PIC");
        let face = a
            .model
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .find(|x| x.offset == o)
            .unwrap();
        assert_eq!(
            face.uv,
            uv0.iter().map(|q| [q[0] * 2, q[1]]).collect::<Vec<_>>()
        );
        assert_eq!(a.doc.changed_count(), 1);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Project onto the top plane.
        a.smoke_press(p[0], p[1], false);
        a.smoke_assign_open("WIDE.PIC");
        let project = a.smoke_find(&|x| matches!(x, Action::AssignMode(2)));
        a.chrome_click(project);
        let top = a.smoke_find(&|x| matches!(x, Action::AssignPlane(1)));
        a.chrome_click(top);
        assert_eq!((a.ed.assign.mode, a.ed.assign.plane), (2, 1));
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        let o = a.ed.mesh_faces[0];
        assert_eq!(a.smoke_texture_of(o), "WIDE.PIC");
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Keep onto a PIC of the same size, then Use shape texture reverses it.
        a.smoke_press(p[0], p[1], false);
        a.smoke_assign_open("SAME.PIC");
        assert_eq!(a.ed.assign.mode, 0);
        a.key(Key::Enter, false, false);
        let o = a.ed.mesh_faces[0];
        assert_eq!(a.smoke_texture_of(o), "SAME.PIC");
        let face = a
            .model
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .find(|x| x.offset == o)
            .unwrap();
        assert_eq!(face.uv, uv0);
        let assigned = a.doc.archive.bytes().unwrap();
        a.smoke_menu_pick(chrome::MENU_MESH, &|x| {
            matches!(x, Action::FaceTexture(TEX_RESTORE))
        });
        assert_eq!(a.ed.mesh_faces, vec![offset], "{}", a.status);
        assert_eq!(a.smoke_texture_of(offset), "KIT.PIC");
        let restored = |a: &App| -> Vec<(usize, String, Vec<[i32; 2]>)> {
            a.model
                .as_ref()
                .unwrap()
                .faces
                .iter()
                .map(|f| (f.offset, f.texture.clone(), f.uv.clone()))
                .collect()
        };
        let now = restored(&a);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), assigned);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert_eq!(restored(&a), now, "Use shape texture restores every face");
        // Refusals keep core's reason: no assignment to remove, an
        // untextured face to clone.
        a.smoke_press(p[0], p[1], false);
        a.smoke_menu_pick(chrome::MENU_MESH, &|x| {
            matches!(x, Action::FaceTexture(TEX_RESTORE))
        });
        assert_eq!(
            a.ed.mesh_refusal.as_deref(),
            Some("No Hangar texture assignment to remove")
        );
        assert!(super::media::shows_text(
            &mut a,
            "No Hangar texture assignment to remove"
        ));
        let (_, flat) = a.smoke_texture_face(false);
        a.smoke_press(flat[0], flat[1], false);
        a.act(Action::FaceTexture(TEX_CLONE));
        assert!(a.status.contains("untextured"), "{}", a.status);
        assert!(a.prompt.is_none());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // The dialog's controls stay inside it and apart at both sizes.
        a.smoke_press(p[0], p[1], false);
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            a.smoke_assign_open("WIDE.PIC");
            let hits = a.layout().hits;
            assert!(hits.len() >= 5, "row, Scale, Project, Cancel, Assign");
            for (i, x) in hits.iter().enumerate() {
                let [hx, hy, hw, hh] = x.rect;
                assert!(hx >= 0 && hy >= 0 && hx + hw <= w && hy + hh <= h);
                for y in &hits[i + 1..] {
                    let [yx, yy, yw, yh] = y.rect;
                    assert!(
                        hx >= yx + yw || yx >= hx + hw || hy >= yy + yh || yy >= hy + hh,
                        "overlapping dialog controls at {w}x{h}"
                    );
                }
            }
            a.key(Key::Escape, false, false);
            assert!(a.prompt.is_none());
        }
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
    }
}
/// Every hit region inside the window and apart from the others.
fn hits_apart(a: &mut App, what: &str) {
    let (w, h) = (a.width, a.height);
    let hits = a.layout().hits;
    for (i, x) in hits.iter().enumerate() {
        let [hx, hy, hw, hh] = x.rect;
        assert!(
            hx >= 0 && hy >= 0 && hx + hw <= w && hy + hh <= h,
            "{what} at {w}x{h}"
        );
        for y in &hits[i + 1..] {
            let [yx, yy, yw, yh] = y.rect;
            assert!(
                hx >= yx + yw || yx >= hx + hw || hy >= yy + yh || yy >= hy + hh,
                "overlapping {what} controls at {w}x{h}"
            );
        }
    }
}
impl App {
    /// Root faces the raster picks at their own centres, below the header.
    fn smoke_visible(&self, textured: bool) -> Vec<(usize, [i32; 2])> {
        let m = self.model_for_paint().unwrap();
        (0..m.faces.len())
            .filter(|f| m.faces[*f].group.is_none() && (m.faces[*f].sub & 4 != 0) == textured)
            .filter_map(|f| {
                let face = &m.faces[f];
                let n = face.indices.len() as i32;
                let c: [i32; 3] = core::array::from_fn(|k| {
                    face.indices
                        .iter()
                        .map(|i| m.vertices[*i].point[k])
                        .sum::<i32>()
                        / n
                });
                let p = self.hp_project(c)?;
                (self.in_viewport(p[0], p[1])
                    && p[1] > 130
                    && self.model_hit(p[0], p[1]).map(|(g, _)| g) == Some(f))
                .then_some((f, p))
            })
            .collect()
    }
    fn smoke_offset(&self, f: usize) -> usize {
        self.model_for_paint().unwrap().faces[f].offset
    }
    /// A point in the viewport over no face and no control.
    fn smoke_empty(&mut self) -> [i32; 2] {
        let hits = self.layout().hits;
        let candidates = [
            [self.left() + 60, 150],
            [self.right() - 60, self.dock_y() - 30],
            [self.left() + 60, self.dock_y() - 30],
        ];
        candidates
            .into_iter()
            .find(|[x, y]| {
                self.in_viewport(*x, *y)
                    && self.model_hit(*x, *y).is_none()
                    && !hits.iter().any(|h| h.contains(*x, *y))
            })
            .expect("An empty viewport point")
    }
    fn smoke_pic(&self, name: &str) -> Pic {
        let at = self.doc.archive.find(name).unwrap();
        Pic::parse(&self.doc.archive.entries[at].read().unwrap()).unwrap()
    }
    fn smoke_tap(&mut self, action: &dyn Fn(Action) -> bool) {
        let r = self.smoke_find(action);
        self.chrome_click(r);
    }
    /// Panel selection outside Edit Mesh and Remap from view, through the
    /// rendered viewport and controls: click, Shift+click add and remove,
    /// Esc and empty space clear, the brush paints on a plain click only,
    /// the selection feeds Replace color's Selected panels, Remap with each
    /// fill (retail texture layout, faces drawn from it, undo to the exact
    /// bytes), the edge-on refusal, painting the new PIC only, Edit Mesh
    /// carrying the selection, the Paint workspace preview, and the dialog
    /// at both window sizes.
    #[inline(never)]
    pub(super) fn smoke_panels(&mut self) {
        let mut a = texture_app();
        // Game-palette indices: KIT.PIC's own colours as the base palette,
        // so a baked index reads back unchanged.
        let kit = a.smoke_pic("KIT.PIC");
        a.palette_override = Some(Box::new(kit.colors(&a.base_palette)));
        a.refresh();
        let original = a.doc.archive.bytes().unwrap();
        let faces = a.smoke_visible(true);
        assert!(faces.len() >= 3, "three textured panels in view");
        let [(f1, p1), (f2, p2), (_, p3)] = [faces[0], faces[1], faces[2]];
        let (o1, o2) = (a.smoke_offset(f1), a.smoke_offset(f2));
        // A click picks one panel; Shift+click adds, then removes.
        a.smoke_press(p1[0], p1[1], false);
        assert_eq!(a.panel_offsets(), vec![o1]);
        a.smoke_press(p2[0], p2[1], true);
        assert_eq!(a.panel_offsets(), vec![o1, o2]);
        assert!(a.status.starts_with("2 panels selected"), "{}", a.status);
        assert!(super::media::shows_text(&mut a, "2 panels selected"));
        assert_eq!(a.texture_faces(), vec![o1, o2]);
        // Amber-deep fill inside, amber edges, with no paint tool on.
        let fill = a.model_pixel(p2[0], p2[1]);
        assert!(fill == Some(c::AMBER_DEEP.0) || fill == Some(c::AMBER.0));
        a.smoke_press(p2[0], p2[1], true);
        assert_eq!(a.panel_offsets(), vec![o1]);
        assert_eq!(a.selected_face, Some(f1));
        a.key(Key::Escape, false, false);
        assert!(a.panel_offsets().is_empty() && a.selected_face.is_none());
        a.smoke_press(p1[0], p1[1], false);
        a.smoke_press(p2[0], p2[1], true);
        let e = a.smoke_empty();
        a.smoke_press(e[0], e[1], false);
        assert!(a.panel_offsets().is_empty(), "empty space clears");
        assert!(!a.doc.dirty());
        // With the brush on, Shift+click still selects and never paints; a
        // plain click paints and keeps the selection; edges only.
        a.smoke_press(p1[0], p1[1], false);
        a.smoke_press(p2[0], p2[1], true);
        a.act(Action::ModelPaint);
        assert!(a.model_paint);
        a.smoke_press(p3[0], p3[1], true);
        assert_eq!(a.panel_offsets().len(), 3);
        a.smoke_press(p3[0], p3[1], true);
        assert_eq!(a.panel_offsets(), vec![o1, o2]);
        assert!(!a.doc.dirty(), "Shift+click never paints");
        assert_ne!(a.model_pixel(p2[0], p2[1]), Some(c::AMBER_DEEP.0));
        a.brush = 5;
        a.smoke_press(p1[0], p1[1], false);
        assert!(a.status.starts_with("Paint stroke applied"), "{}", a.status);
        assert_eq!(
            a.panel_offsets(),
            vec![o1, o2],
            "painting keeps the selection"
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.act(Action::ModelPaint);
        assert!(!a.model_paint);
        // Replace color opens on Selected panels: only their footprint changes.
        let m = a.model_for_paint().unwrap();
        let uv = [m.faces[f1].uv.clone(), m.faces[f2].uv.clone()];
        assert!([f1, f2].iter().all(|f| m.faces[*f].texture == "KIT.PIC"));
        let (_, t) = a.model_hit(p1[0], p1[1]).unwrap();
        let from = kit.pixels[t[1] as usize * 32 + t[0] as usize];
        a.smoke_tap(&|x| matches!(x, Action::ReplaceDialog));
        assert_eq!(
            a.replace.dialog.as_ref().map(|d| d.scope),
            Some(replace_ui::SCOPE_PANELS),
            "{}",
            a.status
        );
        a.smoke_tap(&|x| matches!(x, Action::ReplaceSlot(0)));
        a.smoke_tap(&move |x| matches!(x, Action::ReplaceSwatch(i) if i == from));
        a.smoke_tap(&|x| matches!(x, Action::ReplaceSlot(1)));
        a.smoke_tap(&move |x| matches!(x, Action::ReplaceSwatch(i) if i == from ^ 0x80));
        a.smoke_tap(&|x| matches!(x, Action::Apply));
        assert!(a.status.starts_with("Replaced"), "{}", a.status);
        let region = picture::footprint(32, 32, &[uv[0].as_slice(), uv[1].as_slice()]).unwrap();
        let after = a.smoke_pic("KIT.PIC").pixels;
        let changed: Vec<usize> = (0..1024).filter(|i| after[*i] != kit.pixels[*i]).collect();
        assert!(!changed.is_empty() && changed.iter().all(|i| region[*i]));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Remap from the right side: the box side and the fin face +x.
        a.key(Key::Escape, false, false);
        a.key(Key::Char('3'), false, false);
        assert_eq!((a.yaw, a.pitch), (90, 0));
        let m = a.model_for_paint().unwrap();
        let side: Vec<(usize, [i32; 2])> = a
            .smoke_visible(true)
            .into_iter()
            .filter(|(f, _)| m.faces[*f].normal.is_some_and(|n| n[0] > 30000))
            .collect();
        assert!(side.len() >= 2, "box side and fin");
        let pick = |a: &mut App| {
            a.smoke_press(side[0].1[0], side[0].1[1], false);
            a.smoke_press(side[1].1[0], side[1].1[1], true);
            assert_eq!(a.panel_offsets().len(), 2);
        };
        pick(&mut a);
        // The look before: indices at points across both panels.
        let samples: Vec<[i32; 2]> = side
            .iter()
            .take(2)
            .flat_map(|(_, p)| {
                [-6, -3, 0, 3, 6].into_iter().flat_map(move |dx| {
                    [-2, 0, 2].into_iter().map(move |dy| [p[0] + dx, p[1] + dy])
                })
            })
            .collect();
        let look = |a: &App| -> Vec<Option<u8>> {
            samples
                .iter()
                .map(|p| {
                    let (f, uv) = a.model_hit(p[0], p[1])?;
                    let pic = a.texture_for(&a.model_for_paint()?.faces[f].texture)?;
                    Some(pic.pixels[uv[1] as usize * pic.width + uv[0] as usize])
                })
                .collect()
        };
        let before = look(&a);
        a.smoke_tap(&|x| matches!(x, Action::FaceTexture(TEX_REMAP)));
        assert!(
            matches!(
                a.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::RemapView)
            ),
            "{}",
            a.status
        );
        assert_eq!(a.prompt.as_ref().unwrap().value, "KITT1.PIC");
        let size = a.ed.remap.size;
        assert_eq!(size[0], 256, "SH textures are 256 wide");
        assert!(super::media::shows_text(
            &mut a,
            &format!("{} \u{d7} {} px", size[0], size[1])
        ));
        // The panels span the layout's aspect: 80 units by 20 from the side.
        let span = a.ed.remap.span;
        assert!(
            (span[0] as i64 * 20 - span[1] as i64 * 80).abs() <= 2 * 80,
            "{span:?}"
        );
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            hits_apart(&mut a, "Remap dialog");
            for fill in [0, 1] {
                assert!(a
                    .chrome_hit(&|x| matches!(x, Action::RemapFill(k) if k == fill))
                    .is_some());
            }
            assert!(a.chrome_hit(&|x| matches!(x, Action::Apply)).is_some());
            assert!(a.chrome_hit(&|x| matches!(x, Action::Cancel)).is_some());
        }
        a.smoke_tap(&|x| matches!(x, Action::Apply));
        assert!(a.prompt.is_none(), "{}", a.status);
        assert!(
            a.status
                .starts_with("Remapped 2 panels from the view to KITT1.PIC, 256"),
            "{}",
            a.status
        );
        assert_eq!(a.doc.changed_count(), 2, "the SH and the new PIC");
        let at = a.doc.archive.find("KITT1.PIC").unwrap();
        let bytes = a.doc.archive.entries[at].read().unwrap();
        assert!(picture::is_retail_texture(&bytes));
        let new = a.smoke_pic("KITT1.PIC");
        assert_eq!([new.width as u32, new.height as u32], size);
        assert!(
            a.doc.archive.find("KITT1.ORG").is_none(),
            "no backup until painted"
        );
        assert!(
            a.textures.contains_key("KITT1.PIC"),
            "renders from the copy"
        );
        let m = a.model_for_paint().unwrap();
        let moved = a.panel_offsets();
        assert_eq!(moved.len(), 2, "the selection follows the faces");
        assert!(moved.iter().all(|o| m
            .faces
            .iter()
            .any(|f| f.offset == *o && f.texture == "KITT1.PIC")));
        let now = look(&a);
        let same = before.iter().zip(&now).filter(|(x, y)| x == y).count();
        assert!(same * 10 >= before.len() * 8, "{before:?} {now:?}");
        let remapped = a.doc.archive.bytes().unwrap();
        // Painting the remapped panel changes only the new PIC.
        a.act(Action::ModelPaint);
        a.brush = 7;
        let p = side[0].1;
        a.smoke_press(p[0], p[1], false);
        assert!(a.status.starts_with("Paint stroke applied"), "{}", a.status);
        assert_ne!(a.smoke_pic("KITT1.PIC").pixels, new.pixels);
        assert_eq!(a.smoke_pic("KIT.PIC").pixels, kit.pixels);
        assert!(a.doc.archive.find("KITT1.ORG").is_some());
        assert!(a.doc.archive.find("KIT.ORG").is_none());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), remapped);
        a.act(Action::ModelPaint);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original, "one undo step");
        // Blank: the whole sheet takes the panels' dominant index.
        pick(&mut a);
        a.smoke_tap(&|x| matches!(x, Action::FaceTexture(TEX_REMAP)));
        a.smoke_tap(&|x| matches!(x, Action::RemapFill(1)));
        assert_eq!(a.ed.remap.fill, 1);
        a.key(Key::Enter, false, false);
        assert!(a.status.contains("blank, was KIT.PIC"), "{}", a.status);
        let blank = a.smoke_pic("KITT1.PIC");
        assert!(blank.pixels.iter().all(|x| *x == blank.pixels[0]));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Seen from the front the panels are edge-on: refused, no change.
        pick(&mut a);
        a.key(Key::Char('1'), false, false);
        a.smoke_tap(&|x| matches!(x, Action::FaceTexture(TEX_REMAP)));
        assert!(a.prompt.is_none());
        assert!(
            a.status.contains("turn the view to face the panel"),
            "{}",
            a.status
        );
        assert!(!a.doc.dirty());
        // Edit Mesh takes the same faces in face select, Mesh menu included.
        a.act(Action::MeshMode);
        assert!(a.mesh_edit && a.ed.face_select);
        assert_eq!(a.texture_faces().len(), 2);
        a.yaw = 90;
        a.smoke_menu_pick(chrome::MENU_MESH, &|x| {
            matches!(x, Action::FaceTexture(TEX_REMAP))
        });
        assert!(
            matches!(
                a.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::RemapView)
            ),
            "{}",
            a.status
        );
        a.key(Key::Escape, false, false);
        a.act(Action::MeshMode);
        assert_eq!(a.panel_offsets().len(), 2, "back out with the faces");
        // Paint workspace: the model preview picks panels too.
        a.key(Key::Escape, false, false);
        let kit_entry = a.doc.archive.find("KIT.PIC").unwrap();
        a.open_texture(kit_entry);
        assert!(a.mode == Mode::Media);
        let rect = a
            .chrome_hit(&|x| matches!(x, Action::PanelPick(_)))
            .expect("the Paint workspace model preview");
        let mut found: Vec<(usize, [i32; 2])> = Vec::new();
        'scan: for y in (rect[1] + 20..rect[1] + rect[3]).step_by(9) {
            for x in (rect[0] + 4..rect[0] + rect[2]).step_by(9) {
                if let Some((f, _)) = a.preview_hit(rect, x, y) {
                    if !found.iter().any(|(g, _)| *g == f) {
                        found.push((f, [x, y]));
                        if found.len() == 2 {
                            break 'scan;
                        }
                    }
                }
            }
        }
        assert_eq!(found.len(), 2, "two panels in the preview");
        a.smoke_press(found[0].1[0], found[0].1[1], false);
        a.smoke_press(found[1].1[0], found[1].1[1], true);
        assert_eq!(a.panel_offsets().len(), 2);
        assert!(super::media::shows_text(&mut a, "Remap 2 panels from view"));
        a.key(Key::Escape, false, false);
        assert!(a.panel_offsets().is_empty());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
    }
}
#[cfg(not(windows))]
fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << (128 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}
/// Texels per source unit (Q16) along a face's vertical and horizontal
/// in-plane axes, from its largest fan triangle. Vertical is the up axis
/// projected onto the face (forward for faces lying flat); horizontal runs
/// across it. Their ratio is the face's stretch.
#[cfg(not(windows))]
pub(super) fn face_density(points: &[[i32; 3]], uv: &[[i32; 2]]) -> Option<[u64; 2]> {
    let n = points.len();
    if n < 3 || uv.len() != n {
        return None;
    }
    let sub = |a: [i32; 3], b: [i32; 3]| -> [i128; 3] {
        core::array::from_fn(|k| b[k] as i128 - a[k] as i128)
    };
    let cross = |a: [i128; 3], b: [i128; 3]| -> [i128; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let dot = |a: [i128; 3], b: [i128; 3]| -> i128 { (0..3).map(|k| a[k] * b[k]).sum() };
    let j = (1..n - 1).max_by_key(|j| {
        let c = cross(sub(points[0], points[*j]), sub(points[0], points[j + 1]));
        dot(c, c)
    })?;
    let (e1, e2) = (sub(points[0], points[j]), sub(points[0], points[j + 1]));
    let du =
        |k: usize| -> [i128; 2] { [(uv[k][0] - uv[0][0]) as i128, (uv[k][1] - uv[0][1]) as i128] };
    let (d1, d2) = (du(j), du(j + 1));
    let normal = cross(e1, e2);
    let (g11, g12, g22) = (dot(e1, e1), dot(e1, e2), dot(e2, e2));
    let det = g11 * g22 - g12 * g12;
    if det == 0 {
        return None;
    }
    let small = |v: [i128; 3]| -> Option<[i128; 3]> {
        let m = v.iter().map(|x| x.abs()).max()?;
        (m != 0).then(|| v.map(|x| x * 1024 / m))
    };
    let nn = dot(normal, normal);
    let up = if normal[2].pow(2) * 100 > nn * 81 {
        [0, 1, 0]
    } else {
        [0, 0, 1]
    };
    let n_small = small(normal)?;
    let un = dot(up, n_small);
    let ns = dot(n_small, n_small);
    let vertical = small(core::array::from_fn(|k| up[k] * ns - n_small[k] * un))?;
    let horizontal = small(cross(n_small, vertical))?;
    let density = |d: [i128; 3]| -> u64 {
        let (r1, r2) = (dot(d, e1), dot(d, e2));
        let (a, b) = (g22 * r1 - g12 * r2, g11 * r2 - g12 * r1);
        let t: [i128; 2] = core::array::from_fn(|k| a * d1[k] + b * d2[k]);
        let len = isqrt((t[0] * t[0] + t[1] * t[1]) as u128);
        let unit = isqrt(dot(d, d) as u128);
        (len * 65536 / (det as u128 * unit).max(1)) as u64
    };
    Some([density(vertical), density(horizontal)])
}
/// "3.1" for a Q16 value.
#[cfg(not(windows))]
fn q16(v: u64) -> String {
    density_text(v.min(u32::MAX as u64) as u32)
}
#[cfg(not(windows))]
impl App {
    /// Manual real-data check (`--remap-check`): on each SH, a stretch
    /// census of the textured faces (texels per unit along each face's
    /// vertical and horizontal axes), then Remap from view with Bake on the
    /// most stretched side-facing faces from the side they face, through
    /// the panel selection and the dialog. Checks the retail texture
    /// layout, CODE coverage, bindings and stubs, every other face, the
    /// re-parse and the stretch after; renders the side view before and
    /// after and the new PIC as PNG; reverses with Use shape texture and
    /// undoes that. The remaps stay and the LIB is written create-new.
    pub fn check_remap(&mut self, out: &str, names: &[String]) -> Result<String> {
        use hangar_core::shape_geometry::Geometry;
        if !self.palette_loaded {
            return Err("The LIB has no PALETTE.PAL".into());
        }
        let mut report = String::new();
        for name in names {
            let entry = self.doc.archive.find(name).ok_or("SH not found")?;
            self.select_entry(entry);
            self.mode = Mode::Model;
            self.textured = true;
            self.ed.pose.clear();
            self.refresh();
            let m0 = self.model.clone().ok_or("Not a decodable SH")?;
            let before = self.doc.archive.entries[entry].read()?;
            let g0 = Geometry::parse(&before)?;
            let (lo, hi) = m0
                .vertices
                .iter()
                .fold(([i32::MAX; 3], [i32::MIN; 3]), |(l, h), v| {
                    (
                        core::array::from_fn(|k| l[k].min(v.point[k])),
                        core::array::from_fn(|k| h[k].max(v.point[k])),
                    )
                });
            let points = |f: &model::Face| -> Vec<[i32; 3]> {
                f.indices.iter().map(|i| m0.vertices[*i].point).collect()
            };
            let centre = |f: &model::Face| -> [i32; 3] {
                let p = points(f);
                core::array::from_fn(|k| p.iter().map(|q| q[k]).sum::<i32>() / p.len() as i32)
            };
            // Census: (stretch Q16, face index, [vertical, horizontal]).
            let mut census: Vec<(u64, usize, [u64; 2])> = m0
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| f.sub & 4 != 0 && !f.texture.is_empty())
                .filter_map(|(i, f)| {
                    let d = face_density(&points(f), &f.uv)?;
                    let (big, small) = (d[0].max(d[1]), d[0].min(d[1]).max(1));
                    Some((big * 65536 / small, i, d))
                })
                .collect();
            // UVs collapsed to a line or a point (a colour swatch mapping,
            // under 0.05 texels per unit across) are counted, not ranked.
            let swatches = census.iter().filter(|c| c.2[0].min(c.2[1]) < 3277).count();
            census.retain(|c| c.2[0].min(c.2[1]) >= 3277);
            census.sort_unstable_by(|a, b| b.cmp(a));
            let tag = |f: &model::Face| -> &'static str {
                let c = centre(f);
                let side = f.normal.is_some_and(|n| n[0].abs() >= 29490);
                if side && c[1] <= lo[1] + (hi[1] - lo[1]) * 35 / 100 && c[2] >= (lo[2] + hi[2]) / 2
                {
                    "tail fin"
                } else if side && c[1] >= (lo[1] + hi[1]) / 2 {
                    "forward side (intake/nose)"
                } else if side {
                    "side"
                } else {
                    ""
                }
            };
            let stretched = census.iter().filter(|c| c.0 >= 3 << 15).count();
            report += &format!(
                "{name}: {} textured faces measured ({swatches} more map a line or point of the PIC), {stretched} stretched 1.5x or more; worst:\n",
                census.len()
            );
            for (s, i, d) in census.iter().take(12) {
                let f = &m0.faces[*i];
                report += &format!(
                    "  {:X} {} {:?} n {:?}: {} texels/unit vertical, {} horizontal, stretch {}x {}\n",
                    f.offset,
                    full(&f.texture),
                    centre(f),
                    f.normal.unwrap_or([0; 3]),
                    q16(d[0]),
                    q16(d[1]),
                    q16(*s),
                    tag(f)
                );
            }
            for label in ["tail fin", "forward side (intake/nose)"] {
                if let Some((s, i, d)) = census.iter().find(|(_, i, _)| tag(&m0.faces[*i]) == label)
                {
                    let f = &m0.faces[*i];
                    report += &format!(
                        "  worst {label}: {:X} at {:?}: {} vertical, {} horizontal, stretch {}x\n",
                        f.offset,
                        centre(f),
                        q16(d[0]),
                        q16(d[1]),
                        q16(*s)
                    );
                }
            }
            // The most stretched side faces of one side and one texture,
            // tail fins and forward sides first.
            let mut side: Vec<&(u64, usize, [u64; 2])> = census
                .iter()
                .filter(|(s, i, _)| {
                    *s >= 3 << 15 && m0.faces[*i].normal.is_some_and(|n| n[0].abs() >= 29490)
                })
                .collect();
            side.sort_unstable_by_key(|(s, i, _)| {
                (
                    core::cmp::Reverse(tag(&m0.faces[*i]) != "side"),
                    core::cmp::Reverse(*s),
                )
            });
            let first = side.first().ok_or("No stretched side-facing face")?;
            let right = m0.faces[first.1].normal.is_some_and(|n| n[0] > 0);
            let texture = m0.faces[first.1].texture.clone();
            let mut picked: Vec<usize> = side
                .iter()
                .filter(|(_, i, _)| {
                    let f = &m0.faces[*i];
                    f.texture == texture && f.normal.is_some_and(|n| (n[0] > 0) == right)
                })
                .map(|(_, i, _)| *i)
                .take(4)
                .collect();
            // Side view from the side the faces face.
            self.yaw = if right { 90 } else { 270 };
            self.pitch = 0;
            self.frame();
            self.zoom = 170;
            self.ed.mesh_faces.clear();
            self.selected_face = None;
            let png = |_: &App, file: &str, bytes: Vec<u8>| -> Result<String> {
                let path = format!("{out}/{file}");
                crate::platform::write_new(&path, &bytes)?;
                Ok(path)
            };
            let stem = name.trim_end_matches(".SH");
            report += &format!(
                "  before: {}\n",
                png(
                    self,
                    &format!("{stem}-before.png"),
                    picture::rgba_png(640, 400, &self.render_rgba(640, 400))
                )?
            );
            let mut refused = Vec::new();
            let to = loop {
                if picked.is_empty() {
                    return Err(format!("{name}: every candidate was refused: {refused:?}"));
                }
                self.ed.mesh_faces.clear();
                for (k, f) in picked.iter().enumerate() {
                    self.panel_click(Some(*f), k > 0);
                }
                self.face_texture_action(TEX_REMAP);
                let Some(p) = self
                    .prompt
                    .as_ref()
                    .filter(|p| matches!(p.kind, PromptKind::RemapView))
                else {
                    // Drop the face the refusal names and try again.
                    let at = self.status.split("Face at ").nth(1).and_then(|s| {
                        usize::from_str_radix(s.split(':').next()?.split(' ').next()?, 16).ok()
                    });
                    refused.push(self.status.clone());
                    match at.and_then(|o| picked.iter().position(|f| m0.faces[*f].offset == o)) {
                        Some(k) => {
                            picked.remove(k);
                            continue;
                        }
                        None => return Err(format!("{name}: {}", self.status)),
                    }
                };
                let to = p.value.clone();
                let (size, used) = (self.ed.remap.size, self.ed.remap.used);
                self.key(Key::Enter, false, false);
                if !self.status.starts_with("Remapped") {
                    let status = self.status.clone();
                    self.key(Key::Escape, false, false);
                    let at = status.split("Face at ").nth(1).and_then(|s| {
                        usize::from_str_radix(s.split(':').next()?.split(' ').next()?, 16).ok()
                    });
                    refused.push(status.clone());
                    match at.and_then(|o| picked.iter().position(|f| m0.faces[*f].offset == o)) {
                        Some(k) => {
                            picked.remove(k);
                            continue;
                        }
                        None => return Err(format!("{name}: {status}")),
                    }
                }
                report += &format!(
                    "  remapped {} faces of {} from the {} side to {to}: {} x {} at {} texels/unit\n  {}\n",
                    picked.len(),
                    full(&texture),
                    if right { "right" } else { "left" },
                    size[0],
                    size[1],
                    density_text(used),
                    self.status
                );
                for f in &picked {
                    let s = census.iter().find(|c| c.1 == *f).map_or(0, |c| c.0);
                    report += &format!(
                        "    {:X} {} at {:?}, stretch {}x\n",
                        m0.faces[*f].offset,
                        match tag(&m0.faces[*f]) {
                            "" => "face",
                            t => t,
                        },
                        centre(&m0.faces[*f]),
                        q16(s)
                    );
                }
                break to;
            };
            for r in &refused {
                report += &format!("  refused first: {r}\n");
            }
            let moved = self.panel_offsets();
            let after = self.doc.archive.entries[entry].read()?;
            let g1 = Geometry::parse(&after)?;
            let m1 = self.model.clone().ok_or("No model after the remap")?;
            let pic_at = self.doc.archive.find(&to).ok_or("New PIC missing")?;
            let pic_bytes = self.doc.archive.entries[pic_at].read()?;
            let retail = picture::is_retail_texture(&pic_bytes);
            let coverage = g1.inventory.contiguous()
                && g1.inventory.opaque_bytes() == g0.inventory.opaque_bytes();
            let bindings = g1.inventory.bindings == g0.inventory.bindings
                && g1.inventory.stubs.len() == g0.inventory.stubs.len();
            let (cs0, cs1) = (g0.inventory.code_start, g1.inventory.code_start);
            let originals: Vec<usize> = picked.iter().map(|f| m0.faces[*f].offset - cs0).collect();
            let others = m0
                .faces
                .iter()
                .filter(|f| !originals.contains(&(f.offset - cs0)))
                .all(|f| {
                    m1.faces.iter().any(|g| {
                        g.offset - cs1 == f.offset - cs0 && g.texture == f.texture && g.uv == f.uv
                    })
                });
            let on_new = m1
                .faces
                .iter()
                .filter(|f| moved.contains(&f.offset) && f.texture == to)
                .count();
            let reparsed = Model::parse(&after).is_ok();
            report += &format!(
                "  {to}: retail layout {retail} ({} bytes); inventory {} ({} opaque bytes), bindings {} ({}), faces {} -> {}, {on_new} on {to}, other faces unchanged {others}, re-parse {reparsed}\n",
                pic_bytes.len(),
                if coverage { "complete" } else { "CHANGED" },
                g1.inventory.opaque_bytes(),
                if bindings { "unchanged" } else { "CHANGED" },
                g1.inventory.bindings.len(),
                m0.faces.len(),
                m1.faces.len()
            );
            for f in m1.faces.iter().filter(|f| moved.contains(&f.offset)) {
                let p: Vec<[i32; 3]> = f.indices.iter().map(|i| m1.vertices[*i].point).collect();
                if let Some(d) = face_density(&p, &f.uv) {
                    let (big, small) = (d[0].max(d[1]), d[0].min(d[1]).max(1));
                    report += &format!(
                        "    {:X} now {} vertical, {} horizontal, stretch {}x\n",
                        f.offset,
                        q16(d[0]),
                        q16(d[1]),
                        q16(big * 65536 / small)
                    );
                }
            }
            if !retail
                || !coverage
                || !bindings
                || !others
                || !reparsed
                || on_new != picked.len()
                || m1.faces.len() != m0.faces.len()
            {
                return Err(format!("{name}: remap check failed\n{report}"));
            }
            report += &format!(
                "  remapped panels selected: {}\n",
                png(
                    self,
                    &format!("{stem}-panels.png"),
                    picture::rgba_png(640, 400, &self.render_rgba(640, 400))
                )?
            );
            self.ed.mesh_faces.clear();
            self.selected_face = None;
            report += &format!(
                "  after: {}\n  new PIC: {}\n",
                png(
                    self,
                    &format!("{stem}-after.png"),
                    picture::rgba_png(640, 400, &self.render_rgba(640, 400))
                )?,
                png(
                    self,
                    &format!("{stem}-{}.png", to.trim_end_matches(".PIC")),
                    Pic::parse(&pic_bytes)?.png(&self.base_palette)
                )?
            );
            // Use shape texture returns every drawn face; undo keeps the remap.
            let kept = self.doc.archive.bytes()?;
            self.ed.mesh_faces = moved.clone();
            self.face_texture_action(TEX_RESTORE);
            let back = self.doc.archive.entries[entry].read()?;
            let drawn = |b: &[u8]| -> Result<Vec<DrawnFace>> {
                let cs = Geometry::parse(b)?.inventory.code_start;
                let mut v: Vec<_> = Model::parse(b)?
                    .faces
                    .iter()
                    .map(|f| (f.offset - cs, f.texture.clone(), f.uv.clone(), f.sub))
                    .collect();
                v.sort_unstable();
                Ok(v)
            };
            let same = drawn(&back)? == drawn(&before)?;
            self.act(Action::Undo);
            let undone = self.doc.archive.bytes()? == kept;
            report += &format!(
                "  Use shape texture: every drawn face as before {same}; undo restores the remap {undone}\n"
            );
            if !same || !undone {
                return Err(format!("{name}: reverse round trip failed\n{report}"));
            }
        }
        let lib = format!("{out}/REMAP.LIB");
        crate::platform::write_new(&lib, &self.doc.archive.bytes()?)?;
        let reopened = Archive::parse(crate::platform::read(&lib)?)?;
        report += &format!(
            "Wrote {lib}: {} entries; reopened, every SH re-parses: {}\n",
            reopened.entries.len(),
            names.iter().all(|n| reopened
                .find(n)
                .and_then(|i| reopened.entries[i].read().ok())
                .is_some_and(|b| Model::parse(&b).is_ok()))
        );
        Ok(report)
    }
}
