//! Edit Mesh: vertex and face select modes, click, box and part selection,
//! and region operations through `shape_geometry` (delete, flip, make face,
//! duplicate, extrude, add vertex). Each operation is one undo step; a
//! refusal keeps the reason core gives.
use super::view::{Action, Icon, Layout};
use super::widgets::{pane, Btn, Check, Tone};
use super::*;
use hangar_core::shape_geometry::{self as geo, FaceStyle};
use theme::{metric as m, space};

pub(super) const OP_DELETE: u8 = 0;
pub(super) const OP_FLIP: u8 = 1;
pub(super) const OP_FACE: u8 = 2;
pub(super) const OP_DUPLICATE: u8 = 3;
pub(super) const OP_EXTRUDE: u8 = 4;
pub(super) const OP_VERTEX: u8 = 5;
pub(super) const OP_LINKED: u8 = 6;
pub(super) const OP_BOX: u8 = 7;
pub(super) const OP_FACE_COLOR: u8 = 8;
pub(super) const OP_NEIGHBOUR: u8 = 9;
pub(super) const OP_INVERT: u8 = 10;
/// A box select in progress, in window pixels.
#[derive(Clone, Copy, Debug)]
pub(super) struct BoxSelect {
    pub start: [i32; 2],
    pub end: [i32; 2],
    pub extend: bool,
    pub subtract: bool,
    /// Moved past the drag threshold, or started by B: a box, not a click.
    pub boxing: bool,
}
/// Duplicate or extrude waiting for its move.
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub extrude: bool,
    /// Face file offsets.
    pub faces: Vec<usize>,
}
/// Edit Mesh and Parts state.
#[derive(Default)]
pub(super) struct EditState {
    /// Preview pose by import variable name; never saved, never in undo.
    pub pose: model::Pose,
    /// Stub-driven parts of the shown SH (`shape_parts::parts`), or why not.
    pub parts: Vec<hangar_core::shape_parts::PartInfo>,
    pub parts_note: String,
    pub part_selected: Option<usize>,
    /// Open part-control Select: control index and the Select's rect.
    pub part_menu: Option<(usize, [i32; 4])>,
    /// Face select mode (3); vertex mode (1) otherwise.
    pub face_select: bool,
    /// Selected faces by file offset, which survives edits elsewhere.
    pub mesh_faces: Vec<usize>,
    pub mesh_box: Option<BoxSelect>,
    /// B pressed: the next viewport press starts a box.
    pub box_armed: bool,
    /// R and S pivot: each connected island about its own centre.
    pub pivot_individual: bool,
    /// Why the last Edit Mesh operation was refused.
    pub mesh_refusal: Option<String>,
    /// Flat colour for new faces; `None` copies a neighbouring face.
    pub new_face_color: Option<u8>,
    /// Duplicate or extrude waiting for its move (Shift+D / E, then G).
    pub mesh_pending: Option<Pending>,
    /// The Assign texture dialog.
    pub assign: super::texture_ui::AssignDraft,
    /// The Remap from view dialog.
    pub remap: super::texture_ui::RemapDraft,
    /// Why the faces of the open Clone, Assign or Remap dialog cannot take
    /// another texture, checked when it opened.
    pub texture_refusal: Option<String>,
    /// Faces drawn from Hangar texture assignments in the shown shape.
    pub assigned: super::texture_ui::AssignedCache,
    /// The Runtime markings panel.
    pub markings: super::markings_ui::State,
}
/// Pixels a press must travel before it becomes a box.
const BOX_THRESHOLD: i32 = 4;
/// Integer crossing test of a point against a projected polygon.
fn inside(p: [i32; 2], poly: &[[i32; 2]]) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x =
                a[0] as i64 + (p[1] - a[1]) as i64 * (b[0] - a[0]) as i64 / (b[1] - a[1]) as i64;
            if (p[0] as i64) < x {
                odd = !odd;
            }
        }
    }
    odd
}
/// Whether group `g` is `target` or nested inside it.
fn within(model: &Model, mut g: Option<usize>, target: Option<usize>) -> bool {
    if target.is_none() {
        return g.is_none();
    }
    for _ in 0..64 {
        if g == target {
            return true;
        }
        match g.and_then(|i| model.groups.get(i)) {
            Some(group) => g = group.parent,
            None => return false,
        }
    }
    false
}
impl App {
    /// Inside the 3D viewport, clear of the tool strip.
    pub(super) fn in_viewport(&self, x: i32, y: i32) -> bool {
        x > self.left() + 40 && x < self.right() - 1 && y > 54 && y < self.dock_y()
    }
    /// Selected vertex file offsets, to keep a selection across reloads.
    pub(super) fn picked_offsets(&self) -> Vec<usize> {
        let Some(model) = &self.model else {
            return Vec::new();
        };
        self.mesh_vertices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.offset))
            .collect()
    }
    /// Reselect by stored offset after the model was rebuilt.
    pub(super) fn repick(&mut self, offsets: &[usize]) {
        // Selected faces (Edit Mesh face select and the panels picked
        // outside it) keep the offsets the shown model still draws.
        let mut drawn: Vec<usize> = self
            .model_for_paint()
            .map(|m| m.faces.iter().map(|f| f.offset).collect())
            .unwrap_or_default();
        drawn.sort_unstable();
        self.ed
            .mesh_faces
            .retain(|o| drawn.binary_search(o).is_ok());
        let Some(model) = &self.model else {
            self.mesh_vertices.clear();
            return;
        };
        if self.ed.face_select {
            self.sync_face_vertices();
            return;
        }
        // Keep the selection order (the first is the active vertex).
        let mut picked = Vec::new();
        for o in offsets {
            for (i, v) in model.vertices.iter().enumerate() {
                if v.offset == *o && !picked.contains(&i) {
                    picked.push(i);
                }
            }
        }
        self.mesh_vertices = picked;
    }
    /// In face mode the vertex selection is the corners of the selected faces.
    pub(super) fn sync_face_vertices(&mut self) {
        let Some(model) = &self.model else {
            return;
        };
        let mut faces = self.ed.mesh_faces.clone();
        faces.sort_unstable();
        let mut seen = vec![false; model.vertices.len()];
        let mut out = Vec::new();
        for f in model
            .faces
            .iter()
            .filter(|f| faces.binary_search(&f.offset).is_ok())
        {
            for i in &f.indices {
                if seen.get(*i) == Some(&false) {
                    seen[*i] = true;
                    out.push(*i);
                }
            }
        }
        self.mesh_vertices = out;
    }
    /// Model face indices the operations act on: the selected faces, or in
    /// vertex mode every face whose corners are all selected.
    pub(super) fn selected_faces(&self) -> Vec<usize> {
        let Some(model) = self.preview.as_deref().or(self.model.as_ref()) else {
            return Vec::new();
        };
        // Sorted offsets and a vertex mask keep this linear for large selections.
        let mut faces = self.ed.mesh_faces.clone();
        faces.sort_unstable();
        let mut picked = vec![false; model.vertices.len()];
        for i in &self.mesh_vertices {
            if let Some(p) = picked.get_mut(*i) {
                *p = true;
            }
        }
        (0..model.faces.len())
            .filter(|f| {
                let face = &model.faces[*f];
                if self.ed.face_select {
                    faces.binary_search(&face.offset).is_ok()
                } else {
                    !face.indices.is_empty()
                        && face.indices.iter().all(|i| picked.get(*i) == Some(&true))
                }
            })
            .collect()
    }
    pub(super) fn select_mode(&mut self, face: bool) {
        if face == self.ed.face_select {
            return;
        }
        if face {
            let faces: Vec<usize> = self
                .selected_faces()
                .into_iter()
                .filter_map(|f| self.model.as_ref().map(|m| m.faces[f].offset))
                .collect();
            self.ed.face_select = true;
            self.ed.mesh_faces = faces;
            self.sync_face_vertices();
        } else {
            self.ed.face_select = false;
        }
        self.ed.mesh_refusal = None;
        self.status = if face {
            "Face select (3): click faces, B or drag for a box, L selects a part"
        } else {
            "Vertex select (1): click vertices, B or drag for a box, L selects a part"
        }
        .into();
    }
    /// A selects everything, or clears a complete selection.
    pub(super) fn mesh_toggle_all(&mut self) {
        let Some(model) = &self.model else {
            return;
        };
        if self.ed.face_select {
            if self.ed.mesh_faces.len() >= model.faces.len() {
                self.ed.mesh_faces.clear();
            } else {
                self.ed.mesh_faces = model.faces.iter().map(|f| f.offset).collect();
            }
            self.sync_face_vertices();
        } else {
            let n = model.vertices.len();
            if self.mesh_vertices.len() >= n {
                self.mesh_vertices.clear();
            } else {
                self.mesh_vertices = (0..n).collect();
            }
        }
    }
    /// Edit-mode letter keys; true when handled.
    pub(super) fn edit_key(&mut self, ch: char, shift: bool) -> bool {
        match ch {
            '1' => self.select_mode(false),
            '3' => self.select_mode(true),
            'b' | 'B' => self.mesh_op(OP_BOX),
            'l' | 'L' => {
                let [x, y] = self.mouse;
                self.select_part_at(x, y);
            }
            'x' | 'X' => self.mesh_op(OP_DELETE),
            'f' | 'F' => self.mesh_op(OP_FACE),
            'D' => self.mesh_op(OP_DUPLICATE),
            'd' if shift => self.mesh_op(OP_DUPLICATE),
            'e' | 'E' => self.mesh_op(OP_EXTRUDE),
            _ => return false,
        }
        true
    }
    /// Alt+letter from the backends: Alt+N flips normals in Edit Mesh,
    /// Alt+Z toggles X-ray.
    pub fn alt_key(&mut self, ch: char) {
        if self.mode != Mode::Model || self.prompt.is_some() {
            return;
        }
        if self.mesh_edit && ch.eq_ignore_ascii_case(&'n') {
            self.mesh_op(OP_FLIP);
        } else if ch.eq_ignore_ascii_case(&'z') {
            self.toggle_xray();
        }
    }
    /// The face under the pointer: the raster's face buffer when shaded,
    /// else the nearest projected polygon containing the point.
    pub(super) fn pick_face(&self, x: i32, y: i32) -> Option<usize> {
        if self.textured {
            return self.model_hit(x, y).map(|(f, _)| f);
        }
        let model = self.model.as_ref()?;
        let mut best: Option<(i64, usize)> = None;
        for (fi, f) in model.faces.iter().enumerate() {
            let poly: Option<Vec<[i32; 2]>> = f
                .indices
                .iter()
                .map(|i| self.hp_project(model.vertices.get(*i)?.point))
                .collect();
            let Some(poly) = poly else { continue };
            if poly.len() < 3 || !inside([x, y], &poly) {
                continue;
            }
            let depth = f
                .indices
                .iter()
                .map(|i| self.camera_point(model.vertices[*i].point)[2] as i64)
                .sum::<i64>()
                / f.indices.len() as i64;
            if best.is_none_or(|(d, _)| depth > d) {
                best = Some((depth, fi));
            }
        }
        best.map(|(_, f)| f)
    }
    /// The vertex handle nearest the pointer (`pick_handle`).
    fn pick_vertex(&self, x: i32, y: i32) -> Option<usize> {
        self.pick_handle(x, y)
    }
    /// Left press in the viewport off any vertex marker: a click selects,
    /// a drag (or B) draws a box.
    pub(super) fn edit_press(&mut self, x: i32, y: i32, shift: bool) {
        self.ed.mesh_box = Some(BoxSelect {
            start: [x, y],
            end: [x, y],
            extend: shift,
            subtract: self.ctrl,
            boxing: self.ed.box_armed,
        });
        self.ed.box_armed = false;
    }
    pub(super) fn box_motion(&mut self, x: i32, y: i32) {
        if let Some(b) = self.ed.mesh_box.as_mut() {
            b.end = [x, y];
            if (x - b.start[0]).abs().max((y - b.start[1]).abs()) >= BOX_THRESHOLD {
                b.boxing = true;
            }
        }
    }
    pub(super) fn finish_box(&mut self) {
        let Some(b) = self.ed.mesh_box.take() else {
            return;
        };
        self.ed.mesh_refusal = None;
        let Some(model) = self.model.as_ref() else {
            return;
        };
        if !b.boxing {
            // A click: one face, or clear on empty space.
            let hit = if self.ed.face_select {
                self.pick_face(b.start[0], b.start[1])
                    .and_then(|f| model.faces.get(f))
                    .map(|f| f.offset)
            } else {
                None
            };
            match (hit, b.extend) {
                (Some(o), true) => {
                    if let Some(at) = self.ed.mesh_faces.iter().position(|x| *x == o) {
                        self.ed.mesh_faces.remove(at);
                    } else {
                        self.ed.mesh_faces.push(o);
                    }
                }
                (Some(o), false) => self.ed.mesh_faces = vec![o],
                (None, true) => {}
                (None, false) => {
                    self.ed.mesh_faces.clear();
                    self.mesh_vertices.clear();
                }
            }
            if self.ed.face_select {
                self.sync_face_vertices();
            }
            return;
        }
        let (x0, x1) = (b.start[0].min(b.end[0]), b.start[0].max(b.end[0]));
        let (y0, y1) = (b.start[1].min(b.end[1]), b.start[1].max(b.end[1]));
        let hit =
            |p: Option<[i32; 2]>| p.is_some_and(|[x, y]| x >= x0 && x <= x1 && y >= y0 && y <= y1);
        // Without X-ray the shaded view boxes only what it shows.
        let shown = self.occlusion().map(|(_, faces)| faces);
        if self.ed.face_select {
            let found: Vec<usize> = model
                .faces
                .iter()
                .enumerate()
                .filter(|(i, _)| shown.as_ref().is_none_or(|s| s.get(*i) == Some(&true)))
                .map(|(_, f)| f)
                .filter(|f| !f.indices.is_empty())
                .filter(|f| {
                    let n = f.indices.len() as i64;
                    let c: [i32; 3] = core::array::from_fn(|k| {
                        (f.indices
                            .iter()
                            .map(|i| model.vertices[*i].point[k] as i64)
                            .sum::<i64>()
                            / n) as i32
                    });
                    hit(self.hp_project(c))
                })
                .map(|f| f.offset)
                .collect();
            if !b.extend && !b.subtract {
                self.ed.mesh_faces.clear();
            }
            for o in found {
                let at = self.ed.mesh_faces.iter().position(|x| *x == o);
                match (b.subtract, at) {
                    (true, Some(at)) => {
                        self.ed.mesh_faces.remove(at);
                    }
                    (false, None) => self.ed.mesh_faces.push(o),
                    _ => {}
                }
            }
            self.sync_face_vertices();
        } else {
            let found: Vec<usize> = self
                .vertex_handles()
                .into_iter()
                .filter(|h| h.visible && hit(Some(h.at)))
                .map(|h| h.index)
                .collect();
            if !b.extend && !b.subtract {
                self.mesh_vertices.clear();
            }
            for i in found {
                let at = self.mesh_vertices.iter().position(|x| *x == i);
                match (b.subtract, at) {
                    (true, Some(at)) => {
                        self.mesh_vertices.remove(at);
                    }
                    (false, None) => self.mesh_vertices.push(i),
                    _ => {}
                }
            }
        }
        self.status = format!("{} selected", self.selection_text());
    }
    /// "3 of 286 faces".
    pub(super) fn selection_text(&self) -> String {
        let Some(model) = &self.model else {
            return String::new();
        };
        if self.ed.face_select {
            format!(
                "{} of {}",
                self.ed.mesh_faces.len(),
                view::count(model.faces.len(), "face", "faces")
            )
        } else {
            format!(
                "{} of {}",
                self.mesh_vertices.len(),
                view::count(model.vertices.len(), "vertex", "vertices")
            )
        }
    }
    /// Select every face of the group `g` (None: the root frame).
    fn select_group(&mut self, g: Option<usize>) {
        let Some(model) = &self.model else {
            return;
        };
        let faces: Vec<usize> = model
            .faces
            .iter()
            .filter(|f| within(model, f.group, g))
            .map(|f| f.offset)
            .collect();
        let name = self.group_name(g);
        let n = faces.len();
        self.ed.mesh_faces = faces;
        self.sync_face_vertices();
        self.status = format!("Selected {name}: {}", view::count(n, "face", "faces"));
    }
    /// L: select the part under the pointer.
    pub(super) fn select_part_at(&mut self, x: i32, y: i32) {
        let Some(model) = &self.model else {
            return;
        };
        let group = if self.ed.face_select {
            self.pick_face(x, y).map(|f| model.faces[f].group)
        } else {
            self.pick_vertex(x, y)
                .map(|i| model.vertex_tags.get(i).and_then(|t| t.group))
        };
        match group {
            Some(g) => self.select_group(g),
            None => self.status = "Point at a face or vertex, then press L".into(),
        }
    }
    /// Name of the part drawing group `g`: the stub-driven part whose call
    /// record it is (or encloses it), else the C4 offset, else the body.
    pub(super) fn group_name(&self, mut g: Option<usize>) -> String {
        let Some(model) = &self.model else {
            return String::new();
        };
        for _ in 0..64 {
            let Some(group) = g.and_then(|i| model.groups.get(i)) else {
                break;
            };
            if let Some(p) = self.ed.parts.iter().find(|p| p.id.target == group.offset) {
                return p.name.clone();
            }
            if matches!(group.opcode, 0xc4 | 0xc6) {
                return format!("Static part {:X}", group.offset);
            }
            g = group.parent;
        }
        "Body (root frame)".into()
    }
    /// The SH the edit writes and its bytes.
    fn edit_source(&self) -> Result<(usize, Vec<u8>)> {
        if let Some(reason) = self.mesh_blocked() {
            return Err(reason);
        }
        let entry = self
            .model_entry
            .ok_or("Select the SH entry or its owner to edit geometry")?;
        Ok((entry, self.doc.archive.entries[entry].read()?))
    }
    /// Commit the moved vertices of `after` against `before` as one undo
    /// step: static shapes through their full writer, every other shape
    /// through the region writer. Returns the number of stored vertices moved.
    pub(super) fn commit_points(&mut self, before: &Model, after: &Model) -> Result<usize> {
        let (entry, source) = self.edit_source()?;
        let moved: Vec<(usize, [i32; 3])> = (0..before.vertices.len())
            .filter_map(|i| {
                let p = after.vertices.get(i)?.point;
                (p != before.vertices[i].point).then_some((i, p))
            })
            .collect();
        let mut offsets: Vec<usize> = moved
            .iter()
            .map(|(i, _)| before.vertices[*i].offset)
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        if moved.is_empty() {
            return Ok(0);
        }
        let bytes = if before.writable {
            after.write(&source)?
        } else {
            geo::write_model_points(&source, before, &moved)?
        };
        if bytes != source {
            self.doc.replace(entry, bytes)?;
            self.refresh();
        }
        Ok(offsets.len())
    }
    /// Edit-mode G/R/S preview: about the median, or with individual origins
    /// each connected island of selected faces about its own centre.
    pub(super) fn edit_transform(&self, m: &Model, op: char, t: Transform) -> Result<Model> {
        if self.mesh_vertices.is_empty() {
            return Err("Select vertices or faces first".into());
        }
        if !(self.ed.pivot_individual && self.ed.face_select && op != 'g') {
            let pivot = m
                .median(&self.mesh_vertices)
                .ok_or("Select vertices first")?;
            return m.transform_selection(t, Some(&self.mesh_vertices), pivot);
        }
        // Islands: selected faces joined through shared stored vertices.
        let faces = self.selected_faces();
        let mut island: Vec<usize> = (0..faces.len()).collect();
        let offsets = |f: usize| -> Vec<usize> {
            m.faces[f]
                .indices
                .iter()
                .map(|i| m.vertices[*i].offset)
                .collect()
        };
        for a in 0..faces.len() {
            for b in a + 1..faces.len() {
                let (oa, ob) = (offsets(faces[a]), offsets(faces[b]));
                if oa.iter().any(|o| ob.contains(o)) {
                    let (from, to) = (island[b], island[a]);
                    for x in island.iter_mut() {
                        if *x == from {
                            *x = to;
                        }
                    }
                }
            }
        }
        let mut out = m.clone();
        let mut roots: Vec<usize> = island.clone();
        roots.sort_unstable();
        roots.dedup();
        for r in roots {
            let mut verts = Vec::new();
            for (_, f) in faces.iter().enumerate().filter(|(k, _)| island[*k] == r) {
                for i in &m.faces[*f].indices {
                    if !verts.contains(i) {
                        verts.push(*i);
                    }
                }
            }
            let pivot = m.median(&verts).ok_or("Empty island")?;
            out = out.transform_selection(t, Some(&verts), pivot)?;
        }
        Ok(out)
    }
    /// Face offsets of the operation's faces, refused when empty or in a
    /// part the shown pose rotates.
    fn op_faces(&self, verb: &str) -> Result<Vec<usize>> {
        let model = self.model.as_ref().ok_or("No model")?;
        let faces = self.selected_faces();
        if faces.is_empty() {
            return Err(format!(
                "Select faces to {verb} (3 switches to face select; in vertex select every corner must be selected)"
            ));
        }
        Ok(faces.iter().map(|f| model.faces[*f].offset).collect())
    }
    fn check_unrotated(&self, faces: &[usize]) -> Result<()> {
        let model = self.model.as_ref().ok_or("No model")?;
        for f in model.faces.iter().filter(|f| faces.contains(&f.offset)) {
            if f.indices.iter().any(|i| !geo::unrotated(model, *i)) {
                return Err(
                    "A selected face is in a part the preview pose rotates; reset the pose (Parts) to edit it"
                        .into(),
                );
            }
        }
        Ok(())
    }
    /// Run one Edit Mesh operation; failures keep core's reason.
    pub(super) fn mesh_op(&mut self, op: u8) {
        let result = self.mesh_op_inner(op);
        match result {
            Ok(()) => {
                if !matches!(op, OP_BOX | OP_FACE_COLOR | OP_NEIGHBOUR) {
                    self.ed.mesh_refusal = None;
                }
            }
            Err(e) => {
                self.status = format!("Error: {e}");
                self.ed.mesh_refusal = Some(e);
            }
        }
    }
    fn mesh_op_inner(&mut self, op: u8) -> Result<()> {
        match op {
            OP_BOX => {
                self.ed.box_armed = true;
                self.status = "Box select: drag a rectangle; Shift extends, Ctrl subtracts".into();
                return Ok(());
            }
            OP_LINKED => {
                let model = self.model.as_ref().ok_or("No model")?;
                let groups: Vec<Option<usize>> = self
                    .selected_faces()
                    .iter()
                    .map(|f| model.faces[*f].group)
                    .collect();
                if groups.is_empty() {
                    return Err("Select a face or its vertices first".into());
                }
                let faces: Vec<usize> = model
                    .faces
                    .iter()
                    .filter(|f| groups.iter().any(|g| within(model, f.group, *g)))
                    .map(|f| f.offset)
                    .collect();
                self.ed.mesh_faces = faces;
                self.sync_face_vertices();
                self.status = format!("Linked parts: {}", self.selection_text());
                return Ok(());
            }
            OP_INVERT => {
                let model = self.model.as_ref().ok_or("No model")?;
                if self.ed.face_select {
                    self.ed.mesh_faces = model
                        .faces
                        .iter()
                        .map(|f| f.offset)
                        .filter(|o| !self.ed.mesh_faces.contains(o))
                        .collect();
                    self.sync_face_vertices();
                } else {
                    self.mesh_vertices = (0..model.vertices.len())
                        .filter(|i| !self.mesh_vertices.contains(i))
                        .collect();
                }
                return Ok(());
            }
            OP_NEIGHBOUR => {
                self.ed.new_face_color = None;
                self.status = "New faces copy a neighbouring face's colour".into();
                return Ok(());
            }
            OP_FACE_COLOR => {
                let color = self.ed.new_face_color.unwrap_or(self.brush);
                self.base_color_from = color;
                self.brush = color;
                self.prompt = Some(Prompt {
                    kind: PromptKind::FaceColor,
                    title: "Flat colour for new faces".into(),
                    value: String::new(),
                    axis: 0,
                });
                return Ok(());
            }
            _ => {}
        }
        let (entry, source) = self.edit_source()?;
        match op {
            OP_DELETE => {
                let faces = self.op_faces("delete")?;
                let bytes = geo::delete_faces(&source, &faces)?;
                self.doc.replace(entry, bytes)?;
                self.ed.mesh_faces.clear();
                self.refresh();
                self.status = format!(
                    "Deleted {} in place with same-size jumps. One undo step.",
                    view::count(faces.len(), "face", "faces")
                );
            }
            OP_FLIP => {
                let faces = self.op_faces("flip")?;
                let bytes = geo::flip_faces(&source, &faces)?;
                self.doc.replace(entry, bytes)?;
                self.refresh();
                self.status = format!(
                    "Flipped {} in place. One undo step.",
                    view::count(faces.len(), "normal", "normals")
                );
            }
            OP_FACE => {
                let (corners, style) = self.new_face_plan()?;
                let added = geo::add_face(&source, &corners, &style)?;
                self.doc.replace(entry, added.shape)?;
                self.refresh();
                self.ed.mesh_faces = added.faces.clone();
                self.status = format!(
                    "Added a {}-corner face, drawn after the face at {:X}. One undo step.",
                    corners.len(),
                    added.host.unwrap_or(0)
                );
            }
            OP_DUPLICATE | OP_EXTRUDE => {
                let faces = self.op_faces(if op == OP_EXTRUDE {
                    "extrude"
                } else {
                    "duplicate"
                })?;
                self.check_unrotated(&faces)?;
                self.ed.mesh_pending = Some(Pending {
                    extrude: op == OP_EXTRUDE,
                    faces,
                });
                self.transform_prompt('g');
            }
            OP_VERTEX => {
                let model = self.model.as_ref().ok_or("No model")?;
                let median = model
                    .median(&self.mesh_vertices)
                    .ok_or("Select vertices or faces first")?;
                // Host: a face using a selected vertex, in an unrotated frame.
                let host = model
                    .faces
                    .iter()
                    .find(|f| {
                        f.indices.iter().any(|i| self.mesh_vertices.contains(i))
                            && f.indices.iter().all(|i| geo::unrotated(model, *i))
                    })
                    .ok_or("No face using the selection is drawn in an unrotated frame")?
                    .offset;
                let t = geo::frame_translation(model, host)?;
                let local: [i32; 3] = core::array::from_fn(|k| median[k] - t[k]);
                let added = geo::add_vertices(&source, host, &[local], &[])?;
                self.doc.replace(entry, added.shape)?;
                self.ed.face_select = false;
                self.refresh();
                if let Some(model) = &self.model {
                    self.mesh_vertices = model
                        .vertices
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| added.vertices.contains(&v.offset))
                        .map(|(i, _)| i)
                        .collect();
                }
                self.status = format!(
                    "Added a vertex at the median ({} {} {}), slot {}. One undo step.",
                    median[0],
                    median[1],
                    median[2],
                    added.slots.first().copied().unwrap_or(0)
                );
            }
            _ => {}
        }
        Ok(())
    }
    /// Corners (stored vertex offsets, wound to face the viewer) and style
    /// of a face over the selected vertices.
    fn new_face_plan(&self) -> Result<(Vec<usize>, FaceStyle)> {
        let model = self.model.as_ref().ok_or("No model")?;
        let mut picked: Vec<(usize, [i32; 3])> = Vec::new();
        for i in &self.mesh_vertices {
            let v = model.vertices.get(*i).ok_or("No selected vertex")?;
            if !picked.iter().any(|(o, _)| *o == v.offset) {
                picked.push((v.offset, v.point));
            }
        }
        if picked.len() < 3 {
            return Err("Select 3 or more vertices to make a face (vertex select, 1)".into());
        }
        // Order around the centre as seen in the view.
        let screen: Vec<[i64; 2]> = picked
            .iter()
            .map(|(_, p)| {
                let c = self.camera_point(*p);
                [c[0] as i64, c[1] as i64]
            })
            .collect();
        let n = screen.len() as i64;
        let c = [
            screen.iter().map(|p| p[0]).sum::<i64>() / n,
            screen.iter().map(|p| p[1]).sum::<i64>() / n,
        ];
        let half = |v: [i64; 2]| u8::from(!(v[1] > 0 || (v[1] == 0 && v[0] > 0)));
        let mut order: Vec<usize> = (0..picked.len()).collect();
        order.sort_unstable_by(|a, b| {
            let (u, v) = (
                [screen[*a][0] - c[0], screen[*a][1] - c[1]],
                [screen[*b][0] - c[0], screen[*b][1] - c[1]],
            );
            half(u)
                .cmp(&half(v))
                .then_with(|| 0.cmp(&(u[0] * v[1] - u[1] * v[0])))
        });
        let mut corners: Vec<usize> = order.iter().map(|k| picked[*k].0).collect();
        let points: Vec<[i32; 3]> = order.iter().map(|k| picked[*k].1).collect();
        let normal = model::face_normal(&points).ok_or("The selected vertices are collinear")?;
        if self.camera_point(normal)[2] < 0 {
            corners.reverse();
        }
        let style = match self.ed.new_face_color {
            Some(c) => FaceStyle::flat(c),
            None => model
                .faces
                .iter()
                .find(|f| {
                    f.indices
                        .iter()
                        .any(|i| corners.contains(&model.vertices[*i].offset))
                })
                .map(|f| {
                    if f.sub & 4 != 0 {
                        // A textured neighbour's UVs do not fit the new corners.
                        FaceStyle::flat(f.color)
                    } else {
                        FaceStyle {
                            content: if f.sub & 0x80 != 0 { 0x63 } else { f.sub },
                            color: f.color,
                            texture: None,
                            uv: Vec::new(),
                        }
                    }
                })
                .unwrap_or(FaceStyle::flat(self.brush)),
        };
        Ok((corners, style))
    }
    /// The model-space offset a duplicate or extrude move gives.
    fn pending_delta(t: Transform) -> Result<[i32; 3]> {
        match t {
            Transform::Translate(d) => Ok(d),
            Transform::Move(a, v) => {
                let mut d = [0; 3];
                d[a.min(2)] = v;
                Ok(d)
            }
            _ => Err("Duplicate and extrude take a move".into()),
        }
    }
    fn pending_run(&self, t: Transform) -> Result<Option<geo::Added>> {
        let p = self.ed.mesh_pending.as_ref().ok_or("Nothing pending")?;
        let delta = Self::pending_delta(t)?;
        let (_, source) = self.edit_source()?;
        if p.extrude {
            if delta == [0; 3] {
                return Ok(None);
            }
            geo::extrude_faces(&source, &p.faces, delta, geo::Base::Remove).map(Some)
        } else {
            geo::duplicate_faces(&source, &p.faces, delta).map(Some)
        }
    }
    pub(super) fn pending_preview(&mut self, t: Transform) {
        match self.pending_run(t) {
            Ok(Some(added)) => {
                self.preview = Model::with_pose(&added.shape, &self.ed.pose)
                    .ok()
                    .map(Box::new);
            }
            Ok(None) => {
                self.preview = None;
                self.status = "Type the extrude offset; X, Y or Z locks an axis".into();
            }
            Err(e) => {
                self.preview = None;
                self.status = format!("Error: {e}");
            }
        }
    }
    pub(super) fn apply_pending(&mut self, t: Transform) -> Result<()> {
        let added = self
            .pending_run(t)?
            .ok_or("Extrude needs a non-zero offset")?;
        let p = self.ed.mesh_pending.take().ok_or("Nothing pending")?;
        let (entry, _) = self.edit_source()?;
        self.doc.replace(entry, added.shape)?;
        self.preview = None;
        self.ed.face_select = true;
        self.refresh();
        // Select the moved copies (for an extrude, the new caps).
        if let Some(model) = &self.model {
            self.ed.mesh_faces = model
                .faces
                .iter()
                .filter(|f| added.faces.contains(&f.offset))
                .filter(|f| {
                    !p.extrude
                        || f.indices
                            .iter()
                            .all(|i| added.vertices.contains(&model.vertices[*i].offset))
                })
                .map(|f| f.offset)
                .collect();
        }
        self.sync_face_vertices();
        self.status = format!(
            "{} {}: {}, {}. One undo step.",
            if p.extrude { "Extruded" } else { "Duplicated" },
            view::count(p.faces.len(), "face", "faces"),
            view::count(added.faces.len(), "new face", "new faces"),
            view::count(added.vertices.len(), "new vertex", "new vertices")
        );
        Ok(())
    }
    /// Amber edges of the selected faces in wireframe (the raster fills them).
    pub(super) fn face_edges(&self, o: &mut Layout) {
        let Some(model) = self.preview.as_deref().or(self.model.as_ref()) else {
            return;
        };
        if self.textured {
            return;
        }
        let view = [self.left() + 1, 54, self.right() - 2, self.dock_y() - 1];
        for f in self.selected_faces().iter().take(4096) {
            let face = &model.faces[*f];
            let pts: Vec<Option<[i32; 2]>> = face
                .indices
                .iter()
                .map(|i| self.hp_project(model.vertices.get(*i)?.point))
                .collect();
            for k in 0..pts.len() {
                if let (Some(a), Some(b)) = (pts[k], pts[(k + 1) % pts.len()]) {
                    if let Some((a, b)) = super::clip(a, b, view) {
                        o.canvas.line(a[0], a[1], b[0], b[1], c::AMBER);
                    }
                }
            }
        }
    }
    /// Edit Mesh viewport overlay: vertex markers (vertex select) or face
    /// dots (face select), selected face edges, the box.
    pub(super) fn mesh_overlay(&self, o: &mut Layout) {
        let Some(model) = self.preview.as_deref().or(self.model.as_ref()) else {
            return;
        };
        let visible = |[x, y]: [i32; 2]| {
            x >= self.left() + 40 && x < self.right() - 8 && y >= 60 && y < self.dock_y() - 10
        };
        self.face_edges(o);
        if self.ed.face_select {
            for f in &model.faces {
                if f.indices.is_empty() {
                    continue;
                }
                let n = f.indices.len() as i64;
                let centre: [i32; 3] = core::array::from_fn(|k| {
                    (f.indices
                        .iter()
                        .map(|i| model.vertices[*i].point[k] as i64)
                        .sum::<i64>()
                        / n) as i32
                });
                let Some(p) = self.hp_project(centre).filter(|p| visible(*p)) else {
                    continue;
                };
                if self.ed.mesh_faces.contains(&f.offset) {
                    o.canvas.rect(p[0] - 1, p[1] - 1, 3, 3, c::AMBER);
                } else {
                    o.canvas.rect(p[0], p[1], 2, 2, c::INK_MUTED);
                }
            }
        } else {
            self.draw_vertex_handles(o);
        }
        if let Some(b) = self.ed.mesh_box.filter(|b| b.boxing) {
            let (x0, y0) = (b.start[0].min(b.end[0]), b.start[1].min(b.end[1]));
            let (w, h) = (
                (b.end[0] - b.start[0]).abs() + 1,
                (b.end[1] - b.start[1]).abs() + 1,
            );
            view::border(&mut o.canvas, x0, y0, w, h, c::AMBER);
        }
        self.gizmo_overlay(o);
    }
    /// Edit Mesh inspector: selection, operations, refusals.
    pub(super) fn mesh_inspector(&self, o: &mut Layout) {
        self.inspector_header(o, None);
        let foot = m::BUTTON_H + 2 * space::SPACE_2;
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, foot);
        let Some(model) = &self.model else {
            o.stack_end(s);
            return;
        };
        if self.pane(o, &mut s, pane::MESH_SELECTION, "Selection", Icon::Select) {
            if let Some(rect) = o.prop(&mut s, "Mode") {
                o.segmented(
                    rect,
                    &[
                        (
                            Btn::new("Vertex")
                                .with_icon(Icon::Vertex)
                                .on(!self.ed.face_select),
                            Action::SelectMode(false),
                        ),
                        (
                            Btn::new("Face")
                                .with_icon(Icon::Face)
                                .on(self.ed.face_select),
                            Action::SelectMode(true),
                        ),
                    ],
                );
            }
            o.info(&mut s, "Selected", &self.selection_text(), "");
            let group = if self.ed.face_select {
                model
                    .faces
                    .iter()
                    .find(|f| self.ed.mesh_faces.first() == Some(&f.offset))
                    .map(|f| (f.group, f.offset))
            } else {
                self.mesh_vertices.first().and_then(|i| {
                    Some((
                        model.vertex_tags.get(*i)?.group,
                        model.vertices.get(*i)?.offset,
                    ))
                })
            };
            if let Some((g, offset)) = group {
                o.info(&mut s, "Part", &self.group_name(g), "");
                o.info(&mut s, "Source", &format!("+0x{offset:X}"), "");
            }
            if let Some(v) = (!self.ed.face_select)
                .then(|| self.mesh_vertices.first())
                .flatten()
                .and_then(|i| self.preview.as_deref().unwrap_or(model).vertices.get(*i))
            {
                o.info(
                    &mut s,
                    "Position",
                    &format!("{} {} {}", v.point[0], v.point[1], v.point[2]),
                    "",
                );
            }
            if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                let half = (rect[2] - space::SPACE_1) / 2;
                o.button_ex(
                    [rect[0], rect[1], half, rect[3]],
                    Btn::new("Select part"),
                    Action::MeshOp(OP_LINKED),
                );
                o.button_ex(
                    [rect[0] + half + space::SPACE_1, rect[1], half, rect[3]],
                    Btn::new("Box select"),
                    Action::MeshOp(OP_BOX),
                );
            }
        }
        o.panel_end(&mut s);
        let faces = !self.selected_faces().is_empty();
        if self.pane(o, &mut s, pane::MESH_OPS, "Mesh", Icon::Shape) {
            if let Some(rect) = o.prop(&mut s, "Pivot") {
                o.segmented(
                    rect,
                    &[
                        (
                            Btn::new("Median").on(!self.ed.pivot_individual),
                            Action::Pivot(false),
                        ),
                        (
                            Btn::new("Individual").on(self.ed.pivot_individual),
                            Action::Pivot(true),
                        ),
                    ],
                );
            }
            let ops: [(&str, u8, bool); 6] = [
                ("Delete faces", OP_DELETE, faces),
                ("Flip normals", OP_FLIP, faces),
                ("Duplicate", OP_DUPLICATE, faces),
                ("Extrude", OP_EXTRUDE, faces),
                (
                    "Make face",
                    OP_FACE,
                    !self.ed.face_select && self.mesh_vertices.len() >= 3,
                ),
                ("Add vertex", OP_VERTEX, !self.mesh_vertices.is_empty()),
            ];
            for pair in ops.chunks(2) {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    let half = (rect[2] - space::SPACE_1) / 2;
                    for (k, (label, op, enabled)) in pair.iter().enumerate() {
                        let b = Btn::new(label).enabled(*enabled);
                        let b = if *op == OP_DELETE { b.danger() } else { b };
                        o.button_ex(
                            [
                                rect[0] + k as i32 * (half + space::SPACE_1),
                                rect[1],
                                half,
                                rect[3],
                            ],
                            b,
                            Action::MeshOp(*op),
                        );
                    }
                }
            }
            o.stack_subhead(&mut s, "New faces");
            if let Some(rect) = o.wide(&mut s, m::ROW_H) {
                o.checkbox_row(
                    rect,
                    None,
                    "Copy a neighbour",
                    "",
                    if self.ed.new_face_color.is_none() {
                        Check::On
                    } else {
                        Check::Off
                    },
                    Action::MeshOp(OP_NEIGHBOUR),
                );
            }
            if let Some(rect) = o.prop(&mut s, "Flat colour") {
                let [x, y, w, h] = rect;
                if let Some(color) = self.ed.new_face_color {
                    let rgb = self.base_palette[color as usize];
                    o.canvas.rect(
                        x,
                        y + 2,
                        h - 4,
                        h - 4,
                        theme::Rgb((rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32),
                    );
                }
                let label = match self.ed.new_face_color {
                    Some(c) => format!("Index {c}"),
                    None => "Choose".into(),
                };
                o.select(
                    [x + h, y, w - h, h],
                    Some(Icon::Palette),
                    &label,
                    Action::MeshOp(OP_FACE_COLOR),
                    false,
                );
            }
        }
        o.panel_end(&mut s);
        self.face_texture_pane(o, &mut s, false);
        self.markings_pane(o, &mut s);
        if let Some(why) = &self.ed.mesh_refusal {
            o.stack_notice(&mut s, Tone::Warn, why);
        } else if !model.writable {
            o.stack_notice(
                &mut s,
                Tone::Neutral,
                "Region edits: each face and vertex is checked before it changes; a refused operation names the reason.",
            );
        }
        o.stack_end(s);
        let r = self.right();
        o.button_ex(
            [
                r + 1 + space::SPACE_2,
                self.height - m::STATUSBAR_H - space::SPACE_2 - m::BUTTON_H,
                self.width - r - 1 - 2 * space::SPACE_2,
                m::BUTTON_H,
            ],
            Btn::new("Object mode").with_icon(Icon::Select),
            Action::MeshMode,
        );
    }
}

#[inline(never)]
fn edit_app() -> Box<App> {
    Box::new(App::new())
}
/// The demo LIB with the synthetic parts aircraft as PARTS.SH, selected in
/// Edit Mesh through the mode Select's hit regions.
#[inline(never)]
pub(super) fn parts_app() -> Box<App> {
    let mut a = edit_app();
    a.demo();
    a.doc
        .transaction(
            vec![Entry::new("PARTS.SH", hangar_core::shape_testkit::demo_parts()).unwrap()],
            &[],
        )
        .unwrap();
    a.doc.mark_saved();
    a.select_entry(a.doc.archive.find("PARTS.SH").unwrap());
    a.mode = Mode::Model;
    a.textured = false;
    a.yaw = 35;
    a.pitch = 20;
    a.pose_preset(0);
    a
}
impl App {
    pub(super) fn smoke_press(&mut self, x: i32, y: i32, shift: bool) {
        self.motion(x, y, shift);
        self.pointer(x, y, 1, true, shift);
        self.pointer(x, y, 1, false, shift);
    }
    /// A control's hit rect, scrolling the right editor to it when needed.
    pub(super) fn smoke_find(&mut self, predicate: &dyn Fn(Action) -> bool) -> [i32; 4] {
        if let Some(r) = self.chrome_hit(predicate) {
            return r;
        }
        self.mouse = [self.right() + 20, 300];
        self.wheel(100);
        for _ in 0..80 {
            if let Some(r) = self.chrome_hit(predicate) {
                return r;
            }
            self.mouse = [self.right() + 20, 300];
            self.wheel(-1);
        }
        panic!("Control missing")
    }
    pub(super) fn smoke_keys(&mut self, keys: &str) {
        for ch in keys.chars() {
            self.key(Key::Char(ch), false, ch.is_ascii_uppercase());
        }
    }
    /// The projected centre of model face `f`.
    fn smoke_face_point(&self, f: usize) -> [i32; 2] {
        let m = self.model.as_ref().unwrap();
        let face = &m.faces[f];
        let n = face.indices.len() as i32;
        let c: [i32; 3] = core::array::from_fn(|k| {
            face.indices
                .iter()
                .map(|i| m.vertices[*i].point[k])
                .sum::<i32>()
                / n
        });
        self.hp_project(c).unwrap()
    }
    /// A face the pointer picks at its own centre, drawn by `group`.
    pub(super) fn smoke_pickable(&self, root: bool) -> (usize, [i32; 2]) {
        let m = self.model.as_ref().unwrap();
        (0..m.faces.len())
            .filter(|f| m.faces[*f].group.is_none() == root)
            .find_map(|f| {
                let p = self.smoke_face_point(f);
                (self.in_viewport(p[0], p[1]) && self.pick_face(p[0], p[1]) == Some(f))
                    .then_some((f, p))
            })
            .expect("A pickable face")
    }
    #[inline(never)]
    pub(super) fn smoke_edit_mode(&mut self) {
        let mut a = parts_app();
        let original = a.doc.archive.bytes().unwrap();
        let faces0 = a.model.as_ref().unwrap().faces.len();
        // Enter Edit Mesh through the mode Select.
        let select = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_MODE)));
        a.chrome_click(select);
        let item = a.smoke_find(&|x| matches!(x, Action::ViewportMode(1)));
        a.chrome_click(item);
        assert!(a.mesh_edit && !a.ed.face_select);
        // 3 and the header segmented control switch select modes.
        a.key(Key::Char('3'), false, false);
        assert!(a.ed.face_select);
        let vertex = a.smoke_find(&|x| matches!(x, Action::SelectMode(false)));
        a.chrome_click(vertex);
        assert!(!a.ed.face_select);
        a.key(Key::Char('3'), false, false);
        // Click selects one face, Shift+click toggles it, empty space clears.
        let (f, p) = a.smoke_pickable(true);
        let offset = a.model.as_ref().unwrap().faces[f].offset;
        a.smoke_press(p[0], p[1], false);
        assert_eq!(a.ed.mesh_faces, vec![offset]);
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Line(.., color) if *color == c::AMBER.0)));
        a.smoke_press(p[0], p[1], true);
        assert!(a.ed.mesh_faces.is_empty());
        a.smoke_press(p[0], p[1], false);
        let empty = [a.right() - 30, a.dock_y() - 30];
        assert!(a.pick_face(empty[0], empty[1]).is_none());
        a.smoke_press(empty[0], empty[1], false);
        assert!(a.ed.mesh_faces.is_empty());
        // A selects all faces, again none.
        a.key(Key::Char('a'), false, false);
        assert_eq!(a.ed.mesh_faces.len(), faces0);
        a.key(Key::Char('a'), false, false);
        assert!(a.ed.mesh_faces.is_empty());
        // Drag on empty space draws a box; B starts one anywhere.
        let (l, t) = (a.left() + 45, 60);
        a.motion(l, t, false);
        a.pointer(l, t, 1, true, false);
        a.motion(a.right() - 4, a.dock_y() - 4, false);
        assert!(a.ed.mesh_box.is_some_and(|b| b.boxing));
        a.pointer(a.right() - 4, a.dock_y() - 4, 1, false, false);
        let boxed = a.ed.mesh_faces.len();
        assert!(boxed > 0 && a.status.contains(" of "));
        a.key(Key::Char('b'), false, false);
        a.smoke_press(p[0], p[1], false);
        assert!(
            a.ed.mesh_faces.len() <= 1,
            "B then a click selects one face"
        );
        // L selects the part under the pointer; Select linked part widens.
        let (g, gp) = a.smoke_pickable(false);
        let group = a.model.as_ref().unwrap().faces[g].group;
        a.motion(gp[0], gp[1], false);
        a.key(Key::Char('l'), false, false);
        let part: Vec<usize> = {
            let m = a.model.as_ref().unwrap();
            m.faces
                .iter()
                .filter(|x| x.group == group)
                .map(|x| x.offset)
                .collect()
        };
        assert_eq!(a.ed.mesh_faces, part, "{}", a.status);
        assert!(a.status.starts_with("Selected "));
        // Delete through the Mesh menu: one undo step back to the exact bytes.
        a.smoke_press(p[0], p[1], false);
        let mesh = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_MESH)));
        a.chrome_click(mesh);
        let delete = a.smoke_find(&|x| matches!(x, Action::MeshOp(OP_DELETE)));
        a.chrome_click(delete);
        assert_eq!(
            a.model.as_ref().unwrap().faces.len(),
            faces0 - 1,
            "{}",
            a.status
        );
        assert_eq!(a.doc.changed_count(), 1);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // X deletes too; Alt+N flips.
        a.smoke_press(p[0], p[1], false);
        a.key(Key::Char('x'), false, false);
        assert_eq!(a.model.as_ref().unwrap().faces.len(), faces0 - 1);
        a.act(Action::Undo);
        a.smoke_press(p[0], p[1], false);
        let before = a.model.as_ref().unwrap().faces[f].normal;
        a.alt_key('n');
        let after = a.model.as_ref().unwrap().faces[f].normal;
        assert_eq!(after, before.map(|n| n.map(|v| -v)), "{}", a.status);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Extrude: E, Z lock, 5, Enter. One step, and the new caps selected.
        a.smoke_press(p[0], p[1], false);
        a.key(Key::Char('e'), false, false);
        assert!(a.prompt.is_some() && a.ed.mesh_pending.is_some());
        a.smoke_keys("z5");
        assert!(a.preview.is_some(), "{}", a.status);
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        assert!(a.model.as_ref().unwrap().faces.len() > faces0);
        assert_eq!(a.ed.mesh_faces.len(), 1, "The extruded cap is selected");
        assert_eq!(a.doc.changed_count(), 1);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Shift+D duplicates and moves; Esc cancels without an edit.
        a.smoke_press(p[0], p[1], false);
        a.key(Key::Char('D'), false, true);
        a.smoke_keys("x3");
        a.key(Key::Escape, false, false);
        assert!(!a.doc.dirty() && a.ed.mesh_pending.is_none() && a.preview.is_none());
        a.key(Key::Char('D'), false, true);
        a.smoke_keys("x3");
        a.key(Key::Enter, false, false);
        assert_eq!(
            a.model.as_ref().unwrap().faces.len(),
            faces0 + 1,
            "{}",
            a.status
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // S with individual origins: two separate faces each shrink about
        // their own centre, so both centres stay put.
        let m = a.model.as_ref().unwrap();
        let wings: Vec<usize> = (0..m.faces.len())
            .filter(|f| {
                let face = &m.faces[*f];
                face.group.is_none()
                    && face
                        .indices
                        .iter()
                        .all(|i| m.vertices[*i].point[2] == 0 && m.vertices[*i].point[0].abs() >= 5)
            })
            .collect();
        let (wl, wr) = (
            *wings
                .iter()
                .find(|f| m.vertices[m.faces[**f].indices[0]].point[0] < 0)
                .unwrap(),
            *wings
                .iter()
                .find(|f| m.vertices[m.faces[**f].indices[0]].point[0] > 0)
                .unwrap(),
        );
        let centre = |a: &App, f: usize| {
            let m = a.model.as_ref().unwrap();
            let face = &m.faces[f];
            let n = face.indices.len() as i32;
            core::array::from_fn::<i32, 3, _>(|k| {
                face.indices
                    .iter()
                    .map(|i| m.vertices[*i].point[k])
                    .sum::<i32>()
                    / n
            })
        };
        let (cl, cr) = (centre(&a, wl), centre(&a, wr));
        a.ed.mesh_faces = vec![
            a.model.as_ref().unwrap().faces[wl].offset,
            a.model.as_ref().unwrap().faces[wr].offset,
        ];
        a.sync_face_vertices();
        let individual = a.smoke_find(&|x| matches!(x, Action::Pivot(true)));
        a.chrome_click(individual);
        assert!(a.ed.pivot_individual);
        a.smoke_keys("s50");
        a.key(Key::Enter, false, false);
        assert!(a.prompt.is_none(), "{}", a.status);
        let (nl, nr) = (centre(&a, wl), centre(&a, wr));
        assert!((0..3).all(|k| (nl[k] - cl[k]).abs() <= 1 && (nr[k] - cr[k]).abs() <= 1));
        assert_eq!(a.doc.changed_count(), 1);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Median pivot: the same scale pulls both faces towards the middle.
        a.act(Action::Pivot(false));
        a.smoke_keys("s50");
        a.key(Key::Enter, false, false);
        assert!(centre(&a, wl)[0] > cl[0] + 4);
        a.act(Action::Undo);
        // F makes a face over three selected vertices (vertex select).
        a.key(Key::Char('1'), false, false);
        let roots: Vec<usize> = {
            let m = a.model.as_ref().unwrap();
            (0..m.vertices.len())
                .filter(|i| m.vertex_tags[*i].group.is_none())
                .take(8)
                .collect()
        };
        a.mesh_vertices = vec![roots[0], roots[3], roots[5]];
        a.key(Key::Char('f'), false, false);
        assert_eq!(
            a.model.as_ref().unwrap().faces.len(),
            faces0 + 1,
            "{}",
            a.status
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // A flat colour for new faces from the colour dialog, then F uses it.
        let colour = a.smoke_find(&|x| matches!(x, Action::MeshOp(OP_FACE_COLOR)));
        a.chrome_click(colour);
        assert!(matches!(
            a.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::FaceColor)
        ));
        let swatch = a.smoke_find(&|x| matches!(x, Action::Brush(77)));
        a.chrome_click(swatch);
        a.key(Key::Enter, false, false);
        assert_eq!(a.ed.new_face_color, Some(77));
        a.mesh_vertices = vec![roots[0], roots[3], roots[5]];
        a.key(Key::Char('f'), false, false);
        let made = a.ed.mesh_faces[0];
        let m = a.model.as_ref().unwrap();
        assert_eq!(m.faces.iter().find(|f| f.offset == made).unwrap().color, 77);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let neighbour = a.smoke_find(&|x| matches!(x, Action::MeshOp(OP_NEIGHBOUR)));
        a.chrome_click(neighbour);
        assert!(a.ed.new_face_color.is_none());
        // Add vertex at the median, one undo step; the new vertex is selected.
        a.mesh_vertices = vec![roots[0], roots[3]];
        let n0 = a.model.as_ref().unwrap().vertices.len();
        let add = a.smoke_find(&|x| matches!(x, Action::MeshOp(OP_VERTEX)));
        a.chrome_click(add);
        assert_eq!(
            a.model.as_ref().unwrap().vertices.len(),
            n0 + 1,
            "{}",
            a.status
        );
        assert_eq!(a.mesh_vertices.len(), 1);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Header menus open inside the window at both sizes.
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            for menu in [chrome::MENU_SELECT, chrome::MENU_MESH] {
                let name = a.smoke_find(&|x| matches!(x, Action::Menu(n) if n == menu));
                a.chrome_click(name);
                assert_eq!(a.menu, Some(menu));
                let [mx, my, mw, mh] = a.open_menu_rect().unwrap();
                assert!(mx >= 0 && my >= 0 && mx + mw <= w && my + mh <= h);
                a.key(Key::Escape, false, false);
            }
        }
        let select = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_SELECT)));
        a.chrome_click(select);
        let face = a.smoke_find(&|x| matches!(x, Action::SelectMode(true)));
        a.chrome_click(face);
        assert!(a.ed.face_select && a.menu.is_none());
        a.key(Key::Char('1'), false, false);
        // Refusals keep core's reason: two vertices, and a part the pose rotates.
        a.mesh_vertices = vec![roots[0], roots[1]];
        a.key(Key::Char('f'), false, false);
        assert!(a
            .ed
            .mesh_refusal
            .as_deref()
            .is_some_and(|r| r.contains("3 or more")));
        assert!(a.draw().commands.iter().any(|d| matches!(d,
            Draw::Text(_, _, s, _, _) if s.starts_with("Select 3"))));
        a.ed.pose.insert("_PLgearDown".into(), 1);
        a.ed.pose.insert("_PLgearPos".into(), -8192);
        a.refresh();
        a.key(Key::Char('3'), false, false);
        a.pick_part(0);
        assert!(!a.ed.mesh_faces.is_empty());
        a.key(Key::Char('e'), false, false);
        assert!(a
            .ed
            .mesh_refusal
            .as_deref()
            .is_some_and(|r| r.contains("rotates")));
        a.smoke_keys("g4");
        a.key(Key::Enter, false, false);
        assert!(a.status.contains("rotated part"), "{}", a.status);
        a.key(Key::Escape, false, false);
        assert!(!a.doc.dirty());
        // The raster's face buffer picks faces too.
        a.ed.pose.clear();
        a.refresh();
        a.textured = true;
        let x = (a.left() + a.right()) / 2;
        let y = (54 + a.dock_y()) / 2;
        if let Some(f) = a.pick_face(x, y) {
            a.smoke_press(x, y, false);
            assert_eq!(
                a.ed.mesh_faces,
                vec![a.model.as_ref().unwrap().faces[f].offset]
            );
        }
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
    }
}

#[cfg(not(windows))]
impl App {
    /// Manual real-data check (`--edit-check`): Edit Mesh operations and part
    /// settings on the selected SH through the app's own paths, each one undo
    /// step back to the exact bytes. Edits stay in memory.
    pub fn check_edit(&mut self) -> Result<String> {
        let mut out = String::new();
        let entry = self.model_entry.ok_or("Select an SH entry")?;
        let original = self.doc.archive.entries[entry].read()?;
        let name = self.doc.archive.entries[entry].name.clone();
        self.mode = Mode::Model;
        self.textured = false;
        self.perspective = false;
        self.pose_preset(0);
        self.act(Action::MeshMode);
        self.select_mode(true);
        let restored = |a: &mut App, what: &str, out: &mut String| -> Result<()> {
            let done = a.status.clone();
            a.act(Action::Undo);
            if a.doc.archive.entries[entry].read()? != original {
                return Err(format!("{what}: undo did not restore {name}"));
            }
            *out += &format!("PASS {what}: {done}; undo restores the bytes\n");
            Ok(())
        };
        let model = self.model.as_ref().ok_or("No model")?;
        let root: Vec<usize> = (0..model.faces.len())
            .filter(|f| model.faces[*f].group.is_none() && model.faces[*f].sub & 0x80 == 0)
            .collect();
        let pick = *root.get(root.len() / 2).ok_or("No root face")?;
        let offset = model.faces[pick].offset;
        let ops: [(&str, u8, &str); 4] = [
            ("delete", OP_DELETE, ""),
            ("flip", OP_FLIP, ""),
            ("extrude", OP_EXTRUDE, "z2"),
            ("duplicate", OP_DUPLICATE, "x3"),
        ];
        for (label, op, keys) in ops {
            self.ed.mesh_faces = vec![offset];
            self.sync_face_vertices();
            self.mesh_op(op);
            if !keys.is_empty() {
                for ch in keys.chars() {
                    self.key(Key::Char(ch), false, false);
                }
                self.key(Key::Enter, false, false);
            }
            if !self.doc.dirty() {
                out.push_str(&format!("REFUSED {label}: {}\n", self.status));
                self.key(Key::Escape, false, false);
                continue;
            }
            restored(self, label, &mut out)?;
        }
        // G and S (individual origins) on the face's corners.
        for (label, keys, individual) in [("move", "g3", false), ("scale", "s80", true)] {
            self.ed.mesh_faces = vec![offset];
            self.sync_face_vertices();
            self.ed.pivot_individual = individual;
            for ch in keys.chars() {
                self.key(Key::Char(ch), false, false);
            }
            self.key(Key::Enter, false, false);
            if self.prompt.is_some() || !self.doc.dirty() {
                out.push_str(&format!("REFUSED {label}: {}\n", self.status));
                self.key(Key::Escape, false, false);
                continue;
            }
            restored(self, label, &mut out)?;
        }
        // Make a face over three corners of the picked face, and add a vertex.
        self.select_mode(false);
        let corners: Vec<usize> = self.model.as_ref().ok_or("No model")?.faces[self
            .model
            .as_ref()
            .ok_or("No model")?
            .faces
            .iter()
            .position(|f| f.offset == offset)
            .ok_or("Face lost")?]
        .indices
        .iter()
        .take(3)
        .copied()
        .collect();
        for (label, op) in [("make face", OP_FACE), ("add vertex", OP_VERTEX)] {
            self.mesh_vertices = corners.clone();
            self.mesh_op(op);
            if !self.doc.dirty() {
                out.push_str(&format!("REFUSED {label}: {}\n", self.status));
                continue;
            }
            restored(self, label, &mut out)?;
        }
        // Part settings: every gear direction that flips in place, and back.
        self.act(Action::MeshMode);
        self.open_animation();
        for i in 0..self.ed.parts.len() {
            let Some(k) = self.ed.parts[i].controls.iter().position(|c| {
                matches!(
                    c.setting,
                    hangar_core::shape_parts::Setting::Direction { .. }
                ) && c.allowed == hangar_core::shape_parts::Allowed::Either
            }) else {
                continue;
            };
            self.pick_part(i);
            let negated = matches!(
                self.ed.parts[i].controls[k].setting,
                hangar_core::shape_parts::Setting::Direction { negated: true, .. }
            );
            self.part_value(k, i32::from(!negated));
            if !self.doc.dirty() {
                return Err(format!("{}: {}", self.ed.parts[i].name, self.status));
            }
            let label = format!("{} direction", self.ed.parts[i].name);
            restored(self, &label, &mut out)?;
        }
        self.ed.pose.clear();
        Ok(out)
    }
}
