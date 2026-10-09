//! Shared layout and hit regions: the visible controls and clickable areas stay together.
use super::*;

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
    About,
    Filter,
    /// Outliner type filter: aircraft (0), shapes (1), images (2); again clears.
    TypeFilter(usize),
    Category(usize),
    Entry(usize),
    Related(usize),
    Field(usize),
    PickField(usize),
    /// The dialog value box: a click places the caret.
    PromptText,
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
    /// Context menu: open the Copy to (1) or Move to (2) submenu.
    ContextSub(u8),
    /// Review a copy (false) or move (true) of the selection into LIB `id`.
    TransferTo(u64, bool),
    DeleteEntry,
    LibraryEntry(u64, usize),
    NewLibrary,
    CloseLibrary,
    DiscardChanges,
    /// Save anyway past FA's loader limits (the GameFolder prompt).
    ConfirmSave,
    Animation,
    /// Edit Mesh: vertex (false) or face (true) select mode.
    SelectMode(bool),
    /// Edit Mesh operation `edit_ui::OP_*`.
    MeshOp(u8),
    /// R/S pivot: median (false) or individual origins (true).
    Pivot(bool),
    /// Edit Mesh transform gizmo handle (drag in `App::pointer`).
    Gizmo(gizmo_ui::Handle),
    /// Gizmo mode `gizmo_ui::G_*`: none, move, rotate, scale.
    GizmoMode(u8),
    /// Snap to vertices (magnet) toggle.
    Magnet,
    /// X-ray: pick and box vertices behind faces (Alt+Z).
    Xray,
    /// Parts list row (index into `App::parts`, then static parts).
    PartPick(usize),
    /// Pose preset `animation_ui::PRESETS` index.
    PosePreset(u8),
    PoseReset,
    /// Pose toggle: variable index and value.
    PoseSet(u8, i32),
    /// Open the options Select of the selected part's control.
    PartMenu(usize),
    /// Apply option `value` to the selected part's control.
    PartValue(usize, i32),
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
    /// Export review, Unresolved in source row: keep the stored name.
    CloneKeep(usize),
    /// Open or close the texture picker for a row.
    ClonePick(usize),
    /// Picker item: retarget the open row to this package PIC.
    CloneTexture(usize),
    /// Picker item: type the name of another source PIC.
    ClonePickOther,
    /// Acknowledge exporting with kept unresolved names.
    CloneAck,
    /// Identity panel: edit the short (false) or long (true) name.
    IdentityEdit(bool),
    /// Identity panel: restore the saved short or long name.
    IdentityReset(bool),
    /// Rename the selected aircraft's reference ID (review first).
    RenameAircraft,
    /// Duplicate the selected aircraft in this LIB (wizard, then review).
    DuplicateAircraft,
    /// Duplicate review: copy (false) or share (true) toggle row.
    DuplicateToggle(usize, bool),
    /// Duplicate review: copy (true) or skip the `<ID>.PAL` companion.
    DuplicatePalette(bool),
    /// Rename or duplicate review: back to the previous step.
    IdentityBack,
    BrowserUp,
    BrowserRoots,
    BrowserPick(usize),
    Recent(usize),
    PlayAudio,
    StopAudio,
    PaintToggle,
    Eraser,
    /// The Replace paint tool beside Brush and Eraser.
    ReplaceTool,
    /// Open the Replace color dialog.
    ReplaceDialog,
    /// Replace color dialog: scope `replace_ui::SCOPE_*`.
    ReplaceScope(u8),
    /// Replace color dialog: the From (0) or To (1) swatch takes the grid.
    ReplaceSlot(u8),
    /// Replace color dialog: palette cell.
    ReplaceSwatch(u8),
    /// Replace color dialog: pick the active swatch from the image.
    ReplacePickImage,
    PickColor,
    RestoreTexture,
    RemoveOriginals,
    /// Package: rewrite crashing SH textures in the retail layout.
    RepairTextures,
    Brush(u8),
    Radius(usize),
    OpenTexture(usize),
    Recolor,
    BaseColor(bool),
    PanelTexture,
    RepairPanels,
    MeshMode,
    SelectTool,
    MenuPad,
    MeshVertex(usize),
    MeshAll,
    MeshMove,
    ModelPaint,
    PaintLock,
    Isolate,
    /// Per-face texture action `texture_ui::TEX_*`.
    FaceTexture(u8),
    /// Runtime markings row action `markings_ui::MK_*` on a slot.
    Marking(u16, u8),
    /// Open a Runtime markings row's slot Select.
    MarkingMenu(u16),
    /// Slot Select item: reassign the row's slot (from, to).
    MarkingSlot(u16, u16),
    /// Open a Runtime markings row's Fill Select.
    MarkingFillMenu(u16),
    /// Fill Select item (slot, `markings_ui::FILL_*`).
    MarkingFill(u16, u8),
    /// Fill palette grid: pick index (slot, index).
    MarkingFillColor(u16, u8),
    /// Assign texture dialog: pick PIC `i` (index into the dialog's list).
    AssignPick(usize),
    /// Assign texture dialog: UV mode Keep (0), Scale (1), Project (2).
    AssignMode(u8),
    /// Assign texture dialog: projection plane Auto, Top, Side, Front.
    AssignPlane(u8),
    /// Remap from view dialog: Bake current look (0) or Blank (1).
    RemapFill(u8),
    /// Click on a model preview drawn in `rect` (Paint workspace): pick a
    /// panel, Shift adds or removes (handled in `App::pointer`).
    PanelPick([i32; 4]),
    Hardpoints,
    StationSlew(bool),
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
    DecalMirror,
    DecalPlace,
    DecalCancel,
    DecalApply,
    /// Viewport mode Select: object, edit mesh, hardpoints, parts, paint.
    ViewportMode(u8),
    /// Shading: wireframe, solid, textured.
    Shading(u8),
    /// Press on a NumberField: starts a scrub (handled in `App::pointer`).
    Number(widgets::NumberTarget),
    /// NumberField hover arrow: step down (-1) or up (+1).
    NumberStep(widgets::NumberTarget, i32),
    /// Panel header: collapse or expand; Ctrl+click collapses every other.
    Panel(u8),
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
    /// Pointer for hover; `i32::MIN` while a menu or dialog covers the editors.
    pub(super) mouse: [i32; 2],
    /// Left button held: hovered buttons draw pressed.
    pub(super) pressed: bool,
    /// Window size, for clamping menus.
    pub(super) size: [i32; 2],
    /// The NumberField being scrubbed and its live value.
    pub(super) scrub: Option<(widgets::NumberTarget, i64)>,
    /// Scroll range of the right-hand editor's panel stack, in px.
    pub(super) inspector_max: i32,
    /// The media inspector's model preview: the wheel zooms it.
    pub(super) zoom_rect: Option<[i32; 4]>,
    pub canvas: Canvas,
    pub hits: Vec<Hit>,
}
pub(super) use super::Glyph as Icon;
const GROUPS: [(&str, &str, Icon); 10] = [
    ("Aircraft", "PT", Icon::Aircraft),
    ("Shapes", "SH", Icon::Shape),
    ("Images", "PIC", Icon::Image),
    ("Weapons", "JT", Icon::Weapon),
    ("Ground objects", "OT", Icon::Object),
    ("Palettes", "PAL", Icon::Palette),
    ("Missions", "M", Icon::Mission),
    ("Sounds", "11K", Icon::Sound),
    ("Other resources", "", Icon::Lib),
    ("Original textures", "ORG", Icon::Image),
];
/// Outliner group order: stored originals sit right below their images.
pub(super) const GROUP_ORDER: [usize; 10] = [0, 1, 2, 9, 3, 4, 5, 6, 7, 8];
/// Outliner footer (Open LIB, Export object) height.
pub(super) const OUTLINER_FOOTER: i32 = theme::metric::EDITOR_HEADER_H;
/// Dialog header height.
pub(super) const DIALOG_HEAD: i32 = 36;
/// Browse table column header height.
const TABLE_HEAD_H: i32 = theme::metric::EDITOR_HEADER_H;
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
        "ORG" => 9,
        _ => 8,
    }
}
/// The outliner group icon for an entry name.
pub(super) fn group_icon(name: &str) -> Icon {
    GROUPS[category_of(name)].2
}
pub(super) fn border(d: &mut Canvas, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
    d.line(x, y, x + w - 1, y, color);
    d.line(x, y + h - 1, x + w - 1, y + h - 1, color);
    d.line(x, y, x, y + h - 1, color);
    d.line(x + w - 1, y, x + w - 1, y + h - 1, color);
}
/// "1 reference", "2 references".
pub(super) fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
pub(super) fn text_fit(d: &mut Canvas, x: i32, y: i32, w: i32, s: &str, color: Rgb) {
    d.text(x, y, &fit(s, w, Style::Value), color);
}
pub(super) fn label_fit(d: &mut Canvas, x: i32, y: i32, w: i32, s: &str, color: Rgb) {
    d.label(x, y, &fit(s, w, Style::Label), color);
}
impl Layout {
    pub(super) fn hit(&mut self, rect: [i32; 4], action: Action) {
        self.hits.push(Hit { rect, action });
    }
}
impl App {
    pub(super) fn act(&mut self, a: Action) {
        if !matches!(a, Action::Menu(_) | Action::MenuPad | Action::ContextSub(_)) {
            self.menu = None;
        }
        if !matches!(a, Action::Filter) {
            self.filter_focus = false;
        }
        if !matches!(
            a,
            Action::ClonePick(_)
                | Action::CloneTexture(_)
                | Action::ClonePickOther
                | Action::MenuPad
        ) {
            self.clone_unresolved.pick = None;
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
                    folder: "Drives and locations".into(),
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
            // Brush and Eraser share one stroke path; Model mode keeps its paint toggle.
            Action::PaintToggle => {
                self.finish_stroke();
                if self.mode == Mode::Model {
                    if !self.model_paint {
                        self.act(Action::ModelPaint);
                    }
                } else {
                    self.paint_enabled = !self.paint_enabled || self.eraser || self.replace.on;
                }
                self.eraser = false;
                self.replace.on = false;
                self.pick_color = false;
            }
            Action::Eraser => {
                self.finish_stroke();
                if self.mode == Mode::Model {
                    let on = !self.eraser;
                    if on && !self.model_paint {
                        self.act(Action::ModelPaint);
                    }
                    self.eraser = on;
                } else {
                    self.paint_enabled = !self.paint_enabled || !self.eraser;
                    self.eraser = self.paint_enabled;
                }
                self.replace.on = false;
                self.pick_color = false;
            }
            Action::ReplaceTool => self.replace_tool(),
            Action::ReplaceDialog => self.open_replace_dialog(),
            Action::ReplaceScope(k) => self.replace_scope(k),
            Action::ReplaceSlot(k) => self.replace_slot(k),
            Action::ReplaceSwatch(i) => self.replace_swatch(i),
            Action::ReplacePickImage => self.replace_pick_image(),
            Action::RestoreTexture => {
                let result = self.restore_texture();
                self.result(result);
            }
            Action::RepairTextures => {
                let result = self.repair_fa_textures();
                self.result(result);
            }
            Action::RemoveOriginals => {
                let result = self.remove_originals();
                self.result(result);
            }
            Action::PickColor => {
                self.pick_color = !self.pick_color;
                self.paint_enabled = false;
            }
            Action::Brush(i) => self.brush = i,
            Action::Radius(n) => self.brush_radius = n,
            Action::OpenTexture(i) => self.open_texture(i),
            Action::PaintLock => {
                self.finish_stroke();
                self.paint_lock = !self.paint_lock;
            }
            Action::ModelPaint => {
                self.finish_stroke();
                self.animation_tool = false;
                self.preview = None;
                self.mesh_edit = false;
                self.hp_tool = false;
                self.media_tab = 0;
                self.model_paint = !self.model_paint;
                // The paint toggle always starts with the brush, not the eraser.
                self.eraser = false;
                self.replace.on = false;
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
                    caret: super::Caret::END,
                });
            }
            Action::FaceTexture(op) => self.face_texture_action(op),
            Action::Marking(slot, op) => self.marking_action(slot, op),
            Action::MarkingMenu(slot) => self.open_slot_menu(slot),
            Action::MarkingSlot(from, to) => self.marking_slot(from, to),
            Action::MarkingFillMenu(slot) => self.open_fill_menu(slot),
            Action::MarkingFill(slot, kind) => self.marking_fill(slot, kind),
            Action::MarkingFillColor(slot, i) => self.marking_fill_color(slot, i),
            Action::AssignPick(i) => self.assign_pick(i),
            Action::AssignMode(mode) => self.assign_mode(mode),
            Action::AssignPlane(plane) => self.ed.assign.plane = plane.min(3),
            Action::RemapFill(fill) => self.remap_fill(fill),
            Action::PanelPick(rect) => {
                let [x, y] = self.mouse;
                self.preview_click(rect, x, y, false);
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
                // One face selection: panels picked outside Edit Mesh enter
                // it in face select, and Edit Mesh's faces stay selected.
                if self.mesh_edit && !self.ed.face_select {
                    let faces = self.selected_faces();
                    self.ed.mesh_faces = self
                        .model
                        .as_ref()
                        .map(|m| faces.iter().map(|f| m.faces[*f].offset).collect())
                        .unwrap_or_default();
                } else if !self.mesh_edit && !self.panel_offsets().is_empty() {
                    self.ed.face_select = true;
                }
                self.mesh_edit = !self.mesh_edit;
                if self.mesh_edit && self.ed.face_select {
                    self.sync_face_vertices();
                }
                self.ed.mesh_pending = None;
                self.ed.mesh_box = None;
                self.ed.box_armed = false;
                self.mesh_drag = None;
                self.gizmo.drag = None;
                self.gizmo.snap = None;
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
            // Clicks on menu padding are consumed so they never reach controls beneath.
            Action::MenuPad => {}
            Action::SelectTool => {
                self.finish_stroke();
                self.model_paint = false;
                self.paint_enabled = false;
                self.pick_color = false;
                self.status = "Select tool: click a face to select it".into();
            }
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
                    caret: super::Caret::END,
                });
            }
            Action::CloneKeep(row) => {
                let r = self.clone_choose(row, None);
                self.result(r);
            }
            Action::ClonePick(row) => {
                self.clone_unresolved.pick =
                    (self.clone_unresolved.pick != Some(row)).then_some(row);
            }
            Action::CloneTexture(i) => {
                if let Some(row) = self.clone_unresolved.pick {
                    let pick = self.clone_picks(row).into_iter().nth(i);
                    let r = match pick {
                        Some(name) => self.clone_choose(row, Some(name)),
                        None => Err("No such texture".into()),
                    };
                    self.result(r);
                }
            }
            Action::ClonePickOther => {
                if let Some(row) = self.clone_unresolved.pick.take() {
                    let target = self
                        .clone_draft
                        .as_ref()
                        .and_then(|p| p.unresolved.get(row))
                        .map_or(String::new(), |u| u.target.clone());
                    self.prompt = Some(Prompt {
                        kind: PromptKind::CloneTexture(row),
                        title: format!("Texture for {target}: a PIC in this LIB or a source LIB"),
                        value: String::new(),
                        axis: 0,
                        caret: super::Caret::END,
                    });
                }
            }
            Action::CloneAck => {
                self.clone_unresolved.ack = !self.clone_unresolved.ack;
                self.status = if self.clone_unresolved.ack {
                    format!(
                        "{} kept as in the source LIB; Export new LIB is available",
                        count(
                            self.clone_kept(),
                            "unresolved reference",
                            "unresolved references"
                        )
                    )
                } else {
                    "Export waits for the unresolved references to be acknowledged".into()
                };
            }
            Action::CloneBack => match self.prompt.as_ref().map(|p| (&p.kind, p.value.clone())) {
                Some((PromptKind::CloneReview, _)) => {
                    self.clone_draft = None;
                    self.clone_step(3);
                }
                Some((PromptKind::CloneTitle, value)) => {
                    self.clone_title = value;
                    self.clone_step(2);
                }
                Some((PromptKind::CloneShort, value)) => {
                    self.identity.short = value;
                    self.clone_step(1);
                }
                _ => {}
            },
            Action::IdentityBack => self.identity_back(),
            Action::IdentityEdit(long) => self.identity_edit(long),
            Action::IdentityReset(long) => self.identity_reset(long),
            Action::RenameAircraft => self.rename_aircraft_prompt(),
            Action::DuplicateAircraft => self.begin_duplicate(),
            Action::DuplicateToggle(row, share) => {
                let r = self.duplicate_toggle(row, share);
                self.result(r);
            }
            Action::DuplicatePalette(copy) => self.duplicate_palette(copy),
            Action::Menu(n) => self.menu = if self.menu == Some(n) { None } else { Some(n) },
            Action::Mode(m) => {
                if self.animation_tool {
                    self.animation_tool = false;
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
            Action::About => self.about_open(),
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
            // Placed by the press in `pointer`; nothing else to do.
            Action::PromptText => {}
            Action::Filter => {
                self.filter_focus = true;
                self.category = None;
                self.table_scroll = 0;
            }
            Action::TypeFilter(cat) => {
                self.type_filter = if self.type_filter == Some(cat) {
                    None
                } else {
                    Some(cat)
                };
                self.scroll = 0;
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
                self.validation = Some(report);
                self.status = format!("Package checks: {}", self.package_summary().1);
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
            Action::ContextSub(n) => {
                self.context.sub = n;
            }
            Action::TransferTo(id, moving) => {
                let result = self.prepare_drop(id);
                if result.is_ok() && moving {
                    self.transfer_move = true;
                    self.status = "Review the move; known shared dependencies stay in the source. Files change only when saved".into();
                }
                self.result(result);
            }
            Action::DeleteEntry => {
                let result = self.delete_entry();
                self.result(result);
            }
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
            Action::ConfirmSave => {
                if matches!(
                    self.prompt.as_ref().map(|p| &p.kind),
                    Some(PromptKind::GameFolder)
                ) {
                    if let Some(c) = self.save_check.as_mut() {
                        c.confirmed = true;
                        let path = c.path.clone();
                        self.prompt = None;
                        let result = self.perform_file(FileAction::Save, &path);
                        self.result(result);
                    }
                }
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
            Action::SelectMode(face) => self.select_mode(face),
            Action::MeshOp(op) => self.mesh_op(op),
            Action::Pivot(individual) => self.ed.pivot_individual = individual,
            Action::Gizmo(h) => self.gizmo_press(h),
            Action::GizmoMode(mode) => self.gizmo_mode(mode),
            Action::Magnet => self.toggle_magnet(),
            Action::Xray => self.toggle_xray(),
            Action::PartPick(i) => self.pick_part(i),
            Action::PosePreset(n) => self.pose_preset(n),
            Action::PoseReset => self.pose_reset(),
            Action::PoseSet(var, value) => self.pose_set(var, value),
            Action::PartMenu(k) => self.open_part_menu(k),
            Action::PartValue(k, value) => self.part_value(k, value),
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
                    title: "Squadron library".into(),
                    value: String::new(),
                    axis: 0,
                    caret: super::Caret::END,
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
                    title: "Tail-text ink: palette index 0 to 255".into(),
                    value: format!("{}", self.text_ink()),
                    axis: 0,
                    caret: super::Caret::END,
                })
            }
            Action::DecalText => {
                self.prompt = Some(Prompt {
                    kind: PromptKind::DecalText,
                    title: "Tail number: letters, digits, space, dash, slash, period".into(),
                    value: self.decal_text.clone(),
                    axis: 0,
                    caret: super::Caret::END,
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
                self.preview = None;
                self.hp_tool = self.mode != Mode::Model || !self.hp_tool;
                self.mode = Mode::Model;
                self.hp_visible = true;
                self.selected_face = None;
                self.decal_active = false;
                self.decal_draft = None;
            }
            Action::StationSlew(slew) => self.hp_slew = slew,
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
            Action::ViewportMode(n) => {
                let current = self.viewport_mode();
                if current == n as usize && n != 0 {
                    return;
                }
                // Leave the current mode through its own toggle, then enter the new one.
                match current {
                    1 => self.act(Action::MeshMode),
                    2 => self.hp_tool = false,
                    3 => {
                        self.animation_tool = false;
                        self.ed.part_menu = None;
                        self.preview = None;
                    }
                    4 => self.act(Action::ModelPaint),
                    _ => {}
                }
                self.mode = Mode::Model;
                match n {
                    1 => self.act(Action::MeshMode),
                    2 => self.act(Action::Hardpoints),
                    3 => self.act(Action::Animation),
                    4 => self.act(Action::ModelPaint),
                    _ => {}
                }
            }
            Action::Shading(n) => {
                self.finish_stroke();
                self.textured = n > 0;
                self.flat = n == 1;
                self.perspective &= n == 0;
            }
            Action::Number(t) => self.number_press(t, self.mouse[0]),
            Action::Panel(id) => {
                let bit = 1u64 << id;
                self.panels = if self.ctrl { !bit } else { self.panels ^ bit };
            }
            Action::NumberStep(t, direction) => self.number_step(t, direction),
            Action::Apply => self.key(Key::Enter, false, false),
            Action::Cancel => self.key(Key::Escape, false, false),
        }
    }
    #[cfg(not(windows))]
    pub fn workspace(&mut self, name: &str) -> Result<()> {
        // "STATE@ZOOM" sets up STATE, then zooms the viewport to ZOOM percent.
        if let Some((state, zoom)) = name.rsplit_once('@') {
            let zoom: i32 = zoom.parse().map_err(|_| "Zoom percent after @")?;
            self.workspace(state)?;
            self.zoom = zoom.clamp(10, 1000);
            return Ok(());
        }
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

        // Outliner drag and context menu states over SOURCE.LIB and MYMOD.LIB.
        if name.starts_with("drag-") || name.starts_with("context-") {
            let target = self.drag_fixture()?;
            self.mode = Mode::Model;
            let x = 70;
            let at = |app: &App, row: libraries_ui::Row| {
                app.library_rows()
                    .iter()
                    .skip(app.scroll)
                    .position(|r| *r == row)
                    .map(|i| app.tree_start() + i as i32 * theme::metric::ROW_H + 8)
                    .ok_or("Row not visible")
            };
            let from = at(self, (self.library_id, Some(0), Some(1)))?;
            if let Some(menu) = name.strip_prefix("context-") {
                if menu == "root" {
                    let root = at(self, (target, None, None))?;
                    self.pointer(x, root, 3, true, false);
                    return Ok(());
                }
                self.pointer(x, from, 3, true, false);
                self.pointer(x, from, 3, false, false);
                if menu != "menu" {
                    let n = if menu == "move" { 2 } else { 1 };
                    let item = self
                        .chrome_hit(&|a| matches!(a, Action::ContextSub(i) if i == n))
                        .ok_or("Copy to missing")?;
                    self.motion(item[0] + 30, item[1] + 8, false);
                    let lib = self
                        .chrome_hit(&|a| matches!(a, Action::TransferTo(..)))
                        .ok_or("LIB item missing")?;
                    self.motion(lib[0] + 30, lib[1] + 8, false);
                }
                return Ok(());
            }
            self.motion(x, from, false);
            self.pointer(x, from, 1, true, false);
            let (tx, ty) = match name {
                "drag-graft" => {
                    let to = self.doc.archive.find("DEMO2.PT").ok_or("DEMO2.PT")?;
                    (x + 20, at(self, (self.library_id, Some(0), Some(to)))?)
                }
                "drag-invalid" => (self.left() + 160, 300),
                _ => (x + 20, at(self, (target, Some(0), None))?),
            };
            self.motion(tx, ty, false);
            return Ok(());
        }
        // "menu-N" snapshots dropdown N open over the Model workspace.
        if let Some(n) = name.strip_prefix("menu-") {
            self.mode = Mode::Model;
            self.menu = Some(n.parse().map_err(|_| "Menu number")?);
            return Ok(());
        }
        if name.starts_with("vertex-") {
            return self.snapshot_vertex(name);
        }
        if name.starts_with("gizmo") || name.starts_with("handles") {
            return self.snapshot_gizmo(name);
        }
        if name == "assign-texture" {
            return self.snapshot_assign();
        }
        if name.starts_with("markings") {
            return self.snapshot_markings(name);
        }
        if name.starts_with("replace") {
            return self.snapshot_replace(name);
        }
        if name == "paint-model" || name == "paint-side" {
            self.mode = Mode::Model;
            self.act(Action::ModelPaint);
            if name == "paint-side" {
                // Side view (numpad 3), as for checking tail panels.
                self.yaw = 90;
                self.pitch = 0;
            }
            return Ok(());
        }
        if name == "about" {
            self.mode = Mode::Model;
            self.about_open();
            return Ok(());
        }
        if name == "animation" {
            self.open_animation();
            return Ok(());
        }
        // Edit Mesh and Parts snapshots: the synthetic parts aircraft in the
        // demo, else the opened SH, gear down with the first gear part picked.
        if name == "edit" || name.starts_with("parts") || name == "edit-vertices" {
            if self.path.starts_with("Synthetic") {
                self.doc
                    .replace(0, hangar_core::shape_testkit::demo_parts())?;
                self.doc.mark_saved();
                self.select_entry(0);
            }
            self.mode = Mode::Model;
            self.textured = true;
            self.flat = true;
            self.yaw = 35;
            self.pitch = 20;
            self.pose_preset(0);
            let gear = self
                .ed
                .parts
                .iter()
                .position(|p| p.role == hangar_core::shape_parts::Role::Gear)
                .unwrap_or(0);
            if name.starts_with("parts") {
                self.open_animation();
                // The first gear part whose direction flips in place.
                let flip = (0..self.ed.parts.len()).find_map(|i| {
                    let k = self.ed.parts[i].controls.iter().position(|c| {
                        matches!(
                            c.setting,
                            hangar_core::shape_parts::Setting::Direction { .. }
                        ) && c.allowed == hangar_core::shape_parts::Allowed::Either
                    })?;
                    Some((i, k))
                });
                self.pick_part(flip.map_or(gear, |(i, _)| i));
                if name == "parts-settings" {
                    if let Some((_, k)) = flip {
                        let negated = self.ed.parts[self.ed.part_selected.unwrap_or(0)].controls[k]
                            .setting
                            == hangar_core::shape_parts::Setting::Direction {
                                index: 0,
                                negated: true,
                            };
                        self.act(Action::PartValue(k, i32::from(!negated)));
                    }
                    self.mouse = [self.right() + 20, 300];
                    for _ in 0..80 {
                        self.wheel(-1);
                    }
                }
            } else {
                self.act(Action::MeshMode);
                self.pick_part(gear);
                self.select_mode(name == "edit");
            }
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
        if name == "textured" || name == "solid" {
            self.mode = Mode::Model;
            self.textured = true;
            self.flat = name == "solid";
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
            self.clone_draft = Some(Box::new(self.build_clone()?));
            self.prompt = Some(Prompt {
                kind: PromptKind::CloneReview,
                title: "Review".into(),
                value: String::new(),
                axis: 0,
                caret: super::Caret::END,
            });
            return Ok(());
        }
        if name == "rename-review" || name == "duplicate-review" {
            // The selected PT (the demo's otherwise); NEO / TWIN as the new ID.
            if !self.name().ends_with(".PT") {
                let at = self.doc.archive.find("DEMO.PT").ok_or("Select a PT")?;
                self.select_entry(at);
            }
            if name == "rename-review" {
                self.rename_aircraft_prompt();
                return self.rename_review("NEO");
            }
            self.begin_duplicate();
            self.variant_id = "TWIN".into();
            return self.duplicate_review_prompt();
        }
        if name == "clone-unresolved" {
            // Synthetic dangling texture; the picker is open on its row.
            self.smoke_ghost_review("GHJET");
            self.clone_unresolved.pick = Some(0);
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
    pub(super) fn lib_name(&self) -> &str {
        if self.path.starts_with("Synthetic") {
            "DEMO.LIB"
        } else if self.path.is_empty() {
            "No LIB open"
        } else {
            self.path.rsplit(['/', '\\']).next().unwrap_or(&self.path)
        }
    }
    /// Browse table rows that fit above the dock.
    pub(super) fn browse_rows(&self) -> usize {
        ((self.dock_y() - 54 - TABLE_HEAD_H) / theme::metric::ROW_H).max(1) as usize
    }
    pub(super) fn browser_entries(&self) -> Vec<usize> {
        let f = self.filter.to_ascii_uppercase();
        self.doc
            .archive
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let cat = category_of(&e.name);
                self.category.is_none_or(|n| cat == n)
                    && self.type_filter.is_none_or(|n| cat == n)
                    && e.name.contains(&f)
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
            ("Propulsion", "Neg-G cut-out", "plane.negGLimit"),
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
        // Underlying controls do not show hover while a menu or dialog covers them.
        // A live entry drag draws its own target feedback instead of hover.
        let covered = self.menu.is_some() || self.prompt.is_some() || self.drag_live();
        let mut out = Layout {
            mouse: if covered { [i32::MIN; 2] } else { self.mouse },
            pressed: self.pressed,
            size: [self.width, self.height],
            scrub: self.scrub.map(|s| (s.target, s.value)),
            inspector_max: 0,
            zoom_rect: None,
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
        self.menubar(&mut out);
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
        self.statusbar(&mut out);
        self.drag_ghost(&mut out);
        out.mouse = self.mouse;
        if let Some(menu) = self.menu {
            // Controls under an open menu cannot be reached; drop their hits.
            let rects = self.open_menu_rects();
            out.hits.retain(|h| {
                let [x, y, w, h] = h.rect;
                rects.iter().all(|[mx, my, mw, mh]| {
                    x >= mx + mw || mx >= &(x + w) || y >= my + mh || my >= &(y + h)
                })
            });
            self.menu_layout(&mut out, menu);
        }
        if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::CloneReview))
        {
            self.clone_review(&mut out);
        } else if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::RenameReview))
        {
            self.rename_review_layout(&mut out);
        } else if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::DuplicateReview))
        {
            self.duplicate_review_layout(&mut out);
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
                .is_some_and(|p| matches!(p.kind, PromptKind::BaseColor(_) | PromptKind::FaceColor))
            {
                self.color_dialog(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::About))
            {
                self.about_dialog(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::AssignTexture))
            {
                self.assign_dialog(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::RemapView))
            {
                self.remap_dialog(&mut out);
            } else if self
                .prompt
                .as_ref()
                .is_some_and(|p| matches!(p.kind, PromptKind::ReplaceColor))
            {
                self.replace_dialog(&mut out);
            } else {
                self.prompt_layout(&mut out);
            }
        }

        out
    }
    /// Outliner header geometry: filter field, type filter segmented control,
    /// open button.
    pub(super) fn outliner_header(&self) -> ([i32; 4], [i32; 4], [i32; 4]) {
        use theme::{metric as m, space};
        let l = self.left();
        let y = m::MENUBAR_H + (m::EDITOR_HEADER_H - m::BUTTON_H) / 2;
        let plus = [
            l - space::SPACE_2 - m::ICON_BUTTON,
            y,
            m::ICON_BUTTON,
            m::BUTTON_H,
        ];
        let seg_w = 3 * m::ICON_BUTTON + 2;
        let seg = [plus[0] - space::SPACE_1 - seg_w, y, seg_w, m::BUTTON_H];
        let filter = [
            space::SPACE_2,
            m::MENUBAR_H + (m::EDITOR_HEADER_H - m::FIELD_H) / 2,
            seg[0] - space::SPACE_1 - space::SPACE_2,
            m::FIELD_H,
        ];
        (filter, seg, plus)
    }
    fn outliner(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, dot, notched, type_badge, Btn, Tone};
        let l = self.left();
        let h = self.height;
        let (filter, seg, plus) = self.outliner_header();
        let d = &mut o.canvas;
        d.rect(0, m::MENUBAR_H, l, m::EDITOR_HEADER_H, c::GM_800);
        d.rect(0, m::MENUBAR_H + m::EDITOR_HEADER_H - 1, l, 1, c::GM_1000);
        // Filter field: sunken, `focus` border while typing, ink text.
        let [fx, fy, fw, fh] = filter;
        let hover = o.over(filter);
        let fill = if hover && !self.filter_focus {
            c::GM_1000
        } else {
            c::GM_950
        };
        let edge = if self.filter_focus {
            c::FOCUS
        } else {
            c::LINE_STRONG
        };
        let d = &mut o.canvas;
        notched(d, filter, Some(fill), Some(edge));
        d.icon_sm(
            fx + 5,
            fy + (fh - m::ICON_SM) / 2,
            Icon::Search,
            c::INK_MUTED,
            fill,
        );
        let tx = fx + 5 + m::ICON_SM + space::SPACE_1;
        let room = fx + fw - 5 - tx;
        let base = baseline(fy, fh, Style::Value);
        if self.filter.is_empty() && !self.filter_focus {
            let hint = if text_width("Filter entries", Style::Value) <= room {
                "Filter entries"
            } else {
                "Filter"
            };
            d.styled(
                tx,
                base,
                &fit(hint, room, Style::Value),
                c::INK_FAINT,
                Style::Value,
            );
        } else {
            // A long filter scrolls to keep the caret (or, unfocused, its end) in view.
            let (tx, room) = text_ui::filter_area(filter);
            let caret = self
                .filter_focus
                .then(|| self.text.filter.unwrap_or(Caret::END));
            text_ui::draw(d, tx, base, fy, fh, room, &self.filter, caret);
        }
        o.hit(filter, Action::Filter);
        let types: Vec<(Btn, Action)> = [(0, Icon::Aircraft), (1, Icon::Shape), (2, Icon::Image)]
            .into_iter()
            .map(|(cat, g)| {
                (
                    Btn::icon(g).on(self.type_filter == Some(cat)),
                    Action::TypeFilter(cat),
                )
            })
            .collect();
        o.segmented(seg, &types);
        o.button_ex(plus, Btn::icon(Icon::Plus), Action::File(FileAction::Open));
        let rows = self.library_rows();
        let top = self.tree_start();
        let bottom = h - m::STATUSBAR_H - OUTLINER_FOOTER;
        let visible = self.outliner_rows();
        let indent = m::TREE_INDENT;
        let hovered = if self.drag_live() {
            self.outliner_row_at(self.mouse[0], self.mouse[1])
        } else {
            None
        };
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
            let y = top + row as i32 * m::ROW_H;
            let rect = [0, y, l, m::ROW_H];
            // Active: the entry Properties shows. Selected: the entries it is
            // working with (its linked shape, the model a texture came from).
            let (is_active, is_selected) = match (cat, entry) {
                (_, Some(i)) if active => (
                    *i == self.selected,
                    *i != self.selected
                        && (self.model_entry == Some(*i) || self.context_entry == Some(*i)),
                ),
                (_, Some(i)) => (false, self.external_model == Some((*id, *i))),
                _ => (false, false),
            };
            let look = self.drop_row_look((*id, *cat, *entry), hovered);
            let fill = if let Some((fill, _)) = look {
                fill
            } else if is_active || is_selected {
                c::AMBER_DEEP
            } else if o.over(rect) {
                c::GM_700
            } else if (row + self.scroll) % 2 == 1 {
                c::GM_900
            } else {
                c::GM_800
            };
            let d = &mut o.canvas;
            // A LIB row is two controls: the twisty and the LIB itself.
            let split = if cat.is_none() { 4 + m::ICON_SM + 2 } else { 0 };
            d.rect(0, y, split, m::ROW_H, fill);
            d.rect(split, y, l - split, m::ROW_H, fill);
            if let Some((_, edge)) = look {
                border(d, 0, y, l, m::ROW_H, edge);
            }
            let mid = y + (m::ROW_H - m::ICON) / 2;
            let twisty = y + (m::ROW_H - m::ICON_SM) / 2;
            let right = l - space::SPACE_2;
            match (cat, entry) {
                (Some(cat), Some(i)) => {
                    let e = &doc.archive.entries[*i];
                    let x = 4 + 2 * indent + m::ICON_SM + space::SPACE_1;
                    d.icon(
                        x,
                        mid,
                        GROUPS[*cat].2,
                        if is_active { c::AMBER } else { c::INK_MUTED },
                        fill,
                    );
                    let changed = doc.entry_changed(e);
                    let tx = x + m::ICON + space::SPACE_1;
                    let room = right - tx - if changed { m::DIRTY_DOT + 4 } else { 0 };
                    d.styled(
                        tx,
                        baseline(y, m::ROW_H, Style::Value),
                        &fit(&e.name, room, Style::Value),
                        if is_active { c::AMBER_BRIGHT } else { c::INK },
                        Style::Value,
                    );
                    if changed {
                        dot(
                            d,
                            right - m::DIRTY_DOT,
                            y + (m::ROW_H - m::DIRTY_DOT) / 2,
                            c::AMBER,
                        );
                    }
                    o.hit(
                        rect,
                        if active {
                            Action::Entry(*i)
                        } else {
                            Action::LibraryEntry(*id, *i)
                        },
                    );
                }
                (Some(cat), None) => {
                    let open = !collapsed[*cat] || !filter.is_empty();
                    let x = 4 + indent;
                    d.icon_sm(
                        x,
                        twisty,
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        c::INK_MUTED,
                        fill,
                    );
                    let ix = x + m::ICON_SM + space::SPACE_1;
                    d.icon(ix, mid, GROUPS[*cat].2, c::INK_MUTED, fill);
                    // Count in value-sm, then the type badge, right-aligned.
                    let (name, ext, _) = GROUPS[*cat];
                    let mut rx = right;
                    if !ext.is_empty() {
                        rx -= widgets::badge_width(ext);
                        type_badge(d, rx, y + (m::ROW_H - m::BADGE_H) / 2, ext, Tone::Neutral);
                        rx -= space::SPACE_1;
                    }
                    let n = doc
                        .archive
                        .entries
                        .iter()
                        .filter(|e| category_of(&e.name) == *cat)
                        .count();
                    let count = widgets::format_number(n as i64, 0, true);
                    rx -= text_width(&count, Style::ValueSm);
                    d.styled(
                        rx,
                        baseline(y, m::ROW_H, Style::ValueSm),
                        &count,
                        c::INK_MUTED,
                        Style::ValueSm,
                    );
                    let tx = ix + m::ICON + space::SPACE_1;
                    d.styled(
                        tx,
                        baseline(y, m::ROW_H, Style::Label),
                        &fit(name, rx - space::SPACE_2 - tx, Style::Label),
                        c::INK,
                        Style::Label,
                    );
                    o.hit(
                        rect,
                        if active {
                            Action::Category(*cat)
                        } else {
                            Action::LibraryCategory(*id, *cat)
                        },
                    );
                }
                (None, _) => {
                    d.icon_sm(
                        4,
                        twisty,
                        if root_collapsed {
                            Icon::ChevronRight
                        } else {
                            Icon::ChevronDown
                        },
                        c::INK_MUTED,
                        fill,
                    );
                    let ix = 4 + m::ICON_SM + space::SPACE_1;
                    d.icon(ix, mid, Icon::Lib, c::INK_MUTED, fill);
                    let count = widgets::format_number(doc.archive.entries.len() as i64, 0, true);
                    let mut rx = right - text_width(&count, Style::ValueSm);
                    d.styled(
                        rx,
                        baseline(y, m::ROW_H, Style::ValueSm),
                        &count,
                        c::INK_MUTED,
                        Style::ValueSm,
                    );
                    if doc.dirty() {
                        rx -= m::DIRTY_DOT + space::SPACE_1;
                        dot(d, rx, y + (m::ROW_H - m::DIRTY_DOT) / 2, c::AMBER);
                    }
                    let tx = ix + m::ICON + space::SPACE_1;
                    d.styled(
                        tx,
                        baseline(y, m::ROW_H, Style::Value),
                        &fit(path, rx - space::SPACE_2 - tx, Style::Value),
                        if active { c::INK } else { c::INK_MUTED },
                        Style::Value,
                    );
                    let toggle = 4 + m::ICON_SM + 2;
                    o.hit([toggle, y, l - toggle, m::ROW_H], Action::Library(*id));
                    o.hit([0, y, toggle, m::ROW_H], Action::LibraryToggle(*id));
                }
            }
        }
        if rows.len() > visible {
            // Scroll cue: a thumb at the right edge over the rows.
            let track = bottom - top - 4;
            let thumb = (track * visible as i32 / rows.len() as i32).max(16);
            let span = (rows.len() - visible) as i32;
            let ty = top + 2 + (track - thumb) * (self.scroll as i32).min(span) / span;
            o.canvas.rect(l - 4, ty, 3, thumb, c::GM_600);
        }
        let d = &mut o.canvas;
        d.rect(0, bottom, l, OUTLINER_FOOTER, c::GM_800);
        d.rect(0, bottom, l, 1, c::GM_1000);
        let by = bottom + (OUTLINER_FOOTER - m::BUTTON_H) / 2;
        let open = Btn::new("Open LIB");
        let ow = open.width();
        o.button_ex(
            [space::SPACE_2, by, ow, m::BUTTON_H],
            open,
            Action::File(FileAction::Open),
        );
        let ex = space::SPACE_2 + ow + space::SPACE_1;
        o.button_ex(
            [ex, by, l - space::SPACE_2 - ex, m::BUTTON_H],
            Btn::new("Export object"),
            Action::File(FileAction::Variant),
        );
    }
    fn model_layout(&self, o: &mut Layout) {
        let l = self.left();
        let r = self.right();
        let dock = self.dock_y();
        let width = r - l;
        self.viewport_header(o);
        if let Some(m) = self.preview.as_deref().or(self.model.as_ref()) {
            if self.textured {
                self.draw_model(o, l + 1, 54, width - 2, dock - 54);
            } else {
                self.viewport(&mut o.canvas, m, l + 1, 54, width - 2, dock - 54);
            }
        } else {
            let d = &mut o.canvas;
            d.styled(l + 64, 144, "No shape loaded", c::INK, Style::Title);
            label_fit(
                d,
                l + 64,
                170,
                width - 96,
                "Open a LIB and select an aircraft or shape.",
                c::INK_MUTED,
            );
            o.button_ex(
                [l + 64, 186, 96, theme::metric::BUTTON_H],
                widgets::Btn::new("Open LIB").primary(),
                Action::File(FileAction::Open),
            );
            o.button_ex(
                [l + 168, 186, 96, theme::metric::BUTTON_H],
                widgets::Btn::new("Load demo"),
                Action::Demo,
            );
        }
        let writable = self.model.as_ref().is_some_and(|m| m.writable)
            && self.model_entry == Some(self.selected);
        self.tool_strip(o, writable);
        self.viewport_overlay(o, writable);
        if self.mesh_edit {
            self.mesh_overlay(o);
        } else if self.animation_tool {
            self.face_edges(o);
            if let Some([x, y]) = self.part_origin().and_then(|p| self.hp_project(p)) {
                if self.in_viewport(x, y) {
                    chrome::ring(&mut o.canvas, x, y, 5, c::AMBER, c::AMBER_BRIGHT);
                    o.canvas
                        .styled(x + 8, y - 6, "Pivot", c::AMBER, Style::ValueSm);
                }
            }
        } else {
            self.hardpoint_overlay(o);
        }
    }
    fn browse_layout(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, dot, Btn};
        let l = self.left();
        let r = self.right();
        let width = r - l;
        let top = m::MENUBAR_H;
        let d = &mut o.canvas;
        d.rect(l + 1, top, width - 2, m::EDITOR_HEADER_H, c::GM_800);
        d.rect(
            l + 1,
            top + m::EDITOR_HEADER_H - 1,
            width - 2,
            1,
            c::GM_1000,
        );
        // Breadcrumb: LIB name, chevron, group.
        let open = Btn::new("Open model");
        let bw = open.width();
        let bx = r - 1 - space::SPACE_1 - bw;
        o.button_ex(
            [
                bx,
                top + (m::EDITOR_HEADER_H - m::BUTTON_H) / 2,
                bw,
                m::BUTTON_H,
            ],
            open,
            Action::Mode(Mode::Model),
        );
        let d = &mut o.canvas;
        let x = l + 1 + space::SPACE_3;
        let lib = fit(self.lib_name(), (bx - x) / 2, Style::Value);
        d.styled(
            x,
            baseline(top, m::EDITOR_HEADER_H, Style::Value),
            &lib,
            c::INK,
            Style::Value,
        );
        let cx = x + text_width(&lib, Style::Value) + space::SPACE_1;
        d.icon_sm(
            cx,
            top + (m::EDITOR_HEADER_H - m::ICON_SM) / 2,
            Icon::ChevronRight,
            c::INK_MUTED,
            c::GM_800,
        );
        let group = self
            .category
            .or(self.type_filter)
            .map_or("All entries", |c| GROUPS[c].0);
        let gx = cx + m::ICON_SM + space::SPACE_1;
        d.styled(
            gx,
            baseline(top, m::EDITOR_HEADER_H, Style::Label),
            &fit(group, bx - space::SPACE_2 - gx, Style::Label),
            c::INK_MUTED,
            Style::Label,
        );
        // Column header.
        let hy = top + m::EDITOR_HEADER_H;
        d.rect(l + 1, hy, width - 2, TABLE_HEAD_H, c::GM_900);
        d.rect(l + 1, hy + TABLE_HEAD_H - 1, width - 2, 1, c::GM_1000);
        let bytes_right = l + width * 3 / 4;
        let status_x = bytes_right + space::SPACE_4;
        let head = baseline(hy, TABLE_HEAD_H, Style::Section);
        d.styled(l + 12, head, "ENTRY", c::INK_MUTED, Style::Section);
        d.styled(
            bytes_right - text_width("BYTES", Style::Section),
            head,
            "BYTES",
            c::INK_MUTED,
            Style::Section,
        );
        d.styled(status_x, head, "STATUS", c::INK_MUTED, Style::Section);
        let start = hy + TABLE_HEAD_H;
        for (row, i) in self
            .browser_entries()
            .iter()
            .skip(self.table_scroll)
            .take(self.browse_rows())
            .enumerate()
        {
            let e = &self.doc.archive.entries[*i];
            let y = start + row as i32 * m::ROW_H;
            let rect = [l + 1, y, width - 2, m::ROW_H];
            let selected = *i == self.selected;
            let fill = if selected {
                c::AMBER_DEEP
            } else if o.over(rect) {
                c::GM_700
            } else if row % 2 == 0 {
                c::GM_800
            } else {
                c::GM_900
            };
            let d = &mut o.canvas;
            d.rect(rect[0], y, rect[2], m::ROW_H, fill);
            d.icon(
                l + 10,
                y + (m::ROW_H - m::ICON) / 2,
                GROUPS[category_of(&e.name)].2,
                if selected { c::AMBER } else { c::INK_MUTED },
                fill,
            );
            let base = baseline(y, m::ROW_H, Style::Value);
            d.styled(
                l + 32,
                base,
                &fit(&e.name, bytes_right - 80 - l - 32, Style::Value),
                if selected { c::AMBER_BRIGHT } else { c::INK },
                Style::Value,
            );
            let size = widgets::format_number(e.stored_len() as i64, 0, true);
            d.styled(
                bytes_right - text_width(&size, Style::Value),
                base,
                &size,
                c::INK_MUTED,
                Style::Value,
            );
            let changed = self.doc.entry_changed(e);
            let mut sx = status_x;
            if changed {
                dot(d, sx, y + (m::ROW_H - m::DIRTY_DOT) / 2, c::AMBER);
                sx += m::DIRTY_DOT + space::SPACE_1;
            }
            d.styled(
                sx,
                baseline(y, m::ROW_H, Style::Label),
                &fit(
                    if changed { "Edited" } else { "Saved" },
                    r - 4 - sx,
                    Style::Label,
                ),
                if changed { c::AMBER } else { c::INK_MUTED },
                Style::Label,
            );
            o.hit(rect, Action::Entry(*i));
        }
    }
    /// Right editor header (28px): entry type icon, name and type badge, or
    /// `title` for editors that are not about one entry.
    pub(super) fn inspector_header(&self, o: &mut Layout, title: Option<&str>) {
        use theme::{metric as m, space};
        use widgets::{baseline, type_badge, Tone};
        let r = self.right();
        let w = self.width - r;
        let top = m::MENUBAR_H;
        let d = &mut o.canvas;
        d.rect(r + 1, top, w - 1, m::EDITOR_HEADER_H, c::GM_800);
        d.rect(r + 1, top + m::EDITOR_HEADER_H - 1, w - 1, 1, c::GM_1000);
        let x = r + 1 + space::SPACE_2;
        if let Some(title) = title {
            d.styled(
                x,
                baseline(top, m::EDITOR_HEADER_H, Style::Strong),
                &fit(title, w - 2 * space::SPACE_2, Style::Strong),
                c::INK,
                Style::Strong,
            );
            return;
        }
        d.icon(
            x,
            top + (m::EDITOR_HEADER_H - m::ICON) / 2,
            GROUPS[category_of(self.name())].2,
            c::INK_MUTED,
            c::GM_800,
        );
        let ext = if self.doc.archive.entries.is_empty() {
            ""
        } else {
            extension(self.name())
        };
        let bx = self.width - space::SPACE_2 - widgets::badge_width(ext);
        if !ext.is_empty() {
            type_badge(
                d,
                bx,
                top + (m::EDITOR_HEADER_H - m::BADGE_H) / 2,
                ext,
                Tone::Neutral,
            );
        }
        let tx = x + m::ICON + space::SPACE_2;
        d.styled(
            tx,
            baseline(top, m::EDITOR_HEADER_H, Style::Value),
            &fit(self.name(), bx - space::SPACE_2 - tx, Style::Value),
            c::INK,
            Style::Value,
        );
    }
    /// The scrolling panel area of the right editor below `top`, above `foot`
    /// px of fixed footer.
    pub(super) fn inspector_stack(&self, top: i32, foot: i32) -> widgets::Stack {
        let r = self.right();
        let scroll = if self.inspector_kind == self.inspector_kind() {
            self.inspector_scroll
        } else {
            0
        };
        widgets::Stack::new(
            [
                r + 1,
                top,
                self.width - r - 1,
                self.height - theme::metric::STATUSBAR_H - foot - top,
            ],
            scroll,
        )
    }
    /// Which right editor is showing; its scroll position is kept per kind.
    pub(super) fn inspector_kind(&self) -> u8 {
        match self.mode {
            Mode::Package => 1,
            Mode::Graft => 2,
            Mode::Properties => 3,
            Mode::Model if self.animation_tool => 4,
            Mode::Model if self.mesh_edit => 5,
            Mode::Model if self.hp_tool => 6,
            Mode::Browse => 7,
            _ if self.mode == Mode::Media
                || self.selected_face.is_some()
                || self.model_paint
                || self.media_tab > 0 =>
            {
                10 + self.media_tab
            }
            _ => 0,
        }
    }
    pub(super) fn panel_closed(&self, id: u8) -> bool {
        self.panels & (1 << id) != 0
    }
    /// Begin panel `id` of the right editor.
    pub(super) fn pane(
        &self,
        o: &mut Layout,
        s: &mut widgets::Stack,
        id: u8,
        title: &str,
        icon: Icon,
    ) -> bool {
        let closed = self.panel_closed(id);
        o.panel_begin(s, title, Some(icon), closed, Action::Panel(id), 0);
        !closed
    }
    /// Value control for BRF field `i`: a NumberField for decimal integers,
    /// otherwise a sunken text field that opens the type prompt.
    pub(super) fn field_control(&self, o: &mut Layout, rect: [i32; 4], i: usize) {
        let Some(f) = self.brf.as_ref().and_then(|b| b.fields.get(i)) else {
            return;
        };
        let target = widgets::NumberTarget::Field(i);
        match self.number_spec(target) {
            Some(spec) => o.number(
                rect,
                &widgets::Number {
                    target,
                    spec,
                    label: "",
                    unit: if f.scaled {
                        "scaled"
                    } else {
                        hangar_core::definition::unit(&f.label)
                    },
                    locked: false,
                    axis: None,
                },
            ),
            None => o.text_field(
                rect,
                &format!("{}{}", f.value, if f.scaled { " scaled" } else { "" }),
                self.field_changed(i),
                Action::Field(i),
            ),
        }
    }
    /// The Shape row's Select rect (fixed above the panels).
    pub(super) fn shape_select_rect(&self) -> [i32; 4] {
        use theme::{metric as m, space};
        let r = self.right();
        let x = r + 1 + space::SPACE_1 + space::SPACE_2;
        let w = self.width - space::SPACE_1 - 4 - space::SPACE_2 - x;
        let col = w * m::PROP_LABEL_PCT / 100;
        let cx = x + col + space::SPACE_2;
        [
            cx,
            m::MENUBAR_H + m::EDITOR_HEADER_H + space::SPACE_1,
            x + w - cx - m::ICON_BUTTON - space::SPACE_1,
            m::FIELD_H,
        ]
    }
    /// Shapes this entry links to (its own model first), for the Shape Select.
    pub(super) fn shape_items(&self) -> Vec<(String, Action)> {
        let mut items = Vec::new();
        if let Some(i) = self.model_entry {
            items.push((self.doc.archive.entries[i].name.clone(), Action::Entry(i)));
        } else if let Some((id, i)) = self.external_model {
            if let Some(e) = self
                .libraries
                .iter()
                .find(|l| l.id == id)
                .and_then(|l| l.doc.archive.entries.get(i))
            {
                items.push((e.name.clone(), Action::LibraryEntry(id, i)));
            }
        }
        if let Some(scan) = self.dependencies.get(self.name()) {
            for link in &scan.links {
                if extension(&link.target) == "SH" && !items.iter().any(|(n, _)| *n == link.target)
                {
                    if let Some(i) = self.doc.archive.find(&link.target) {
                        items.push((link.target.clone(), Action::Entry(i)));
                    }
                }
            }
        }
        items
    }
    fn inspector(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{pane, Btn};
        let r = self.right();
        self.inspector_header(o, None);
        if self.mode == Mode::Browse {
            self.entry_inspector(o);
            return;
        }
        // Shape: a Select of the shapes this entry uses; the link button
        // shows its references in the dock. Entries without one skip the row.
        let shapes = self.shape_items();
        let mut s = match shapes.first() {
            Some((name, _)) => {
                let select = self.shape_select_rect();
                let y = select[1];
                let x = r + 1 + space::SPACE_1 + space::SPACE_2;
                o.prop_row(
                    [
                        x,
                        y,
                        self.width - space::SPACE_1 - 4 - space::SPACE_2 - x,
                        m::ROW_H,
                    ],
                    "Shape",
                );
                o.select(
                    select,
                    Some(Icon::Shape),
                    name,
                    Action::Menu(chrome::MENU_SHAPE),
                    self.menu == Some(chrome::MENU_SHAPE),
                );
                o.icon_button(
                    select[0] + select[2] + space::SPACE_1,
                    y - 1,
                    Btn::icon(Icon::Link).ghost().on(self.dock == 4),
                    Action::Dock(4),
                );
                self.inspector_stack(y + m::FIELD_H + space::SPACE_2, 0)
            }
            None => self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0),
        };
        self.identity_panel(o, &mut s);
        let rows = self.property_rows();
        let mut group = "";
        for (section, label, i) in &rows {
            if group != *section {
                if !group.is_empty() {
                    o.panel_end(&mut s);
                }
                group = section;
                let (id, icon) = match *section {
                    "Envelope" => (pane::ENVELOPE, Icon::Flight),
                    "Propulsion" => (pane::PROPULSION, Icon::Engine),
                    "Weights" => (pane::WEIGHTS, Icon::Measure),
                    "Handling" => (pane::HANDLING, Icon::Sliders),
                    _ => (pane::STRUCTURE, Icon::Damage),
                };
                self.pane(o, &mut s, id, section, icon);
            }
            if let Some(rect) = o.prop(&mut s, label) {
                self.field_control(o, rect, *i);
            }
        }
        if !group.is_empty() {
            o.panel_end(&mut s);
        }
        if let Some(model) = &self.model {
            if self.pane(o, &mut s, pane::GEOMETRY, "Geometry", Icon::Shape) {
                o.info(
                    &mut s,
                    "Vertices",
                    &widgets::format_number(model.vertices.len() as i64, 0, true),
                    "",
                );
                o.info(
                    &mut s,
                    "Faces",
                    &widgets::format_number(model.faces.len() as i64, 0, true),
                    "",
                );
                o.info(
                    &mut s,
                    "Records",
                    if model.writable {
                        "Static, editable"
                    } else {
                        "Animated, preview only"
                    },
                    "",
                );
                if let Some(rect) = o.prop(&mut s, "Base color") {
                    let label = self
                        .dominant_color()
                        .map_or("Textured".into(), |c| format!("Palette {c}"));
                    o.button_ex(
                        rect,
                        Btn::new(&label).with_icon(Icon::Palette),
                        Action::BaseColor(false),
                    );
                }
            }
            o.panel_end(&mut s);
            if !model.textures.is_empty() {
                if self.pane(o, &mut s, pane::TEXTURES, "Textures", Icon::Image) {
                    for name in model.textures.iter().take(6) {
                        let n = if name.contains('.') {
                            name.clone()
                        } else {
                            format!("{name}.PIC")
                        };
                        match self.doc.archive.find(&n) {
                            Some(i) => {
                                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                                    o.button_ex(
                                        rect,
                                        Btn::new(&format!("Paint {n}")).with_icon(Icon::Brush),
                                        Action::OpenTexture(i),
                                    );
                                }
                            }
                            None => o.stack_notice(
                                &mut s,
                                widgets::Tone::Warn,
                                &format!(
                                    "{n} is not in this LIB. Open the LIB that holds it to paint."
                                ),
                            ),
                        }
                    }
                }
                o.panel_end(&mut s);
            }
            self.markings_pane(o, &mut s);
        }
        if !self.doc.archive.entries.is_empty() {
            if self.pane(o, &mut s, pane::ENTRY_ACTIONS, "Entry", Icon::Lib) {
                let mut buttons = vec![
                    ("Export entry", Action::File(FileAction::Export)),
                    ("Replace entry", Action::File(FileAction::Replace)),
                ];
                if self.model.is_some() {
                    buttons.push(("Export geometry as OBJ", Action::File(FileAction::Obj)));
                }
                if !rows.is_empty() {
                    buttons.push(("Edit all fields", Action::Mode(Mode::Properties)));
                }
                for (title, action) in buttons {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(rect, Btn::new(title), action);
                    }
                }
            }
            o.panel_end(&mut s);
        }
        o.stack_end(s);
    }
    fn entry_inspector(&self, o: &mut Layout) {
        use theme::metric as m;
        use widgets::{pane, Btn};
        let Some(e) = self.doc.archive.entries.get(self.selected) else {
            return;
        };
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0);
        self.identity_panel(o, &mut s);
        if self.pane(o, &mut s, pane::LIB_ENTRY, "Lib entry", Icon::Lib) {
            o.info(&mut s, "Type", GROUPS[category_of(&e.name)].0, "");
            o.info(
                &mut s,
                "Stored size",
                &widgets::format_number(e.stored_len() as i64, 0, true),
                "B",
            );
            let offset = if self.doc.entry_changed(e) {
                "Repacked".into()
            } else {
                format!("0x{:08X}", e.source_offset())
            };
            o.info(&mut s, "Source offset", &offset, "");
            o.info(&mut s, "Compression", &format!("flag {}", e.flag()), "");
        }
        o.panel_end(&mut s);
        let links = self.dependencies.get(&e.name).map_or(0, |s| s.links.len());
        let users = self.dependencies.incoming(&e.name).count();
        if self.pane(o, &mut s, pane::RELATIONS, "References", Icon::Link) {
            o.info(&mut s, "References", &format!("{links}"), "");
            o.info(&mut s, "Direct users", &format!("{users}"), "");
            o.info(
                &mut s,
                "Aircraft users",
                &format!("{}", self.aircraft_users.len()),
                "",
            );
            if let Some([x, y, w, h]) = o.wide(&mut s, m::ROW_H) {
                o.canvas.styled(
                    x,
                    widgets::baseline(y, h, Style::Label),
                    &fit("Stored names in this LIB only.", w, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                );
            }
            if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                o.button_ex(
                    rect,
                    Btn::new("Show references").with_icon(Icon::Link),
                    Action::Dock(4),
                );
            }
        }
        o.panel_end(&mut s);
        if self.pane(o, &mut s, pane::ENTRY_ACTIONS, "Entry", Icon::Lib) {
            for (title, action) in [
                ("Open in Model", Action::Mode(Mode::Model)),
                ("Export entry", Action::File(FileAction::Export)),
                ("Replace entry", Action::File(FileAction::Replace)),
            ] {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(rect, Btn::new(title), action);
                }
            }
        }
        o.panel_end(&mut s);
        o.stack_end(s);
    }

    /// Raw BRF field table: name, type, value (NumberField), on-disk value
    /// and a reset button for changed operands. `full` is the Flight editor
    /// (with its own header and the group filter); otherwise the dock.
    fn fields_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32, full: bool) {
        use theme::{metric as m, space};
        use widgets::{baseline, Btn};
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
            let d = &mut o.canvas;
            d.rect(x, y + m::EDITOR_HEADER_H - 1, w, 1, c::GM_1000);
            let title = self
                .field_group
                .map_or("All fields", |g| grafting_ui::aspect_view(g).0);
            d.styled(
                x + space::SPACE_3,
                baseline(y, m::EDITOR_HEADER_H, Style::Strong),
                &fit(title, w - 2 * space::SPACE_3, Style::Strong),
                c::INK,
                Style::Strong,
            );
            top += m::EDITOR_HEADER_H;
        }
        let wide = w > 570;
        let typed = w > 440;
        let reset_w = m::ROW_H;
        let vx = x + if wide { w * 52 / 100 } else { w / 2 };
        let tx = x + w * 36 / 100;
        let ox = x + w * 78 / 100;
        let vw = if wide {
            ox - vx - space::SPACE_2
        } else {
            x + w - reset_w - 6 - vx
        };
        let d = &mut o.canvas;
        d.rect(x, top, w, m::ROW_H, c::GM_900);
        let head = baseline(top, m::ROW_H, Style::Section);
        d.styled(x + 10, head, "FIELD", c::INK_MUTED, Style::Section);
        if typed {
            d.styled(tx, head, "TYPE", c::INK_MUTED, Style::Section);
        }
        d.styled(vx, head, "VALUE", c::INK_MUTED, Style::Section);
        if wide {
            d.styled(ox, head, "ON DISK", c::INK_MUTED, Style::Section);
        }
        let Some(b) = &self.brf else {
            d.styled(
                x + space::SPACE_3,
                baseline(top + m::ROW_H + space::SPACE_2, m::ROW_H, Style::Label),
                &fit(
                    "Select a PT, JT, NT or OT definition to edit its fields.",
                    w - 2 * space::SPACE_3,
                    Style::Label,
                ),
                c::INK_MUTED,
                Style::Label,
            );
            return;
        };
        let rowh = m::ROW_H;
        let start = top + m::ROW_H;
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
            let pick = [x, yy, vx - x, rowh];
            let fill = if i == self.field_selected {
                c::AMBER_DEEP
            } else if o.over(pick) {
                c::GM_700
            } else if row % 2 == 0 {
                c::GM_800
            } else {
                c::GM_900
            };
            let d = &mut o.canvas;
            // The name and type pick the row; the value field sits beside it.
            d.rect(x, yy, vx - x, rowh, fill);
            d.rect(vx, yy, x + w - vx, rowh, fill);
            let base = baseline(yy, rowh, Style::Value);
            d.styled(
                x + 10,
                base,
                &fit(
                    &f.label,
                    if typed { tx - x - 20 } else { vx - x - 20 },
                    Style::Value,
                ),
                c::INK_MUTED,
                Style::Value,
            );
            if typed {
                let kind = if f.scaled {
                    format!("{} \u{b7} scaled", f.kind)
                } else {
                    f.kind.clone()
                };
                d.styled(
                    tx,
                    base,
                    &fit(&kind, vx - tx - 10, Style::Value),
                    c::INK_MUTED,
                    Style::Value,
                );
            }
            o.hit(pick, Action::PickField(i));
            self.field_control(o, [vx, yy, vw, rowh], i);
            let changed = self.field_changed(i);
            if wide {
                let old = self
                    .original_brf
                    .as_ref()
                    .and_then(|b| b.fields.get(i))
                    .map_or_else(|| "new".into(), |f| f.value.clone());
                o.canvas.styled(
                    ox,
                    base,
                    &fit(&old, x + w - reset_w - 8 - ox, Style::Value),
                    c::INK_MUTED,
                    Style::Value,
                );
            }
            if changed && self.original_brf.is_some() {
                o.button_on(
                    [x + w - reset_w - 2, yy, reset_w, rowh],
                    Btn::icon(Icon::Rotate).ghost(),
                    Action::ResetField(i),
                    fill,
                );
            }
        }
    }
    fn dock_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        use theme::{metric as m, space};
        use widgets::baseline;
        o.canvas.rect(x, y, w, h, c::GM_800);
        self.dock_header(o, x, y, w);
        let top = m::EDITOR_HEADER_H;
        if self.dock == 4 {
            self.references_layout(o, x, y + top, w, h - top);
            return;
        }
        if self.dock == 3 && self.context_model.is_some() {
            self.draw_model(o, x, y + top, w, h - top);
            let rect = [x, y + top, w, h - top];
            o.hit(rect, Action::PanelPick(rect));
            o.canvas.styled(
                x + space::SPACE_3,
                y + top + 16,
                &fit(
                    "Live model \u{b7} click selects panels, Shift adds \u{b7} MMB orbit \u{b7} wheel zoom",
                    w - 24,
                    Style::ValueSm,
                ),
                c::INK_MUTED,
                Style::ValueSm,
            );
        } else if self.dock == 0 && self.brf.is_some() {
            self.fields_layout(o, x, y + top, w, h - top, false);
        } else if self.dock == 1 || (self.dock == 0 && self.brf.is_none()) {
            o.canvas.rect(x, y + top, w, h - top, c::GM_950);
            let cell = text_width("00 ", Style::Value);
            let count = ((w - 88) / cell).clamp(4, 16) as usize;
            for (row, bytes) in self
                .data
                .chunks(count)
                .take(((h - top - space::SPACE_2) / m::ROW_H).max(0) as usize)
                .enumerate()
            {
                let yy = y + top + space::SPACE_1 + row as i32 * m::ROW_H;
                let base = baseline(yy, m::ROW_H, Style::Value);
                o.canvas.styled(
                    x + space::SPACE_3,
                    base,
                    &format!("{:06X}", row * count),
                    c::INK_MUTED,
                    Style::Value,
                );
                let mut value = String::new();
                for b in bytes {
                    value.push_str(&format!("{b:02X} "));
                }
                o.canvas
                    .styled(x + 76, base, value.trim_end(), c::INK, Style::Value);
            }
        } else {
            // Details: what the entry is, the last message, where it lives.
            let mut yy = y + top + space::SPACE_2;
            let col = 96.min(w / 3);
            let error = self.status.starts_with("Error:");
            let palette = self.palette_label();
            for (label, value) in [
                ("Entry", self.name()),
                ("Decoded", self.detail.as_str()),
                ("LIB", self.path.as_str()),
                ("Palette", palette.as_str()),
            ] {
                o.canvas.styled(
                    x + space::SPACE_3 + col - text_width(label, Style::Label),
                    baseline(yy, m::ROW_H, Style::Label),
                    label,
                    c::INK_MUTED,
                    Style::Label,
                );
                o.canvas.styled(
                    x + space::SPACE_3 + col + space::SPACE_2,
                    baseline(yy, m::ROW_H, Style::Value),
                    &fit(
                        value,
                        w - col - 2 * space::SPACE_3 - space::SPACE_2,
                        Style::Value,
                    ),
                    c::INK,
                    Style::Value,
                );
                yy += m::ROW_H + space::SPACE_1;
            }
            if !self.status.is_empty() && yy + 30 < y + h {
                widgets::notice(
                    &mut o.canvas,
                    x + space::SPACE_3,
                    yy,
                    w - 2 * space::SPACE_3,
                    if error {
                        widgets::Tone::Danger
                    } else {
                        widgets::Tone::Neutral
                    },
                    self.status.strip_prefix("Error: ").unwrap_or(&self.status),
                );
            }
        }
    }

    /// Dialog surface (no shadow): `gm-800` with a `line-strong` keyline, a
    /// 36px header with the title in `title` style. Returns the body rect.
    pub(super) fn dialog_frame(&self, o: &mut Layout, rect: [i32; 4], title: &str) -> [i32; 4] {
        use theme::space;
        use widgets::{baseline, notched};
        let [x, y, w, h] = rect;
        let d = &mut o.canvas;
        notched(d, rect, Some(c::GM_800), Some(c::LINE_STRONG));
        d.rect(x + 1, y + DIALOG_HEAD - 1, w - 2, 1, c::GM_1000);
        d.styled(
            x + space::SPACE_4,
            baseline(y, DIALOG_HEAD, Style::Title),
            &fit(title, w - 2 * space::SPACE_4, Style::Title),
            c::INK,
            Style::Title,
        );
        [
            x + space::SPACE_4,
            y + DIALOG_HEAD + space::SPACE_3,
            w - 2 * space::SPACE_4,
            h - DIALOG_HEAD - space::SPACE_3 - space::SPACE_4,
        ]
    }
    /// Sunken text input with a `focus` border, the end of `value` and a caret.
    pub(super) fn dialog_input(&self, o: &mut Layout, rect: [i32; 4], value: &str, caret: Caret) {
        use widgets::{baseline, notched};
        let [_, y, _, h] = rect;
        notched(&mut o.canvas, rect, Some(c::GM_950), Some(c::FOCUS));
        let (tx, room) = text_ui::prompt_area(rect);
        let base = baseline(y, h, Style::Value);
        text_ui::draw(&mut o.canvas, tx, base, y, h, room, value, Some(caret));
        o.hit(rect, Action::PromptText);
    }
    /// Dialog footer buttons, right-aligned at the bottom of `rect`: a ghost
    /// Cancel (when given) and the dialog's action; `left` buttons start at
    /// the left edge.
    pub(super) fn dialog_actions(
        &self,
        o: &mut Layout,
        rect: [i32; 4],
        left: &[(&str, Action)],
        cancel: Option<&str>,
        main: Option<widgets::Btn>,
        action: Action,
    ) {
        use theme::{metric as m, space};
        use widgets::Btn;
        let [x, y, w, h] = rect;
        let by = y + h - space::SPACE_4 - m::BUTTON_H;
        let mut lx = x + space::SPACE_4;
        for (title, a) in left {
            let b = Btn::new(title);
            let bw = b.width();
            o.button_ex([lx, by, bw, m::BUTTON_H], b, *a);
            lx += bw + space::SPACE_2;
        }
        let mut rx = x + w - space::SPACE_4;
        if let Some(b) = main {
            let bw = b.width().max(80);
            rx -= bw;
            o.button_ex([rx, by, bw, m::BUTTON_H], b, action);
            rx -= space::SPACE_2;
        }
        if let Some(title) = cancel {
            let b = Btn::new(title).ghost();
            let bw = b.width();
            o.button_ex([rx - bw, by, bw, m::BUTTON_H], b, Action::Cancel);
        }
    }
    fn prompt_layout(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, notice, Btn, Tone};
        let p = self.prompt.as_ref().unwrap();
        let w = (self.width - 48).min(640);
        // The loader-limit notice can run to several lines.
        let limits = match (&p.kind, &self.save_check) {
            (PromptKind::GameFolder, Some(c)) => Some(format!(
                "{} FA crashes or corrupts memory at startup past these limits. Save anyway only if you will move files out of this folder before playing.",
                c.problems.join(" ")
            )),
            _ => None,
        };
        // Clone texture for selected faces says up front when it is refused;
        // any error wraps in a notice below the hint.
        let clone = matches!(p.kind, PromptKind::FaceClone);
        let refused = clone && self.ed.texture_refusal.is_some();
        let problem = if clone {
            self.texture_problem()
        } else {
            self.status.strip_prefix("Error: ").map(String::from)
        };
        let inner = w - 2 * space::SPACE_4;
        let h = match (&limits, &problem) {
            (Some(text), _) => (DIALOG_HEAD
                + widgets::notice_height(inner, text)
                + m::BUTTON_H
                + 3 * space::SPACE_4)
                .clamp(196, self.height - 16),
            (None, Some(text)) => (196 + widgets::notice_height(inner, text) - m::ROW_H
                + space::SPACE_1)
                .clamp(196, self.height - 16),
            (None, None) => 196,
        };
        let rect = [(self.width - w) / 2, self.height / 2 - h / 2, w, h];
        o.hits.clear();
        let body = self.dialog_frame(o, rect, &p.title);
        let [bx, by, bw, _] = body;
        if let Some(text) = &limits {
            notice(&mut o.canvas, bx, by, bw, Tone::Danger, text);
            self.dialog_actions(
                o,
                rect,
                &[],
                Some("Cancel"),
                Some(Btn::new("Save anyway").with_icon(Icon::Warning).danger()),
                Action::ConfirmSave,
            );
            return;
        }
        if matches!(p.kind, PromptKind::Discard | PromptKind::CloseLibrary) {
            notice(
                &mut o.canvas,
                bx,
                by,
                bw,
                Tone::Warn,
                "Unsaved edits will be discarded. Files on disk are unchanged; Cancel returns to the editor.",
            );
            self.dialog_actions(
                o,
                rect,
                &[],
                Some("Cancel"),
                Some(Btn::new("Discard changes").with_icon(Icon::Close).danger()),
                Action::DiscardChanges,
            );
            return;
        }
        o.canvas.styled(
            bx,
            baseline(by, m::ROW_H, Style::Label),
            if matches!(p.kind, PromptKind::File(_)) {
                "File path"
            } else {
                "Value"
            },
            c::INK_MUTED,
            Style::Label,
        );
        self.dialog_input(o, [bx, by + m::ROW_H, bw, 26], &p.value, p.caret);
        let hint = if let PromptKind::Transform(op) = p.kind {
            let lock = match (p.axis, op) {
                (0..=2, _) => format!("Axis {}", ['X', 'Y', 'Z'][p.axis]),
                (_, 'g') => "Free: one value moves X, or X Y Z".into(),
                (_, 'r') => format!("View axis {}", ['X', 'Y', 'Z'][self.view_axis()]),
                _ => "Uniform".into(),
            };
            format!("{lock}. X, Y or Z toggles the axis lock; integer values.")
        } else if matches!(p.kind, PromptKind::StationMove) {
            format!(
                "Axis {}. X, Y or Z picks the axis; integer offset.",
                ['X', 'Y', 'Z'][p.axis.min(2)]
            )
        } else {
            "Enter applies, Esc cancels, Ctrl+A clears.".into()
        };
        let hy = by + m::ROW_H + 26 + space::SPACE_1;
        o.canvas.styled(
            bx,
            baseline(hy, m::ROW_H, Style::Label),
            &fit(&hint, bw, Style::Label),
            c::INK_MUTED,
            Style::Label,
        );
        if let Some(text) = &problem {
            notice(
                &mut o.canvas,
                bx,
                hy + m::ROW_H + space::SPACE_1,
                bw,
                Tone::Danger,
                text,
            );
        }
        let left: &[(&str, Action)] = match p.kind {
            PromptKind::CloneTitle if !self.identity.in_place => &[
                ("Add source LIB", Action::File(FileAction::CloneSource)),
                ("Back", Action::CloneBack),
            ],
            PromptKind::CloneTitle | PromptKind::CloneShort => &[("Back", Action::CloneBack)],
            _ => &[],
        };
        self.dialog_actions(
            o,
            rect,
            left,
            Some("Cancel"),
            Some(
                Btn::new(if matches!(p.kind, PromptKind::File(FileAction::Open)) {
                    "Open LIB"
                } else {
                    "Apply"
                })
                .primary()
                .enabled(!refused),
            ),
            Action::Apply,
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
            let (x, y) = (hit.rect[0] + hit.rect[2] / 2, hit.rect[1] + hit.rect[3] / 2);
            // Outliner entries select on release.
            app.click(x, y, 1, true);
            app.click(x, y, 1, false);
        }
        self.smoke_widgets();
        self.smoke_chrome();
        self.smoke_about();
        self.smoke_outliner();
        self.smoke_inspector();
        self.smoke_hit_geometry();
        self.smoke_dependencies();
        self.smoke_graft();
        self.smoke_libraries();
        self.smoke_text_editing();
        self.smoke_identity();
        self.smoke_library_moves();
        self.smoke_material_tools();
        self.smoke_advanced_tools();
        self.smoke_render_and_brush();
        self.smoke_paint_tools();
        self.smoke_originals();
        self.smoke_object_tools();
        self.smoke_edit_mode();
        self.smoke_gizmo();
        self.smoke_add_vertex();
        self.smoke_parts_panel();
        self.smoke_face_textures();
        self.smoke_markings();
        self.smoke_texture_refusals();
        self.smoke_panels();
        self.smoke_replace();
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
        // A click without a drag on the property NumberField types a value.
        let t = widgets::NumberTarget::Field(weight);
        click(self, |a| matches!(a, Action::Number(n) if n == t));
        self.click(self.mouse[0], self.mouse[1], 1, false);
        assert!(self.prompt.is_some(), "Click types a value");
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
            .any(|d| matches!(d,Draw::Text(_,_,s,color,_) if s=="12,000"&&*color==c::AMBER.0)));
        click(self, |a| matches!(a, Action::Menu(1)));
        click(self, |a| matches!(a, Action::Undo));
        assert!(!self.doc.dirty());
        click(self, |a| matches!(a, Action::Mode(Mode::Browse)));
        assert!(self.browser_entries().contains(&self.selected));
        click(self, |a| matches!(a, Action::Mode(Mode::Package)));
        click(self, |a| matches!(a, Action::Validate));
        // The demo DEMO.PIC keeps its embedded palette: its texture-layout
        // error is the only one.
        assert_eq!(self.validation.as_ref().unwrap().errors, 1);
        assert_eq!(self.texture_layout_errors(), 1);
        self.file_prompt(FileAction::Open);
        click(self, |a| matches!(a, Action::Cancel));
        assert!(self.prompt.is_none());
        self.smoke_toolbar_and_menus();
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
/// Hit regions drawn as lines over the viewport (station markers, vertex
/// handles) and the menu surface, which sits over the menu's own items.
fn free_hit(a: Action) -> bool {
    matches!(
        a,
        Action::HardpointSelect(_) | Action::MeshVertex(_) | Action::Gizmo(_) | Action::MenuPad
    )
}
impl App {
    /// Every hit region lies inside the control it draws (rect fills within
    /// 1px of the region reach all four edges, hovered when needed), stays in
    /// the window, and no two regions overlap.
    pub(super) fn smoke_geometry(&mut self, state: &str) {
        let (w, h) = (self.width, self.height);
        let hits: Vec<Hit> = self.layout().hits;
        let covered = |draw: &Canvas, [x, y, rw, rh]: [i32; 4]| {
            let mut u = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
            for d in &draw.commands {
                let r = match d {
                    Draw::Rect(a, b, c, d, _) => [*a, *b, a + c, b + d],
                    Draw::Bitmap(a, b, c, d, _) => [*a, *b, a + *c as i32, b + *d as i32],
                    _ => continue,
                };
                if r[0] >= x - 1 && r[1] >= y - 1 && r[2] <= x + rw + 1 && r[3] <= y + rh + 1 {
                    u = [
                        u[0].min(r[0]),
                        u[1].min(r[1]),
                        u[2].max(r[2]),
                        u[3].max(r[3]),
                    ];
                }
            }
            u[0] <= x + 1 && u[1] <= y + 1 && u[2] >= x + rw - 1 && u[3] >= y + rh - 1
        };
        let plain = self.draw();
        for (i, hit) in hits.iter().enumerate() {
            let [x, y, rw, rh] = hit.rect;
            assert!(
                x >= 0 && y >= 0 && x + rw <= w && y + rh <= h && rw > 0 && rh > 0,
                "{state} {w}x{h}: hit region {:?} outside the window",
                hit.rect
            );
            if free_hit(hit.action) {
                continue;
            }
            if !covered(&plain, hit.rect) {
                let mouse = self.mouse;
                self.mouse = [x + rw / 2, y + rh / 2];
                // Controls beside an open menu show hover once it closes.
                let menu = self.menu;
                if !self.in_open_menu(self.mouse[0], self.mouse[1]) {
                    self.menu = None;
                }
                let hovered = self.draw();
                self.menu = menu;
                self.mouse = mouse;
                assert!(
                    covered(&hovered, hit.rect),
                    "{state} {w}x{h}: hit region {:?} exceeds its drawn control",
                    hit.rect
                );
            }
            for other in &hits[i + 1..] {
                let [ox, oy, ow, oh] = other.rect;
                assert!(
                    free_hit(other.action)
                        || x >= ox + ow
                        || ox >= x + rw
                        || y >= oy + oh
                        || oy >= y + rh,
                    "{state} {w}x{h}: hit regions {:?} and {:?} overlap",
                    hit.rect,
                    other.rect
                );
            }
        }
    }
    /// Hit-region geometry for every restyled editor and dialog at both
    /// window sizes.
    pub(super) fn smoke_hit_geometry(&mut self) {
        for (w, h) in [(1280, 800), (800, 600)] {
            for state in 0..31 {
                self.libraries.clear();
                self.demo();
                self.width = w;
                self.height = h;
                self.mouse = [0, 0];
                self.textured = false;
                let pic = self.doc.archive.find("DEMO.PIC").unwrap();
                let name = match state {
                    0 => {
                        self.select_entry(0);
                        "model"
                    }
                    1 => {
                        self.select_entry(1);
                        self.mode = Mode::Model;
                        "model PT"
                    }
                    2 => {
                        self.select_entry(1);
                        self.mode = Mode::Browse;
                        "browse"
                    }
                    3 => {
                        self.select_entry(1);
                        self.act(Action::Mode(Mode::Properties));
                        "envelope"
                    }
                    4 => {
                        self.select_entry(1);
                        self.mode = Mode::Properties;
                        "all fields"
                    }
                    5 => {
                        self.select_entry(1);
                        self.pin_donor().unwrap();
                        self.graft_mask = !0;
                        self.refresh_graft();
                        "graft"
                    }
                    6 => {
                        self.act(Action::Validate);
                        "package"
                    }
                    7..=9 => {
                        self.select_entry(pic);
                        self.media_tab = state as u8 - 7;
                        "paint"
                    }
                    10 => {
                        self.select_entry(1);
                        self.mode = Mode::Model;
                        self.act(Action::Hardpoints);
                        "hardpoints"
                    }
                    11 => {
                        self.select_entry(0);
                        self.dock = 4;
                        "references"
                    }
                    12 => {
                        self.select_entry(1);
                        self.number_prompt(widgets::NumberTarget::Field(0));
                        "prompt"
                    }
                    13 => {
                        self.doc.mark_unsaved();
                        self.close();
                        "discard"
                    }
                    14 => {
                        self.select_entry(0);
                        self.base_color_prompt(false);
                        "base color"
                    }
                    15 => {
                        self.file_prompt(FileAction::Open);
                        "browser"
                    }
                    16 => {
                        self.select_entry(0);
                        self.mode = Mode::Model;
                        self.act(Action::ModelPaint);
                        "model paint"
                    }
                    17 => {
                        self.select_entry(0);
                        self.dock = 1;
                        self.mode = Mode::Model;
                        self.act(Action::Panel(widgets::pane::GEOMETRY));
                        "collapsed panel"
                    }
                    18 => {
                        self.select_entry(0);
                        self.copy_resource().unwrap();
                        let mut bytes = picture::demo();
                        bytes.push(0);
                        let mut target = Archive::empty();
                        target.entries.push(Entry::new("DEMO.PIC", bytes).unwrap());
                        self.install_library(Document::new(target), "TARGET.LIB".into())
                            .unwrap();
                        self.refresh();
                        self.paste_resources().unwrap();
                        "transfer review"
                    }
                    25 => {
                        // The outliner context menu with Copy to open.
                        let source = self.library_id;
                        self.install_library(Document::new(Archive::empty()), "TARGET.LIB".into())
                            .unwrap();
                        self.refresh();
                        self.switch_library(source).unwrap();
                        self.mode = Mode::Model;
                        self.scroll = 0;
                        let row = self.chrome_hit(&|a| matches!(a, Action::Entry(_))).unwrap();
                        self.pointer(row[0] + 70, row[1] + 8, 3, true, false);
                        let sub = self
                            .chrome_hit(&|a| matches!(a, Action::ContextSub(1)))
                            .unwrap();
                        self.motion(sub[0] + 20, sub[1] + 8, false);
                        assert_eq!(self.menu, Some(chrome::MENU_CONTEXT));
                        assert_eq!(self.context.sub, 1);
                        "context menu"
                    }
                    26 => {
                        self.select_entry(1);
                        self.rename_aircraft_prompt();
                        self.rename_review("NEO").unwrap();
                        "rename review"
                    }
                    27 | 28 => {
                        self.select_entry(1);
                        self.begin_duplicate();
                        self.variant_id = "TWIN".into();
                        if state == 27 {
                            self.duplicate_review_prompt().unwrap();
                            "duplicate review"
                        } else {
                            self.clone_step(2);
                            "duplicate step"
                        }
                    }
                    29 => {
                        self.select_entry(1);
                        self.mode = Mode::Model;
                        self.identity_commit(false, "Changed").unwrap();
                        "identity panel changed"
                    }
                    20..=24 => {
                        self.doc
                            .replace(0, hangar_core::shape_testkit::demo_parts())
                            .unwrap();
                        self.doc.mark_saved();
                        self.select_entry(0);
                        self.mode = Mode::Model;
                        self.pose_preset(0);
                        if state >= 22 {
                            self.open_animation();
                            self.pick_part(0);
                        } else {
                            self.act(Action::MeshMode);
                            self.pick_part(0);
                            self.select_mode(state == 20);
                        }
                        match state {
                            20 => {
                                self.textured = true;
                                self.ed.mesh_refusal = Some("A refusal reason from core".into());
                                "edit faces"
                            }
                            21 => "edit vertices",
                            22 => "parts",
                            23 => {
                                self.smoke_find(&|x| matches!(x, Action::PartMenu(3)));
                                "part settings"
                            }
                            _ => {
                                self.act(Action::PartValue(3, 0));
                                "non-retail part setting"
                            }
                        }
                    }
                    30 => {
                        self.about_open();
                        "about"
                    }
                    _ => {
                        self.select_entry(1);
                        self.begin_clone();
                        self.variant_id = "NEWJET".into();
                        self.clone_draft = Some(Box::new(self.build_clone().unwrap()));
                        self.prompt = Some(Prompt {
                            kind: PromptKind::CloneReview,
                            title: "Review".into(),
                            value: String::new(),
                            axis: 0,
                            caret: super::Caret::END,
                        });
                        "export review"
                    }
                };
                self.smoke_geometry(name);
                self.menu = None;
                self.graft_donor = None;
                self.graft_mask = 0;
                self.transfer_plan = None;
                self.clone_draft = None;
                self.prompt = None;
                self.browser = None;
                self.identity_cancel();
                self.dock = 0;
                self.panels = 0;
                self.doc.mark_saved();
            }
        }
        self.width = 1280;
        self.height = 800;
        self.demo();
    }
    /// Property panels through their hit regions: collapse (and Ctrl+click),
    /// wheel scrolling with the scroll cue, a BRF NumberField scrub,
    /// Backspace reset to the file on disk, and undo.
    fn smoke_inspector(&mut self) {
        use widgets::{pane, NumberTarget};
        let find = |app: &App, predicate: &dyn Fn(Action) -> bool| app.chrome_hit(predicate);
        let press = |app: &mut App, r: [i32; 4]| app.chrome_click(r);
        for (w, h) in [(1280, 800), (800, 600)] {
            self.demo();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            // Identity collapsed: the envelope rows stay in view at 800x600.
            self.panels = 1 << pane::IDENTITY;
            self.select_entry(1);
            let original = self.doc.archive.bytes().unwrap();
            let fields = &self.brf.as_ref().unwrap().fields;
            let speed = fields
                .iter()
                .position(|f| f.label == "object._maxSpeed")
                .unwrap();
            let weight = fields
                .iter()
                .position(|f| f.label == "object.weight")
                .unwrap();
            let neg_g = fields
                .iter()
                .position(|f| f.label == "plane.negGLimit")
                .unwrap();
            let speed_field =
                |a: Action| matches!(a, Action::Number(NumberTarget::Field(i)) if i == speed);
            // Reviewed engine fields show their stored unit beside the value.
            assert!(find(
                self,
                &|a| matches!(a, Action::Number(NumberTarget::Field(i)) if i == neg_g)
            )
            .is_some());
            assert!(self
                .draw()
                .commands
                .iter()
                .any(|d| matches!(d, Draw::Text(_, _, s, _, _) if s == "1/256 s")));
            // Collapse and expand Envelope from its header.
            let header = find(self, &|a| matches!(a, Action::Panel(pane::ENVELOPE))).unwrap();
            assert_eq!(header[3], theme::metric::PANEL_HEADER_H);
            assert!(find(self, &speed_field).is_some());
            press(self, header);
            assert!(self.panel_closed(pane::ENVELOPE));
            assert!(
                find(self, &speed_field).is_none(),
                "Collapsed rows are hidden"
            );
            press(self, header);
            assert!(!self.panel_closed(pane::ENVELOPE));
            // Ctrl+click keeps only that panel open.
            let weights = find(self, &|a| matches!(a, Action::Panel(pane::WEIGHTS))).unwrap();
            self.modifiers(true);
            press(self, weights);
            self.modifiers(false);
            assert!(!self.panel_closed(pane::WEIGHTS) && self.panel_closed(pane::ENVELOPE));
            self.panels = 1 << pane::IDENTITY;
            // Wheel over the panels scrolls them; the thumb shows the position.
            let max = self.layout().inspector_max;
            assert!(max > 0, "Panels overflow at {w}x{h}");
            self.mouse = [self.right() + 40, self.height / 2];
            self.wheel(-1);
            assert!(self.inspector_scroll > 0);
            let moved = find(self, &|a| matches!(a, Action::Panel(pane::ENVELOPE)));
            assert!(moved.is_none_or(|r| r[1] < header[1]), "Panels scroll up");
            assert!(self.draw().commands.iter().any(
                |d| matches!(d, Draw::Rect(x, _, 3, _, color) if *x > self.right() && *color == c::GM_600.0)
            ));
            self.wheel(-1000);
            assert_eq!(self.inspector_scroll, max);
            self.wheel(1000);
            assert_eq!(self.inspector_scroll, 0);
            // Scrub the weight NumberField 20px right: +10, one undo step.
            let t = NumberTarget::Field(weight);
            let disk = self.number_spec(t).unwrap().value;
            let field = find(self, &|a| matches!(a, Action::Number(n) if n == t))
                .expect("Weight NumberField");
            let (x, y) = (field[0] + field[2] / 2, field[1] + field[3] / 2);
            self.smoke_drag(field, 20);
            assert_eq!(self.number_spec(t).unwrap().value, disk + 10);
            assert_eq!(self.doc.changed_count(), 1);
            let shown = widgets::format_number(disk + 10, 0, true);
            assert!(self.draw().commands.iter().any(
                |d| matches!(d, Draw::Text(_, _, s, color, _) if *s == shown && *color == c::AMBER.0)
            ));
            // Backspace over the field restores the operand on disk.
            self.motion(x, y, false);
            self.key(Key::Backspace, false, false);
            assert_eq!(self.number_spec(t).unwrap().value, disk);
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
            self.doc.undo();
            self.refresh();
            assert_eq!(self.number_spec(t).unwrap().value, disk + 10);
            self.doc.undo();
            self.refresh();
            assert_eq!(self.doc.archive.bytes().unwrap(), original);
            assert!(!self.doc.dirty());
            // The link button shows references; the Shape Select opens a shape.
            let link = find(self, &|a| matches!(a, Action::Dock(4))).unwrap();
            press(self, link);
            assert_eq!(self.dock, 4);
            let select = find(self, &|a| matches!(a, Action::Menu(chrome::MENU_SHAPE))).unwrap();
            assert_eq!(select, self.shape_select_rect());
            press(self, select);
            assert_eq!(self.menu, Some(chrome::MENU_SHAPE));
            let menu = self.open_menu_rect().unwrap();
            assert!(menu[0] + menu[2] <= w && menu[1] + menu[3] <= h);
            let item = find(self, &|a| matches!(a, Action::Entry(0))).unwrap();
            assert!(item[1] >= menu[1], "Shape item in the dropdown");
            press(self, item);
            assert_eq!(self.selected, 0, "Shape Select opens the SH");
            assert!(self.menu.is_none());
        }
        self.width = 1280;
        self.height = 800;
        self.demo();
    }
    /// Outliner header and rows through their hit regions: type filter,
    /// group collapse, the filter field, hover, active vs selected rows and
    /// the stored-originals group below Images.
    fn smoke_outliner(&mut self) {
        let hit = |app: &App, predicate: &dyn Fn(Action) -> bool| {
            app.layout()
                .hits
                .into_iter()
                .find(|h| predicate(h.action) && h.rect[0] < app.left())
                .map(|h| h.rect)
                .expect("Outliner control missing")
        };
        let press = |app: &mut App, r: [i32; 4]| app.chrome_click(r);
        for (w, h) in [(1280, 800), (800, 600)] {
            self.demo();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            self.select_entry(1);
            let all = self.library_rows().len();
            // Type filter: Shapes only, then back to every group.
            let shapes = hit(self, &|a| matches!(a, Action::TypeFilter(1)));
            press(self, shapes);
            assert_eq!(self.type_filter, Some(1));
            assert!(self
                .library_rows()
                .iter()
                .all(|(_, cat, _)| cat.is_none_or(|c| c == 1)));
            assert!(self.browser_entries().iter().all(|i| {
                category_of(&self.doc.archive.entries[*i].name) == 1
                    || self.category.is_some_and(|c| c != 1)
            }));
            press(self, shapes);
            assert_eq!(self.type_filter, None);
            assert_eq!(self.library_rows().len(), all);
            // Group rows collapse and expand; their count and badge are drawn.
            let group = hit(self, &|a| matches!(a, Action::Category(1)));
            assert_eq!(group[3], theme::metric::ROW_H);
            assert_eq!(group[1], self.tree_start() + 3 * theme::metric::ROW_H);
            press(self, group);
            assert!(self.collapsed[1] && self.library_rows().len() < all);
            press(self, group);
            assert!(!self.collapsed[1]);
            assert!(self
                .draw()
                .commands
                .iter()
                .any(|d| matches!(d, Draw::Text(_, _, s, _, Style::ValueSm) if s == "6")));
            // Active entry: amber-bright text; its linked shape is selected.
            let fills = |app: &App, entry: usize| {
                let r = hit(app, &|a| matches!(a, Action::Entry(i) if i == entry));
                app.draw()
                    .commands
                    .into_iter()
                    .filter_map(|d| match d {
                        Draw::Rect(x, y, w, h, c) if [x, y, w, h] == r => Some(c),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            self.mouse = [0, 0];
            assert!(fills(self, 1).contains(&c::AMBER_DEEP.0));
            assert!(
                fills(self, 0).contains(&c::AMBER_DEEP.0),
                "Linked SH selected"
            );
            assert!(self.draw().commands.iter().any(|d| matches!(d,
                Draw::Text(_, _, s, color, _) if s == "DEMO.PT" && *color == c::AMBER_BRIGHT.0)));
            // Hover is gm-700; zebra rows alternate gm-800 / gm-900.
            let row = hit(self, &|a| matches!(a, Action::Entry(3)));
            self.mouse = [row[0] + 4, row[1] + 4];
            assert!(fills(self, 3).contains(&c::GM_700.0));
            self.mouse = [0, 0];
            assert!(fills(self, 3).contains(&c::GM_800.0) || fills(self, 3).contains(&c::GM_900.0));
            // The filter field takes focus, shows ink text with a caret.
            let field = hit(self, &|a| matches!(a, Action::Filter));
            press(self, field);
            assert!(self.filter_focus);
            for ch in "_A".chars() {
                self.key(Key::Char(ch), false, false);
            }
            assert!(self.draw().commands.iter().any(|d| matches!(d,
                Draw::Text(_, _, s, color, _) if s == "_A" && *color == c::INK.0)));
            assert!(
                self.library_rows()
                    .iter()
                    .all(|(_, _, e)| e
                        .is_none_or(|i| self.doc.archive.entries[i].name.contains("_A")))
            );
            self.key(Key::Escape, false, false);
            self.filter.clear();
            // Stored originals list right below Images.
            self.doc.import("DEMO.ORG", picture::demo()).unwrap();
            self.refresh();
            let cats: Vec<usize> = self
                .library_rows()
                .iter()
                .filter(|(_, _, e)| e.is_none())
                .filter_map(|(_, c, _)| *c)
                .collect();
            let images = cats.iter().position(|c| *c == 2).unwrap();
            assert_eq!(cats[images + 1], 9, "Original textures follow Images");
            self.doc.undo();
            self.refresh();
        }
        self.width = 1280;
        self.height = 800;
        self.demo();
    }
    /// Select tool, shading toggle, menu padding and hover under overlays.
    fn smoke_toolbar_and_menus(&mut self) {
        let find = |app: &App, predicate: &dyn Fn(Action) -> bool| app.chrome_hit(predicate);
        self.width = 1280;
        self.height = 800;
        self.select_entry(0);
        self.mode = Mode::Model;
        self.model_paint = true;
        let tool = find(self, &|a| matches!(a, Action::SelectTool)).expect("Select tool");
        self.click(tool[0] + 4, tool[1] + 4, 1, true);
        assert!(!self.model_paint, "Select tool leaves paint mode");
        let (zoom, pan) = (self.zoom, self.pan);
        // Shading is one segmented control: each member selects its mode.
        for (n, textured, flat) in [(1u8, true, true), (2, true, false), (0, false, false)] {
            let seg = find(self, &|a| matches!(a, Action::Shading(m) if m == n)).expect("Shading");
            self.click(seg[0] + seg[2] / 2, seg[1] + seg[3] / 2, 1, true);
            self.click(seg[0] + seg[2] / 2, seg[1] + seg[3] / 2, 1, false);
            assert_eq!((self.textured, self.flat), (textured, flat), "Shading {n}");
        }
        self.hp_visible = false;
        let button = find(self, &|a| matches!(a, Action::HardpointVisibility)).unwrap();
        self.mouse = [button[0] + 4, button[1] + 4];
        // Hovered buttons fill gm-600 inside their notched keyline.
        let face = [button[0] + 1, button[1] + 1, button[2] - 2, button[3] - 2];
        let hovered = |app: &App| {
            app.draw().commands.iter().any(|d| {
                matches!(d, Draw::Rect(x, y, w, h, color)
                    if [*x, *y, *w, *h] == face && *color == c::GM_600.0)
            })
        };
        assert!(hovered(self));
        self.act(Action::Menu(0));
        assert!(!hovered(self), "No hover under an open menu");
        // Padding inside the drawn menu consumes the click and keeps the menu open.
        let menu = find(self, &|a| matches!(a, Action::MenuPad)).unwrap();
        self.click(menu[0] + 1, menu[1] + 1, 1, true);
        assert_eq!(self.menu, Some(0));
        self.click(menu[0] + menu[2] - 2, menu[1] + menu[3] - 2, 1, true);
        assert_eq!(self.menu, Some(0));
        assert!(self.zoom == zoom && self.pan == pan && self.mode == Mode::Model);
        self.key(Key::Escape, false, false);
        assert!(self.menu.is_none());
        assert_eq!(count(1, "reference", "references"), "1 reference");
        assert_eq!(
            count(2, "aircraft user", "aircraft users"),
            "2 aircraft users"
        );
    }
}
