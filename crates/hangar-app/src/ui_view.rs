//! Shared layout and hit regions: the visible controls and clickable areas stay together.
use super::*;
use alloc::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) enum Action {
    Menu(usize),
    Mode(Mode),
    File(FileAction),
    Demo,
    Undo,
    Redo,
    Close,
    Help,
    Filter,
    Category(usize),
    Entry(usize),
    Related(usize),
    Field(usize),
    PickField(usize),
    ResetField(usize),
    View(u8),
    Transform(char),
    Dock(u8),
    Validate,
    ClearSources,
    Library(u64),
    LibraryToggle(u64),
    LibraryCategory(u64, usize),
    TransferMove(bool),
    LibraryEntry(u64, usize),
    NewLibrary,
    CloseLibrary,
    DiscardChanges,
    Animation,
    AnimationPart(i32),
    PartPosition(usize),
    AnimationState(usize),
    AnimationReset,
    CopyResource,
    PasteResources,
    RenameResource(bool),
    TransferChoice(usize, bool),
    TransferDependencies,
    TransferNote,
    PinDonor,
    GraftDonor(usize),
    GraftGroup(usize),
    ApplyGraft,
    FieldGroup(usize),
    EnvelopeStep(i32),
    Apply,
    Cancel,
    CloneBack,
    BrowserUp,
    BrowserRoots,
    BrowserPick(usize),
    Recent(usize),
    PlayAudio,
    StopAudio,
    PaintToggle,
    PickColor,
    Brush(u8),
    Radius(usize),
    OpenTexture(usize),
    Recolor,
    BaseColor(bool),
    PanelTexture,
    RepairPanels,
    MeshMode,
    MeshVertex(usize),
    MeshAll,
    MeshMove,
    Textured,
    ModelPaint,
    PaintLock,
    Isolate,
    Hardpoints,
    StationSlew,
    HardpointVisibility,
    HardpointSelect(usize),
    HardpointStep(i32),
    HardpointAdd(bool),
    HardpointRemove,
    StationField(usize),
    StationFields,
    MediaTab(u8),
    MaterialPrompt(u8),
    DecalLibrary,
    DecalSaved(usize),
    DecalForget(usize),
    DecalText,
    DecalInk,
    DecalPreset(bool),
    DecalSetting(u8),
    DecalMirror,
    DecalPlace,
    DecalCancel,
    DecalApply,
}
pub(super) struct Hit {
    pub(super) rect: [i32; 4],
    pub action: Action,
}
impl Hit {
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.rect[0]
            && y >= self.rect[1]
            && x < self.rect[0] + self.rect[2]
            && y < self.rect[1] + self.rect[3]
    }
}
pub(super) struct Layout {
    mouse: [i32; 2],
    pub canvas: Canvas,
    pub hits: Vec<Hit>,
}
#[derive(Clone, Copy)]
pub(super) enum Icon {
    Logo,
    Lib,
    Aircraft,
    Shape,
    Image,
    Weapon,
    Object,
    Palette,
    Mission,
    Sound,
    Search,
    Plus,
    Select,
    Move,
    Rotate,
    Scale,
    Frame,
    Brush,
    Flight,
    Check,
    Warn,
    Link,
}
const GROUPS: [(&str, &str, Icon); 9] = [
    ("Aircraft", "PT", Icon::Aircraft),
    ("Shapes", "SH", Icon::Shape),
    ("Images", "PIC", Icon::Image),
    ("Weapons", "JT", Icon::Weapon),
    ("Ground objects", "OT", Icon::Object),
    ("Palettes", "PAL", Icon::Palette),
    ("Missions", "M", Icon::Mission),
    ("Sounds", "SND", Icon::Sound),
    ("Other resources", "...", Icon::Lib),
];
pub(super) fn category_of(name: &str) -> usize {
    match extension(name) {
        "PT" => 0,
        "SH" => 1,
        "PIC" => 2,
        "JT" => 3,
        "OT" | "NT" => 4,
        "PAL" => 5,
        "M" | "MM" => 6,
        "5K" | "11K" | "22K" | "WAV" => 7,
        _ => 8,
    }
}
pub(super) fn icon(d: &mut Canvas, x: i32, y: i32, i: Icon, color: Rgb) {
    let paths: &[&[(i32, i32)]] = match i {
        Icon::Logo => &[
            &[(8, 1), (15, 8), (8, 15), (1, 8), (8, 1)],
            &[(5, 8), (11, 8)],
            &[(8, 5), (8, 11)],
        ],
        Icon::Aircraft => &[
            &[(8, 1), (8, 14)],
            &[(8, 6), (2, 10), (2, 11), (8, 9), (14, 11), (14, 10), (8, 6)],
            &[(5, 14), (8, 13), (11, 14)],
        ],
        Icon::Shape => &[
            &[(8, 1), (14, 4), (14, 12), (8, 15), (2, 12), (2, 4), (8, 1)],
            &[(2, 4), (8, 8), (14, 4)],
            &[(8, 8), (8, 15)],
        ],
        Icon::Lib => &[
            &[(2, 3), (14, 3), (14, 6), (2, 6), (2, 3)],
            &[(3, 6), (3, 14), (13, 14), (13, 6)],
            &[(6, 9), (10, 9)],
        ],
        Icon::Image => &[
            &[(1, 2), (15, 2), (15, 14), (1, 14), (1, 2)],
            &[(2, 12), (6, 8), (9, 11), (12, 6), (15, 10)],
            &[(4, 5), (5, 5)],
        ],
        Icon::Weapon => &[
            &[(2, 14), (12, 3), (14, 2), (14, 5), (4, 15), (2, 14)],
            &[(5, 10), (2, 9), (6, 7)],
            &[(7, 12), (8, 15), (10, 10)],
        ],
        Icon::Object => &[
            &[(1, 8), (8, 2), (15, 8)],
            &[(3, 7), (3, 14), (13, 14), (13, 7)],
            &[(6, 14), (6, 10), (10, 10), (10, 14)],
        ],
        Icon::Palette => &[
            &[
                (8, 1),
                (3, 3),
                (1, 8),
                (3, 13),
                (8, 15),
                (10, 13),
                (9, 10),
                (14, 9),
                (15, 6),
                (12, 2),
                (8, 1),
            ],
            &[(4, 5), (5, 5)],
            &[(8, 4), (9, 4)],
            &[(4, 9), (5, 9)],
        ],
        Icon::Mission => &[
            &[(3, 1), (10, 1), (14, 5), (14, 15), (3, 15), (3, 1)],
            &[(10, 1), (10, 5), (14, 5)],
            &[(6, 8), (11, 8)],
            &[(6, 11), (11, 11)],
        ],
        Icon::Sound => &[
            &[(2, 6), (5, 6), (9, 2), (9, 14), (5, 10), (2, 10), (2, 6)],
            &[(12, 5), (14, 8), (12, 11)],
        ],
        Icon::Search => &[
            &[
                (6, 2),
                (2, 4),
                (2, 8),
                (5, 11),
                (9, 10),
                (11, 7),
                (10, 3),
                (6, 2),
            ],
            &[(10, 10), (15, 15)],
        ],
        Icon::Plus => &[&[(8, 3), (8, 13)], &[(3, 8), (13, 8)]],
        Icon::Select => &[&[(3, 2), (13, 7), (8, 9), (6, 14), (3, 2)]],
        Icon::Move => &[
            &[(8, 1), (8, 15)],
            &[(1, 8), (15, 8)],
            &[(5, 4), (8, 1), (11, 4)],
            &[(5, 12), (8, 15), (11, 12)],
            &[(4, 5), (1, 8), (4, 11)],
            &[(12, 5), (15, 8), (12, 11)],
        ],
        Icon::Rotate => &[
            &[
                (13, 5),
                (10, 2),
                (5, 2),
                (2, 6),
                (2, 10),
                (6, 14),
                (11, 13),
                (14, 9),
            ],
            &[(13, 1), (13, 5), (9, 5)],
        ],
        Icon::Scale => &[
            &[(2, 14), (14, 2)],
            &[(9, 2), (14, 2), (14, 7)],
            &[(2, 9), (2, 14), (7, 14)],
        ],
        Icon::Brush => &[
            &[(2, 13), (5, 10), (8, 13), (2, 13)],
            &[(6, 9), (12, 2), (15, 5), (9, 11)],
        ],
        Icon::Frame => &[
            &[(2, 6), (2, 2), (6, 2)],
            &[(10, 2), (14, 2), (14, 6)],
            &[(14, 10), (14, 14), (10, 14)],
            &[(6, 14), (2, 14), (2, 10)],
        ],
        Icon::Flight => &[
            &[(2, 12), (2, 7), (5, 3), (11, 3), (14, 7), (14, 12)],
            &[(8, 11), (11, 6)],
        ],
        Icon::Check => &[&[(2, 8), (6, 12), (14, 3)]],
        Icon::Warn => &[
            &[(8, 1), (15, 14), (1, 14), (8, 1)],
            &[(8, 5), (8, 9)],
            &[(8, 11), (8, 12)],
        ],
        Icon::Link => &[
            &[(6, 10), (3, 10), (1, 8), (1, 5), (4, 2), (7, 2), (9, 4)],
            &[
                (7, 12),
                (9, 14),
                (12, 14),
                (15, 11),
                (15, 8),
                (13, 6),
                (10, 6),
            ],
            &[(5, 11), (11, 5)],
        ],
    };
    for path in paths {
        for pair in path.windows(2) {
            d.line(
                x + pair[0].0,
                y + pair[0].1,
                x + pair[1].0,
                y + pair[1].1,
                color,
            );
        }
    }
}
pub(super) fn border(d: &mut Canvas, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
    d.line(x, y, x + w - 1, y, color);
    d.line(x, y + h - 1, x + w - 1, y + h - 1, color);
    d.line(x, y, x, y + h - 1, color);
    d.line(x + w - 1, y, x + w - 1, y + h - 1, color);
}
pub(super) fn text_fit(d: &mut Canvas, x: i32, y: i32, w: i32, s: &str, color: Rgb) {
    d.text(x, y, &short(s, (w.max(0) / 7) as usize), color);
}
pub(super) fn label_fit(d: &mut Canvas, x: i32, y: i32, w: i32, s: &str, color: Rgb) {
    d.label(x, y, &short(s, (w.max(0) / 7) as usize), color);
}
pub(super) fn badge(d: &mut Canvas, x: i32, y: i32, s: &str) {
    let w = s.len() as i32 * 7 + 8;
    d.rect(x, y, w, 16, c::GM_600);
    d.text(x + 4, y + 12, s, c::INK_MUTED);
}
fn chevron(d: &mut Canvas, x: i32, y: i32, open: bool) {
    if open {
        d.line(x, y, x + 4, y + 4, c::INK_MUTED);
        d.line(x + 4, y + 4, x + 8, y, c::INK_MUTED);
    } else {
        d.line(x, y, x + 4, y + 4, c::INK_MUTED);
        d.line(x + 4, y + 4, x, y + 8, c::INK_MUTED);
    }
}
impl Layout {
    pub(super) fn hit(&mut self, rect: [i32; 4], action: Action) {
        self.hits.push(Hit { rect, action });
    }
    pub(super) fn button(&mut self, rect: [i32; 4], title: &str, action: Action, active: bool) {
        let [x, y, w, h] = rect;
        let d = &mut self.canvas;
        d.rect(
            x,
            y,
            w,
            h,
            if active {
                c::AMBER_DEEP
            } else if self.mouse[0] >= x
                && self.mouse[0] < x + w
                && self.mouse[1] >= y
                && self.mouse[1] < y + h
            {
                c::GM_600
            } else {
                c::GM_700
            },
        );
        border(d, x, y, w, h, c::GM_1000);
        d.line(x + 1, y + 1, x + w - 2, y + 1, c::GM_600);
        label_fit(
            d,
            x + 9,
            y + h / 2 + 4,
            w - 16,
            title,
            if active { c::AMBER } else { c::INK },
        );
        self.hit(rect, action);
    }
    pub(super) fn tool(
        &mut self,
        rect: [i32; 4],
        i: Icon,
        action: Action,
        enabled: bool,
        active: bool,
    ) {
        let [x, y, w, h] = rect;
        self.canvas.rect(
            x,
            y,
            w,
            h,
            if active {
                c::AMBER_DEEP
            } else if self.mouse[0] >= x
                && self.mouse[0] < x + w
                && self.mouse[1] >= y
                && self.mouse[1] < y + h
            {
                c::GM_600
            } else {
                c::GM_700
            },
        );
        border(&mut self.canvas, x, y, w, h, c::GM_1000);
        icon(
            &mut self.canvas,
            x + (w - 16) / 2,
            y + (h - 16) / 2,
            i,
            if !enabled {
                c::INK_FAINT
            } else if active {
                c::AMBER
            } else {
                c::INK_MUTED
            },
        );
        if enabled {
            self.hit(rect, action);
        }
    }
}
impl App {
    pub(super) fn act(&mut self, a: Action) {
        if !matches!(a, Action::Menu(_)) {
            self.menu = None;
        }
        if !matches!(a, Action::Filter) {
            self.filter_focus = false;
        }
        match a {
            Action::BrowserUp => {
                if let Some(b) = &self.browser {
                    let p = Self::parent_path(&b.folder);
                    self.browse_folder(&p);
                }
            }
            Action::BrowserRoots => {
                self.browser = Some(Browser {
                    folder: "Drives / locations".into(),
                    files: crate::platform::roots()
                        .into_iter()
                        .map(|p| FileItem {
                            name: p.clone(),
                            path: p,
                            directory: true,
                        })
                        .collect(),
                    scroll: 0,
                });
            }
            Action::BrowserPick(i) => self.browse_pick(i),
            Action::Recent(i) => self.recent_open(i),
            Action::PlayAudio => {
                let result = Pcm::parse(self.name(), &self.data)
                    .map(|p| p.wav())
                    .and_then(crate::platform::play_audio);
                match result {
                    Ok(()) => self.status = "Audio preview started".into(),
                    Err(e) => self.status = format!("Error: {e}"),
                };
            }
            Action::StopAudio => crate::platform::stop_audio(),
            Action::PaintToggle => {
                self.paint_enabled = !self.paint_enabled;
                self.pick_color = false;
            }
            Action::PickColor => {
                self.pick_color = !self.pick_color;
                self.paint_enabled = false;
            }
            Action::Brush(i) => self.brush = i,
            Action::Radius(n) => self.brush_radius = n,
            Action::OpenTexture(i) => self.open_texture(i),
            Action::Textured => {
                self.textured = !self.textured;
                self.perspective = false;
            }
            Action::PaintLock => {
                self.finish_stroke();
                self.paint_lock = !self.paint_lock;
            }
            Action::ModelPaint => {
                self.finish_stroke();
                self.animation_tool = false;
                self.animation_state.clear();
                self.preview = None;
                self.mesh_edit = false;
                self.hp_tool = false;
                self.media_tab = 0;
                self.model_paint = !self.model_paint;
                self.perspective = false;
                self.textured = true;
            }
            Action::Isolate => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::Isolate,
                    title: "Clone PIC and retarget this shape's decoded references (new 8.3 name)"
                        .into(),
                    value: "LIVERY.PIC".into(),
                    axis: 0,
                });
            }
            Action::RepairPanels => {
                let result = self.repair_panels();
                self.result(result);
            }
            Action::PanelTexture => {
                let result = self.create_panel_texture();
                self.result(result);
            }
            Action::MeshMode => {
                self.finish_stroke();
                self.animation_tool = false;
                self.animation_state.clear();
                self.mesh_edit = !self.mesh_edit;
                self.mesh_drag = None;
                self.model_paint = false;
                self.paint_enabled = false;
                self.decal_draft = None;
                self.preview = None;
                self.hp_tool = false;
                self.hp_visible = false;
                self.decal_active = false;
                self.mode = Mode::Model;
                self.media_tab = 0;
                self.selected_face = None;
            }
            Action::MeshVertex(i) => self.mesh_select(i),
            Action::MeshAll => self.mesh_toggle_all(),
            Action::MeshMove => self.mesh_transform_prompt('g'),
            Action::BaseColor(face) => self.base_color_prompt(face),
            Action::Recolor => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::Recolor,
                    title: "Surface palette remap: FROM TO (0..255), decoded untextured faces"
                        .into(),
                    value: format!(
                        "{} {}",
                        self.selected_face
                            .and_then(|i| self.model.as_ref().and_then(|m| m.faces.get(i)))
                            .map_or(self.brush, |f| f.color),
                        self.brush
                    ),
                    axis: 0,
                });
            }
            Action::CloneBack => {
                if self
                    .prompt
                    .as_ref()
                    .is_some_and(|p| matches!(p.kind, PromptKind::CloneReview))
                {
                    self.clone_draft = None;
                    self.prompt = Some(Prompt {
                        kind: PromptKind::CloneTitle,
                        title: "Export object / step 2: display name".into(),
                        value: self.clone_title.clone(),
                        axis: 0,
                    });
                } else {
                    if let Some(p) = &self.prompt {
                        self.clone_title = p.value.clone();
                    }
                    self.prompt = Some(Prompt {
                        kind: PromptKind::CloneId,
                        title: "Export object / step 1: new object ID".into(),
                        value: self.variant_id.clone(),
                        axis: 0,
                    });
                }
            }
            Action::Menu(n) => self.menu = if self.menu == Some(n) { None } else { Some(n) },
            Action::Mode(m) => {
                if self.animation_tool {
                    self.animation_tool = false;
                    self.animation_state.clear();
                    self.preview = None;
                }
                if m == Mode::Model && self.model.is_none() {
                    if let Some(i) = self.context_entry {
                        let face = self.selected_face;
                        self.select_entry(i);
                        self.selected_face = face;
                    }
                }
                if m == Mode::Media && self.pic.is_none() {
                    if let Some(name) = self
                        .model
                        .as_ref()
                        .and_then(|m| m.textures.iter().next())
                        .cloned()
                    {
                        let name = if name.contains('.') {
                            name
                        } else {
                            format!("{name}.PIC")
                        };
                        if let Some(i) = self.doc.archive.find(&name) {
                            self.open_texture(i);
                            return;
                        }
                    }
                }
                self.mode = m;
                if m == Mode::Properties
                    && self.field_group.is_none()
                    && !self.envelope_rows().is_empty()
                {
                    self.field_group = Some(hangar_core::definition::Aspect::Envelope);
                }
            }
            Action::File(f) => self.file_prompt(f),
            Action::Demo => self.demo(),
            Action::Close => self.close(),
            Action::Help => {
                self.dock = 2;
                self.key(Key::F1, false, false);
            }
            Action::Undo => {
                self.doc.undo();
                self.refresh();
                self.status = self.doc.summary();
            }
            Action::Redo => {
                self.doc.redo();
                self.refresh();
                self.status = self.doc.summary();
            }
            Action::Filter => {
                self.filter_focus = true;
                self.category = None;
                self.table_scroll = 0;
            }
            Action::Category(n) => {
                self.collapsed[n] = !self.collapsed[n];
                self.category = Some(n);
                self.table_scroll = 0;
            }
            Action::Entry(i) => self.select_entry(i),
            Action::Related(i) => {
                self.finish_stroke();
                self.filter.clear();
                if self
                    .doc
                    .archive
                    .entries
                    .get(i)
                    .is_some_and(|e| e.name.ends_with(".PIC"))
                    && self.model.is_some()
                {
                    self.open_texture(i);
                } else {
                    self.select_entry(i);
                }
                self.mode = match extension(self.name()) {
                    "PT" | "SH" => Mode::Model,
                    "PIC" | "5K" | "11K" | "WAV" => Mode::Media,
                    _ => Mode::Browse,
                };
                self.dock = 4;
            }
            Action::Field(i) => self.edit_field(i),
            Action::PickField(i) => self.field_selected = i,
            Action::ResetField(i) => {
                if let Some(value) = self
                    .original_brf
                    .as_ref()
                    .and_then(|b| b.fields.get(i))
                    .map(|f| f.value.clone())
                {
                    let result = self
                        .brf
                        .as_ref()
                        .ok_or_else(|| "No fields".to_string())
                        .and_then(|b| b.edit(&self.data, i, &value, extension(self.name())))
                        .and_then(|b| self.doc.replace(self.selected, b));
                    self.result(result);
                    self.refresh();
                }
            }
            Action::View(n) => {
                if n == 0 {
                    self.frame();
                } else {
                    self.key(Key::Num(n), false, false);
                }
            }
            Action::Transform(c) => self.key(Key::Char(c), false, false),
            Action::Dock(n) => self.dock = n,
            Action::Validate => {
                self.finish_stroke();
                let report = self.package_report();
                self.status = report.summary();
                self.validation = Some(report);
                self.validation_scroll = 0;
                self.mode = Mode::Package;
            }
            Action::ClearSources => {
                self.dependency_catalogs.clear();
                self.refresh();
                self.status = "Source catalogs cleared".into();
            }
            Action::LibraryToggle(id) => self.toggle_library(id),
            Action::LibraryCategory(id, cat) => {
                let result = self.switch_library(id);
                self.result(result);
                self.act(Action::Category(cat));
            }
            Action::TransferMove(moving) => self.transfer_move = moving,
            Action::Library(id) => {
                let result = self.switch_library(id);
                self.result(result);
            }
            Action::LibraryEntry(id, i) => {
                let result = self.switch_library(id);
                if result.is_ok() {
                    self.filter.clear();
                    self.select_entry(i);
                    self.dock = 4;
                }
                self.result(result);
            }
            Action::NewLibrary => {
                let result = self.install_library(
                    Document::new(Archive::empty()),
                    format!("Untitled{}.LIB", self.next_library_id),
                );
                if result.is_ok() {
                    self.mode = Mode::Browse;
                    self.context_model = None;
                    self.context_entry = None;
                    self.refresh();
                }
                self.result(result);
            }
            Action::DiscardChanges => match self.prompt.as_ref().map(|p| &p.kind) {
                Some(PromptKind::Discard) => {
                    self.prompt = None;
                    self.quit = true;
                }
                Some(PromptKind::CloseLibrary) => {
                    self.prompt = None;
                    self.discard_library();
                }
                _ => {}
            },
            Action::Animation => self.open_animation(),
            Action::AnimationPart(step) => {
                let n = self
                    .preview
                    .as_ref()
                    .or(self.model.as_ref())
                    .map_or(0, |m| m.parts.len());
                if n > 0 {
                    self.animation_part =
                        (self.animation_part as i32 + step).rem_euclid(n as i32) as usize;
                }
            }
            Action::PartPosition(axis) => self.part_position_prompt(axis),
            Action::AnimationState(address) => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::AnimationState(address),
                    title: format!("Preview state {address:08X} / integer value; does not edit SH"),
                    value: self
                        .animation_state
                        .get(&address)
                        .copied()
                        .unwrap_or(0)
                        .to_string(),
                    axis: 0,
                });
            }
            Action::AnimationReset => {
                self.animation_state.clear();
                let result = self.animation_preview();
                self.result(result);
            }
            Action::CloseLibrary => {
                let result = self.close_library();
                self.result(result);
            }
            Action::CopyResource => {
                let result = self.copy_resource();
                self.result(result);
            }
            Action::PasteResources => {
                let result = self.paste_resources();
                self.result(result);
            }
            Action::RenameResource(duplicate) => self.rename_prompt(duplicate),
            Action::TransferChoice(i, take) => {
                if let Some(item) = self.transfer_plan.as_mut().and_then(|p| p.items.get_mut(i)) {
                    item.choice = if take {
                        hangar_core::resource_ops::Choice::TakeSource
                    } else {
                        hangar_core::resource_ops::Choice::KeepTarget
                    };
                }
            }
            Action::TransferDependencies => {
                self.include_dependencies = !self.include_dependencies;
                let result = self.paste_resources();
                self.result(result);
            }
            Action::TransferNote => {
                self.transfer_note = (self.transfer_note + 1)
                    % self
                        .transfer_note_pages((self.width - 32).min(900))
                        .len()
                        .max(1);
            }
            Action::MediaTab(tab) => {
                self.media_tab = tab;
                self.hp_tool = false;
                if self.pic.is_some() || self.name().ends_with(".PAL") {
                    self.mode = Mode::Media;
                } else if self.model.is_some() {
                    self.mode = Mode::Model;
                }
            }
            Action::MaterialPrompt(kind) => self.material_prompt(kind),
            Action::DecalLibrary => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::DecalLibrary,
                    title: "Imported PNG / squadron library".into(),
                    value: String::new(),
                    axis: 0,
                });
            }
            Action::DecalSaved(i) => {
                if let Some(path) = self.decal_paths.get(i).cloned() {
                    let result = self.load_decal(&path, false);
                    if result.is_ok() {
                        self.prompt = None;
                    }
                    self.result(result);
                }
            }
            Action::DecalForget(i) => {
                if i < self.decal_paths.len() {
                    self.decal_paths.remove(i);
                    let result = crate::platform::save_decals(&self.decal_paths);
                    self.result(result);
                }
            }
            Action::DecalInk => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::DecalInk,
                    title: "Tail-text ink / palette index 0..255".into(),
                    value: format!("{}", self.text_ink()),
                    axis: 0,
                })
            }
            Action::DecalText => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::DecalText,
                    title: "Tail number / letters, digits, space, dash, slash, period".into(),
                    value: self.decal_text.clone(),
                    axis: 0,
                })
            }
            Action::DecalPreset(next) => {
                if next {
                    self.decal_preset =
                        (self.decal_preset + 1) % hangar_core::decal::NATIONAL_NAMES.len();
                }
                let result =
                    hangar_core::decal::Image::national(self.decal_preset).and_then(|image| {
                        self.set_decal(
                            image,
                            hangar_core::decal::NATIONAL_NAMES[self.decal_preset].into(),
                            false,
                        )
                    });
                self.result(result);
            }
            Action::DecalSetting(key) => self.decal_setting_prompt(key),
            Action::DecalMirror => {
                let mut p = self.decal_placement;
                p.mirror = !p.mirror;
                if let Some(entry) = self
                    .decal_draft
                    .as_ref()
                    .map(|d| d.entry)
                    .or_else(|| self.texture_target())
                {
                    let result = self.prepare_decal(entry, p);
                    self.result(result);
                } else {
                    self.decal_placement = p;
                }
            }
            Action::DecalPlace => {
                if self.decal_image.is_some() {
                    self.decal_active = true;
                    self.paint_enabled = false;
                    self.model_paint = false;
                    self.status = "Click the texture or a model panel to preview the decal".into();
                } else {
                    self.status =
                        "Import PNG, enter tail text or choose a national marking first".into();
                }
            }
            Action::DecalCancel => {
                self.decal_draft = None;
                self.decal_active = false;
                self.decal_dragging = false;
            }
            Action::DecalApply => {
                let result = self.apply_decal();
                self.result(result);
            }
            Action::Hardpoints => {
                self.animation_tool = false;
                self.animation_state.clear();
                self.preview = None;
                self.hp_tool = self.mode != Mode::Model || !self.hp_tool;
                self.mode = Mode::Model;
                self.hp_visible = true;
                self.selected_face = None;
                self.decal_active = false;
                self.decal_draft = None;
            }
            Action::StationSlew => self.hp_slew = !self.hp_slew,
            Action::HardpointVisibility => self.hp_visible = !self.hp_visible,
            Action::HardpointSelect(i) => {
                self.hp_selected = i;
                self.hp_tool = true;
            }
            Action::HardpointStep(step) => {
                let n = self.hp_context.as_ref().map_or(0, |c| c.stations.len());
                if n > 0 {
                    self.hp_selected =
                        (self.hp_selected as i32 + step).rem_euclid(n as i32) as usize;
                }
            }
            Action::HardpointAdd(duplicate) => {
                let result = self.station_add(duplicate, false);
                self.result(result);
            }
            Action::HardpointRemove => {
                let result = self.station_remove();
                self.result(result);
            }
            Action::StationFields => {
                if let Some(entry) = self.hp_context.as_ref().map(|c| c.entry) {
                    let station = self.hp_selected;
                    self.select_entry(entry);
                    self.field_group = Some(hangar_core::definition::Aspect::Hardpoints);
                    self.field_scroll = 1 + station * 12;
                    self.mode = Mode::Properties;
                }
            }
            Action::StationField(column) => self.station_prompt(column),
            Action::PinDonor => {
                let result = self.pin_donor();
                self.result(result);
            }
            Action::GraftDonor(i) => {
                let result = self.choose_graft_donor(i);
                self.result(result);
            }
            Action::GraftGroup(i) => {
                if let Some(group) = hangar_core::definition::ASPECTS.get(i) {
                    self.graft_mask ^= group.bit();
                    self.refresh_graft();
                }
            }
            Action::ApplyGraft => {
                let result = self.apply_graft();
                self.result(result);
            }
            Action::EnvelopeStep(delta) => {
                let n = self.envelope_rows().len();
                if n > 0 {
                    self.envelope_selected =
                        (self.envelope_selected as i32 + delta).rem_euclid(n as i32) as usize;
                    self.envelope_scroll = 0;
                }
            }
            Action::FieldGroup(i) => {
                self.envelope_selected = 0;
                self.envelope_scroll = 0;
                self.field_group = hangar_core::definition::ASPECTS.get(i).copied();
                self.field_scroll = 0;
                self.mode = Mode::Properties;
            }
            Action::Apply => self.key(Key::Enter, false, false),
            Action::Cancel => self.key(Key::Escape, false, false),
        }
    }
    #[cfg(not(windows))]
    pub fn workspace(&mut self, name: &str) -> Result<()> {
        if name == "libraries" || name == "move-review" {
            self.path = "SOURCE.LIB".into();
            let source = self.library_id;
            self.install_library(Document::new(Archive::empty()), "TARGET.LIB".into())?;
            let target = self.library_id;
            self.refresh();
            self.switch_library(source)?;
            self.select_entry(0);
            if name == "move-review" {
                self.prepare_drop(target)?;
            } else {
                self.scroll = 0;
            }
            return Ok(());
        }

        if name == "paint-model" {
            self.mode = Mode::Model;
            self.act(Action::ModelPaint);
            return Ok(());
        }
        if name == "animation" {
            self.open_animation();
            return Ok(());
        }
        if name == "stations" {
            self.act(Action::Hardpoints);
            self.hp_slew = true;
            return Ok(());
        }
        if name == "discard" {
            self.doc.mark_unsaved();
            self.close();
            return Ok(());
        }

        if name == "envelope" {
            self.select_entry(1);
            self.act(Action::Mode(Mode::Properties));
            return Ok(());
        }
        if name == "mesh" || name == "base-color" || name == "auto-texture" {
            self.doc.replace(0, model::demo_shape())?;
            self.doc.mark_saved();
            self.palette_override = Some(Box::new(
                Pic::parse(&picture::demo())?.colors(&[[0; 3]; 256]),
            ));
            self.select_entry(0);
            self.mode = Mode::Model;
            self.textured = true;
            if name == "mesh" {
                self.mesh_edit = true;
                self.mesh_vertices = vec![0];
            }
            if name == "base-color" {
                self.base_color_prompt(false);
            }
            if name == "auto-texture" {
                self.selected_face = Some(0);
                self.create_panel_texture()?;
            }
            return Ok(());
        }
        if name == "hardpoints" {
            self.select_entry(1);
            self.mode = Mode::Model;
            self.hp_tool = true;
            self.hp_visible = true;
            return Ok(());
        }
        if name == "materials" {
            self.select_entry(0);
            self.selected_face = self
                .model
                .as_ref()
                .and_then(|m| m.faces.iter().position(|f| !f.uv.is_empty()));
            self.media_tab = 1;
            self.mode = Mode::Model;
            return Ok(());
        }
        if name == "decals" || name == "decal-text" {
            self.select_entry(0);
            self.selected_face = self
                .model
                .as_ref()
                .and_then(|m| m.faces.iter().position(|f| !f.uv.is_empty()));
            let texture = self.texture_target().ok_or("No texture")?;
            self.open_texture(texture);
            self.media_tab = 2;
            if name == "decal-text" {
                self.tail_text("01")?;
            } else {
                self.set_decal(
                    hangar_core::decal::Image::national(0)?,
                    "US stars and bars".into(),
                    false,
                )?;
            }
            self.decal_setting(2, "24")?;
            return Ok(());
        }
        if name == "libraries" || name == "transfer-review" {
            self.path = "SOURCE.LIB".into();
            self.select_entry(0);
            self.copy_resource()?;
            let mut target = Archive::empty();
            let mut pic = picture::demo();
            pic.push(0);
            target.entries.push(Entry::new("DEMO.PIC", pic)?);
            self.install_library(Document::new(target), "TARGET.LIB".into())?;
            self.refresh();
            self.mode = Mode::Browse;
            if name == "transfer-review" {
                self.paste_resources()?;
            }
            return Ok(());
        }
        if name == "graft-review" {
            self.select_entry(1);
            self.pin_donor()?;
            let b = self.brf.as_ref().ok_or("No definition")?;
            let weight = b
                .fields
                .iter()
                .position(|f| f.label == "object.weight")
                .ok_or("No weight")?;
            let bytes = b.edit(&self.data, weight, "23456", "PT")?;
            self.doc.import("TARGET.PT", bytes)?;
            self.select_entry(self.doc.archive.entries.len() - 1);
            self.mode = Mode::Graft;
            self.graft_mask = hangar_core::definition::Aspect::Weights.bit();
            self.refresh_graft();
            return Ok(());
        }
        if name == "references" {
            self.mode = if self.pic.is_some() {
                Mode::Media
            } else {
                Mode::Model
            };
            self.dock = 4;
            return Ok(());
        }
        if name == "validation" {
            self.act(Action::Validate);
            return Ok(());
        }
        if name == "textured" {
            self.mode = Mode::Model;
            self.textured = true;
            return Ok(());
        }
        if name == "browser" {
            self.file_prompt(FileAction::Open);
            return Ok(());
        }
        if name == "uv" {
            if let Some((fi, face)) = self.model.as_ref().and_then(|m| {
                m.faces
                    .iter()
                    .enumerate()
                    .find(|(_, f)| !f.uv.is_empty() && !f.texture.is_empty())
            }) {
                let name = if face.texture.contains('.') {
                    face.texture.clone()
                } else {
                    format!("{}.PIC", face.texture)
                };
                if let Some(i) = self.doc.archive.find(&name) {
                    self.selected_face = Some(fi);
                    self.open_texture(i);
                    return Ok(());
                }
            }
        }
        if name == "clone-review" {
            self.begin_clone();
            self.variant_id = "NEWJET".into();
            self.clone_title = "Export object".into();
            self.clone_draft = Some(self.build_clone()?);
            self.prompt = Some(Prompt {
                kind: PromptKind::CloneReview,
                title: "Review".into(),
                value: String::new(),
                axis: 0,
            });
            return Ok(());
        }
        self.mode = match name {
            "browse" => Mode::Browse,
            "model" => Mode::Model,
            "flight" => Mode::Properties,
            "graft" => Mode::Graft,
            "package" => Mode::Package,
            "media" => Mode::Media,
            _ => return Err("Unknown workspace".into()),
        };
        Ok(())
    }
    fn lib_name(&self) -> &str {
        if self.path.starts_with("Synthetic") {
            "DEMO.LIB"
        } else if self.path.is_empty() {
            "No LIB open"
        } else {
            self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path)
        }
    }
    pub(super) fn browser_entries(&self) -> Vec<usize> {
        let f = self.filter.to_ascii_uppercase();
        self.doc
            .archive
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                self.category.is_none_or(|n| category_of(&e.name) == n) && e.name.contains(&f)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn field_changed(&self, i: usize) -> bool {
        self.brf
            .as_ref()
            .and_then(|b| b.fields.get(i))
            .is_some_and(|f| {
                self.original_brf
                    .as_ref()
                    .and_then(|b| b.fields.get(i))
                    .is_none_or(|old| old.value != f.value)
            })
    }
    pub(super) fn inspector_max_scroll(&self) -> i32 {
        let rows = self.property_rows();
        let mut groups = BTreeSet::new();
        for row in &rows {
            groups.insert(row.0);
        }
        ((116 + rows.len() as i32 * 25 + groups.len() as i32 * 37 - (self.height - 50)).max(0) + 23)
            / 24
    }
    fn property_rows(&self) -> Vec<(&'static str, &'static str, usize)> {
        let mut result = Vec::new();
        let Some(b) = &self.brf else {
            return result;
        };
        let names = [
            ("Envelope", "Max speed", "object._maxSpeed"),
            ("Envelope", "Corner speed", "object._cornerSpeed"),
            ("Envelope", "Min speed", "object._minSpeed"),
            ("Envelope", "Max altitude", "object.maxAlt"),
            ("Envelope", "Envelope min", "plane.envMin"),
            ("Envelope", "Envelope max", "plane.envMax"),
            ("Propulsion", "Engines", "plane.engines"),
            ("Propulsion", "Thrust", "plane.thrust"),
            ("Propulsion", "Fuel use", "plane.fuelConsumption"),
            ("Propulsion", "AB fuel use", "plane.aftFuelConsumption"),
            ("Propulsion", "Internal fuel", "plane.internalFuel"),
            ("Weights", "Empty weight", "object.weight"),
            ("Weights", "Max takeoff", "plane.maxTakeoffWeight"),
            ("Handling", "Turn rate", "object._turnRate"),
            ("Handling", "Bank rate", "object._bankRate"),
            ("Handling", "Max climb", "object.maxClimb"),
            ("Handling", "Max dive", "object.maxDive"),
            ("Structure", "Hit points", "object.hitPoints"),
            ("Structure", "Year", "object.year"),
        ];
        for (group, label, name) in names {
            if let Some(i) = b.fields.iter().position(|f| f.label == name) {
                result.push((group, label, i));
            }
        }
        result
    }
    pub fn draw(&self) -> Canvas {
        self.layout().canvas
    }
    pub(super) fn layout(&self) -> Layout {
        let mut out = Layout {
            mouse: self.mouse,
            canvas: Canvas {
                commands: Vec::new(),
            },
            hits: Vec::new(),
        };
        let w = self.width;
        let h = self.height;
        let l = self.left();
        let r = self.right();
        let dock = self.dock_y();
        out.canvas.rect(0, 0, w, h, c::GM_800);
        out.canvas.rect(0, 0, w, 26, c::GM_950);
        icon(&mut out.canvas, 8, 5, Icon::Logo, c::AMBER);
        out.canvas.label(30, 18, "Hangar", c::INK);
        out.hit([4, 0, 78, 26], Action::Demo);
        let menus = [
            ("File", 86, 34),
            ("Edit", 124, 34),
            ("Lib", 163, 28),
            ("Entry", 196, 42),
            ("View", 243, 38),
            ("Tools", 286, 42),
            ("Help", 333, 36),
        ];
        for (i, (name, x, width)) in menus.iter().enumerate() {
            if self.menu == Some(i) {
                out.canvas.rect(*x, 0, *width, 26, c::GM_700);
            }
            out.canvas.label(*x + 4, 18, name, c::INK_MUTED);
            out.hit([*x, 0, *width, 26], Action::Menu(i));
        }
        let tabs = [
            ("Browse", Mode::Browse, 62),
            ("Model", Mode::Model, 60),
            ("Flight", Mode::Properties, 56),
            ("Graft", Mode::Graft, 54),
            ("Package", Mode::Package, 70),
            ("Paint", Mode::Media, 52),
        ];
        out.canvas.line(378, 4, 378, 22, c::LINE_STRONG);
        out.canvas.rect(384, 1, 366, 25, c::GM_900);
        let mut tx = 386;
        for (name, m, width) in tabs {
            let active = self.mode == m;
            out.canvas
                .rect(tx, 3, width, 23, if active { c::GM_700 } else { c::GM_800 });
            border(
                &mut out.canvas,
                tx,
                3,
                width,
                23,
                if active { c::STEEL } else { c::LINE_STRONG },
            );
            if active {
                out.canvas.rect(tx + 1, 3, width - 2, 2, c::AMBER);
                out.canvas.line(tx + 1, 25, tx + width - 2, 25, c::GM_700);
            }
            out.canvas.label(
                tx + 9,
                18,
                name,
                if self.mode == m { c::INK } else { c::INK_MUTED },
            );
            out.hit([tx, 0, width, 26], Action::Mode(m));
            tx += width + 2;
        }
        if w > 1050 {
            text_fit(
                &mut out.canvas,
                w - 220,
                18,
                204,
                &format!(
                    "{}{}",
                    self.lib_name(),
                    if hangar_core::save::protected_name(&self.path).is_some() {
                        " [Protected]"
                    } else {
                        ""
                    }
                ),
                c::INK_MUTED,
            );
            if self.doc.dirty() {
                out.canvas.rect(w - 234, 10, 5, 5, c::AMBER);
            }
        }
        if self.mode == Mode::Package {
            self.package_layout(&mut out);
        } else {
            self.outliner(&mut out);
            out.canvas.rect(l + 1, 26, r - l - 2, h - 48, c::GM_950);
            if self.mode == Mode::Model {
                self.model_layout(&mut out);
            } else if self.mode == Mode::Browse {
                self.browse_layout(&mut out);
            } else if self.mode == Mode::Properties {
                self.fields_layout(&mut out, l + 1, 26, r - l - 2, dock - 26, true);
            } else if self.mode == Mode::Media {
                self.media_layout(&mut out);
            } else {
                self.graft_layout(&mut out);
            }
            if self.mode == Mode::Model && self.animation_tool {
                self.animation_inspector(&mut out);
            } else if self.mode == Mode::Model && self.mesh_edit {
                self.mesh_inspector(&mut out);
            } else if self.mode == Mode::Model && self.hp_tool {
                self.hardpoint_inspector(&mut out);
            } else if self.mode == Mode::Graft {
                self.graft_inspector(&mut out);
            } else if self.mode == Mode::Properties {
                self.definition_inspector(&mut out);
            } else if self.mode == Mode::Media
                || self.selected_face.is_some()
                || (self.mode == Mode::Model && self.model_paint)
                || (self.mode == Mode::Model && self.media_tab > 0)
            {
                self.media_inspector(&mut out);
            } else {
                self.inspector(&mut out);
            }
            self.dock_layout(&mut out, l + 1, dock, r - l - 2, h - dock - 22);
            out.canvas.line(l, 26, l, h - 22, c::GM_1000);
            out.canvas.line(r, 26, r, h - 22, c::GM_1000);
        }
        out.canvas.rect(0, h - 22, w, 22, c::GM_950);
        let msg = if !self.status.starts_with("Opened ")
            && !self.status.starts_with("Ready")
            && !self.status.starts_with("Synthetic demo")
            && !self.status.is_empty()
        {
            self.status.as_str()
        } else {
            match self.mode {
                Mode::Model => "G Move   R Rotate   S Scale   MMB Orbit   Shift+MMB Pan",
                Mode::Browse => "Click Select   Ctrl+F Filter   Ctrl+E Export   Delete Remove",
                Mode::Properties => "Click value Edit   Wheel Scroll   Ctrl+Z Undo",
                Mode::Graft => "Choose donor   Select aspects   Review changes   Apply graft",
                Mode::Package => {
                    "Ctrl+B Package   Retail names protected / custom saves keep backups"
                }
                Mode::Media => "Paint indexed colors / one stroke per undo / Ctrl+S package",
            }
        };
        text_fit(
            &mut out.canvas,
            10,
            h - 7,
            w - 245,
            msg,
            if self.status.starts_with("Error:") {
                c::DANGER
            } else {
                c::INK_MUTED
            },
        );
        text_fit(
            &mut out.canvas,
            w - 220,
            h - 7,
            208,
            &format!(
                "{} entries  {}",
                self.doc.archive.entries.len(),
                if self.doc.dirty() {
                    "* Modified"
                } else {
                    "Saved"
                }
            ),
            if self.doc.dirty() {
                c::AMBER
            } else {
                c::INK_FAINT
            },
        );
        if let Some(menu) = self.menu {
            self.menu_layout(&mut out, menu);
        }
        if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::CloneReview))
        {
            self.clone_review(&mut out);
        } else if self.prompt.is_some() {
            if matches!(self.prompt.as_ref().unwrap().kind, PromptKind::File(_))
                && self.browser.is_some()
            {
                self.browser_layout(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::TransferReview))
            {
                self.transfer_review(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::DecalLibrary))
            {
                self.decal_library_layout(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::BaseColor(_)))
            {
                self.color_dialog(&mut out);
            } else {
                self.prompt_layout(&mut out);
            }
        }

        out
    }
    fn outliner(&self, o: &mut Layout) {
        let l = self.left();
        let h = self.height;
        let d = &mut o.canvas;
        d.rect(0, 26, l, 28, c::GM_800);
        d.rect(6, 30, l - 40, 20, c::GM_950);
        border(
            d,
            6,
            30,
            l - 40,
            20,
            if self.filter_focus {
                c::FOCUS
            } else {
                c::LINE_STRONG
            },
        );
        icon(d, 10, 32, Icon::Search, c::INK_MUTED);
        text_fit(
            d,
            30,
            44,
            l - 68,
            if self.filter.is_empty() {
                "Filter entries"
            } else {
                &self.filter
            },
            if self.filter_focus {
                c::AMBER
            } else {
                c::INK_MUTED
            },
        );
        o.hit([6, 30, l - 40, 20], Action::Filter);
        o.tool(
            [l - 29, 29, 24, 23],
            Icon::Plus,
            Action::File(FileAction::Open),
            true,
            false,
        );
        let rows = self.library_rows();
        let visible = ((h - self.tree_start() - 54) / 22).max(1) as usize;
        for (row, (id, cat, entry)) in rows.iter().skip(self.scroll).take(visible).enumerate() {
            let active = *id == self.library_id;
            let (doc, path, root_collapsed, collapsed, filter) = if active {
                (
                    &self.doc,
                    self.lib_name(),
                    self.root_collapsed,
                    &self.collapsed,
                    &self.filter,
                )
            } else {
                let library = self.libraries.iter().find(|l| l.id == *id).unwrap();
                (
                    &library.doc,
                    library
                        .path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&library.path),
                    library.root_collapsed,
                    &library.collapsed,
                    &library.filter,
                )
            };
            let y = self.tree_start() + row as i32 * 22;
            if let Some(cat) = cat {
                if let Some(i) = entry {
                    let e = &doc.archive.entries[*i];
                    let selected = active && *i == self.selected;
                    o.canvas.rect(
                        0,
                        y,
                        l,
                        22,
                        if selected { c::AMBER_DEEP } else { c::GM_900 },
                    );
                    icon(
                        &mut o.canvas,
                        46,
                        y + 3,
                        GROUPS[*cat].2,
                        if selected { c::AMBER } else { c::INK_MUTED },
                    );
                    text_fit(
                        &mut o.canvas,
                        68,
                        y + 16,
                        l - 94,
                        &e.name,
                        if selected { c::AMBER } else { c::INK },
                    );
                    if doc.entry_changed(e) {
                        o.canvas.rect(l - 13, y + 9, 5, 5, c::AMBER);
                    }
                    o.hit(
                        [0, y, l, 22],
                        if active {
                            Action::Entry(*i)
                        } else {
                            Action::LibraryEntry(*id, *i)
                        },
                    );
                } else {
                    o.canvas.rect(0, y, l, 22, c::GM_800);
                    chevron(
                        &mut o.canvas,
                        24,
                        y + 7,
                        !collapsed[*cat] || !filter.is_empty(),
                    );
                    icon(&mut o.canvas, 40, y + 3, GROUPS[*cat].2, c::INK_MUTED);
                    label_fit(&mut o.canvas, 62, y + 16, l - 104, GROUPS[*cat].0, c::INK);
                    badge(&mut o.canvas, l - 36, y + 3, GROUPS[*cat].1);
                    o.hit(
                        [0, y, l, 22],
                        if active {
                            Action::Category(*cat)
                        } else {
                            Action::LibraryCategory(*id, *cat)
                        },
                    );
                }
            } else {
                o.canvas
                    .rect(0, y, l, 22, if active { c::GM_700 } else { c::GM_900 });
                chevron(&mut o.canvas, 8, y + 8, !root_collapsed);
                icon(
                    &mut o.canvas,
                    24,
                    y + 3,
                    Icon::Lib,
                    if active { c::STEEL } else { c::INK_MUTED },
                );
                text_fit(
                    &mut o.canvas,
                    46,
                    y + 16,
                    l - 92,
                    path,
                    if active { c::INK } else { c::INK_MUTED },
                );
                text_fit(
                    &mut o.canvas,
                    l - 39,
                    y + 16,
                    35,
                    &format!("{}", doc.archive.entries.len()),
                    c::INK_MUTED,
                );
                if doc.dirty() {
                    o.canvas.rect(l - 49, y + 9, 5, 5, c::AMBER);
                }
                o.hit([22, y, l - 22, 22], Action::Library(*id));
                o.hit([0, y, 22, 22], Action::LibraryToggle(*id));
            }
        }
        o.canvas.line(0, h - 51, l, h - 51, c::GM_1000);
        o.button(
            [8, h - 46, 76, 20],
            "Open LIB",
            Action::File(FileAction::Open),
            false,
        );
        o.button(
            [92, h - 46, l - 100, 20],
            "Export object",
            Action::File(FileAction::Variant),
            false,
        );
    }
    fn model_layout(&self, o: &mut Layout) {
        let l = self.left();
        let r = self.right();
        let dock = self.dock_y();
        let width = r - l;
        let d = &mut o.canvas;
        d.rect(l + 1, 26, width - 2, 28, c::GM_800);
        icon(d, l + 8, 32, Icon::Select, c::INK_MUTED);
        o.button(
            [l + 27, 29, 92, 22],
            if self.mesh_edit {
                "Edit mesh"
            } else {
                "Object mode"
            },
            Action::MeshMode,
            self.mesh_edit,
        );
        o.button(
            [l + 125, 29, 105, 22],
            "Hardpoints",
            Action::Hardpoints,
            self.hp_tool,
        );
        o.button(
            [l + 235, 29, 78, 22],
            "Parts",
            Action::Animation,
            self.animation_tool,
        );
        if width > 590 {
            o.button(
                [r - 254, 29, 94, 22],
                if self.textured {
                    "Wireframe"
                } else {
                    "Textured"
                },
                Action::Textured,
                self.textured,
            );
            o.button(
                [r - 153, 29, 65, 22],
                "Top",
                Action::View(7),
                self.pitch == 90,
            );
            o.button([r - 84, 29, 77, 22], "Frame", Action::View(0), false);
        }
        if let Some(m) = self.preview.as_ref().or(self.model.as_ref()) {
            if self.textured {
                self.draw_model(o, l + 1, 54, width - 2, dock - 54);
            } else {
                self.viewport(&mut o.canvas, m, l + 1, 54, width - 2, dock - 54);
            }
        } else {
            let d = &mut o.canvas;
            d.label(l + 32, 144, "A workshop for Fighters Anthology", c::INK);
            label_fit(
                d,
                l + 32,
                174,
                width - 64,
                "Open a LIB and select an aircraft or shape.",
                c::INK_MUTED,
            );
            o.button(
                [l + 32, 204, 128, 26],
                "Open LIB",
                Action::File(FileAction::Open),
                true,
            );
            o.button([l + 172, 204, 124, 26], "Load demo", Action::Demo, false);
        }
        let writable = self.model.as_ref().is_some_and(|m| m.writable)
            && self.model_entry == Some(self.selected);
        let x = l + 8;
        for (n, (i, a, enabled)) in [
            (Icon::Select, Action::View(0), true),
            (Icon::Move, Action::Transform('g'), writable),
            (Icon::Rotate, Action::Transform('r'), writable),
            (Icon::Scale, Action::Transform('s'), writable),
            (Icon::Frame, Action::View(0), true),
            (Icon::Brush, Action::ModelPaint, self.model.is_some()),
        ]
        .into_iter()
        .enumerate()
        {
            o.tool(
                [x, 64 + n as i32 * 31, 28, 28],
                i,
                a,
                enabled,
                if n == 5 {
                    self.model_paint
                } else {
                    n == 0 && !self.model_paint
                },
            );
        }
        let label = if self.perspective {
            "Perspective"
        } else if self.pitch == 90 {
            "Top / Orthographic"
        } else if self.yaw == 90 && self.pitch == 0 {
            "Side / Orthographic"
        } else if self.yaw == 0 && self.pitch == 0 {
            "Front / Orthographic"
        } else {
            "User / Orthographic"
        };
        let d = &mut o.canvas;
        d.text(l + 51, 75, label, c::INK);
        if let Some(i) = self.model_entry {
            text_fit(
                d,
                l + 51,
                95,
                width - 130,
                &self.doc.archive.entries[i].name,
                c::INK_MUTED,
            );
        }
        if let Some(m) = &self.model {
            d.text(
                l + 51,
                115,
                &format!("{} verts / {} faces", m.vertices.len(), m.faces.len()),
                c::INK_MUTED,
            );
        }
        if let Some(i) = self.model_entry {
            let users = self
                .dependencies
                .incoming(&self.doc.archive.entries[i].name)
                .count();
            if users > 1 {
                label_fit(
                    d,
                    l + 51,
                    136,
                    width - 110,
                    &format!("Shared shape / {users} direct users in this LIB"),
                    c::AMBER,
                );
            }
        }
        let gx = r - 43;
        let gy = 92;
        d.line(gx - 19, gy, gx + 19, gy, c::AXIS_X);
        d.line(gx, gy + 20, gx, gy - 20, c::AXIS_Y);
        d.text(gx + 14, gy + 4, "X", c::AXIS_X);
        d.text(gx - 3, gy - 18, "Y", c::AXIS_Y);
        d.text(gx - 3, gy + 5, "Z", c::AXIS_Z);
        text_fit(
            d,
            l + 51,
            dock - 12,
            width - 150,
            if self.mesh_edit {
                if writable {
                    "Edit mode / vertices"
                } else {
                    "Edit mode / inspect only"
                }
            } else if writable {
                "Object mode / editable static mesh"
            } else {
                "Object mode / static preview"
            },
            c::INK_MUTED,
        );
        d.text(r - 65, dock - 12, &format!("{}%", self.zoom), c::INK_FAINT);
        if self.mesh_edit {
            self.mesh_overlay(o);
        } else {
            self.hardpoint_overlay(o);
        }
    }
    fn browse_layout(&self, o: &mut Layout) {
        let l = self.left();
        let r = self.right();
        let width = r - l;
        let d = &mut o.canvas;
        d.rect(l + 1, 26, width - 2, 28, c::GM_800);
        text_fit(
            d,
            l + 10,
            44,
            width - 130,
            &format!(
                "{}  >  {}",
                self.lib_name(),
                self.category.map_or("All entries", |c| GROUPS[c].0)
            ),
            c::INK_MUTED,
        );
        o.button(
            [r - 101, 29, 94, 22],
            "Open model",
            Action::Mode(Mode::Model),
            false,
        );
        let d = &mut o.canvas;
        d.rect(l + 1, 54, width - 2, 26, c::GM_900);
        d.label(l + 12, 72, "ENTRY", c::INK_MUTED);
        d.label(l + width / 2, 72, "BYTES", c::INK_MUTED);
        d.label(r - 85, 72, "STATUS", c::INK_MUTED);
        for (row, i) in self
            .browser_entries()
            .iter()
            .skip(self.table_scroll)
            .take(((self.dock_y() - 80) / 24).max(0) as usize)
            .enumerate()
        {
            let e = &self.doc.archive.entries[*i];
            let y = 80 + row as i32 * 24;
            let d = &mut o.canvas;
            let selected = *i == self.selected;
            d.rect(
                l + 1,
                y,
                width - 2,
                24,
                if selected {
                    c::AMBER_DEEP
                } else if row % 2 == 0 {
                    c::GM_800
                } else {
                    c::GM_900
                },
            );
            icon(
                d,
                l + 10,
                y + 4,
                GROUPS[category_of(&e.name)].2,
                if selected { c::AMBER } else { c::INK_MUTED },
            );
            text_fit(
                d,
                l + 34,
                y + 16,
                width / 2 - 40,
                &e.name,
                if selected { c::AMBER } else { c::INK },
            );
            text_fit(
                d,
                l + width / 2,
                y + 16,
                width / 2 - 96,
                &format!("{}", e.stored_len()),
                c::INK_MUTED,
            );
            text_fit(
                d,
                r - 85,
                y + 16,
                79,
                if self.doc.entry_changed(e) {
                    "EDITED"
                } else {
                    "SAVED"
                },
                if self.doc.entry_changed(e) {
                    c::AMBER
                } else {
                    c::INK_FAINT
                },
            );
            o.hit([l + 1, y, width - 2, 24], Action::Entry(*i));
        }
    }
    fn inspector(&self, o: &mut Layout) {
        let r = self.right();
        let w = self.width - r;
        let h = self.height;
        let d = &mut o.canvas;
        d.rect(r + 1, 26, w - 1, 28, c::GM_800);
        icon(
            d,
            r + 10,
            32,
            GROUPS[category_of(self.name())].2,
            c::INK_MUTED,
        );
        text_fit(d, r + 34, 44, w - 84, self.name(), c::INK);
        badge(
            d,
            self.width - 38,
            32,
            if self.doc.archive.entries.is_empty() {
                "--"
            } else {
                extension(self.name())
            },
        );
        if self.mode == Mode::Browse {
            self.entry_inspector(o);
            return;
        }
        let shape = self
            .model_entry
            .map(|i| self.doc.archive.entries[i].name.as_str())
            .or_else(|| {
                self.external_model.and_then(|(id, i)| {
                    self.libraries
                        .iter()
                        .find(|l| l.id == id)
                        .and_then(|l| l.doc.archive.entries.get(i))
                        .map(|e| e.name.as_str())
                })
            })
            .unwrap_or("No linked shape");
        o.canvas.label(r + 12, 78, "Shape", c::INK_MUTED);
        o.canvas.rect(r + 60, 62, w - 96, 22, c::GM_700);
        text_fit(&mut o.canvas, r + 82, 77, w - 118, shape, c::INK);
        icon(&mut o.canvas, r + 63, 65, Icon::Shape, c::INK_MUTED);
        if let Some(i) = self.model_entry {
            o.hit([r + 60, 62, w - 96, 22], Action::Entry(i));
        } else if let Some((id, i)) = self.external_model {
            o.hit([r + 60, 62, w - 96, 22], Action::LibraryEntry(id, i));
        }
        o.tool(
            [self.width - 30, 62, 24, 22],
            Icon::Link,
            Action::Mode(Mode::Model),
            self.model.is_some(),
            false,
        );
        let base = self
            .dominant_color()
            .map_or("Base color: textured / Materials".into(), |c| {
                format!("Base color / palette {c}")
            });
        o.button(
            [r + 12, 89, w - 24, 22],
            &base,
            Action::BaseColor(false),
            false,
        );
        let rows = self.property_rows();
        if !rows.is_empty() {
            let mut y = 116 - self.inspector_scroll * 24;
            let mut group = "";
            for (section, label, i) in rows {
                if group != section {
                    group = section;
                    y += 8;
                    if y >= 110 && y + 25 < h - 50 {
                        o.canvas.rect(r + 8, y, w - 16, 25, c::GM_900);
                        border(&mut o.canvas, r + 8, y, w - 16, 25, c::GM_1000);
                        icon(&mut o.canvas, r + 14, y + 5, Icon::Flight, c::INK_MUTED);
                        o.canvas.label(r + 33, y + 17, section, c::INK);
                    }
                    y += 29;
                }
                if y >= 110 && y + 22 < h - 50 {
                    let f = &self.brf.as_ref().unwrap().fields[i];
                    let d = &mut o.canvas;
                    label_fit(d, r + 12, y + 15, 118, label, c::INK_MUTED);
                    let fx = r + 132;
                    d.rect(fx, y, w - 146, 21, c::GM_950);
                    border(d, fx, y, w - 146, 21, c::LINE_STRONG);
                    text_fit(
                        d,
                        fx + 7,
                        y + 15,
                        w - 176,
                        &format!("{}{}", if f.scaled { "^" } else { "" }, f.value),
                        if self.field_changed(i) {
                            c::AMBER
                        } else {
                            c::INK
                        },
                    );
                    o.hit([fx, y, w - 146, 21], Action::Field(i));
                }
                y += 25;
            }
            o.button(
                [r + 10, h - 46, w - 20, 21],
                "All fields / Flight",
                Action::Mode(Mode::Properties),
                false,
            );
        } else {
            let d = &mut o.canvas;
            d.rect(r + 8, 118, w - 16, 140, c::GM_900);
            border(d, r + 8, 118, w - 16, 140, c::GM_1000);
            d.label(r + 20, 140, "Geometry", c::INK);
            if let Some(m) = &self.model {
                d.label(r + 20, 169, "Vertices", c::INK_MUTED);
                d.text(r + 132, 169, &format!("{}", m.vertices.len()), c::INK);
                d.label(r + 20, 195, "Faces", c::INK_MUTED);
                d.text(r + 132, 195, &format!("{}", m.faces.len()), c::INK);
                label_fit(
                    d,
                    r + 20,
                    229,
                    w - 40,
                    if m.writable {
                        "Static records editable"
                    } else {
                        "Animated records: preview only"
                    },
                    c::STEEL,
                );
            } else {
                label_fit(d, r + 20, 174, w - 40, "No decoded geometry", c::INK_MUTED);
            }
            o.button(
                [r + 10, 276, w - 20, 23],
                "Export entry",
                Action::File(FileAction::Export),
                false,
            );
            o.button(
                [r + 10, 309, w - 20, 23],
                "Replace entry",
                Action::File(FileAction::Replace),
                false,
            );
            if self.model.is_some() {
                o.button(
                    [r + 10, 342, w - 20, 23],
                    "Export geometry as OBJ",
                    Action::File(FileAction::Obj),
                    false,
                );
            }
            if let Some(m) = &self.model {
                let mut y = 390;
                for name in m.textures.iter().take(6) {
                    let n = if name.contains('.') {
                        name.clone()
                    } else {
                        format!("{name}.PIC")
                    };
                    if let Some(i) = self.doc.archive.find(&n) {
                        o.button(
                            [r + 10, y, w - 20, 23],
                            &format!("Paint {n}"),
                            Action::OpenTexture(i),
                            false,
                        );
                    } else {
                        label_fit(
                            &mut o.canvas,
                            r + 16,
                            y + 15,
                            w - 32,
                            &format!("Missing: {n}"),
                            c::AMBER,
                        );
                    }
                    y += 28;
                }
            }
            if h > 680 {
                o.canvas.label(r + 16, 580, "VIEWPORT", c::INK_FAINT);
                o.canvas
                    .label(r + 16, 606, "MMB orbit / Shift+MMB pan", c::INK_MUTED);
                o.canvas
                    .label(r + 16, 630, "Wheel zoom / Home frame", c::INK_MUTED);
            }
        }
    }
    fn entry_inspector(&self, o: &mut Layout) {
        let r = self.right();
        let w = self.width - r;
        let Some(e) = self.doc.archive.entries.get(self.selected) else {
            return;
        };
        let d = &mut o.canvas;
        d.rect(r + 8, 62, w - 16, 151, c::GM_900);
        border(d, r + 8, 62, w - 16, 151, c::GM_1000);
        d.label(r + 20, 84, "Lib entry", c::INK);
        for (row, (label, value)) in [
            ("Type", GROUPS[category_of(&e.name)].0.to_string()),
            ("Stored size", format!("{} B", e.stored_len())),
            (
                "Source offset",
                if self.doc.entry_changed(e) {
                    "Repacked".into()
                } else {
                    format!("0x{:08X}", e.source_offset())
                },
            ),
            ("Compression", format!("flag {}", e.flag())),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 110 + row as i32 * 24;
            d.label(r + 20, y, label, c::INK_MUTED);
            text_fit(d, r + 134, y, w - 152, &value, c::INK);
        }
        let links = self.dependencies.get(&e.name).map_or(0, |s| s.links.len());
        let users = self.dependencies.incoming(&e.name).count();
        d.label(r + 20, 244, "Resource relationships", c::INK);
        d.label(
            r + 20,
            272,
            &format!("{links} references / {users} direct users"),
            c::INK_MUTED,
        );
        d.label(
            r + 20,
            298,
            &format!("{} aircraft users in this LIB", self.aircraft_users.len()),
            c::STEEL,
        );
        label_fit(
            d,
            r + 20,
            324,
            w - 40,
            "Stored references / current LIB only",
            c::INK_FAINT,
        );
        for (y, title, action) in [
            (344, "References / users", Action::Dock(4)),
            (380, "Open in Model", Action::Mode(Mode::Model)),
            (416, "Export entry", Action::File(FileAction::Export)),
            (452, "Replace entry", Action::File(FileAction::Replace)),
        ] {
            o.button([r + 10, y, w - 20, 24], title, action, false);
        }
    }
    fn fields_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32, full: bool) {
        if full
            && self.field_group == Some(hangar_core::definition::Aspect::Envelope)
            && !self.envelope_rows().is_empty()
        {
            self.envelope_layout(o, x, y, w, h);
            return;
        }
        o.canvas.rect(x, y, w, h, c::GM_800);
        let mut top = y;
        if full {
            o.canvas
                .label(x + 12, y + 19, "Definition fields / stored values", c::INK);
            top += 28;
        }
        let wide = w > 570;
        let typed = w > 440;
        let vx = x + if wide { w * 59 / 100 } else { w / 2 };
        let tx = x + w * 44 / 100;
        let ox = x + w * 80 / 100;
        let valuew = if wide { ox - vx - 12 } else { w / 2 - 36 };
        o.canvas.rect(x, top, w, 24, c::GM_900);
        o.canvas.label(x + 10, top + 16, "FIELD", c::INK_MUTED);
        if typed {
            o.canvas.label(tx, top + 16, "TYPE", c::INK_MUTED);
        }
        o.canvas.label(vx, top + 16, "VALUE", c::INK_MUTED);
        if wide {
            o.canvas.label(ox, top + 16, "ON DISK", c::INK_MUTED);
        }
        let Some(b) = &self.brf else {
            label_fit(
                &mut o.canvas,
                x + 12,
                top + 50,
                w - 24,
                "Select a PT / JT / NT / OT definition to edit.",
                c::INK_FAINT,
            );
            return;
        };
        let rowh = if full { 26 } else { 22 };
        let start = top + 24;
        for (row, (i, f)) in b
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                !full
                    || self.field_group.is_none_or(|group| {
                        hangar_core::definition::aspect(&f.label) == Some(group)
                    })
            })
            .skip(self.field_scroll)
            .take(((y + h - start) / rowh).max(0) as usize)
            .enumerate()
        {
            let yy = start + row as i32 * rowh;
            let d = &mut o.canvas;
            d.rect(
                x,
                yy,
                w,
                rowh,
                if i == self.field_selected {
                    c::AMBER_DEEP
                } else if row % 2 == 0 {
                    c::GM_800
                } else {
                    c::GM_900
                },
            );
            text_fit(
                d,
                x + 10,
                yy + 16,
                if typed { tx - x - 20 } else { w / 2 - 20 },
                &f.label,
                c::INK_MUTED,
            );
            if typed {
                text_fit(d, tx, yy + 16, vx - tx - 10, &f.kind, c::INK_FAINT);
            }
            text_fit(
                d,
                vx,
                yy + 16,
                valuew,
                &format!("{}{}", if f.scaled { "^" } else { "" }, f.value),
                if self.field_changed(i) {
                    c::AMBER
                } else {
                    c::INK
                },
            );
            if wide {
                let old = self
                    .original_brf
                    .as_ref()
                    .and_then(|b| b.fields.get(i))
                    .map_or_else(
                        || "new".into(),
                        |f| format!("{}{}", if f.scaled { "^" } else { "" }, f.value),
                    );
                text_fit(d, ox, yy + 16, x + w - ox - 32, &old, c::INK_FAINT);
            }
            o.hit([x, yy, vx - x, rowh], Action::PickField(i));
            o.hit([vx, yy, valuew, rowh], Action::Field(i));
            if self.field_changed(i) && self.original_brf.is_some() {
                icon(&mut o.canvas, x + w - 24, yy + 4, Icon::Rotate, c::STEEL);
                o.hit([x + w - 29, yy, 29, rowh], Action::ResetField(i));
            }
        }
    }
    fn dock_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        o.canvas.rect(x, y, w, h, c::GM_800);
        o.canvas.line(x, y, x + w, y, c::GM_1000);
        let compact = w < 450;
        let mut tx = x + 4;
        for (id, title, width) in if compact {
            [
                (0, "Raw", 48),
                (1, "Hex", 40),
                (2, "Details", 60),
                (4, "Links", 62),
                (3, "3D preview", 86),
            ]
        } else {
            [
                (0, "Raw fields", 90),
                (1, "Hex", 52),
                (2, "Details", 72),
                (4, "References", 94),
                (3, "3D preview", 86),
            ]
        } {
            if id == 3 && !(self.mode == Mode::Media && self.context_model.is_some()) {
                continue;
            }
            o.button(
                [tx, y + 4, width, 22],
                title,
                Action::Dock(id),
                self.dock == id,
            );
            tx += width + 3;
        }
        if w > tx - x + 140 {
            text_fit(
                &mut o.canvas,
                tx + 8,
                y + 20,
                x + w - tx - 16,
                self.name(),
                c::INK_MUTED,
            );
        }
        if self.dock == 4 {
            self.references_layout(o, x, y + 30, w, h - 30);
            return;
        }
        if self.dock == 3 && self.context_model.is_some() {
            self.draw_model(o, x, y + 30, w, h - 30);
            label_fit(
                &mut o.canvas,
                x + 12,
                y + 49,
                w - 24,
                "Live model / MMB orbit / wheel zoom",
                c::INK_FAINT,
            );
        } else if self.dock == 0 && self.brf.is_some() {
            self.fields_layout(o, x, y + 30, w, h - 30, false);
        } else if self.dock == 1 || (self.dock == 0 && self.brf.is_none()) {
            o.canvas.rect(x, y + 30, w, h - 30, c::GM_950);
            let count = ((w - 88) / 21).clamp(4, 16) as usize;
            for (row, bytes) in self
                .data
                .chunks(count)
                .take(((h - 43) / 20).max(0) as usize)
                .enumerate()
            {
                let yy = y + 49 + row as i32 * 20;
                o.canvas
                    .text(x + 12, yy, &format!("{:06X}", row * count), c::INK_FAINT);
                let mut value = String::new();
                for b in bytes {
                    value.push_str(&format!("{b:02X} "));
                }
                o.canvas.text(x + 76, yy, &value, c::INK_MUTED);
            }
        } else {
            label_fit(
                &mut o.canvas,
                x + 14,
                y + 56,
                w - 28,
                &self.detail,
                c::STEEL,
            );
            text_fit(
                &mut o.canvas,
                x + 14,
                y + 82,
                w - 28,
                &self.status,
                if self.status.starts_with("Error:") {
                    c::DANGER
                } else {
                    c::INK_MUTED
                },
            );
            text_fit(
                &mut o.canvas,
                x + 14,
                y + 109,
                w - 28,
                &self.path,
                c::INK_FAINT,
            );
        }
    }
    fn menu_layout(&self, o: &mut Layout, menu: usize) {
        let xs = [86, 124, 163, 196, 243, 286, 333];
        let x = xs[menu];
        let items: Vec<(&str, Action)> = match menu {
            0 => vec![
                ("Open LIB        Ctrl+O", Action::File(FileAction::Open)),
                ("New empty LIB", Action::NewLibrary),
                ("Close active LIB", Action::CloseLibrary),
                ("Package LIB     Ctrl+S", Action::File(FileAction::Save)),
                ("Load synthetic demo", Action::Demo),
                ("Close", Action::Close),
            ],
            1 => vec![
                ("Undo            Ctrl+Z", Action::Undo),
                ("Redo      Ctrl+Shift+Z", Action::Redo),
            ],
            2 => vec![
                ("Add entry       Ctrl+I", Action::File(FileAction::Import)),
                (
                    "Export object + resources",
                    Action::File(FileAction::Variant),
                ),
                ("From loose SH file...", Action::File(FileAction::VariantSh)),
                ("Validate package", Action::Validate),
            ],
            3 => vec![
                ("Copy resource   Ctrl+C", Action::CopyResource),
                ("Paste resources Ctrl+V", Action::PasteResources),
                ("Rename resource", Action::RenameResource(false)),
                ("Duplicate resource", Action::RenameResource(true)),
                ("References / users", Action::Dock(4)),
                ("Export entry    Ctrl+E", Action::File(FileAction::Export)),
                ("Replace entry", Action::File(FileAction::Replace)),
                ("Export geometry as OBJ", Action::File(FileAction::Obj)),
                ("Use as graft donor", Action::PinDonor),
                ("Graft characteristics", Action::Mode(Mode::Graft)),
                ("Preview / Paint media", Action::Mode(Mode::Media)),
                ("Hardpoint tools", Action::Hardpoints),
                ("Animation / parts", Action::Animation),
                ("Repair generated panel mappings", Action::RepairPanels),
                ("Decals / markings", Action::MediaTab(2)),
                ("Base color", Action::BaseColor(false)),
                ("Panel color", Action::BaseColor(true)),
                ("Remap color indices", Action::Recolor),
            ],
            4 => vec![
                ("Frame all        Home", Action::View(0)),
                ("Front               1", Action::View(1)),
                ("Side                3", Action::View(3)),
                ("Top                 7", Action::View(7)),
                ("Toggle projection   5", Action::View(5)),
                ("Textured / wireframe", Action::Textured),
            ],
            5 => vec![
                (
                    "Export object + resources",
                    Action::File(FileAction::Variant),
                ),
                ("From loose SH file...", Action::File(FileAction::VariantSh)),
                ("Graft characteristics", Action::Mode(Mode::Graft)),
                ("Copy one donor field", Action::File(FileAction::Graft)),
            ],
            _ => vec![
                ("Controls           F1", Action::Help),
                ("Load synthetic demo", Action::Demo),
            ],
        };
        o.canvas
            .rect(x + 3, 29, 222, items.len() as i32 * 26 + 8, c::GM_1000);
        o.canvas
            .rect(x, 26, 222, items.len() as i32 * 26 + 8, c::GM_700);
        border(
            &mut o.canvas,
            x,
            26,
            222,
            items.len() as i32 * 26 + 8,
            c::LINE_STRONG,
        );
        for (i, (label, a)) in items.into_iter().enumerate() {
            let y = 30 + i as i32 * 26;
            let hover = self.mouse[0] >= x
                && self.mouse[0] < x + 222
                && self.mouse[1] >= y
                && self.mouse[1] < y + 26;
            if hover {
                o.canvas.rect(x + 2, y, 218, 26, c::AMBER_DEEP);
            }
            o.canvas
                .text(x + 10, y + 17, label, if hover { c::AMBER } else { c::INK });
            o.hit([x + 2, y, 218, 26], a);
        }
    }
    fn prompt_layout(&self, o: &mut Layout) {
        let p = self.prompt.as_ref().unwrap();
        let w = (self.width - 48).min(710);
        let x = (self.width - w) / 2;
        let y = self.height / 2 - 112;
        o.hits.clear();
        let d = &mut o.canvas;
        d.rect(x + 5, y + 6, w, 224, c::GM_1000);
        d.rect(x, y, w, 224, c::GM_800);
        border(d, x, y, w, 224, c::LINE_STRONG);
        d.rect(x + 1, y + 1, w - 2, 34, c::GM_700);
        icon(d, x + 12, y + 9, Icon::Lib, c::STEEL);
        label_fit(d, x + 38, y + 22, w - 54, &p.title, c::INK);
        if matches!(p.kind, PromptKind::Discard | PromptKind::CloseLibrary) {
            label_fit(
                d,
                x + 18,
                y + 70,
                w - 36,
                "Unsaved edits will be discarded. Saved files are unchanged.",
                c::INK_MUTED,
            );
            label_fit(
                d,
                x + 18,
                y + 102,
                w - 36,
                "Cancel returns to the editor.",
                c::INK_MUTED,
            );
            o.button(
                [x + w - 300, y + 181, 110, 26],
                "Cancel",
                Action::Cancel,
                true,
            );
            o.button(
                [x + w - 176, y + 181, 158, 26],
                "Discard changes",
                Action::DiscardChanges,
                false,
            );
            return;
        }
        d.label(
            x + 18,
            y + 60,
            if matches!(p.kind, PromptKind::File(_)) {
                "File path"
            } else {
                "Value"
            },
            c::INK_MUTED,
        );
        d.rect(x + 16, y + 74, w - 32, 31, c::GM_950);
        border(d, x + 16, y + 74, w - 32, 31, c::FOCUS);
        let limit = ((w - 54) / 7) as usize;
        let value: String = p
            .value
            .chars()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        d.text(x + 26, y + 94, &format!("{value}_"), c::INK);
        let hint = if let PromptKind::Transform(op) = p.kind {
            let lock = match (p.axis, op) {
                (0..=2, _) => format!("Axis {}", ['X', 'Y', 'Z'][p.axis]),
                (_, 'g') => "Free: one value moves X, or X Y Z".into(),
                (_, 'r') => format!("View axis {}", ['X', 'Y', 'Z'][self.view_axis()]),
                _ => "Uniform".into(),
            };
            format!("{lock} / X Y Z toggles the axis lock / integer value")
        } else if matches!(p.kind, PromptKind::StationMove) {
            format!(
                "Axis {} / X Y Z to constrain / integer value",
                ['X', 'Y', 'Z'][p.axis.min(2)]
            )
        } else {
            "Enter applies / Esc cancels / Ctrl+A clears".into()
        };
        label_fit(d, x + 18, y + 129, w - 36, &hint, c::INK_MUTED);
        if self.status.starts_with("Error:") {
            icon(d, x + 18, y + 143, Icon::Warn, c::DANGER);
            label_fit(d, x + 42, y + 157, w - 60, &self.status, c::DANGER);
        }
        if matches!(p.kind, PromptKind::CloneTitle) {
            o.button(
                [x + 16, y + 181, 138, 26],
                "Add source LIB",
                Action::File(FileAction::CloneSource),
                false,
            );
        }
        if matches!(p.kind, PromptKind::CloneTitle) {
            o.button([x + 167, y + 181, 78, 26], "Back", Action::CloneBack, false);
        }
        o.button(
            [x + w - 204, y + 181, 86, 26],
            "Cancel",
            Action::Cancel,
            false,
        );
        o.button(
            [x + w - 107, y + 181, 89, 26],
            if matches!(p.kind, PromptKind::File(FileAction::Open)) {
                "Open LIB"
            } else {
                "Apply"
            },
            Action::Apply,
            true,
        );
    }
}

impl App {
    pub fn smoke_layout(&mut self) {
        fn click(app: &mut App, predicate: impl Fn(Action) -> bool) {
            let hit = app
                .layout()
                .hits
                .into_iter()
                .find(|h| predicate(h.action))
                .expect("Visible control missing");
            app.click(
                hit.rect[0] + hit.rect[2] / 2,
                hit.rect[1] + hit.rect[3] / 2,
                1,
                true,
            );
        }
        self.smoke_dependencies();
        self.smoke_graft();
        self.smoke_libraries();
        self.smoke_library_moves();
        self.smoke_material_tools();
        self.smoke_advanced_tools();
        self.smoke_render_and_brush();
        self.smoke_object_tools();
        self.demo();
        self.width = 1280;
        self.height = 800;
        self.mode = Mode::Model;
        self.scroll = 0;
        click(self, |a| matches!(a, Action::Category(0)));
        click(self, |a| matches!(a, Action::Entry(1)));
        assert_eq!(self.selected, 1);
        assert_eq!(self.model_entry, Some(0));
        let weight = self
            .brf
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .position(|f| f.label == "object.weight")
            .unwrap();
        click(self, |a| matches!(a,Action::Field(i) if i==weight));
        self.key(Key::Char('a'), true, false);
        for c in "12000".chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        assert!(self.field_changed(weight));
        assert_eq!(self.doc.changed_count(), 1);
        assert!(self
            .draw()
            .commands
            .iter()
            .any(|d| matches!(d,Draw::Text(_,_,s,color) if s=="12000"&&*color==c::AMBER.0)));
        click(self, |a| matches!(a, Action::Menu(1)));
        click(self, |a| matches!(a, Action::Undo));
        assert!(!self.doc.dirty());
        click(self, |a| matches!(a, Action::Mode(Mode::Browse)));
        assert!(self.browser_entries().contains(&self.selected));
        click(self, |a| matches!(a, Action::Mode(Mode::Package)));
        click(self, |a| matches!(a, Action::Validate));
        assert!(self.validation.as_ref().unwrap().errors == 0);
        self.file_prompt(FileAction::Open);
        click(self, |a| matches!(a, Action::Cancel));
        assert!(self.prompt.is_none());
        self.width = 800;
        self.height = 600;
        for mode in [
            Mode::Browse,
            Mode::Model,
            Mode::Properties,
            Mode::Graft,
            Mode::Package,
        ] {
            self.mode = mode;
            for hit in self.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= self.width
                        && hit.rect[1] + hit.rect[3] <= self.height,
                    "Control outside minimum window"
                );
            }
        }
    }
}
