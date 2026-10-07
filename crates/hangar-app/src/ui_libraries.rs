//! Multiple documents share immutable payload buffers and retain independent history.
use super::view::{count, Action, Icon, Layout};
use super::widgets::Item;
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
    pub(super) collapsed: [bool; 10],
    pub(super) root_collapsed: bool,
    category: Option<usize>,
    required: Vec<String>,
    suggested: Option<String>,
    palette: Option<Box<[[u8; 3]; 256]>>,
    dock: u8,
    scroll: usize,
    table_scroll: usize,
}
/// A press on an outliner entry; `live` once it moves past `DRAG_THRESHOLD`.
#[derive(Clone, Copy)]
pub(super) struct ResourceDrag {
    pub library: u64,
    pub entry: usize,
    pub start: [i32; 2],
    pub live: bool,
}
/// Manhattan distance a press travels before it becomes a drag.
pub(super) const DRAG_THRESHOLD: i32 = 6;
/// What releasing a live drag at the pointer would do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum DropTarget {
    /// Review a copy into this LIB.
    Copy(u64),
    /// Open Graft with the dragged entry as donor and this entry as target.
    Graft(usize),
    /// Nothing; the reason shows in the drag chip.
    Invalid(&'static str),
}
pub(super) type Row = (u64, Option<usize>, Option<usize>);
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
    /// First outliner row, just below the 28px outliner header.
    pub(super) fn tree_start(&self) -> i32 {
        theme::metric::MENUBAR_H + theme::metric::EDITOR_HEADER_H
    }
    /// Outliner rows that fit above the footer.
    pub(super) fn outliner_rows(&self) -> usize {
        ((self.height - self.tree_start() - super::view::OUTLINER_FOOTER) / theme::metric::ROW_H)
            .max(1) as usize
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
        self.ed.pose.clear();
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
        self.status = "Active LIB changed; its edits and undo history are kept".into();
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
        self.ed.pose.clear();
        self.hp_tool = false;
        self.hp_drag = None;
        self.mesh_drag = None;
        self.model_paint = false;
        self.paint_enabled = false;
        self.selected = 0;
        self.category = None;
        self.collapsed = [true; 10];
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
        // Ctrl+C/V includes linked files by default; drops opt out after this call.
        self.include_dependencies = true;
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
            "Copied a snapshot of {}. Select a target LIB and paste resources",
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
                "Duplicate resource: new 8.3 name".into()
            } else {
                "Rename resource and its reviewed references: new 8.3 name".into()
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
        self.status = moved.map_or("Resources copied in one undo step".into(), |n| {
            format!("Moved {n} source entries; shared dependencies stay; each LIB has its own undo")
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
        use theme::{metric as m, space};
        use widgets::{baseline, notched, Btn, Check};
        let Some(plan) = &self.transfer_plan else {
            return;
        };
        let (w, h) = ((self.width - 32).min(900), (self.height - 48).min(650));
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        let title = self
            .prompt
            .as_ref()
            .map_or("Resource review", |p| p.title.as_str());
        let [bx, by, bw, _] = self.dialog_frame(o, [x, y, w, h], title);
        let summary = if self.transfer_move {
            format!(
                "{} incoming. Shared links stay at the source; one undo step per LIB.",
                count(plan.items.len(), "resource", "resources")
            )
        } else {
            format!(
                "Target {}: {}, {}, one undo step.",
                self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path),
                count(plan.items.len(), "resource", "resources"),
                count(plan.removals.len(), "removal", "removals")
            )
        };
        o.canvas.styled(
            bx,
            baseline(by, m::ROW_H, Style::Label),
            &fit(&summary, bw, Style::Label),
            c::INK,
            Style::Label,
        );
        // Scope and Copy / Move.
        let sy = by + m::ROW_H + space::SPACE_1;
        let mut room = bw;
        if self.transfer_source.is_some() {
            let items = [
                (
                    Btn::new("Copy").on(!self.transfer_move),
                    Action::TransferMove(false),
                ),
                (
                    Btn::new("Move").on(self.transfer_move),
                    Action::TransferMove(true),
                ),
            ];
            let sw = Layout::segmented_width(&items).max(120);
            o.segmented([bx + bw - sw, sy, sw, m::BUTTON_H], &items);
            room -= sw + space::SPACE_3;
        }
        if self.transfer_is_copy {
            o.checkbox_row(
                [bx, sy + 1, room, m::ROW_H],
                Some(Icon::Link),
                "Include linked files",
                if self.include_dependencies {
                    "object and its resources"
                } else {
                    "this item only"
                },
                if self.include_dependencies {
                    Check::On
                } else {
                    Check::Off
                },
                Action::TransferDependencies,
            );
        }
        // One row per incoming resource; collisions choose keep or take.
        let pages = self.transfer_note_pages(w);
        let page = self.transfer_note.min(pages.len().saturating_sub(1));
        let note = pages
            .get(page)
            .map(|lines| lines.iter().map(|l| l.trim()).collect::<Vec<_>>().join(" "));
        let note_h = note
            .as_ref()
            .map_or(0, |t| widgets::notice_height(bw, t.trim()));
        let list = sy + m::BUTTON_H + space::SPACE_3;
        let notes_y = y + h - space::SPACE_4 - m::BUTTON_H - space::SPACE_3 - note_h;
        let pitch = 2 * m::ROW_H + space::SPACE_1;
        let visible = ((notes_y - space::SPACE_2 - list) / pitch).max(1) as usize;
        for (row, item) in plan
            .items
            .iter()
            .skip(self.transfer_scroll)
            .take(visible)
            .enumerate()
        {
            let i = row + self.transfer_scroll;
            let yy = list + row as i32 * pitch;
            let d = &mut o.canvas;
            notched(d, [bx, yy, bw, 2 * m::ROW_H], Some(c::GM_900), None);
            let choice_w = 2 * 104 + 2;
            let tw = bw - choice_w - 3 * space::SPACE_2;
            d.icon(
                bx + 6,
                yy + 2,
                super::view::group_icon(&item.entry.name),
                c::INK_MUTED,
                c::GM_900,
            );
            d.styled(
                bx + 6 + m::ICON + space::SPACE_2,
                baseline(yy, m::ROW_H, Style::Value),
                &fit(&item.entry.name, tw - m::ICON, Style::Value),
                c::INK,
                Style::Value,
            );
            d.styled(
                bx + 6 + m::ICON + space::SPACE_2,
                baseline(yy + m::ROW_H, m::ROW_H, Style::Label),
                &fit(
                    if plan.follows_kept(item) {
                        "Stored original of a kept target PIC; not copied"
                    } else if item.conflict {
                        "A different resource with this name is in the target"
                    } else if item.previous.is_some() {
                        "Already in the target; unchanged or rewritten"
                    } else {
                        "New resource"
                    },
                    tw - m::ICON,
                    Style::Label,
                ),
                c::INK_MUTED,
                Style::Label,
            );
            let cx = bx + bw - space::SPACE_2 - choice_w;
            if item.conflict && !plan.follows_kept(item) {
                o.segmented(
                    [
                        cx,
                        yy + (2 * m::ROW_H - m::BUTTON_H) / 2,
                        choice_w,
                        m::BUTTON_H,
                    ],
                    &[
                        (
                            Btn::new("Keep target").on(item.choice == Choice::KeepTarget),
                            Action::TransferChoice(i, false),
                        ),
                        (
                            Btn::new("Take source").on(item.choice == Choice::TakeSource),
                            Action::TransferChoice(i, true),
                        ),
                    ],
                );
            } else {
                let state = if plan.follows_kept(item) {
                    "Follows its PIC"
                } else if item.choice == Choice::TakeSource {
                    "Applies"
                } else {
                    "Keeps the identical target"
                };
                o.canvas.styled(
                    cx,
                    baseline(yy, 2 * m::ROW_H, Style::Label),
                    &fit(state, choice_w, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                );
            }
        }
        if let Some(text) = &note {
            widgets::notice(
                &mut o.canvas,
                bx,
                notes_y,
                bw,
                widgets::Tone::Warn,
                text.trim(),
            );
        }
        let ready = plan.ready();
        let left_buttons: Vec<(String, Action)> = if pages.len() > 1 {
            vec![(
                format!("Notes {} of {}", page + 1, pages.len()),
                Action::TransferNote,
            )]
        } else {
            Vec::new()
        };
        let left: Vec<(&str, Action)> =
            left_buttons.iter().map(|(t, a)| (t.as_str(), *a)).collect();
        self.dialog_actions(
            o,
            [x, y, w, h],
            &left,
            Some("Cancel"),
            Some(
                Btn::new(if self.transfer_move {
                    "Move resources"
                } else {
                    "Apply resources"
                })
                .primary()
                .enabled(ready),
            ),
            Action::Apply,
        );
        if !ready {
            let ey = y + h - space::SPACE_4 - m::BUTTON_H;
            let ex = x + w / 3;
            o.canvas
                .icon(ex, ey + 3, Icon::Warning, c::AMBER, c::GM_800);
            o.canvas.styled(
                ex + m::ICON + space::SPACE_1,
                baseline(ey, m::BUTTON_H, Style::Label),
                &fit("Resolve every collision to apply", w / 3, Style::Label),
                c::INK,
                Style::Label,
            );
        }
    }
}

impl App {
    #[inline(never)]
    pub(super) fn smoke_libraries(&mut self) {
        let mut app = library_test_app();
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
        assert!(!app.transfer_move, "A drop opens the review with Copy");
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
            for cat in super::view::GROUP_ORDER {
                if self.type_filter.is_some_and(|t| t != cat) {
                    continue;
                }
                let closed = &collapsed[cat];
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
        // Copy is the safe default; Move is one click away in the review.
        self.transfer_move = false;
        self.transfer_plan.as_mut().unwrap().notes.push("When moving, known shared dependencies stay in the source. Linked files supplied by other LIBs are copied. Each changed LIB has its own undo step.".into());
        if let Some(prompt) = &mut self.prompt {
            prompt.title = format!(
                "Transfer {}: choose scope, then Copy or Move",
                self.clipboard.as_ref().unwrap().name
            );
        }
        self.status =
            "Review the copy, or choose Move to take it out of the source; files change only when saved"
                .into();
        Ok(())
    }
    /// Stored file name of open LIB `id`.
    pub(super) fn library_name(&self, id: u64) -> &str {
        if id == self.library_id {
            return self.lib_name();
        }
        self.libraries
            .iter()
            .find(|l| l.id == id)
            .map_or("", |l| l.path.rsplit(['/', '\\']).next().unwrap_or(&l.path))
    }
    /// The outliner row drawn under (x, y).
    pub(super) fn outliner_row_at(&self, x: i32, y: i32) -> Option<Row> {
        let top = self.tree_start();
        let bottom = self.height - theme::metric::STATUSBAR_H - super::view::OUTLINER_FOOTER;
        if self.mode == Mode::Package || x < 0 || x >= self.left() || y < top || y >= bottom {
            return None;
        }
        let row = ((y - top) / theme::metric::ROW_H) as usize;
        if row >= self.outliner_rows() {
            return None;
        }
        self.library_rows().get(self.scroll + row).copied()
    }
    pub(super) fn drag_live(&self) -> bool {
        self.resource_drag.is_some_and(|d| d.live)
    }
    /// What releasing `drag` at (x, y) would do. Other LIBs take copies;
    /// same-type definitions in the dragged entry's LIB take a graft.
    pub(super) fn drop_target_at(&self, drag: ResourceDrag, x: i32, y: i32) -> DropTarget {
        let Some((id, _, entry)) = self.outliner_row_at(x, y) else {
            return DropTarget::Invalid("Drop on a LIB in the outliner");
        };
        if drag.library != self.library_id {
            return DropTarget::Invalid("Source LIB is no longer active");
        }
        if id != drag.library {
            return DropTarget::Copy(id);
        }
        let ext = |i: usize| self.doc.archive.entries.get(i).map(|e| extension(&e.name));
        match entry {
            Some(to) if to == drag.entry => DropTarget::Invalid("Drop on another LIB"),
            Some(to) if ext(to) == ext(drag.entry) => {
                if self.selected == drag.entry && self.brf.is_some() {
                    DropTarget::Graft(to)
                } else {
                    DropTarget::Invalid("Graft takes definitions only")
                }
            }
            _ => DropTarget::Invalid("Already in this LIB"),
        }
    }
    pub(super) fn drop_target(&self) -> Option<DropTarget> {
        let drag = self.resource_drag.filter(|d| d.live)?;
        Some(self.drop_target_at(drag, self.mouse[0], self.mouse[1]))
    }
    /// Release a live drag at (x, y).
    pub(super) fn drop_resource(&mut self, drag: ResourceDrag, x: i32, y: i32) -> Result<()> {
        match self.drop_target_at(drag, x, y) {
            DropTarget::Copy(id) => {
                self.selected = drag.entry;
                self.prepare_drop(id)?;
            }
            DropTarget::Graft(to) => {
                self.selected = drag.entry;
                self.pin_donor()?;
                self.select_entry(to);
                self.mode = Mode::Graft;
            }
            DropTarget::Invalid(reason) => self.status = format!("Not dropped: {reason}"),
        }
        Ok(())
    }
    /// Status bar text for a live drag: what releasing would do.
    pub(super) fn drag_status(&self) -> Option<String> {
        let drag = self.resource_drag.filter(|d| d.live)?;
        let name = self.doc.archive.entries.get(drag.entry)?.name.as_str();
        Some(match self.drop_target()? {
            DropTarget::Copy(id) => {
                let lib = self.library_name(id);
                if hangar_core::save::protected_name(lib).is_some() {
                    format!("Release to copy {name} to {lib}; it saves only under a new name")
                } else {
                    format!("Release to copy {name} to {lib}")
                }
            }
            DropTarget::Graft(to) => format!(
                "Release to graft {name} onto {}",
                self.doc.archive.entries[to].name
            ),
            DropTarget::Invalid(reason) => format!("{reason}; release does nothing"),
        })
    }
    /// Fill and outline for outliner `row` under a live drag: the hovered
    /// copy target and its LIB root in amber, a graft target in steel.
    pub(super) fn drop_row_look(&self, row: Row, hovered: Option<Row>) -> Option<(Rgb, Rgb)> {
        match self.drop_target()? {
            DropTarget::Copy(id)
                if row.0 == id
                    && (Some(row) == hovered || (row.1.is_none() && row.2.is_none())) =>
            {
                Some((c::AMBER_DEEP, c::AMBER))
            }
            DropTarget::Graft(to) if row.0 == self.library_id && row.2 == Some(to) => {
                Some((c::STEEL_DEEP, c::STEEL))
            }
            _ => None,
        }
    }
    /// The drag chip beside the pointer: type icon, entry name and what a
    /// release would do (or why it would not).
    pub(super) fn drag_ghost(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        let Some(drag) = self.resource_drag.filter(|d| d.live) else {
            return;
        };
        let (Some(target), Some(entry)) =
            (self.drop_target(), self.doc.archive.entries.get(drag.entry))
        else {
            return;
        };
        let (label, color) = match target {
            DropTarget::Copy(id) => (format!("Copy to {}", self.library_name(id)), c::INK),
            DropTarget::Graft(to) => (
                format!("Graft with {}", self.doc.archive.entries[to].name),
                c::STEEL,
            ),
            DropTarget::Invalid(reason) => (reason.to_string(), c::INK_MUTED),
        };
        let name_w = text_width(&entry.name, Style::Value);
        let w = space::SPACE_2
            + m::ICON
            + space::SPACE_1
            + name_w
            + space::SPACE_3
            + text_width(&label, Style::Label)
            + space::SPACE_2;
        let w = w.min(self.width - 2);
        let h = m::BUTTON_H;
        let x = (self.mouse[0] + space::SPACE_3)
            .min(self.width - w - 1)
            .max(0);
        let y = (self.mouse[1] + space::SPACE_3)
            .min(self.height - m::STATUSBAR_H - h - 1)
            .max(0);
        let d = &mut o.canvas;
        widgets::notched(d, [x, y, w, h], Some(c::GM_800), Some(c::LINE_STRONG));
        let mut tx = x + space::SPACE_2;
        d.icon(
            tx,
            y + (h - m::ICON) / 2,
            super::view::group_icon(&entry.name),
            c::INK_MUTED,
            c::GM_800,
        );
        tx += m::ICON + space::SPACE_1;
        d.styled(
            tx,
            widgets::baseline(y, h, Style::Value),
            &entry.name,
            c::INK,
            Style::Value,
        );
        tx += name_w + space::SPACE_3;
        d.styled(
            tx,
            widgets::baseline(y, h, Style::Label),
            &fit(&label, x + w - space::SPACE_2 - tx, Style::Label),
            color,
            Style::Label,
        );
    }
    pub(super) fn toggle_library(&mut self, id: u64) {
        if id == self.library_id {
            self.root_collapsed = !self.root_collapsed;
        } else if let Some(l) = self.libraries.iter_mut().find(|l| l.id == id) {
            l.root_collapsed = !l.root_collapsed;
        }
        let rows = self.outliner_rows();
        self.scroll = self
            .scroll
            .min(self.library_rows().len().saturating_sub(rows));
    }
}

impl App {
    /// SOURCE.LIB (the demo plus DEMO2.PT, DEMO.PT selected) beside MYMOD.LIB
    /// holding MYJET.PT; returns MYMOD.LIB's id with SOURCE.LIB active.
    pub(super) fn drag_fixture(&mut self) -> Result<u64> {
        self.libraries.clear();
        self.clipboard = None;
        self.doc = Document::new(Archive::empty());
        self.demo();
        let mut source = self.doc.archive.clone();
        source.entries.push(Entry::new(
            "DEMO2.PT",
            hangar_core::brf::demo_with_records(),
        )?);
        self.doc = Document::new(source);
        self.path = "SOURCE.LIB".into();
        let id = self.library_id;
        let mut target = Archive::empty();
        target
            .entries
            .push(Entry::new("MYJET.PT", hangar_core::brf::demo())?);
        self.install_library(Document::new(target), "MYMOD.LIB".into())?;
        let target = self.library_id;
        self.refresh();
        self.switch_library(id)?;
        self.select_entry(1);
        self.collapsed[0] = false;
        self.scroll = 0;
        self.status.clear();
        Ok(target)
    }
    /// Drag feedback through real pointer events: the threshold, the chip,
    /// amber copy targets, steel graft targets, invalid reasons, the status
    /// bar, Esc, and release opening the review with Copy.
    #[inline(never)]
    pub(super) fn smoke_drag_feedback(&mut self) {
        let texts = |app: &App| -> Vec<(i32, i32, String)> {
            app.draw()
                .commands
                .into_iter()
                .filter_map(|d| match d {
                    Draw::Text(x, y, s, _, _) => Some((x, y, s)),
                    _ => None,
                })
                .collect()
        };
        let fills = |app: &App, color: Rgb| -> Vec<[i32; 4]> {
            app.draw()
                .commands
                .into_iter()
                .filter_map(|d| match d {
                    Draw::Rect(x, y, w, h, c) if c == color.0 => Some([x, y, w, h]),
                    _ => None,
                })
                .collect()
        };
        let outlined = |app: &App, color: Rgb| {
            app.draw()
                .commands
                .iter()
                .filter_map(|d| match d {
                    Draw::Line(x0, y0, x1, y1, c) if *c == color.0 && y0 == y1 && *x0 == 0 => {
                        Some((*y0, *x1))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        for (w, h) in [(800, 600), (1280, 800)] {
            let target = self.drag_fixture().unwrap();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            let source = self.library_id;
            let row = |app: &App, action: &dyn Fn(Action) -> bool| app.chrome_hit(action).unwrap();
            let entry = row(self, &|a| matches!(a, Action::Entry(1)));
            let (x, y) = (entry[0] + 70, entry[1] + 8);
            // Under the threshold the press is still a click: no chip.
            self.motion(x, y, false);
            self.pointer(x, y, 1, true, false);
            self.motion(x + 3, y + 2, false);
            assert!(!self.drag_live());
            assert!(self.drag_status().is_none());
            // Over another LIB root: amber fill and outline, chip, status.
            let root = row(self, &|a| matches!(a, Action::Library(id) if id == target));
            self.motion(root[0] + 40, root[1] + 8, false);
            assert!(self.drag_live());
            assert_eq!(self.drop_target(), Some(DropTarget::Copy(target)));
            assert!(fills(self, c::AMBER_DEEP)
                .iter()
                .any(|r| r[1] == root[1] && r[0] + r[2] == self.left()));
            assert!(outlined(self, c::AMBER).contains(&(root[1], self.left() - 1)));
            let t = texts(self);
            let chip = t.iter().find(|(_, _, s)| s == "Copy to MYMOD.LIB").unwrap();
            assert!(chip.0 + text_width(&chip.2, Style::Label) <= w);
            assert!(t
                .iter()
                .any(|(_, ty, s)| *ty > h - 22 && s == "Release to copy DEMO.PT to MYMOD.LIB"));
            // Outliner hover does not compete with the drag highlight.
            assert!(fills(self, c::GM_700)
                .iter()
                .all(|r| r[0] + r[2] != self.left() || r[3] != theme::metric::ROW_H));
            // A group row of that LIB is a target too; release opens Copy.
            self.motion(x, root[1] + 28, false);
            assert_eq!(self.drop_target(), Some(DropTarget::Copy(target)));
            self.pointer(x, root[1] + 28, 1, false, false);
            assert_eq!(self.library_id, target);
            assert!(matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::TransferReview)
            ));
            assert!(!self.transfer_move, "Drops open the review with Copy");
            self.key(Key::Escape, false, false);
            assert!(self.doc.archive.find("DEMO.PT").is_none());
            self.switch_library(source).unwrap();
            // Esc cancels a live drag; the release then does nothing.
            let entry = row(self, &|a| matches!(a, Action::Entry(1)));
            self.motion(x, entry[1] + 8, false);
            self.pointer(x, entry[1] + 8, 1, true, false);
            self.motion(root[0] + 40, root[1] + 8, false);
            assert!(self.drag_live());
            self.key(Key::Escape, false, false);
            assert!(!self.drag_live() && self.drag_status().is_none());
            self.pointer(root[0] + 40, root[1] + 8, 1, false, false);
            assert!(self.prompt.is_none() && self.library_id == source);
            // Invalid targets: no highlight, the reason in the chip.
            let entry = row(self, &|a| matches!(a, Action::Entry(1)));
            self.motion(x, entry[1] + 8, false);
            self.pointer(x, entry[1] + 8, 1, true, false);
            let sh = row(self, &|a| matches!(a, Action::Entry(0)));
            self.motion(x, sh[1] + 8, false);
            assert_eq!(
                self.drop_target(),
                Some(DropTarget::Invalid("Already in this LIB"))
            );
            assert!(texts(self)
                .iter()
                .any(|(_, _, s)| s == "Already in this LIB"));
            assert!(outlined(self, c::AMBER).is_empty() && outlined(self, c::STEEL).is_empty());
            assert!(fills(self, c::GM_700)
                .iter()
                .all(|r| r[0] + r[2] != self.left() || r[3] != theme::metric::ROW_H));
            // The chip stays in the window at the far corner.
            self.motion(w - 2, h - 2, false);
            let t = texts(self);
            let chip = t
                .iter()
                .find(|(_, _, s)| s == "Drop on a LIB in the outliner")
                .unwrap();
            assert!(chip.0 + text_width(&chip.2, Style::Label) <= w && chip.1 < h - 22);
            self.pointer(w - 2, h - 2, 1, false, false);
            assert!(self.prompt.is_none() && self.status.starts_with("Not dropped"));
            // A same-type definition is a graft target in steel.
            let entry = row(self, &|a| matches!(a, Action::Entry(1)));
            self.motion(x, entry[1] + 8, false);
            self.pointer(x, entry[1] + 8, 1, true, false);
            let to = self.doc.archive.find("DEMO2.PT").unwrap();
            let graft = row(self, &|a| matches!(a, Action::Entry(i) if i == to));
            self.motion(x, graft[1] + 8, false);
            assert_eq!(self.drop_target(), Some(DropTarget::Graft(to)));
            assert!(fills(self, c::STEEL_DEEP).iter().any(|r| r[1] == graft[1]));
            assert!(outlined(self, c::STEEL).contains(&(graft[1], self.left() - 1)));
            let t = texts(self);
            assert!(t.iter().any(|(_, _, s)| s == "Graft with DEMO2.PT"));
            assert!(t
                .iter()
                .any(|(_, _, s)| s == "Release to graft DEMO.PT onto DEMO2.PT"));
            self.pointer(x, graft[1] + 8, 1, false, false);
            assert!(self.mode == Mode::Graft);
            assert_eq!(self.selected, to);
            assert_eq!(self.graft_donor.as_ref().unwrap().name, "DEMO.PT");
            self.graft_donor = None;
        }
        self.libraries.clear();
        self.width = 1280;
        self.height = 800;
        self.demo();
    }
}
/// The outliner context menu: where it opened, its open submenu (0 none,
/// 1 Copy to, 2 Move to) and whether it belongs to a LIB root.
#[derive(Clone, Copy, Default)]
pub(super) struct ContextMenu {
    pub at: [i32; 2],
    pub sub: u8,
    pub root: bool,
}
/// A submenu LIB (name, id), or a disabled note with no id.
type SubItem = (String, Option<u64>);
impl App {
    /// RMB on an outliner entry or LIB root: select it (switching LIB when
    /// needed) and open the context menu at the pointer.
    pub(super) fn open_context_menu(&mut self, x: i32, y: i32) -> bool {
        let Some((id, cat, entry)) = self.outliner_row_at(x, y) else {
            return false;
        };
        let root = match (cat, entry) {
            (_, Some(i)) => {
                if id != self.library_id {
                    self.act(Action::LibraryEntry(id, i));
                } else if i != self.selected {
                    self.act(Action::Entry(i));
                }
                if id != self.library_id || self.selected != i {
                    return false;
                }
                false
            }
            (None, None) => {
                let result = self.switch_library(id);
                if result.is_err() {
                    self.result(result);
                    return false;
                }
                true
            }
            _ => return false,
        };
        self.context = ContextMenu {
            at: [x, y],
            sub: 0,
            root,
        };
        self.menu = Some(super::chrome::MENU_CONTEXT);
        true
    }
    /// Context menu items. Actions that cannot run here are drawn disabled.
    pub(super) fn context_items(&self) -> Vec<Item<'static>> {
        let paste = Item::new("Paste", Action::PasteResources)
            .key("Ctrl+V")
            .enabled(self.clipboard.is_some());
        if self.context.root {
            return vec![
                paste,
                Item::new(
                    if self.root_collapsed {
                        "Expand"
                    } else {
                        "Collapse"
                    },
                    Action::LibraryToggle(self.library_id),
                ),
                Item::sep(),
                Item::new("Package LIB", Action::File(FileAction::Save)).key("Ctrl+S"),
                Item::new("Close LIB", Action::CloseLibrary).key("Ctrl+W"),
            ];
        }
        let others = !self.libraries.is_empty();
        let object = self
            .doc
            .archive
            .entries
            .get(self.selected)
            .is_some_and(|e| matches!(super::view::category_of(&e.name), 0 | 3 | 4));
        let mut items = vec![
            Item::new("Copy to", Action::ContextSub(1))
                .sub()
                .on(self.context.sub == 1)
                .enabled(others),
            Item::new("Move to", Action::ContextSub(2))
                .sub()
                .on(self.context.sub == 2)
                .enabled(others),
            Item::sep(),
            Item::new("Copy", Action::CopyResource).key("Ctrl+C"),
            paste,
        ];
        // An aircraft duplicates and renames with its private resources.
        if self.name().ends_with(".PT") {
            items.push(Item::new(
                "Duplicate aircraft\u{2026}",
                Action::DuplicateAircraft,
            ));
            items.push(Item::new(
                "Rename reference ID\u{2026}",
                Action::RenameAircraft,
            ));
        } else {
            items.push(Item::new("Duplicate", Action::RenameResource(true)).key("Ctrl+D"));
            items.push(Item::new("Rename\u{2026}", Action::RenameResource(false)));
        }
        items.extend([
            Item::sep(),
            Item::new("Export entry\u{2026}", Action::File(FileAction::Export)).key("Ctrl+E"),
        ]);
        if object {
            items.push(Item::new(
                "Export object\u{2026}",
                Action::File(FileAction::Variant),
            ));
        }
        items.push(Item::sep());
        items.push(Item::new("Delete", Action::DeleteEntry).key("Del"));
        items
    }
    /// The open submenu's LIBs (outliner order) and its top-left anchor;
    /// LIBs beyond the window height collapse into one disabled note.
    fn context_sub(&self) -> Option<(Vec<SubItem>, i32, i32)> {
        use theme::{metric as m, space};
        let sub = self.context.sub;
        if sub == 0 || self.context.root {
            return None;
        }
        let main = self.open_menu_rect()?;
        let items = self.context_items();
        let parent = items
            .iter()
            .position(|i| matches!(i.action, Some(Action::ContextSub(n)) if n == sub))?;
        let mut libs: Vec<(u64, String)> = self
            .libraries
            .iter()
            .map(|l| (l.id, self.library_name(l.id).to_string()))
            .collect();
        libs.sort_unstable_by_key(|l| l.0);
        let fit = ((self.height - 2 * space::SPACE_1 - 2) / m::MENU_ITEM_H).max(2) as usize;
        let mut entries: Vec<SubItem> = Vec::new();
        let overflow = libs.len() > fit;
        for (id, name) in libs.iter().take(if overflow { fit - 1 } else { fit }) {
            entries.push((name.clone(), Some(*id)));
        }
        if overflow {
            entries.push((
                format!("{} more open LIBs; drag to them", libs.len() - (fit - 1)),
                None,
            ));
        }
        let (w, _) = widgets::menu_size(&self.context_sub_items(&entries));
        let x = if main[0] + main[2] + w <= self.width {
            main[0] + main[2]
        } else {
            main[0] - w
        };
        let y = widgets::menu_item_y(&items, main[1], parent) - space::SPACE_1 - 1;
        Some((entries, x, y))
    }
    fn context_sub_items<'a>(&self, entries: &'a [SubItem]) -> Vec<Item<'a>> {
        let moving = self.context.sub == 2;
        entries
            .iter()
            .map(|(name, id)| match id {
                Some(id) => Item::new(name, Action::TransferTo(*id, moving)).icon(Icon::Lib),
                None => Item::new(name, Action::MenuPad).enabled(false),
            })
            .collect()
    }
    /// The open submenu's drawn rect.
    pub(super) fn context_sub_rect(&self) -> Option<[i32; 4]> {
        let (entries, x, y) = self.context_sub()?;
        let (w, h) = widgets::menu_size(&self.context_sub_items(&entries));
        Some([
            x.min(self.width - w).max(0),
            y.min(self.height - h).max(0),
            w,
            h,
        ])
    }
    pub(super) fn context_sub_layout(&self, o: &mut Layout) {
        if let Some((entries, x, y)) = self.context_sub() {
            o.menu(x, y, &self.context_sub_items(&entries), Action::MenuPad);
        }
    }
    /// Hovering Copy to or Move to opens its submenu; hovering another item
    /// of the main menu closes it.
    pub(super) fn context_hover(&mut self, x: i32, y: i32) {
        let over = |r: Option<[i32; 4]>| {
            r.is_some_and(|[rx, ry, rw, rh]| x >= rx && y >= ry && x < rx + rw && y < ry + rh)
        };
        if over(self.context_sub_rect()) || !over(self.open_menu_rect()) {
            return;
        }
        let hit = self
            .layout()
            .hits
            .into_iter()
            .rev()
            .find(|h| h.contains(x, y))
            .map(|h| h.action);
        match hit {
            Some(Action::ContextSub(n)) => self.context.sub = n,
            Some(Action::MenuPad) | None => {}
            Some(_) => self.context.sub = 0,
        }
    }
}
impl App {
    /// The outliner context menu through RMB and its hit regions: selection,
    /// Copy to and Move to reviews, disabled items, LIB roots, dismissal,
    /// clamping, and RMB elsewhere keeping its cancel role.
    #[inline(never)]
    pub(super) fn smoke_context_menu(&mut self) {
        use super::chrome::MENU_CONTEXT;
        let hit = |app: &App, p: &dyn Fn(Action) -> bool| app.chrome_hit(p);
        let inside = |app: &App| {
            app.open_menu_rects()
                .iter()
                .all(|[x, y, w, h]| *x >= 0 && *y >= 0 && x + w <= app.width && y + h <= app.height)
        };
        for (w, h) in [(800, 600), (1280, 800)] {
            let target = self.drag_fixture().unwrap();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            let source = self.library_id;
            let demo2 = self.doc.archive.find("DEMO2.PT").unwrap();
            let rmb = |app: &mut App, r: [i32; 4]| {
                app.motion(r[0] + 70, r[1] + 8, false);
                app.pointer(r[0] + 70, r[1] + 8, 3, true, false);
                app.pointer(r[0] + 70, r[1] + 8, 3, false, false);
            };
            // RMB selects the entry under the pointer and opens the menu there.
            let row = hit(self, &|a| matches!(a, Action::Entry(i) if i == demo2)).unwrap();
            rmb(self, row);
            assert_eq!(self.menu, Some(MENU_CONTEXT));
            assert_eq!(self.selected, demo2);
            let rect = self.open_menu_rect().unwrap();
            assert_eq!([rect[0], rect[1]], [row[0] + 70, row[1] + 8]);
            // Paste is disabled with an empty clipboard: drawn, but no hit.
            assert!(hit(self, &|a| matches!(a, Action::PasteResources)).is_none());
            assert!(self.draw().commands.iter().any(
                |d| matches!(d, Draw::Text(_, _, s, c, _) if s == "Paste" && *c == c::INK_FAINT.0)
            ));
            // On an aircraft, Duplicate and Rename carry its private resources.
            assert!(hit(self, &|a| matches!(a, Action::RenameResource(_))).is_none());
            for action in [
                Action::CopyResource,
                Action::DuplicateAircraft,
                Action::RenameAircraft,
                Action::File(FileAction::Export),
                Action::File(FileAction::Variant),
                Action::DeleteEntry,
            ] {
                let r = hit(self, &|a| {
                    core::mem::discriminant(&a) == core::mem::discriminant(&action)
                })
                .unwrap();
                assert!(r[0] >= rect[0] && r[1] >= rect[1] && r[1] + r[3] <= rect[1] + rect[3]);
            }
            // Hover opens Copy to; its LIB item opens the review with Copy.
            let copy_to = hit(self, &|a| matches!(a, Action::ContextSub(1))).unwrap();
            self.motion(copy_to[0] + 20, copy_to[1] + 8, false);
            assert_eq!(self.context.sub, 1);
            assert!(inside(self));
            let lib = hit(
                self,
                &|a| matches!(a, Action::TransferTo(id, false) if id == target),
            )
            .unwrap();
            self.chrome_click(lib);
            assert!(self.menu.is_none());
            assert_eq!(self.library_id, target);
            assert!(matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::TransferReview)
            ));
            assert!(!self.transfer_move, "Copy to opens the review with Copy");
            assert_eq!(
                self.transfer_plan.as_ref().unwrap().items[0].entry.name,
                "DEMO2.PT"
            );
            self.key(Key::Escape, false, false);
            self.switch_library(source).unwrap();
            // Move to: a click opens the submenu; its LIB item preselects Move.
            let row = hit(self, &|a| matches!(a, Action::Entry(1))).unwrap();
            rmb(self, row);
            assert_eq!(self.selected, 1);
            let move_to = hit(self, &|a| matches!(a, Action::ContextSub(2))).unwrap();
            self.chrome_click(move_to);
            assert_eq!((self.menu, self.context.sub), (Some(MENU_CONTEXT), 2));
            let lib = hit(
                self,
                &|a| matches!(a, Action::TransferTo(id, true) if id == target),
            )
            .unwrap();
            self.chrome_click(lib);
            assert!(self.transfer_move, "Move to opens the review with Move");
            self.key(Key::Escape, false, false);
            self.switch_library(source).unwrap();
            // Dismissal: a click outside, Esc, and RMB outside each close it.
            let viewport = [self.left() + 200, 300];
            for close in 0..3 {
                rmb(self, row);
                assert_eq!(self.menu, Some(MENU_CONTEXT));
                let mode = self.mode;
                match close {
                    0 => self.click(viewport[0], viewport[1], 1, true),
                    1 => self.key(Key::Escape, false, false),
                    _ => self.click(viewport[0], viewport[1], 3, true),
                }
                assert!(self.menu.is_none() && self.prompt.is_none());
                assert!(self.mode == mode && self.library_id == source);
                self.click(viewport[0], viewport[1], 1, false);
                self.click(viewport[0], viewport[1], 3, false);
                assert!(self.menu.is_none());
            }
            // Copy, then Paste is enabled in the target LIB's root menu.
            rmb(self, row);
            self.chrome_click(hit(self, &|a| matches!(a, Action::CopyResource)).unwrap());
            assert!(self.clipboard.is_some() && self.menu.is_none());
            let root = hit(self, &|a| matches!(a, Action::Library(id) if id == target)).unwrap();
            rmb(self, root);
            assert_eq!(self.library_id, target);
            assert!(self.context.root);
            assert!(hit(self, &|a| matches!(
                a,
                Action::TransferTo(..) | Action::ContextSub(_)
            ))
            .is_none());
            self.chrome_click(
                hit(
                    self,
                    &|a| matches!(a, Action::LibraryToggle(id) if id == target),
                )
                .unwrap(),
            );
            assert!(self.root_collapsed && self.menu.is_none());
            self.toggle_library(target);
            rmb(self, root);
            self.chrome_click(hit(self, &|a| matches!(a, Action::PasteResources)).unwrap());
            assert!(matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::TransferReview)
            ));
            self.key(Key::Escape, false, false);
            self.switch_library(source).unwrap();
            // Delete through the menu is one undo step.
            let before = self.doc.archive.bytes().unwrap();
            let row = hit(self, &|a| matches!(a, Action::Entry(i) if i == demo2)).unwrap();
            rmb(self, row);
            self.chrome_click(hit(self, &|a| matches!(a, Action::DeleteEntry)).unwrap());
            assert!(self.doc.archive.find("DEMO2.PT").is_none());
            self.act(Action::Undo);
            assert_eq!(self.doc.archive.bytes().unwrap(), before);
            // Category rows and the viewport open no menu; RMB there cancels.
            let group = hit(self, &|a| matches!(a, Action::Category(0))).unwrap();
            rmb(self, group);
            assert!(self.menu.is_none());
            self.click(viewport[0], viewport[1], 3, true);
            assert!(self.menu.is_none());
            // Clamped at the window corner, the submenu included.
            rmb(self, row);
            self.context.at = [w - 4, h - 4];
            self.context.sub = 1;
            assert!(inside(self));
            for hit in self.layout().hits {
                let [x, y, rw, rh] = hit.rect;
                assert!(x >= 0 && y >= 0 && x + rw <= w && y + rh <= h);
            }
            self.menu = None;
            // A single open LIB: Copy to and Move to are disabled.
            self.libraries.clear();
            let row = hit(self, &|a| matches!(a, Action::Entry(1))).unwrap();
            rmb(self, row);
            assert_eq!(self.menu, Some(MENU_CONTEXT));
            assert!(hit(self, &|a| matches!(a, Action::ContextSub(_))).is_none());
            self.key(Key::Escape, false, false);
        }
        self.libraries.clear();
        self.clipboard = None;
        self.width = 1280;
        self.height = 800;
        self.demo();
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
        assert!(!a.transfer_move, "Drops default to Copy");
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
        // Move stays one click away in the review.
        let seg = a
            .chrome_hit(&|h| matches!(h, Action::TransferMove(true)))
            .unwrap();
        a.chrome_click(seg);
        assert!(a.transfer_move);
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
        let mut d = library_test_app();
        d.smoke_drag_feedback();
        d.smoke_context_menu();
    }
}
