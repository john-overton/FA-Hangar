//! Edit Mesh Add vertex tool: hover a drawn face to preview the 3D point
//! under the pointer (the raster's face buffer picks the face; integer
//! barycentrics over its fan give the point, `hangar_core::surface`), snap
//! it to the face's corners, edge midpoints and centre with the magnet, and
//! click to add a vertex there (`shape_geometry::add_vertices`) or
//! Shift+click to split the face into a fan around it
//! (`shape_geometry::split_faces`; a point on an edge splits the faces that
//! share the edge). Each placement is one undo step.
use super::view::{Action, Layout};
use super::*;
use hangar_core::shape_geometry as geo;
use hangar_core::surface::{self, Target};

/// The Add vertex tool, boxed in `EditState`.
#[derive(Clone, Debug, Default)]
pub(super) struct AddTool {
    /// Clicks split the face (Split face at point); Shift inverts.
    pub split: bool,
    /// Shift held at the last pointer motion.
    pub shift: bool,
    pub hover: Option<AddPoint>,
}
/// The point a click would place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AddPoint {
    /// Model face index and file offset.
    pub face: usize,
    pub offset: usize,
    /// Model coordinates of the point.
    pub point: [i32; 3],
    pub snap: Option<Target>,
    /// On screen.
    pub at: [i32; 2],
}
impl App {
    /// The Add vertex tool is on (Edit Mesh in the Model workspace).
    pub(super) fn add_tool_on(&self) -> bool {
        self.ed.add_vertex.is_some()
            && self.mesh_edit
            && self.mode == Mode::Model
            && self.model.is_some()
    }
    /// Turn the tool on (or switch its click between add and split); the
    /// same command again turns it off.
    pub(super) fn add_vertex_tool(&mut self, split: bool) {
        if let Some(t) = &self.ed.add_vertex {
            if t.split == split {
                self.add_vertex_cancel();
                return;
            }
        }
        if let Some(why) = self.mesh_blocked() {
            self.status = format!("Error: {why}");
            return;
        }
        if !self.mesh_edit || self.model.is_none() {
            self.status = "Open a shape in Edit Mesh to add vertices".into();
            return;
        }
        self.ed.add_vertex = Some(Box::new(AddTool {
            split,
            ..AddTool::default()
        }));
        self.add_vertex_hover(self.mouse[0], self.mouse[1], false);
        self.status = if split {
            "Split face: click a face to split it around the point; Shift+click adds a vertex only; Alt skips snapping; Esc cancels"
        } else {
            "Add vertex: click a face to place a vertex; Shift+click splits the face; Alt skips snapping; Esc cancels"
        }
        .into();
    }
    pub(super) fn add_vertex_cancel(&mut self) {
        if self.ed.add_vertex.take().is_some() {
            self.status = "Add vertex cancelled; nothing changed".into();
        }
    }
    /// The point on the drawn face under `(x, y)`, snapped unless `alt`.
    pub(super) fn add_point_at(&self, x: i32, y: i32, alt: bool) -> Result<AddPoint> {
        if !self.in_viewport(x, y) {
            return Err("Point at a face in the viewport".into());
        }
        if self.perspective && !self.textured {
            return Err("Use orthographic view to place vertices".into());
        }
        let model = self.model.as_ref().ok_or("No model")?;
        let face = self.pick_face(x, y).ok_or("Point at a face")?;
        let f = &model.faces[face];
        let points: Vec<[i32; 3]> = f
            .indices
            .iter()
            .map(|i| model.vertices.get(*i).map(|v| v.point))
            .collect::<Option<_>>()
            .ok_or("Face corner missing")?;
        let frame = self.view_frame(self.view_size()).ok_or("No view")?;
        let screen: Vec<[i64; 2]> = points
            .iter()
            .map(|p| {
                let q = frame.project16(*p);
                [q[0], q[1]]
            })
            .collect();
        let at16 = [
            (x - self.left() - 1) as i64 * 16 + 8,
            (y - 54) as i64 * 16 + 8,
        ];
        let located = surface::locate(&screen, at16).ok_or("The face has no area in this view")?;
        let mut point = located.mix(&points).ok_or("Face corner missing")?;
        let mut snap = None;
        if self.gizmo.magnet && !alt {
            if let Some(b) = self.basis(point) {
                let tolerance =
                    hangar_core::gizmo::div_round(gizmo_ui::SNAP_PX * b.k, b.r).max(1) as i32;
                if let Some(t) = surface::snap(&points, point, tolerance) {
                    point = t.point;
                    snap = Some(t);
                }
            }
        }
        let at = self.hp_project(point).ok_or("No view")?;
        Ok(AddPoint {
            face,
            offset: f.offset,
            point,
            snap,
            at,
        })
    }
    /// Track the pointer: the preview and its readout.
    pub(super) fn add_vertex_hover(&mut self, x: i32, y: i32, shift: bool) {
        if !self.add_tool_on() {
            return;
        }
        let hover = self.add_point_at(x, y, self.replace.alt);
        let split = self.ed.add_vertex.as_ref().is_some_and(|t| t.split) != shift;
        if let Some(t) = self.ed.add_vertex.as_mut() {
            t.shift = shift;
            t.hover = hover.as_ref().ok().cloned();
        }
        match hover {
            Ok(p) => {
                let [px, py, pz] = p.point;
                self.status = format!(
                    "{} at ({px} {py} {pz}) on the face at {:X}{}",
                    if split { "Split" } else { "Add vertex" },
                    p.offset,
                    p.snap
                        .map(|s| format!(" \u{b7} {}", s.kind.label()))
                        .unwrap_or_default()
                );
            }
            Err(e) if self.in_viewport(x, y) => self.status = e,
            Err(_) => {}
        }
    }
    /// A click with the tool: add a vertex, or split (Shift inverts).
    pub(super) fn add_vertex_click(&mut self, x: i32, y: i32, shift: bool) -> Result<()> {
        let split = self.ed.add_vertex.as_ref().is_some_and(|t| t.split) != shift;
        let p = self.add_point_at(x, y, self.replace.alt)?;
        if split {
            self.split_at(&p)
        } else {
            self.place_vertex(&p)
        }
    }
    /// The faces' frame translation, refusing faces the pose rotates.
    fn face_translation(&self, offset: usize) -> Result<[i32; 3]> {
        let model = self.model.as_ref().ok_or("No model")?;
        let f = model
            .faces
            .iter()
            .find(|f| f.offset == offset)
            .ok_or("Face lost")?;
        if f.indices.iter().any(|i| !geo::unrotated(model, *i)) {
            return Err(
                "The face is in a part the preview pose rotates; reset the pose (Parts) to edit it"
                    .into(),
            );
        }
        geo::frame_translation(model, offset)
    }
    /// Select the vertices stored at these coordinate offsets.
    fn select_stored(&mut self, offsets: &[usize]) {
        if let Some(model) = &self.model {
            self.mesh_vertices = model
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, v)| offsets.contains(&v.offset))
                .map(|(i, _)| i)
                .collect();
        }
    }
    /// Add a vertex at `p`, stored in the face's frame and drawn where the
    /// face is drawn; it becomes the selection.
    pub(super) fn place_vertex(&mut self, p: &AddPoint) -> Result<()> {
        let (entry, source) = self.edit_source()?;
        let t = self.face_translation(p.offset)?;
        let local: [i32; 3] = core::array::from_fn(|k| p.point[k] - t[k]);
        let added = geo::add_vertices(&source, p.offset, &[local], &[])?;
        self.doc.replace(entry, added.shape)?;
        self.ed.face_select = false;
        self.ed.add_vertex = None;
        self.refresh();
        self.select_stored(&added.vertices);
        let [x, y, z] = p.point;
        self.status = format!(
            "Added a vertex at ({x} {y} {z}){}, slot {}; F makes a face, G moves it. One undo step.",
            p.snap
                .map(|s| format!(" on the {}", s.kind.label().to_ascii_lowercase()))
                .unwrap_or_default(),
            added.slots.first().copied().unwrap_or(0)
        );
        Ok(())
    }
    /// The faces a split at `point` on face `offset` replaces: the face,
    /// and when the point lies on one of its edges every other face with
    /// that edge (by corner points), so no crack opens.
    fn split_faces_at(&self, offset: usize, point: [i32; 3]) -> Result<Vec<usize>> {
        let model = self.model.as_ref().ok_or("No model")?;
        let f = model
            .faces
            .iter()
            .find(|f| f.offset == offset)
            .ok_or("Face lost")?;
        let points: Vec<[i32; 3]> = f.indices.iter().map(|i| model.vertices[*i].point).collect();
        let mut faces = vec![offset];
        if let Some(e) = surface::on_edge(&points, point) {
            let (a, b) = (points[e], points[(e + 1) % points.len()]);
            faces.extend(
                model
                    .faces
                    .iter()
                    .filter(|g| g.offset != offset)
                    .filter(|g| {
                        let n = g.indices.len();
                        (0..n).any(|k| {
                            let p = model.vertices[g.indices[k]].point;
                            let q = model.vertices[g.indices[(k + 1) % n]].point;
                            (p, q) == (a, b) || (p, q) == (b, a)
                        })
                    })
                    .map(|g| g.offset),
            );
            faces.sort_unstable();
            faces.dedup();
            if faces.len() > 2 {
                return Err(format!(
                    "{} faces share that edge; split edges shared by at most two",
                    faces.len()
                ));
            }
        }
        Ok(faces)
    }
    /// Split the face under `p` (and its edge neighbour) into a fan of
    /// triangles around a new vertex at the point; the new vertex becomes
    /// the selection.
    pub(super) fn split_at(&mut self, p: &AddPoint) -> Result<()> {
        let faces = self.split_faces_at(p.offset, p.point)?;
        self.split_run(&faces, p.point, p.snap.map(|s| s.kind.label()))
    }
    fn split_run(&mut self, faces: &[usize], point: [i32; 3], kind: Option<&str>) -> Result<()> {
        let (entry, source) = self.edit_source()?;
        let t = self.face_translation(faces[0])?;
        for f in &faces[1..] {
            if self.face_translation(*f)? != t {
                return Err("The faces sharing that edge are in different parts".into());
            }
        }
        let local: [i32; 3] = core::array::from_fn(|k| point[k] - t[k]);
        let split = geo::split_faces(&source, faces, local)?;
        self.doc.replace(entry, split.shape)?;
        self.ed.face_select = false;
        self.ed.add_vertex = None;
        self.ed.mesh_faces.clear();
        self.refresh();
        self.select_stored(&split.vertices);
        let [x, y, z] = point;
        self.status = format!(
            "Split {} into {} around ({x} {y} {z}){}. One undo step.",
            view::count(faces.len(), "face", "faces"),
            view::count(split.faces.len(), "triangle", "triangles"),
            kind.map(|k| format!(" at the {}", k.to_ascii_lowercase()))
                .unwrap_or_default()
        );
        Ok(())
    }
    /// Split edge at midpoint: two selected vertices that are an edge of a
    /// face; the faces sharing it split into fans around its midpoint.
    pub(super) fn split_edge(&mut self) -> Result<()> {
        let model = self.model.as_ref().ok_or("No model")?;
        let mut ends: Vec<[i32; 3]> = self
            .mesh_vertices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.point))
            .collect();
        ends.sort_unstable();
        ends.dedup();
        let [a, b] = ends[..] else {
            return Err("Select the two vertices of an edge (vertex select, 1)".into());
        };
        let face = model
            .faces
            .iter()
            .find(|f| {
                let n = f.indices.len();
                (0..n).any(|k| {
                    let p = model.vertices[f.indices[k]].point;
                    let q = model.vertices[f.indices[(k + 1) % n]].point;
                    (p, q) == (a, b) || (p, q) == (b, a)
                })
            })
            .ok_or("The two selected vertices are not an edge of a face")?
            .offset;
        let mid = surface::midpoint(a, b);
        if mid == a || mid == b {
            return Err("The edge is too short to split".into());
        }
        let faces = self.split_faces_at(face, mid)?;
        self.split_run(&faces, mid, Some("Edge midpoint"))
    }
    /// Exactly two vertex positions are selected in vertex select.
    pub(super) fn connect_ready(&self) -> bool {
        !self.ed.face_select && self.selected_points().len() == 2
    }
    /// The distinct points of the selected vertices.
    fn selected_points(&self) -> Vec<[i32; 3]> {
        let Some(model) = self.model.as_ref() else {
            return Vec::new();
        };
        let mut p: Vec<[i32; 3]> = self
            .mesh_vertices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.point))
            .collect();
        p.sort_unstable();
        p.dedup();
        p
    }
    /// Connect vertices (J) as a menu item: disabled, with the reason as
    /// its badge, unless exactly two vertices are selected.
    pub(super) fn connect_item(&self) -> super::widgets::Item<'static> {
        let item = super::widgets::Item::new(
            "Connect vertices",
            Action::MeshOp(super::edit_ui::OP_CONNECT),
        )
        .key("J");
        if self.connect_ready() {
            item
        } else {
            item.enabled(false).badge("Select 2 vertices")
        }
    }
    /// J, Connect vertices: FA shapes have no free-standing edges, so two
    /// selected vertices are joined by cutting every face that has both as
    /// non-adjacent corners along that diagonal into two faces.
    pub(super) fn connect_vertices(&mut self) -> Result<()> {
        if self.ed.face_select {
            return Err("Connect vertices works in vertex select (1): select two vertices".into());
        }
        let ends = self.selected_points();
        let [a, b] = ends[..] else {
            return Err(format!(
                "Select exactly 2 vertices to connect ({} selected)",
                ends.len()
            ));
        };
        let model = self.model.as_ref().ok_or("No model")?;
        let mut cuts: Vec<(usize, [usize; 2])> = Vec::new();
        let mut edge = None;
        for f in &model.faces {
            let at = |p: [i32; 3]| f.indices.iter().position(|i| model.vertices[*i].point == p);
            let (Some(ka), Some(kb)) = (at(a), at(b)) else {
                continue;
            };
            let n = f.indices.len();
            if (ka + 1) % n == kb || (kb + 1) % n == ka {
                edge.get_or_insert(f.offset);
            } else {
                cuts.push((f.offset, [ka, kb]));
            }
        }
        if cuts.is_empty() {
            return Err(match edge {
                Some(o) => format!("Already connected by an edge of face {o:X}"),
                None => "No panel has both vertices as corners. Select 3 or more vertices and press F to make a face".into(),
            });
        }
        let (entry, source) = self.edit_source()?;
        let split = geo::connect_corners(&source, &cuts)?;
        self.doc.replace(entry, split.shape)?;
        self.ed.add_vertex = None;
        self.refresh();
        if let Some(model) = &self.model {
            self.mesh_vertices = (0..model.vertices.len())
                .filter(|i| [a, b].contains(&model.vertices[*i].point))
                .collect();
        }
        self.status = format!(
            "Connected 2 vertices: cut {} along the diagonal into {}. One undo step.",
            view::count(cuts.len(), "face", "faces"),
            view::count(split.faces.len(), "face", "faces")
        );
        Ok(())
    }
    /// The preview: the hovered face outlined, the point marked (amber
    /// rings when snapped) with the snap kind, and for a split the fan.
    pub(super) fn add_vertex_overlay(&self, o: &mut Layout) {
        let Some(tool) = self.ed.add_vertex.as_ref().filter(|_| self.add_tool_on()) else {
            return;
        };
        let Some(p) = &tool.hover else {
            return;
        };
        let Some(model) = self.model.as_ref() else {
            return;
        };
        let Some(face) = model.faces.get(p.face) else {
            return;
        };
        let view = [self.left() + 1, 54, self.right() - 2, self.dock_y() - 1];
        let corners: Vec<[i32; 2]> = face
            .indices
            .iter()
            .filter_map(|i| self.hp_project(model.vertices.get(*i)?.point))
            .collect();
        let split = tool.split != tool.shift;
        for k in 0..corners.len() {
            let (a, b) = (corners[k], corners[(k + 1) % corners.len()]);
            if let Some((a, b)) = clip(a, b, view) {
                o.canvas.line(a[0], a[1], b[0], b[1], c::AMBER);
            }
            if split {
                if let Some((a, b)) = clip(p.at, a, view) {
                    gizmo_ui::dashed(&mut o.canvas, a, b, c::AMBER_BRIGHT);
                }
            }
        }
        let [x, y] = p.at;
        if !self.in_viewport(x, y) {
            return;
        }
        if p.snap.is_some() {
            chrome::ring(&mut o.canvas, x, y, 7, c::AMBER_BRIGHT, c::GM_1000);
        }
        chrome::ring(&mut o.canvas, x, y, 4, c::AMBER_BRIGHT, c::AMBER);
        if let Some(s) = p.snap {
            o.canvas
                .styled(x + 11, y - 9, s.kind.label(), c::AMBER, Style::ValueSm);
        }
    }
}

/// The parts aircraft in Edit Mesh vertex select, Solid shading, at
/// `w` x `h`, with a root face picked at its centre.
#[inline(never)]
fn vertex_app(w: i32, h: i32) -> (Box<App>, usize, [i32; 2], Vec<[i32; 3]>) {
    let mut a = super::edit_ui::parts_app();
    (a.width, a.height) = (w, h);
    a.act(Action::MeshMode);
    a.select_mode(false);
    a.textured = true;
    a.flat = true;
    let (f, p) = a.smoke_pickable(true);
    let m = a.model.as_ref().unwrap();
    let points = m.faces[f]
        .indices
        .iter()
        .map(|i| m.vertices[*i].point)
        .collect();
    (a, f, p, points)
}
impl App {
    /// Add vertex: hover preview, snapping and Alt, click placement with
    /// undo, Esc and right-click, Split face and Split edge; at both sizes.
    #[inline(never)]
    pub(super) fn smoke_add_vertex(&mut self) {
        for (w, h) in [(800, 600), (1280, 800)] {
            smoke_add_place(w, h);
            smoke_add_split(w, h);
            smoke_connect(w, h);
        }
    }
}
fn gm_len(dx: i32, dy: i32) -> i32 {
    hangar_core::gizmo::isqrt(dx as i64 * dx as i64 + dy as i64 * dy as i64) as i32
}
#[inline(never)]
fn smoke_add_place(w: i32, h: i32) {
    let (mut a, f, p, points) = vertex_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let n0 = a.model.as_ref().unwrap().vertices.len();
    // The inspector button turns the tool on.
    let add = a.smoke_find(&|x| matches!(x, Action::MeshOp(super::edit_ui::OP_VERTEX)));
    a.chrome_click(add);
    assert!(a.add_tool_on(), "{}", a.status);
    // Hover with Alt: the free point under the pointer, on the face's plane.
    a.alt_modifier(true);
    a.motion(p[0], p[1], false);
    let hover =
        a.ed.add_vertex
            .as_ref()
            .unwrap()
            .hover
            .clone()
            .expect("Hover preview");
    assert_eq!((hover.face, hover.snap), (f, None));
    let centre = surface::centre(&points);
    let b = a.basis(centre).unwrap();
    let reach = (2 * b.k / b.r.max(1) + 1) as i32;
    assert!(
        (0..3).all(|k| (hover.point[k] - centre[k]).abs() <= reach),
        "{:?} vs {centre:?}",
        hover.point
    );
    assert!(surface::plane_distance(&points, hover.point).unwrap() <= 1);
    assert!((hover.at[0] - p[0]).abs() <= 1 && (hover.at[1] - p[1]).abs() <= 1);
    // The preview: the face outlined in amber, the point marked.
    let d = a.draw();
    let at = hover.at;
    assert!(
        d.commands.iter().any(|c| {
            matches!(c, Draw::Rect(x, y, w, h, color) if *color == c::AMBER.0
                && *x <= at[0] && at[0] < x + w && *y <= at[1] && at[1] < y + h)
        }),
        "Preview marker"
    );
    assert!(d
        .commands
        .iter()
        .any(|c| matches!(c, Draw::Line(.., color) if *color == c::AMBER.0)));
    // Magnet: 3 px off an edge midpoint towards the centre snaps onto it.
    a.alt_modifier(false);
    let mid = surface::midpoint(points[0], points[1]);
    let q = a.hp_project(mid).unwrap();
    let (dx, dy) = (p[0] - q[0], p[1] - q[1]);
    let len = gm_len(dx, dy).max(1);
    let near = [q[0] + 3 * dx / len, q[1] + 3 * dy / len];
    a.motion(near[0], near[1], false);
    let hover =
        a.ed.add_vertex
            .as_ref()
            .unwrap()
            .hover
            .clone()
            .expect("Hover");
    let snap = hover.snap.expect("Snapped");
    assert_eq!(
        (snap.kind, hover.point),
        (surface::SnapKind::EdgeMidpoint, mid)
    );
    assert!(a.status.contains("Edge midpoint"), "{}", a.status);
    assert!(a
        .draw()
        .commands
        .iter()
        .any(|c| matches!(c, Draw::Text(_, _, s, ..) if s == "Edge midpoint")));
    // Alt bypasses the snap.
    a.alt_modifier(true);
    let free =
        a.ed.add_vertex
            .as_ref()
            .unwrap()
            .hover
            .as_ref()
            .unwrap()
            .snap;
    assert!(free.is_none());
    a.alt_modifier(false);
    a.smoke_geometry("add vertex tool");
    // A click places the vertex there: selected, one undo step.
    a.smoke_press(near[0], near[1], false);
    let m = a.model.as_ref().unwrap();
    assert_eq!(m.vertices.len(), n0 + 1, "{}", a.status);
    assert_eq!(a.mesh_vertices.len(), 1);
    assert_eq!(m.vertices[a.mesh_vertices[0]].point, mid);
    assert!(!a.add_tool_on() && !a.ed.face_select);
    // The loose new vertex shows while selected.
    let new = a.mesh_vertices[0];
    assert!(a.vertex_handles().iter().any(|h| h.index == new));
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Esc and right-click cancel; nothing changes.
    a.mesh_op(super::edit_ui::OP_VERTEX);
    assert!(a.add_tool_on());
    a.key(Key::Escape, false, false);
    assert!(!a.add_tool_on());
    a.mesh_op(super::edit_ui::OP_VERTEX);
    a.pointer(p[0], p[1], 3, true, false);
    assert!(!a.add_tool_on() && a.menu.is_none());
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Empty space places nothing.
    a.mesh_op(super::edit_ui::OP_VERTEX);
    let empty = [a.right() - 30, a.dock_y() - 30];
    a.smoke_press(empty[0], empty[1], false);
    assert!(
        a.add_tool_on() && a.status.starts_with("Error:"),
        "{}",
        a.status
    );
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
#[inline(never)]
fn smoke_add_split(w: i32, h: i32) {
    let (mut a, _, p, points) = vertex_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let faces0 = a.model.as_ref().unwrap().faces.len();
    // Shift+click with the magnet at the face centre: a fan of one
    // triangle per edge around the centre.
    a.mesh_op(super::edit_ui::OP_VERTEX);
    a.motion(p[0], p[1], true);
    let hover = a.ed.add_vertex.as_ref().unwrap().hover.clone().unwrap();
    assert_eq!(
        hover.snap.map(|s| s.kind),
        Some(surface::SnapKind::FaceCentre)
    );
    a.smoke_geometry("split preview");
    a.smoke_press(p[0], p[1], true);
    let m = a.model.as_ref().unwrap();
    assert_eq!(m.faces.len(), faces0 - 1 + points.len(), "{}", a.status);
    let new = a.mesh_vertices[0];
    assert_eq!(m.vertices[new].point, surface::centre(&points));
    assert!(a.status.starts_with("Split 1 face into"), "{}", a.status);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Split edge at midpoint from the Mesh menu: the edge's two vertices
    // selected; every face with that edge splits around the midpoint.
    let m = a.model.as_ref().unwrap();
    let (u, v) = (points[0], points[1]);
    let sharing = m
        .faces
        .iter()
        .filter(|f| {
            let n = f.indices.len();
            (0..n).any(|k| {
                let (x, y) = (
                    m.vertices[f.indices[k]].point,
                    m.vertices[f.indices[(k + 1) % n]].point,
                );
                (x, y) == (u, v) || (x, y) == (v, u)
            })
        })
        .map(|f| f.indices.len() - 2)
        .collect::<Vec<usize>>();
    a.mesh_vertices = (0..m.vertices.len())
        .filter(|i| [u, v].contains(&m.vertices[*i].point))
        .collect();
    let menu = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_MESH)));
    a.chrome_click(menu);
    let item = a.smoke_find(&|x| matches!(x, Action::MeshOp(super::edit_ui::OP_SPLIT_EDGE)));
    a.chrome_click(item);
    if sharing.len() <= 2 {
        let m = a.model.as_ref().unwrap();
        assert_eq!(
            m.faces.len(),
            faces0 + sharing.iter().sum::<usize>(),
            "{}",
            a.status
        );
        let new = a.mesh_vertices[0];
        assert_eq!(m.vertices[new].point, surface::midpoint(u, v));
        a.act(Action::Undo);
    } else {
        assert!(a.status.starts_with("Error:"), "{}", a.status);
    }
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
/// A root face with four or more corners whose corners 0 and 2 have their
/// own shown handles: the face, the two points and their handle positions.
/// A face, two of its corner points and their handle positions.
type Quad = (usize, [[i32; 3]; 2], [[i32; 2]; 2]);
fn quad_corners(a: &App) -> Option<Quad> {
    let m = a.model.as_ref()?;
    let handles = a.vertex_handles();
    (0..m.faces.len()).find_map(|f| {
        let face = &m.faces[f];
        if face.group.is_some() || face.indices.len() < 4 {
            return None;
        }
        let p = [0, 2].map(|k| m.vertices[face.indices[k]].point);
        let at = [0, 1].map(|k| {
            handles
                .iter()
                .find(|h| m.vertices[h.index].point == p[k])
                .map(|h| h.at)
        });
        let at = [at[0]?, at[1]?];
        let picks = (0..2).all(|k| {
            a.pick_handle(at[k][0], at[k][1])
                .is_some_and(|i| m.vertices[i].point == p[k])
        });
        (picks && a.in_viewport(at[0][0], at[0][1]) && a.in_viewport(at[1][0], at[1][1]))
            .then_some((f, p, at))
    })
}
/// Faces that have both points as non-adjacent corners.
fn diagonal_faces(a: &App, p: [[i32; 3]; 2]) -> usize {
    let m = a.model.as_ref().unwrap();
    m.faces
        .iter()
        .filter(|f| {
            let n = f.indices.len();
            let at = |q: [i32; 3]| f.indices.iter().position(|i| m.vertices[*i].point == q);
            matches!((at(p[0]), at(p[1])), (Some(x), Some(y))
                if (x + 1) % n != y && (y + 1) % n != x)
        })
        .count()
}
/// Connect vertices: two corners picked through their handles, then J,
/// the Mesh menu, the inspector button and the viewport menu; adjacent,
/// lone and unshared selections refused; one undo step each.
#[inline(never)]
fn smoke_connect(w: i32, h: i32) {
    let (mut a, ..) = vertex_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let faces0 = a.model.as_ref().unwrap().faces.len();
    let (f, p, at) = quad_corners(&a).expect("A quad with two shown opposite corners");
    let cut = diagonal_faces(&a, p);
    assert!(cut >= 1);
    let select = |a: &mut App| {
        let empty = [a.right() - 30, a.dock_y() - 30];
        a.smoke_press(empty[0], empty[1], false);
        a.smoke_press(at[0][0], at[0][1], false);
        a.smoke_press(at[1][0], at[1][1], true);
        assert!(a.connect_ready(), "{:?}", a.mesh_vertices);
    };
    let check = |a: &mut App, how: &str| {
        let m = a.model.as_ref().unwrap();
        assert_eq!(m.faces.len(), faces0 + cut, "{how}: {}", a.status);
        assert!(a.status.starts_with("Connected 2 vertices"), "{}", a.status);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original, "{how}");
    };
    // J.
    select(&mut a);
    a.key(Key::Char('j'), false, false);
    check(&mut a, "J");
    // Mesh menu, keycap J.
    select(&mut a);
    let menu = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_MESH)));
    a.chrome_click(menu);
    let item = a.smoke_find(&|x| matches!(x, Action::MeshOp(super::edit_ui::OP_CONNECT)));
    a.chrome_click(item);
    check(&mut a, "menu");
    // Inspector button.
    select(&mut a);
    let button = a.smoke_find(&|x| matches!(x, Action::MeshOp(super::edit_ui::OP_CONNECT)));
    a.chrome_click(button);
    check(&mut a, "button");
    // Viewport right-click menu.
    select(&mut a);
    let spot = [a.left() + 60, a.dock_y() - 20];
    a.motion(spot[0], spot[1], false);
    a.pointer(spot[0], spot[1], 3, true, false);
    assert_eq!(a.menu, Some(gizmo_ui::MENU_GIZMO));
    a.smoke_geometry("connect context menu");
    let item = a.smoke_find(&|x| matches!(x, Action::MeshOp(super::edit_ui::OP_CONNECT)));
    a.chrome_click(item);
    check(&mut a, "context menu");
    // Adjacent corners: already an edge, nothing changes.
    let m = a.model.as_ref().unwrap();
    let face = &m.faces[f];
    let ends = [0, 1].map(|k| m.vertices[face.indices[k]].point);
    a.mesh_vertices = (0..m.vertices.len())
        .filter(|i| ends.contains(&m.vertices[*i].point))
        .collect();
    assert_eq!(diagonal_faces(&a, ends), 0);
    a.key(Key::Char('j'), false, false);
    assert!(
        a.status
            .starts_with("Error: Already connected by an edge of face"),
        "{}",
        a.status
    );
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // One vertex: disabled with the reason, J refuses.
    a.mesh_vertices.truncate(1);
    a.key(Key::Char('j'), false, false);
    assert!(
        a.status.contains("Select exactly 2 vertices"),
        "{}",
        a.status
    );
    let menu = a.smoke_find(&|x| matches!(x, Action::Menu(chrome::MENU_MESH)));
    a.chrome_click(menu);
    assert!(a
        .layout()
        .hits
        .iter()
        .all(|h| !matches!(h.action, Action::MeshOp(super::edit_ui::OP_CONNECT))));
    assert!(a.draw().commands.iter().any(|d| matches!(d,
        Draw::Text(_, _, s, ..) if s == "Select 2 vertices")));
    a.key(Key::Escape, false, false);
    // No shared face: two vertices of different, unconnected faces.
    let m = a.model.as_ref().unwrap();
    let lone = (0..m.vertices.len()).find(|&i| {
        let q = m.vertices[i].point;
        let shared = m.faces.iter().any(|f| {
            let pts: Vec<[i32; 3]> = f.indices.iter().map(|k| m.vertices[*k].point).collect();
            pts.contains(&q) && pts.contains(&p[0])
        });
        !shared && m.faces.iter().any(|f| f.indices.contains(&i))
    });
    if let Some(i) = lone {
        let first = (0..m.vertices.len())
            .find(|k| m.vertices[*k].point == p[0])
            .unwrap();
        a.mesh_vertices = vec![first, i];
        a.key(Key::Char('j'), false, false);
        assert!(
            a.status
                .starts_with("Error: No panel has both vertices as corners."),
            "{}",
            a.status
        );
    }
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
