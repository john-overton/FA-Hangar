//! Edit Mesh vertex drags and transactional panel texture creation.
use super::view::Action;
use super::*;
use hangar_core::shape_edit;
pub(super) struct PanelPlan {
    pub entry: usize,
    pub original: Entry,
    pub shape: Vec<u8>,
    pub name: String,
    pub picture: Vec<u8>,
    pub face: usize,
}
pub(super) struct MeshDrag {
    pub entry: usize,
    pub original: Entry,
    pub model: Model,
    pub reference: [i32; 3],
    pub delta: [i32; 3],
    /// Source point under the cursor at press: the drag keeps this grab offset.
    pub grab: [i32; 3],
    pub start: [i32; 2],
    pub pressed: usize,
    pub moving: bool,
    pub axis: Option<usize>,
}
/// Pixels the pointer must travel before a vertex press becomes a drag.
const DRAG_THRESHOLD: i32 = 4;
impl App {
    pub(super) fn repair_panels(&mut self) -> Result<()> {
        self.finish_stroke();
        let entry = self
            .model_entry
            .or(self.context_entry)
            .ok_or("Select the local SH or its owning object")?;
        let source = self.doc.archive.entries[entry].read()?;
        if let Some(repaired) = shape_edit::repair_panel_layout(&source)? {
            let panels = repaired.panels;
            self.doc.replace(entry, repaired.shape)?;
            self.refresh();
            self.status = format!(
                "Repaired {panels} generated panels / painted PICs preserved / one undo step"
            );
        } else {
            self.status = "No legacy generated-panel layout found / nothing changed".into();
        }
        Ok(())
    }
    pub(super) fn panel_plan(&self, face: usize) -> Result<PanelPlan> {
        let entry = self
            .model_entry
            .or(self.context_entry)
            .ok_or("Open the shape owner's LIB to add a panel texture")?;
        if !self.palette_loaded {
            return Err("Load the base PAL before generating a panel texture".into());
        }
        let original = self.doc.archive.entries[entry].clone();
        let stem: String = original
            .name
            .split('.')
            .next()
            .unwrap()
            .chars()
            .take(6)
            .collect();
        let name = (0..256)
            .map(|n| format!("{stem}{n:02X}.PIC"))
            .find(|name| {
                self.doc.archive.find(name).is_none()
                    && self.stroke.as_ref().is_none_or(|s| s.name != *name)
                    && !self.stroke_parked.iter().any(|s| s.name == *name)
                    && !self
                        .libraries
                        .iter()
                        .any(|l| l.doc.archive.find(name).is_some())
                    && !self
                        .dependency_catalogs
                        .iter()
                        .any(|(_, names)| names.contains(name))
            })
            .ok_or("No free generated texture name; export under a new object ID")?;
        let source = self
            .panel_draft
            .as_ref()
            .filter(|p| p.entry == entry)
            .map(|p| p.shape.clone())
            .unwrap_or(original.read()?);
        let result = shape_edit::texture_panel(&source, face, &name, 64, &self.base_palette)?;
        Ok(PanelPlan {
            entry,
            original,
            shape: result.shape,
            name,
            picture: result.picture,
            face: result.face,
        })
    }
    pub(super) fn create_panel_texture(&mut self) -> Result<()> {
        let plan = self.panel_plan(self.selected_face.ok_or("Select a flat-color panel")?)?;
        let entry = plan.entry;
        let name = plan.name.clone();
        let face = plan.face;
        self.doc.transaction(
            vec![
                Entry::new(&plan.original.name, plan.shape)?,
                Entry::new(&name, plan.picture)?,
            ],
            &[],
        )?;
        if self.context_entry == Some(entry) {
            self.context_model = Some(Model::parse(&self.doc.archive.entries[entry].read()?)?);
        }
        self.refresh();
        self.selected_face = Some(face);
        let i = self.doc.archive.find(&name).unwrap();
        self.open_texture(i);
        self.media_tab = 0;
        self.status = format!("Created {name}, mapped selected panel / one undo step");
        Ok(())
    }
    pub(super) fn start_generated_stroke(&mut self, face: usize) -> Result<()> {
        if self.stroke_parked.len() + usize::from(self.stroke.is_some()) >= 64 {
            return Err("Stroke limit: release before generating more panel textures".into());
        }
        let plan = self.panel_plan(face)?;
        let pic = Pic::parse(&plan.picture)?;
        self.preview = Some(Model::parse(&plan.shape)?);
        self.selected_face = Some(plan.face);
        if let Some(s) = self.stroke.take() {
            self.stroke_parked.push(s);
        }
        self.stroke = Some(Stroke {
            entry: self.doc.archive.entries.len() + self.stroke_parked.len(),
            name: plan.name.clone(),
            bytes: plan.picture.clone(),
            // A new panel sheet's original is its solid face color.
            erase: Some(Box::new(pic.clone())),
            pic,
            last: None,
            original: None,
        });
        self.panel_draft = Some(Box::new(plan));
        self.painting = true;
        Ok(())
    }
    pub(super) fn mesh_select(&mut self, i: usize) {
        self.mesh_vertices = vec![i];
        self.selected_face = None;
    }
    /// Press on a vertex: Shift toggles it in the selection; a plain press keeps an
    /// existing selection containing it so a drag moves them all, and a click without
    /// dragging selects only that vertex on release.
    pub(super) fn mesh_press(&mut self, i: usize, shift: bool) {
        self.selected_face = None;
        if shift {
            if let Some(at) = self.mesh_vertices.iter().position(|v| *v == i) {
                self.mesh_vertices.remove(at);
            } else {
                self.mesh_vertices.insert(0, i);
            }
            return;
        }
        if !self.mesh_vertices.contains(&i) {
            self.mesh_vertices = vec![i];
        }
        if let Some(reason) = self.mesh_blocked() {
            self.status = reason;
            return;
        }
        if self.perspective && !self.textured {
            self.status = "Vertex selected / switch to an orthographic view (5) to drag".into();
            return;
        }
        let (Some(entry), Some(model)) = (self.model_entry, &self.model) else {
            return;
        };
        let Some(v) = model.vertices.get(i) else {
            return;
        };
        let Ok(grab) = self.cursor_station(self.mouse[0], self.mouse[1], v.point) else {
            return;
        };
        self.mesh_drag = Some(Box::new(MeshDrag {
            entry,
            original: self.doc.archive.entries[entry].clone(),
            model: model.clone(),
            reference: v.point,
            delta: [0; 3],
            grab,
            start: self.mouse,
            pressed: i,
            moving: false,
            axis: None,
        }));
    }
    pub(super) fn mesh_motion(&mut self, x: i32, y: i32) -> Result<()> {
        let d = self.mesh_drag.as_ref().ok_or("No mesh drag")?;
        if !d.moving && (x - d.start[0]).abs().max((y - d.start[1]).abs()) < DRAG_THRESHOLD {
            return Ok(());
        }
        if self.perspective && !self.textured {
            return Err("Switch to an orthographic view (5) to drag vertices".into());
        }
        let point = self
            .cursor_station(x, y, d.reference)
            .map_err(|_| "Vertex outside signed source-coordinate range")?;
        let mut delta: [i32; 3] = core::array::from_fn(|k| point[k] - d.grab[k]);
        if let Some(axis) = d.axis {
            for (k, n) in delta.iter_mut().enumerate() {
                if k != axis {
                    *n = 0;
                }
            }
        }
        let preview = d.model.transform_selection(
            Transform::Translate(delta),
            Some(&self.mesh_vertices),
            [0; 3],
        )?;
        let d = self.mesh_drag.as_mut().unwrap();
        d.moving = true;
        d.delta = delta;
        self.status = format!(
            "Dragging {} vertices / {} / Esc cancels",
            self.mesh_vertices.len(),
            match d.axis {
                Some(a) => format!("{} axis locked", ['X', 'Y', 'Z'][a]),
                None => "X Y Z locks an axis".into(),
            }
        );
        self.preview = Some(preview);
        Ok(())
    }
    /// X/Y/Z during a vertex drag toggles the axis lock, as in the transform prompt.
    pub(super) fn mesh_drag_axis(&mut self, axis: usize) {
        if let Some(d) = self.mesh_drag.as_mut() {
            d.axis = if d.axis == Some(axis) {
                None
            } else {
                Some(axis)
            };
            if d.moving {
                let [x, y] = self.mouse;
                let result = self.mesh_motion(x, y);
                self.result(result);
            }
        }
    }
    pub(super) fn finish_mesh_drag(&mut self) -> Result<()> {
        if let Some(d) = self.mesh_drag.take() {
            self.preview = None;
            if !d.moving {
                self.mesh_vertices = vec![d.pressed];
                return Ok(());
            }
            if d.delta == [0; 3] {
                return Ok(());
            }
            if !self
                .doc
                .archive
                .entries
                .get(d.entry)
                .is_some_and(|e| e.same_storage(&d.original))
            {
                return Err("Shape changed during drag".into());
            }
            let moved = d.model.transform_selection(
                Transform::Translate(d.delta),
                Some(&self.mesh_vertices),
                [0; 3],
            )?;
            let n = self.commit_points(&d.model, &moved)?;
            self.status = format!(
                "{} moved / one undo step",
                view::count(n, "stored vertex", "stored vertices")
            );
        }
        Ok(())
    }
    /// Why vertex edits are unavailable, if they are.
    pub(super) fn mesh_blocked(&self) -> Option<String> {
        self.model.as_ref()?;
        if self.model_entry.is_none() {
            let owner = self
                .external_model
                .and_then(|(id, i)| {
                    let l = self.libraries.iter().find(|l| l.id == id)?;
                    let name = &l.doc.archive.entries.get(i)?.name;
                    Some(format!(
                        "{name} in {}",
                        l.path.rsplit(['/', '\\']).next().unwrap_or(&l.path)
                    ))
                })
                .unwrap_or_else(|| "another LIB".into());
            return Some(format!(
                "Shape is stored in {owner}; switch to that LIB to edit vertices"
            ));
        }
        None
    }
    pub(super) fn mesh_transform_prompt(&mut self, op: char) {
        if let Some(reason) = self.mesh_blocked() {
            self.status = reason;
            return;
        }
        if self.mesh_vertices.is_empty() {
            self.status = "Select vertices first".into();
            return;
        }
        self.transform_prompt(op);
    }
}

#[inline(never)]
fn smoke_app() -> Box<App> {
    Box::new(App::new())
}
#[inline(never)]
fn smoke_palette() -> Box<[[u8; 3]; 256]> {
    Box::new(Pic::parse(&picture::demo()).unwrap().colors(&[[0; 3]; 256]))
}
impl App {
    #[inline(never)]
    pub(super) fn smoke_object_tools(&mut self) {
        let mut a = smoke_app();
        a.demo();
        a.select_entry(1);
        a.act(Action::Mode(Mode::Properties));
        assert_eq!(
            a.field_group,
            Some(hangar_core::definition::Aspect::Envelope)
        );
        let rows = a.envelope_rows();
        assert_eq!(rows.len(), 3);
        let index = rows[0][4];
        let original = a.doc.archive.bytes().unwrap();
        // Envelope cells are NumberFields: a click types, a drag scrubs.
        let t = super::widgets::NumberTarget::Field(index);
        let hit = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::Number(n) if n == t) && h.rect[1] < a.dock_y())
            .unwrap();
        let (x, y) = (hit.rect[0] + hit.rect[2] / 2, hit.rect[1] + hit.rect[3] / 2);
        let before = a.number_spec(t).unwrap().value;
        a.motion(x, y, false);
        a.click(x, y, 1, true);
        a.motion(x - 10, y, false);
        a.click(x - 10, y, 1, false);
        assert_eq!(a.number_spec(t).unwrap().value, before - 5);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.click(x, y, 1, true);
        a.click(x, y, 1, false);
        assert!(a.prompt.is_some(), "Click types an envelope value");
        a.key(Key::Char('a'), true, false);
        for c in "125".chars() {
            a.key(Key::Char(c), false, false);
        }
        a.key(Key::Enter, false, false);
        assert_eq!(a.brf.as_ref().unwrap().fields[index].value, "125");
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let next = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::EnvelopeStep(1)))
            .unwrap();
        a.click(next.rect[0] + 4, next.rect[1] + 4, 1, true);
        assert_eq!(a.envelope_selected, 1);
        a.doc.replace(0, model::demo_shape()).unwrap();
        a.doc.mark_saved();
        a.palette_override = Some(smoke_palette());
        a.select_entry(0);
        a.mode = Mode::Model;
        a.textured = true;
        let original = a.doc.archive.bytes().unwrap();
        a.base_color_prompt(false);
        a.brush = 150;
        a.key(Key::Enter, false, false);
        assert!(a
            .model
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .all(|f| f.color == 150));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let x = (a.left() + a.right()) / 2;
        let y = (54 + a.dock_y()) / 2;
        let (face, uv) = a.model_hit(x, y).expect("Synthetic panel hit");
        // The textured raster frames on the committed shape, so a G preview moves it
        // away from the cursor exactly as the wireframe and vertex markers do.
        for c in "g300".chars() {
            a.key(Key::Char(c), false, false);
        }
        assert!(a.preview.is_some());
        assert!(a.model_hit(x, y).is_none(), "Textured G preview moves");
        a.key(Key::Escape, false, false);
        assert!(a.preview.is_none());
        assert_eq!(uv, [-1, -1]);
        a.selected_face = Some(face);
        a.model_paint = true;
        a.brush = 255;
        a.mouse = [x, y];
        a.paint_model_hit(face, uv);
        assert!(a.painting, "{}", a.status);
        assert!(a.panel_draft.is_some());
        assert!(!a.doc.dirty());
        a.draw();
        a.key(Key::Escape, false, false);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.selected_face = Some(face);
        a.paint_model_hit(face, uv);
        a.finish_stroke();
        assert!(a.doc.dirty());
        assert_eq!(a.doc.archive.entries.len(), 10);
        let generated = a.model.as_ref().unwrap().faces[face].texture.clone();
        assert!(a.doc.archive.find(&generated).is_some());
        let bytes = a.doc.archive.bytes().unwrap();
        assert!(Archive::parse(bytes).unwrap().find(&generated).is_some());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.model_paint = true;
        a.act(Action::MeshMode);
        assert!(!a.model_paint);
        assert!(a.mesh_edit);
        a.mesh_vertices = vec![0];
        let before = a.model.as_ref().unwrap().vertices[0].point;
        let type_keys = |a: &mut App, keys: &str| {
            for c in keys.chars() {
                a.key(Key::Char(c), false, false);
            }
        };
        type_keys(&mut a, "g2 3 4");
        a.key(Key::Enter, false, false);
        assert_eq!(
            a.model.as_ref().unwrap().vertices[0].point,
            [before[0] + 2, before[1] + 3, before[2] + 4]
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Edit-mode R and S act on the selection about its median, never the whole shape.
        let points = |a: &App| -> Vec<[i32; 3]> {
            a.model
                .as_ref()
                .unwrap()
                .vertices
                .iter()
                .map(|v| v.point)
                .collect()
        };
        let start = points(&a);
        a.mesh_vertices = vec![1, 2];
        type_keys(&mut a, "s200");
        a.key(Key::Enter, false, false);
        let scaled = points(&a);
        assert_eq!(scaled[1], [-160, -35, 0]);
        assert_eq!(scaled[2], [160, -35, 0]);
        assert!((0..start.len())
            .filter(|i| ![1, 2].contains(i))
            .all(|i| scaled[i] == start[i]));
        a.act(Action::Undo);
        type_keys(&mut a, "rz90");
        a.key(Key::Enter, false, false);
        let turned = points(&a);
        assert_eq!(turned[1], [0, -115, 0]);
        assert_eq!(turned[2], [0, 45, 0]);
        assert_eq!(turned[0], start[0]);
        assert_eq!(a.mesh_vertices, vec![1, 2], "Selection survives the edit");
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert_eq!(a.mesh_vertices, vec![1, 2], "Selection survives undo");
        a.key(Key::Char('a'), false, false);
        assert_eq!(a.mesh_vertices.len(), start.len());
        a.key(Key::Char('a'), false, false);
        assert!(a.mesh_vertices.is_empty());
        a.key(Key::Char('g'), false, false);
        assert!(a.prompt.is_none() && a.status.contains("Select vertices"));
        // Object mode: S with no axis lock scales uniformly about the origin.
        a.mesh_edit = false;
        type_keys(&mut a, "s50");
        a.key(Key::Enter, false, false);
        let halved = points(&a);
        assert!((0..start.len()).all(|i| halved[i] == start[i].map(|n| n * 50 / 100)));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.mesh_edit = true;
        a.mesh_vertices = vec![0];
        let [x, y] = a.hp_project(before).unwrap();
        let project = |a: &App, i: usize| {
            a.hp_project(a.preview.as_ref().or(a.model.as_ref()).unwrap().vertices[i].point)
                .unwrap()
        };
        // A click below the drag threshold selects without editing.
        a.mesh_vertices = vec![0, 1];
        a.pointer(x + 1, y + 1, 1, true, false);
        assert_eq!(
            a.mesh_vertices,
            vec![0, 1],
            "Press keeps a selection to drag"
        );
        a.motion(x + 3, y + 2, false);
        assert!(a.preview.is_none());
        a.pointer(x + 3, y + 2, 1, false, false);
        assert!(!a.doc.dirty());
        assert_eq!(
            a.mesh_vertices,
            vec![0],
            "Click selects only the pressed vertex"
        );
        // Shift+click extends and toggles the selection.
        let [x1, y1] = project(&a, 1);
        a.pointer(x1, y1, 1, true, true);
        a.pointer(x1, y1, 1, false, true);
        assert_eq!(a.mesh_vertices, vec![1, 0]);
        a.pointer(x1, y1, 1, true, true);
        a.pointer(x1, y1, 1, false, true);
        assert_eq!(a.mesh_vertices, vec![0]);
        a.pointer(x1, y1, 1, true, true);
        a.pointer(x1, y1, 1, false, true);
        // Dragging keeps the grab offset rather than snapping the vertex to the cursor.
        a.pointer(x + 4, y + 4, 1, true, false);
        a.motion(x + 14, y + 9, false);
        assert!(a.preview.is_some());
        assert!(!a.doc.dirty());
        // Source units are coarser than pixels; allow their rounding.
        let [px, py] = project(&a, 0);
        assert!(
            (px - (x + 10)).abs() <= 2 && (py - (y + 5)).abs() <= 2,
            "Grab offset"
        );
        let moved = |a: &App, i: usize| {
            let (p, m) = (a.preview.as_ref().unwrap(), a.model.as_ref().unwrap());
            core::array::from_fn::<i32, 3, _>(|k| p.vertices[i].point[k] - m.vertices[i].point[k])
        };
        assert_eq!(moved(&a, 0), moved(&a, 1), "Selection moves together");
        a.key(Key::Char('z'), true, false);
        assert!(a.mesh_drag.is_none());
        assert!(a.preview.is_none());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.pointer(x + 2, y + 1, 1, true, false);
        a.motion(x + 12, y + 6, false);
        a.key(Key::Char('x'), false, false);
        let locked = a.mesh_drag.as_ref().unwrap().delta;
        assert!(
            locked[0] != 0 && locked[1] == 0 && locked[2] == 0,
            "X axis lock"
        );
        a.pointer(x + 12, y + 6, 1, false, false);
        assert!(a.doc.dirty());
        assert_eq!(a.mesh_vertices.len(), 2);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        assert_eq!(a.mesh_vertices.len(), 2, "Selection survives undo");
        a.act(Action::Redo);
        assert!(a.doc.dirty());
        a.act(Action::Undo);
        a.mesh_vertices = vec![0, 999];
        a.refresh();
        assert_eq!(a.mesh_vertices, vec![0], "Stale vertex indices are dropped");
        a.perspective = true;
        a.textured = false;
        a.pointer(x, y, 1, true, false);
        assert!(a.mesh_drag.is_none() && a.status.contains("orthographic"));
        a.pointer(x, y, 1, false, false);
        a.perspective = false;
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            a.select_entry(1);
            a.mode = Mode::Properties;
            a.field_group = Some(hangar_core::definition::Aspect::Envelope);
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Envelope control bounds"
                );
            }
            a.select_entry(0);
            a.mode = Mode::Model;
            a.mesh_edit = true;
            a.mesh_vertices = vec![0];
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Mesh control bounds"
                );
            }
            a.base_color_prompt(false);
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Palette dialog bounds"
                );
            }
            a.key(Key::Escape, false, false);
        }
    }
}

#[cfg(not(windows))]
impl App {
    pub fn check_panel_repair(&mut self) -> Result<()> {
        self.mode = Mode::Model;
        self.textured = true;
        self.perspective = false;
        let before = self.doc.archive.bytes()?;
        let images = |app: &App| {
            app.draw()
                .commands
                .into_iter()
                .filter_map(|d| {
                    if let Draw::Bitmap(_, _, _, _, p) = d {
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let pixels = images(self);
        self.repair_panels()?;
        if !self.doc.dirty() {
            return Err("Repair did not change a legacy SH".into());
        }
        if images(self) != pixels {
            return Err("Repair changed viewport pixels".into());
        }
        let repaired = self.doc.archive.bytes()?;
        self.doc.undo();
        self.refresh();
        if self.doc.archive.bytes()? != before {
            return Err("Repair undo did not restore archive".into());
        }
        self.doc.redo();
        self.refresh();
        if self.doc.archive.bytes()? != repaired {
            return Err("Repair redo differs".into());
        }
        Ok(())
    }
}
