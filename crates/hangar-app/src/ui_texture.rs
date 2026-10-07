//! Per-face textures: clone the selected faces' PIC for those faces only,
//! assign another PIC with Keep, Scale or Project UVs, and return faces to
//! the shape's texture. Each is one undo step through `shape_texture`.
use super::view::{Action, Icon, Layout};
use super::widgets::{pane, Btn, Tone};
use super::*;
use hangar_core::shape_texture::{self as tex, Plane, UvMode};
use theme::{metric as m, space};

pub(super) const TEX_CLONE: u8 = 0;
pub(super) const TEX_ASSIGN: u8 = 1;
pub(super) const TEX_RESTORE: u8 = 2;
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
    /// selection, or the face picked for painting.
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
        self.selected_face
            .and_then(|i| model.faces.get(i))
            .map(|f| vec![f.offset])
            .unwrap_or_default()
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
        self.refresh();
        if self.mesh_edit && self.mode == Mode::Model {
            self.ed.face_select = true;
            self.ed.mesh_faces = faces.to_vec();
            self.sync_face_vertices();
        } else if let Some(model) = self.model_for_paint() {
            self.selected_face = faces
                .first()
                .and_then(|o| model.faces.iter().position(|f| f.offset == *o));
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
            let items: [(&str, Action, bool, bool); 3] = [
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
