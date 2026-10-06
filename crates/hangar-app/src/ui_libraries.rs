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
    filter: String,
    collapsed: [bool; 9],
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
        78 + self.libraries.len() as i32 * 22
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
        let incoming = self.libraries.remove(at);
        let current = self.capture_library();
        self.libraries.insert(at, current);
        self.restore_library(incoming);
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
        if self.libraries.len() >= 7 {
            return Err("At most 8 LIBs can be open; close one first".into());
        }
        let bytes = doc
            .archive
            .entries
            .iter()
            .map(Entry::stored_len)
            .sum::<usize>()
            + self
                .doc
                .archive
                .entries
                .iter()
                .map(Entry::stored_len)
                .sum::<usize>()
            + self
                .libraries
                .iter()
                .flat_map(|l| &l.doc.archive.entries)
                .map(Entry::stored_len)
                .sum::<usize>();
        if bytes > 256 * 1024 * 1024 {
            return Err("Open LIBs exceed the 256 MiB stored-resource budget".into());
        }
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
        self.selected = 0;
        self.category = None;
        self.collapsed = [true; 9];
        self.scroll = 0;
        self.table_scroll = 0;
        self.field_group = None;
        self.field_scroll = 0;
        self.inspector_scroll = 0;
        self.required.clear();
        self.suggested_output = None;
        self.palette_override = None;
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
                title: "Close active LIB and discard its edits? Type DISCARD".into(),
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
        self.transfer_is_copy = true;
        self.transfer_scroll = 0;
        self.transfer_note = 0;
        self.prompt = Some(Prompt {
            kind: PromptKind::TransferReview,
            title: format!(
                "Copy {} from {}",
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
        plan.apply(&mut self.doc)?;
        if let Some(name) = first {
            if let Some(i) = self.doc.archive.find(&name) {
                self.selected = i;
            }
        }
        self.transfer_plan = None;
        self.refresh();
        self.status = "Resource transaction applied / one Ctrl+Z undo step".into();
        Ok(())
    }
    pub(super) fn transfer_note_pages(&self, w: i32) -> Vec<Vec<String>> {
        let mut pages = Vec::new();
        if let Some(plan) = &self.transfer_plan {
            for note in &plan.notes {
                let lines = super::dependencies_ui::wrap(note, ((w - 28) / 7).max(16) as usize);
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
            &format!(
                "Target: {} / {} resources / {} removals / 1 undo step",
                self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path),
                plan.items.len(),
                plan.removals.len()
            ),
            c::STEEL,
        );
        if self.transfer_is_copy {
            o.button(
                [x + 14, y + 60, w - 28, 24],
                if self.include_dependencies {
                    "[x] Include discovered dependencies"
                } else {
                    "[ ] Include discovered dependencies"
                },
                Action::TransferDependencies,
                self.include_dependencies,
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
                "Apply resources",
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
