//! Multiple documents share immutable payload buffers and retain independent history.
use super::view::{border, label_fit, text_fit, Action, Layout};
use super::*;
use hangar_core::resource_ops::{self, Choice};

pub(super) struct Library {
    pub id: u64,
    pub path: String,
    pub file_backed: bool,
    pub doc: Document,
    pub dependencies: hangar_core::dependencies::Index,
    selected: usize,
    field_state: (usize, usize, Option<hangar_core::definition::Aspect>, i32),
    context_model: Option<Model>,
    context_entry: Option<usize>,
    selected_face: Option<usize>,
    mode: Mode,
    camera: (i32, i32, i32, [i32; 2], bool),
    image: (i32, [i32; 2]),
    pub(super) filter: String,
    pub(super) collapsed: [bool; 9],
    pub(super) root_collapsed: bool,
    category: Option<usize>,
    required: Vec<String>,
    suggested: Option<String>,
    palette: Option<Box<[[u8; 3]; 256]>>,
    dock: u8,
    scroll: usize,
    table_scroll: usize,
}
pub(super) struct Clipboard {
    pub archive: Archive,
    additional: Vec<Archive>,
    pub name: String,
    pub path: String,
}
fn normalized(path: &str) -> String {
    let path = path.replace('\\', "/");
    let absolute = if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        path
    } else {
        format!("{}/{}", crate::platform::current_dir(), path)
    };
    let absolute = absolute.replace('\\', "/");
    let mut parts = Vec::new();
    for part in absolute.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let p = format!("/{}", parts.join("/"));
    if cfg!(windows) {
        p.to_ascii_uppercase()
    } else {
        p
    }
}
impl App {
    pub(super) fn resolve_resource(&self, name: &str) -> Option<(u64, usize, &Entry)> {
        if let Some(i) = self.doc.archive.find(name) {
            return Some((self.library_id, i, &self.doc.archive.entries[i]));
        }
        let mut matches = self.libraries.iter().filter_map(|l| {
            l.doc
                .archive
                .find(name)
                .map(|i| (l.id, i, &l.doc.archive.entries[i]))
        });
        let entry = matches.next()?;
        if matches.next().is_some() {
            None
        } else {
            Some(entry)
        }
    }
    pub(super) fn tree_start(&self) -> i32 {
        56
    }
    fn capture_library(&mut self) -> Library {
        Library {
            id: self.library_id,
            file_backed: self.file_backed,
            field_state: (
                self.field_scroll,
                self.field_selected,
                self.field_group,
                self.inspector_scroll,
            ),
            context_model: self.context_model.take(),
            context_entry: self.context_entry.take(),
            selected_face: self.selected_face.take(),
            dependencies: core::mem::take(&mut self.dependencies),
            path: core::mem::take(&mut self.path),
            doc: core::mem::replace(&mut self.doc, Document::new(Archive::empty())),
            selected: self.selected,
            mode: self.mode,
            camera: (self.yaw, self.pitch, self.zoom, self.pan, self.perspective),
            image: (self.image_zoom, self.image_pan),
            filter: core::mem::take(&mut self.filter),
            collapsed: self.collapsed,
            root_collapsed: self.root_collapsed,
            category: self.category,
            required: core::mem::take(&mut self.required),
            suggested: self.suggested_output.take(),
            palette: self.palette_override.take(),
            dock: self.dock,
            scroll: self.scroll,
            table_scroll: self.table_scroll,
        }
    }
    fn restore_library(&mut self, library: Library) {
        self.library_id = library.id;
        self.file_backed = library.file_backed;
        self.path = library.path;
        self.doc = library.doc;
        self.dependencies = library.dependencies;
        self.selected = library.selected;
        self.mode = library.mode;
        self.filter = library.filter;
        self.collapsed = library.collapsed;
        self.root_collapsed = library.root_collapsed;
        self.category = library.category;
        self.required = library.required;
        self.suggested_output = library.suggested;
        self.palette_override = library.palette;
        self.dock = library.dock;
        self.scroll = library.scroll;
        self.table_scroll = library.table_scroll;
        self.context_model = library.context_model;
        self.context_entry = library.context_entry;
        self.selected_face = library.selected_face;
        self.model_paint = false;
        self.mesh_edit = false;
        self.animation_tool = false;
        self.animation_state.clear();
        self.mesh_vertices.clear();
        self.envelope_selected = 0;
        self.envelope_scroll = 0;
        self.refresh();
        (self.yaw, self.pitch, self.zoom, self.pan, self.perspective) = library.camera;
        (self.image_zoom, self.image_pan) = library.image;
        (
            self.field_scroll,
            self.field_selected,
            self.field_group,
            self.inspector_scroll,
        ) = library.field_state;
    }
    pub(super) fn switch_library(&mut self, id: u64) -> Result<()> {
        if id == self.library_id {
            return Ok(());
        }
        self.finish_stroke();
        let at = self
            .libraries
            .iter()
            .position(|l| l.id == id)
            .ok_or("LIB is no longer open")?;
        let scroll = self.scroll;
        let incoming = self.libraries.remove(at);
        let current = self.capture_library();
        self.libraries.insert(at, current);
        self.restore_library(incoming);
        self.scroll = scroll;
        self.status = "Active LIB changed / edits and undo history retained".into();
        Ok(())
    }
    pub(super) fn other_library_at(&self, path: &str) -> bool {
        self.libraries
            .iter()
            .any(|l| l.file_backed && normalized(&l.path) == normalized(path))
    }
    pub(super) fn open_existing_library(&mut self, path: &str) -> Result<bool> {
        let name = normalized(path);
        if self.file_backed && normalized(&self.path) == name && !self.path.is_empty() {
            return Ok(true);
        }
        if let Some(id) = self
            .libraries
            .iter()
            .find(|l| l.file_backed && normalized(&l.path) == name)
            .map(|l| l.id)
        {
            self.switch_library(id)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn install_library(&mut self, doc: Document, path: String) -> Result<()> {
        self.finish_stroke();
        if !self.path.is_empty() || !self.doc.archive.entries.is_empty() {
            let current = self.capture_library();
            self.libraries.push(current);
        }
        self.library_id = self.next_library_id;
        self.file_backed = false;
        self.next_library_id += 1;
        self.doc = doc;
        self.path = path;
        self.mesh_edit = false;
        self.mesh_vertices.clear();
        self.animation_tool = false;
        self.animation_state.clear();
        self.hp_tool = false;
        self.hp_drag = None;
        self.mesh_drag = None;
        self.model_paint = false;
        self.paint_enabled = false;
        self.selected = 0;
        self.category = None;
        self.collapsed = [true; 9];
        self.root_collapsed = false;
        self.scroll = 0;
        self.table_scroll = 0;
        self.field_group = None;
        self.field_scroll = 0;
        self.inspector_scroll = 0;
        self.required.clear();
        self.suggested_output = None;
        self.palette_override = None;
        self.scroll = self
            .library_rows()
            .iter()
            .position(|(id, cat, _)| *id == self.library_id && cat.is_none())
            .unwrap_or(0);
        Ok(())
    }
    pub(super) fn any_dirty(&self) -> bool {
        self.doc.dirty() || self.libraries.iter().any(|l| l.doc.dirty())
    }
    pub(super) fn close_library(&mut self) -> Result<()> {
        self.finish_stroke();
        if self.doc.dirty() {
            self.prompt = Some(Prompt {
                kind: PromptKind::CloseLibrary,
                title: "Close active LIB without saving?".into(),
                value: String::new(),
                axis: 0,
            });
            return Ok(());
        }
        self.discard_library();
        Ok(())
    }
    pub(super) fn discard_library(&mut self) {
        if let Some(library) = self.libraries.pop() {
            self.restore_library(library);
        } else {
            self.doc = Document::new(Archive::empty());
            self.path.clear();
            self.file_backed = false;
            self.selected = 0;
            self.context_model = None;
            self.context_entry = None;
            self.refresh();
        }
        self.status = "Closed LIB; other open LIBs retained".into();
    }
    pub(super) fn copy_resource(&mut self) -> Result<()> {
        self.transfer_source = None;
        self.transfer_move = false;
        self.finish_stroke();
        if self.doc.archive.entries.get(self.selected).is_none() {
            return Err("Select a resource to copy".into());
        }
        self.clipboard = Some(Clipboard {
            archive: self.doc.archive.clone(),
            additional: self
                .libraries
                .iter()
                .map(|l| l.doc.archive.clone())
                .collect(),
            name: self.name().into(),
            path: self.path.clone(),
        });
        self.status = format!(
            "Copied {} snapshot / select a target LIB and Paste resources",
            self.name()
        );
        Ok(())
    }
    pub(super) fn paste_resources(&mut self) -> Result<()> {
        let source = self
            .clipboard
            .as_ref()
            .ok_or("Copy a source resource first")?;
        let mut sources = vec![&source.archive];
        sources.extend(source.additional.iter());
        self.transfer_plan = Some(resource_ops::transfer_from(
            &sources,
            &self.doc.archive,
            &source.name,
            self.include_dependencies,
        )?);
        if self.transfer_source.is_some() {
            self.transfer_plan.as_mut().unwrap().notes.push("When moving, known shared dependencies stay in the source. Linked files supplied by other LIBs are copied. Each changed LIB has its own undo step.".into());
        }
        self.transfer_is_copy = true;
        self.transfer_scroll = 0;
        self.transfer_note = 0;
        self.prompt = Some(Prompt {
            kind: PromptKind::TransferReview,
            title: format!(
                "{} {} from {}",
                if self.transfer_source.is_some() {
                    "Transfer"
                } else {
                    "Copy"
                },
                source.name,
                source
                    .path
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or(&source.path)
            ),
            value: String::new(),
            axis: 0,
        });
        Ok(())
    }
    pub(super) fn rename_prompt(&mut self, duplicate: bool) {
        self.transfer_source = None;
        self.transfer_move = false;
        self.prompt = Some(Prompt {
            kind: PromptKind::ResourceName(duplicate),
            title: if duplicate {
                "Duplicate resource / new 8.3 name".into()
            } else {
                "Rename resource and reviewed references / new 8.3 name".into()
            },
            value: self.name().into(),
            axis: 0,
        });
    }
    pub(super) fn review_resource_name(&mut self, name: &str, duplicate: bool) -> Result<()> {
        if !duplicate
            && self.libraries.iter().any(|l| {
                l.doc.archive.find(self.name()).is_none()
                    && l.dependencies.incoming(self.name()).next().is_some()
            })
        {
            return Err("Another open LIB references this resource; isolate it or update that LIB before renaming".into());
        }
        self.transfer_plan = Some(resource_ops::rename(
            &self.doc.archive,
            self.name(),
            name.trim(),
            duplicate,
        )?);
        self.transfer_scroll = 0;
        self.transfer_note = 0;
        self.transfer_is_copy = false;
        self.prompt = Some(Prompt {
            kind: PromptKind::TransferReview,
            title: format!(
                "{} {} -> {}",
                if duplicate { "Duplicate" } else { "Rename" },
                self.name(),
                name.trim().to_ascii_uppercase()
            ),
            value: String::new(),
            axis: 0,
        });
        Ok(())
    }
    pub(super) fn apply_transfer(&mut self) -> Result<()> {
        let plan = self.transfer_plan.as_ref().ok_or("No resource review")?;
        let first = plan.items.first().map(|i| i.entry.name.clone());
        let moved = if self.transfer_move {
            let id = self.transfer_source.ok_or("No move source")?;
            let source = self
                .libraries
                .iter_mut()
                .find(|l| l.id == id)
                .ok_or("Source LIB is no longer open")?;
            let root = &self
                .clipboard
                .as_ref()
                .ok_or("No move source snapshot")?
                .name;
            Some(resource_ops::move_between(
                &mut source.doc,
                &mut self.doc,
                plan,
                root,
            )?)
        } else {
            plan.apply(&mut self.doc)?;
            None
        };
        if let Some(name) = first {
            if let Some(i) = self.doc.archive.find(&name) {
                self.selected = i;
            }
        }
        self.transfer_plan = None;
        self.refresh();
        self.status = moved.map_or("Resources copied / one undo step".into(), |n| {
            format!(
                "Moved {n} source entries / shared dependencies kept / undo available in each LIB"
            )
        });
        self.transfer_source = None;
        self.transfer_move = false;
        Ok(())
    }
    pub(super) fn transfer_note_pages(&self, w: i32) -> Vec<Vec<String>> {
        let mut pages = Vec::new();
        if let Some(plan) = &self.transfer_plan {
            for note in &plan.notes {
                let note = if self.transfer_move {
                    note.replace("Copies only", "Moves only")
                } else {
                    note.clone()
                };
                let lines = super::dependencies_ui::wrap(&note, ((w - 28) / 7).max(16) as usize);
                for chunk in lines.chunks(3) {
                    pages.push(chunk.to_vec());
                }
            }
        }
        pages
    }
    pub(super) fn transfer_review(&self, o: &mut Layout) {
        let Some(plan) = &self.transfer_plan else {
            return;
        };
        let (w, h) = ((self.width - 32).min(900), (self.height - 48).min(650));
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        o.canvas.rect(x, y, w, h, c::GM_800);
        border(&mut o.canvas, x, y, w, h, c::LINE_STRONG);
        label_fit(
            &mut o.canvas,
            x + 14,
            y + 25,
            w - 28,
            self.prompt
                .as_ref()
                .map_or("Resource review", |p| p.title.as_str()),
            c::INK,
        );
        label_fit(
            &mut o.canvas,
            x + 14,
            y + 49,
            w - 28,
            &if self.transfer_move {
                format!(
                    "{} incoming / shared links kept at source / one undo per LIB",
                    plan.items.len()
                )
            } else {
                format!(
                    "Target: {} / {} resources / {} removals / 1 undo step",
                    self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path),
                    plan.items.len(),
                    plan.removals.len()
                )
            },
            c::STEEL,
        );
        if self.transfer_is_copy {
            o.button(
                [
                    x + 14,
                    y + 60,
                    if self.transfer_source.is_some() {
                        w - 226
                    } else {
                        w - 28
                    },
                    24,
                ],
                if self.include_dependencies {
                    "[x] Object and linked files"
                } else {
                    "[ ] Item only / click for linked files"
                },
                Action::TransferDependencies,
                self.include_dependencies,
            );
        }
        if self.transfer_source.is_some() {
            o.button(
                [x + w - 200, y + 60, 88, 24],
                "Copy",
                Action::TransferMove(false),
                !self.transfer_move,
            );
            o.button(
                [x + w - 104, y + 60, 90, 24],
                "Move",
                Action::TransferMove(true),
                self.transfer_move,
            );
        }
        let visible = ((h - 235) / 46).max(1) as usize;
        for (row, item) in plan
            .items
            .iter()
            .skip(self.transfer_scroll)
            .take(visible)
            .enumerate()
        {
            let i = row + self.transfer_scroll;
            let yy = y + 96 + row as i32 * 46;
            o.canvas.rect(x + 12, yy, w - 24, 43, c::GM_900);
            text_fit(
                &mut o.canvas,
                x + 22,
                yy + 16,
                w - 260,
                &item.entry.name,
                c::INK,
            );
            label_fit(
                &mut o.canvas,
                x + 22,
                yy + 35,
                w - 260,
                if item.conflict {
                    "Different target resource"
                } else if item.previous.is_some() {
                    "Existing / unchanged or rewritten"
                } else {
                    "New resource"
                },
                c::INK_MUTED,
            );
            if item.conflict {
                o.button(
                    [x + w - 236, yy + 9, 102, 25],
                    "Keep target",
                    Action::TransferChoice(i, false),
                    item.choice == Choice::KeepTarget,
                );
                o.button(
                    [x + w - 126, yy + 9, 102, 25],
                    "Take source",
                    Action::TransferChoice(i, true),
                    item.choice == Choice::TakeSource,
                );
            } else {
                label_fit(
                    &mut o.canvas,
                    x + w - 212,
                    yy + 25,
                    184,
                    if item.choice == Choice::TakeSource {
                        "Apply"
                    } else {
                        "Keep identical target"
                    },
                    c::STEEL,
                );
            }
        }
        let pages = self.transfer_note_pages(w);
        let page = self.transfer_note.min(pages.len().saturating_sub(1));
        if let Some(lines) = pages.get(page) {
            for (i, line) in lines.iter().enumerate() {
                text_fit(
                    &mut o.canvas,
                    x + 14,
                    y + h - 130 + i as i32 * 18,
                    w - 28,
                    line,
                    c::AMBER,
                );
            }
        }
        if pages.len() > 1 {
            o.button(
                [x + w - 152, y + h - 83, 138, 24],
                &format!("Notes {}/{}", page + 1, pages.len()),
                Action::TransferNote,
                false,
            );
        }
        label_fit(
            &mut o.canvas,
            x + 14,
            y + h - 65,
            w - 184,
            "Wheel scroll / one undo step",
            c::INK_FAINT,
        );
        o.button(
            [x + 14, y + h - 47, 92, 28],
            "Cancel",
            Action::Cancel,
            false,
        );
        if plan.ready() {
            o.button(
                [x + w - 168, y + h - 47, 154, 28],
                if self.transfer_move {
                    "Move resources"
                } else {
                    "Apply resources"
                },
                Action::Apply,
                true,
            );
        } else {
            label_fit(
                &mut o.canvas,
                x + 126,
                y + h - 27,
                w - 144,
                "Resolve all collisions to apply",
                c::AMBER,
            );
        }
    }
}

impl App {
    #[inline(never)]
    pub(super) fn smoke_libraries(&mut self) {
        let mut app = App::new();
        app.demo();
        app.path = "SOURCE.LIB".into();
        let source_id = app.library_id;
        app.select_entry(1);
        let b = app.brf.as_ref().unwrap();
        let weight = b
            .fields
            .iter()
            .position(|f| f.label == "object.weight")
            .unwrap();
        let changed = b.edit(&app.data, weight, "22222", "PT").unwrap();
        app.doc.replace(1, changed).unwrap();
        app.select_entry(0);
        app.zoom = 217;
        app.yaw = 41;
        let source_bytes = app.doc.archive.bytes().unwrap();
        let mut target = Archive::empty();
        let mut pic = picture::demo();
        pic.push(0);
        target.entries.push(Entry::new("DEMO.PIC", pic).unwrap());
        let target_bytes = target.bytes().unwrap();
        app.install_library(Document::new(target), "TARGET.LIB".into())
            .unwrap();
        let target_id = app.library_id;
        app.refresh();
        app.switch_library(source_id).unwrap();
        assert!(app.doc.dirty());
        assert_eq!(app.zoom, 217);
        assert_eq!(app.yaw, 41);
        assert_eq!(app.selected, 0);
        app.copy_resource().unwrap();
        app.switch_library(target_id).unwrap();
        app.paste_resources().unwrap();
        assert!(!app.transfer_plan.as_ref().unwrap().ready());
        for (w, h) in [(800, 600), (1280, 800)] {
            app.width = w;
            app.height = h;
            for hit in app.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h
                );
            }
        }
        let collision = app
            .transfer_plan
            .as_ref()
            .unwrap()
            .items
            .iter()
            .position(|i| i.conflict)
            .unwrap();
        app.act(Action::TransferChoice(collision, true));
        app.key(Key::Enter, false, false);
        assert!(app.prompt.is_none());
        assert!(app.doc.archive.find("DEMO.SH").is_some());
        assert!(app.doc.dirty());
        assert_eq!(
            app.libraries
                .iter()
                .find(|l| l.id == source_id)
                .unwrap()
                .doc
                .archive
                .bytes()
                .unwrap(),
            source_bytes
        );
        app.act(Action::Undo);
        assert_eq!(app.doc.archive.bytes().unwrap(), target_bytes);
        assert!(!app.doc.dirty());
        app.close();
        assert!(!app.quit && app.prompt.is_some());
        app.key(Key::Escape, false, false);
        app.switch_library(source_id).unwrap();
        app.act(Action::Undo);
        assert!(!app.doc.dirty());
        // Drag a source resource onto the other LIB root, then cancel its review.
        app.width = 800;
        app.height = 600;
        app.select_entry(0);
        app.mode = Mode::Model;
        let from = app
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::Entry(0)) && h.rect[0] == 0)
            .unwrap();
        app.click(from.rect[0] + 70, from.rect[1] + 8, 1, true);
        let to = app
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action,Action::Library(id) if id==target_id))
            .unwrap();
        app.motion(to.rect[0] + 70, to.rect[1] + 8, false);
        app.click(to.rect[0] + 70, to.rect[1] + 8, 1, false);
        assert_eq!(app.library_id, target_id);
        assert!(matches!(
            app.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::TransferReview)
        ));
        app.key(Key::Escape, false, false);
        // A model defined in a third LIB can resolve one unique open-library shape.
        let mut third = Archive::empty();
        third
            .entries
            .push(Entry::new("THIRD.PT", hangar_core::brf::demo()).unwrap());
        app.install_library(Document::new(third), "THIRD.LIB".into())
            .unwrap();
        app.refresh();
        assert!(app.model.is_some());
        assert_eq!(app.external_model, Some((source_id, 0)));
        // Edit mode on another LIB's shape explains itself instead of ignoring the press.
        app.mode = Mode::Model;
        app.mesh_edit = true;
        app.mesh_press(0, false);
        assert!(app.mesh_drag.is_none());
        assert!(app.status.contains("switch to that LIB"), "{}", app.status);
        app.key(Key::Char('g'), false, false);
        assert!(app.prompt.is_none());
        app.mesh_edit = false;
        assert!(app
            .package_report()
            .checks
            .iter()
            .any(|c| c.message.contains("external provider")));
        // Normalized paths reuse documents and Save As cannot overwrite another one.
        app.file_backed = true;
        assert!(app.open_existing_library("./THIRD.LIB").unwrap());
        app.switch_library(target_id).unwrap();
        assert!(app.other_library_at("THIRD.LIB"));
        for (w, h) in [(800, 600), (1280, 800)] {
            app.width = w;
            app.height = h;
            for mode in [
                Mode::Browse,
                Mode::Model,
                Mode::Properties,
                Mode::Graft,
                Mode::Package,
                Mode::Media,
            ] {
                app.mode = mode;
                for hit in app.layout().hits {
                    assert!(
                        hit.rect[0] >= 0
                            && hit.rect[1] >= 0
                            && hit.rect[0] + hit.rect[2] <= w
                            && hit.rect[1] + hit.rect[3] <= h
                    );
                }
            }
        }
    }
}

impl App {
    pub(super) fn library_rows(&self) -> Vec<(u64, Option<usize>, Option<usize>)> {
        let mut libraries = vec![(
            self.library_id,
            &self.doc,
            self.root_collapsed,
            &self.collapsed,
            &self.filter,
        )];
        libraries.extend(
            self.libraries
                .iter()
                .map(|l| (l.id, &l.doc, l.root_collapsed, &l.collapsed, &l.filter)),
        );
        libraries.sort_unstable_by_key(|l| l.0);
        let mut rows = Vec::new();
        for (id, doc, root, collapsed, filter) in libraries {
            rows.push((id, None, None));
            if root {
                continue;
            }
            let filter = filter.to_ascii_uppercase();
            for (cat, closed) in collapsed.iter().enumerate() {
                let entries: Vec<_> = doc
                    .archive
                    .entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| {
                        super::view::category_of(&e.name) == cat && e.name.contains(&filter)
                    })
                    .map(|(i, _)| i)
                    .collect();
                if entries.is_empty() {
                    continue;
                }
                rows.push((id, Some(cat), None));
                if !closed || !filter.is_empty() {
                    rows.extend(entries.into_iter().map(|i| (id, Some(cat), Some(i))));
                }
            }
        }
        rows
    }
    pub(super) fn prepare_drop(&mut self, target: u64) -> Result<()> {
        let source = self.library_id;
        self.copy_resource()?;
        self.switch_library(target)?;
        self.include_dependencies = false;
        self.paste_resources()?;
        self.transfer_source = Some(source);
        self.transfer_move = true;
        self.transfer_plan.as_mut().unwrap().notes.push("When moving, known shared dependencies stay in the source. Linked files supplied by other LIBs are copied. Each changed LIB has its own undo step.".into());
        if let Some(prompt) = &mut self.prompt {
            prompt.title = format!(
                "Transfer {} / choose scope and Copy or Move",
                self.clipboard.as_ref().unwrap().name
            );
        }
        self.status =
            "Review item only or linked resources before moving / files are unchanged until saved"
                .into();
        Ok(())
    }
    pub(super) fn toggle_library(&mut self, id: u64) {
        if id == self.library_id {
            self.root_collapsed = !self.root_collapsed;
        } else if let Some(l) = self.libraries.iter_mut().find(|l| l.id == id) {
            l.root_collapsed = !l.root_collapsed;
        }
        let rows = ((self.height - self.tree_start() - 54) / 22).max(1) as usize;
        self.scroll = self
            .scroll
            .min(self.library_rows().len().saturating_sub(rows));
    }
}

#[inline(never)]
fn library_test_app() -> Box<App> {
    Box::new(App::new())
}
impl App {
    #[inline(never)]
    pub(super) fn smoke_library_moves(&mut self) {
        let mut a = library_test_app();
        a.demo();
        a.path = "SOURCE.LIB".into();
        let source = a.library_id;
        a.doc.mark_unsaved();
        a.file_prompt(FileAction::Open);
        assert!(matches!(
            a.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::File(FileAction::Open))
        ));
        a.key(Key::Escape, false, false);
        assert!(a.doc.dirty());
        let bytes = a.doc.archive.bytes().unwrap();
        a.install_library(Document::new(Archive::empty()), "TARGET.LIB".into())
            .unwrap();
        let target = a.library_id;
        a.refresh();
        a.switch_library(source).unwrap();
        a.select_entry(0);
        a.prepare_drop(target).unwrap();
        assert!(a.transfer_move);
        assert!(!a.include_dependencies);
        assert_eq!(a.transfer_plan.as_ref().unwrap().items.len(), 1);
        a.act(Action::TransferDependencies);
        assert_eq!(a.transfer_plan.as_ref().unwrap().items.len(), 2);
        a.act(Action::TransferMove(false));
        a.apply_transfer().unwrap();
        a.prompt = None;
        assert_eq!(a.doc.archive.entries.len(), 2);
        assert_eq!(
            a.libraries
                .iter()
                .find(|l| l.id == source)
                .unwrap()
                .doc
                .archive
                .bytes()
                .unwrap(),
            bytes
        );
        a.act(Action::Undo);
        a.switch_library(source).unwrap();
        a.select_entry(0);
        a.prepare_drop(target).unwrap();
        a.apply_transfer().unwrap();
        a.prompt = None;
        assert!(a.doc.archive.find("DEMO.SH").is_some());
        assert!(a
            .libraries
            .iter()
            .find(|l| l.id == source)
            .unwrap()
            .doc
            .archive
            .find("DEMO.SH")
            .is_none());
        a.act(Action::Undo);
        a.switch_library(source).unwrap();
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), bytes);
        for i in 0..24 {
            a.install_library(Document::new(Archive::empty()), format!("EXTRA{i}.LIB"))
                .unwrap();
            a.refresh();
        }
        assert_eq!(a.libraries.len() + 1, 26);
        a.toggle_library(source);
        assert!(a
            .library_rows()
            .iter()
            .all(|(id, cat, _)| *id != source || cat.is_none()));
        a.width = 800;
        a.height = 600;
        a.scroll = 0;
        a.mode = Mode::Model;
        for hit in a.layout().hits {
            assert!(
                hit.rect[0] >= 0
                    && hit.rect[1] >= 0
                    && hit.rect[0] + hit.rect[2] <= 800
                    && hit.rect[1] + hit.rect[3] <= 600
            );
        }
        a.mouse = [20, 300];
        a.wheel(-100);
        assert!(a
            .layout()
            .hits
            .iter()
            .any(|h| matches!(h.action,Action::Library(id) if id==a.library_id)));
        let last = a.library_id;
        a.toggle_library(last);
        assert!(a.root_collapsed);
        a.toggle_library(last);
        assert!(!a.root_collapsed);
        a.switch_library(source).unwrap();
        assert!(a.root_collapsed);
    }
}
