//! Initial SH edit-mode tools and transactional panel texture creation.
use super::view::{border, label_fit, text_fit, Action, Layout};
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
}
impl App {
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
            pic,
            last: None,
            original: None,
        });
        self.panel_draft = Some(Box::new(plan));
        self.painting = true;
        Ok(())
    }
    pub(super) fn mesh_overlay(&self, o: &mut Layout) {
        if !self.mesh_edit {
            return;
        }
        let Some(model) = self.preview.as_ref().or(self.model.as_ref()) else {
            return;
        };
        for (i, v) in model.vertices.iter().enumerate() {
            let Some([x, y]) = self.hp_project(v.point) else {
                continue;
            };
            if x < self.left() + 40 || x >= self.right() - 8 || y < 60 || y >= self.dock_y() - 10 {
                continue;
            }
            let selected = self.mesh_vertices.contains(&i);
            let c = if selected { c::AMBER } else { c::STEEL };
            o.canvas.rect(x - 2, y - 2, 5, 5, c);
            if selected {
                border(&mut o.canvas, x - 4, y - 4, 9, 9, c);
            }
            o.hit([x - 5, y - 5, 11, 11], Action::MeshVertex(i));
        }
    }
    pub(super) fn mesh_select(&mut self, i: usize, drag: bool) {
        self.mesh_vertices = vec![i];
        self.selected_face = None;
        if !drag {
            return;
        }
        let Some(entry) = self.model_entry else {
            return;
        };
        let Some(model) = &self.model else {
            return;
        };
        if !model.writable {
            self.status = model.reason.clone();
            return;
        }
        if let Some(v) = model.vertices.get(i) {
            self.mesh_drag = Some(Box::new(MeshDrag {
                entry,
                original: self.doc.archive.entries[entry].clone(),
                model: model.clone(),
                reference: v.point,
                delta: [0; 3],
            }));
        }
    }
    pub(super) fn mesh_motion(&mut self, x: i32, y: i32) -> Result<()> {
        let d = self.mesh_drag.as_ref().ok_or("No mesh drag")?;
        let point = self.cursor_station(x, y, d.reference)?;
        let delta = core::array::from_fn(|k| point[k] - d.reference[k]);
        let mut preview = d.model.clone();
        for i in &self.mesh_vertices {
            for (k, n) in delta.iter().enumerate() {
                preview.vertices[*i].point[k] = d.model.vertices[*i].point[k]
                    .checked_add(*n)
                    .ok_or("Coordinate overflow")?;
            }
        }
        self.mesh_drag.as_mut().unwrap().delta = delta;
        self.preview = Some(preview);
        Ok(())
    }
    pub(super) fn finish_mesh_drag(&mut self) -> Result<()> {
        if let Some(d) = self.mesh_drag.take() {
            self.preview = None;
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
            let bytes =
                shape_edit::move_vertices(&d.original.read()?, &self.mesh_vertices, d.delta)?;
            self.doc.replace(d.entry, bytes)?;
            self.refresh();
            self.status = "Vertices moved / one undo step".into();
        }
        Ok(())
    }
    pub(super) fn mesh_move_prompt(&mut self) {
        let Some(model) = &self.model else {
            return;
        };
        if !model.writable {
            self.status = model.reason.clone();
            return;
        }
        if self.mesh_vertices.is_empty() {
            self.status = "Select vertices first".into();
            return;
        }
        self.prompt = Some(Prompt {
            kind: PromptKind::MeshMove,
            title: "Move selected vertices / source offsets X Y Z".into(),
            value: "0 0 0".into(),
            axis: 0,
        });
    }
    pub(super) fn mesh_move(&mut self, text: &str) -> Result<()> {
        let entry = self.model_entry.ok_or("Open the SH owner before editing")?;
        let n: Vec<i32> = text
            .split_whitespace()
            .map(str::parse)
            .collect::<core::result::Result<_, _>>()
            .map_err(|_| "Enter three integer offsets")?;
        let delta: [i32; 3] = n.try_into().map_err(|_| "Enter X Y Z offsets")?;
        let bytes = shape_edit::move_vertices(
            &self.doc.archive.entries[entry].read()?,
            &self.mesh_vertices,
            delta,
        )?;
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Selected vertices moved / Ctrl+Z undo".into();
        Ok(())
    }
    pub(super) fn mesh_inspector(&self, o: &mut Layout) {
        let (r, w, h) = (self.right(), self.width - self.right(), self.height);
        o.canvas
            .label(r + 12, 44, "SH EDIT MODE / source records", c::INK);
        let Some(model) = &self.model else {
            return;
        };
        label_fit(
            &mut o.canvas,
            r + 12,
            77,
            w - 24,
            &format!(
                "{} vertices / {} faces",
                model.vertices.len(),
                model.faces.len()
            ),
            c::INK,
        );
        label_fit(
            &mut o.canvas,
            r + 12,
            103,
            w - 24,
            &format!("{} decoded records", model.records.len()),
            c::STEEL,
        );
        o.button(
            [r + 12, 122, w - 24, 24],
            "Select all vertices (A)",
            Action::MeshAll,
            false,
        );
        if let Some(i) = self
            .mesh_vertices
            .first()
            .copied()
            .filter(|i| *i < model.vertices.len())
        {
            let vertex = &self.preview.as_ref().unwrap_or(model).vertices[i];
            text_fit(
                &mut o.canvas,
                r + 12,
                174,
                w - 24,
                &format!("Vertex {} / {} selected", i + 1, self.mesh_vertices.len()),
                c::AMBER,
            );
            for k in 0..3 {
                text_fit(
                    &mut o.canvas,
                    r + 12,
                    202 + k as i32 * 26,
                    w - 24,
                    &format!("{}  {}", ['X', 'Y', 'Z'][k], vertex.point[k]),
                    c::INK,
                );
            }
            text_fit(
                &mut o.canvas,
                r + 12,
                294,
                w - 24,
                &format!("Source +0x{:X}", vertex.offset),
                c::INK_MUTED,
            );
        }
        if model.writable {
            o.button(
                [r + 12, 330, w - 24, 26],
                "Move selection (G)",
                Action::MeshMove,
                false,
            );
            label_fit(
                &mut o.canvas,
                r + 12,
                386,
                w - 24,
                "Drag in orthographic view.",
                c::INK_MUTED,
            );
            label_fit(
                &mut o.canvas,
                r + 12,
                410,
                w - 24,
                "Normals/centers update on apply.",
                c::INK_MUTED,
            );
        } else {
            for (i, line) in super::dependencies_ui::wrap(&model.reason, ((w - 24) / 7) as usize)
                .iter()
                .take(7)
                .enumerate()
            {
                label_fit(
                    &mut o.canvas,
                    r + 12,
                    340 + i as i32 * 20,
                    w - 24,
                    line,
                    c::AMBER,
                );
            }
        }
        o.button(
            [r + 12, h - 56, w - 24, 26],
            "Object mode (Tab)",
            Action::MeshMode,
            false,
        );
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
        let hit = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action,Action::Field(i) if i==index) && h.rect[1] < a.dock_y())
            .unwrap();
        a.click(hit.rect[0] + 4, hit.rect[1] + 4, 1, true);
        a.key(Key::Char('a'), true, false);
        for c in "125".chars() {
            a.key(Key::Char(c), false, false);
        }
        a.key(Key::Enter, false, false);
        assert_eq!(a.brf.as_ref().unwrap().fields[index].value, "125");
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.act(Action::EnvelopeStep(1));
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
        a.mesh_move("2 3 4").unwrap();
        assert_eq!(
            a.model.as_ref().unwrap().vertices[0].point,
            [before[0] + 2, before[1] + 3, before[2] + 4]
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let [x, y] = a.hp_project(before).unwrap();
        a.mesh_select(0, true);
        a.mesh_motion(x + 12, y + 6).unwrap();
        assert!(a.preview.is_some());
        assert!(!a.doc.dirty());
        a.key(Key::Char('z'), true, false);
        assert!(a.mesh_drag.is_none());
        assert!(a.preview.is_none());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.mesh_select(0, true);
        a.mesh_motion(x + 12, y + 6).unwrap();
        a.click(x + 12, y + 6, 1, false);
        assert!(a.doc.dirty());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
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
