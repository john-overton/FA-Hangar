use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use hangar_core::{
    archive::{Archive, Entry},
    audio::Pcm,
    authoring::{self, Variant},
    brf::Brf,
    document::Document,
    model::{self, Model, Transform},
    picture::{self, Pic},
    Result,
};
#[path = "../../../tore-hangar-design/tokens/theme.rs"]
#[allow(dead_code)]
#[rustfmt::skip]
pub mod theme;
use theme::{color as c, Rgb};
#[derive(Clone, Debug)]
pub enum Draw {
    Rect(i32, i32, i32, i32, u32),
    Line(i32, i32, i32, i32, u32),
    Text(i32, i32, String, u32),
    Label(i32, i32, String, u32),
    Bitmap(i32, i32, usize, usize, Vec<u32>),
}
pub struct Canvas {
    pub commands: Vec<Draw>,
}
impl Canvas {
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Rgb) {
        if w > 0 && h > 0 {
            self.commands.push(Draw::Rect(x, y, w, h, color.0));
        }
    }
    pub fn line(&mut self, x: i32, y: i32, a: i32, b: i32, color: Rgb) {
        self.commands.push(Draw::Line(x, y, a, b, color.0));
    }
    pub fn label(&mut self, x: i32, y: i32, s: &str, color: Rgb) {
        self.commands.push(Draw::Label(x, y, s.into(), color.0));
    }
    pub fn text(&mut self, x: i32, y: i32, s: &str, color: Rgb) {
        self.commands.push(Draw::Text(x, y, s.into(), color.0));
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Browse,
    Model,
    Properties,
    Graft,
    Package,
    Media,
}
#[derive(Clone, Copy)]
pub enum FileAction {
    Open,
    Save,
    Import,
    Replace,
    Export,
    Obj,
    Graft,
    Variant,
    VariantSh,
    CloneSource,
    Png,
    Wav,
    Palette,
    ReferenceSource,
    Report,
    GraftLibrary,
    Decal,
}
#[derive(Clone)]
enum PromptKind {
    File(FileAction),
    Field(usize),
    Transform(char),
    Discard,
    VariantId,
    VariantTitle,
    VariantReview,
    CloneId,
    CloneTitle,
    CloneReview,
    Recolor,
    BaseColor(bool),
    MeshMove,
    Isolate,
    CloseLibrary,
    ResourceName(bool),
    StationValue(usize),
    StationMove,
    PaletteIndex,
    PaletteColor,
    UvMaterial,
    FamilyTexture,
    DecalText,
    DecalInk,
    DecalSetting(u8),
    DecalLibrary,
    TransferReview,
}
struct Prompt {
    kind: PromptKind,
    title: String,
    value: String,
    axis: usize,
}
#[derive(Clone, Copy)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Home,
    Tab,
    F1,
    Num(u8),
}
#[derive(Clone)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub directory: bool,
}
struct Stroke {
    entry: usize,
    name: String,
    generated: Option<Box<mesh_ui::PanelPlan>>,
    bytes: Vec<u8>,
    pic: Pic,
    last: Option<(usize, usize)>,
}
struct Browser {
    folder: String,
    files: Vec<FileItem>,
    scroll: usize,
}
pub struct App {
    pub width: i32,
    pub height: i32,
    pub doc: Document,
    pub path: String,
    pub status: String,
    pub selected: usize,
    hp_context: Option<Box<hardpoint_ui::Context>>,
    hp_selected: usize,
    hp_tool: bool,
    mesh_edit: bool,
    mesh_vertices: Vec<usize>,
    mesh_drag: Option<Box<mesh_ui::MeshDrag>>,
    hp_visible: bool,
    hp_drag: Option<hardpoint_ui::Drag>,
    media_tab: u8,
    decal_image: Option<hangar_core::decal::Image>,
    decal_name: String,
    decal_paths: Vec<String>,
    decal_preset: usize,
    decal_text: String,
    decal_is_text: bool,
    decal_ink: Option<u8>,
    decal_placement: hangar_core::decal::Placement,
    decal_draft: Option<Box<material_ui::DecalDraft>>,
    decal_active: bool,
    decal_dragging: bool,
    libraries: Vec<libraries_ui::Library>,
    library_id: u64,
    file_backed: bool,
    external_model: Option<(u64, usize)>,
    next_library_id: u64,
    clipboard: Option<libraries_ui::Clipboard>,
    transfer_plan: Option<hangar_core::resource_ops::Plan>,
    transfer_scroll: usize,
    transfer_note: usize,
    transfer_is_copy: bool,
    include_dependencies: bool,
    resource_drag: Option<(u64, usize, [i32; 2])>,
    scroll: usize,
    field_scroll: usize,
    field_selected: usize,
    field_group: Option<hangar_core::definition::Aspect>,
    envelope_selected: usize,
    envelope_scroll: usize,
    graft_donor: Option<grafting_ui::Donor>,
    graft_library: Option<(String, Archive)>,
    graft_mask: u16,
    graft_preview: hangar_core::definition::Graft,
    graft_scroll: usize,
    mode: Mode,
    data: Vec<u8>,
    brf: Option<Brf>,
    model: Option<Model>,
    preview: Option<Model>,
    detail: String,
    prompt: Option<Prompt>,
    yaw: i32,
    pitch: i32,
    zoom: i32,
    pan: [i32; 2],
    perspective: bool,
    mouse: [i32; 2],
    drag: bool,
    filter: String,
    filter_focus: bool,
    pub quit: bool,
    clone_draft: Option<hangar_core::clone_aircraft::Package>,
    clone_sources: Vec<String>,
    clone_title: String,
    clone_scroll: usize,
    suggested_output: Option<String>,
    variant_shape: Vec<u8>,
    variant_id: String,
    variant_draft: Option<Variant>,
    required: Vec<String>,
    collapsed: [bool; 9],
    category: Option<usize>,
    menu: Option<usize>,
    dock: u8,
    table_scroll: usize,
    inspector_scroll: i32,
    original_brf: Option<Brf>,
    model_entry: Option<usize>,
    validation: Option<hangar_core::validation::Report>,
    dependencies: hangar_core::dependencies::Index,
    dependency_catalogs: Vec<(String, Vec<String>)>,
    aircraft_users: Vec<String>,
    reference_scroll: usize,
    validation_scroll: usize,
    changes_scroll: usize,
    browser: Option<Browser>,
    recent: Vec<String>,
    pic: Option<Pic>,
    base_palette: Box<[[u8; 3]; 256]>,
    palette_loaded: bool,
    palette_override: Option<Box<[[u8; 3]; 256]>>,
    image_zoom: i32,
    image_pan: [i32; 2],
    image_drag: bool,
    brush: u8,
    base_color_from: u8,
    brush_radius: usize,
    painting: bool,
    last_paint: Option<(usize, usize)>,
    paint_enabled: bool,
    pick_color: bool,
    textures: BTreeMap<String, Pic>,
    textured: bool,
    stroke: Option<Stroke>,
    selected_face: Option<usize>,
    model_paint: bool,
    context_model: Option<Model>,
    context_entry: Option<usize>,
}
fn extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
}
fn short(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    if s.chars().count() > n && n > 3 {
        t = t.chars().take(n - 3).collect();
        t.push_str("...");
    }
    t
}
impl App {
    pub fn new() -> Self {
        Self {
            width: 1280,
            height: 800,
            doc: Document::new(Archive::empty()),
            path: String::new(),
            status: "Ready | Open a Fighters Anthology LIB or load the synthetic demo".into(),
            selected: 0,
            hp_context: None,
            hp_selected: 0,
            hp_tool: false,
            mesh_edit: false,
            mesh_vertices: Vec::new(),
            mesh_drag: None,
            hp_visible: false,
            hp_drag: None,
            media_tab: 0,
            decal_image: None,
            decal_name: String::new(),
            decal_paths: crate::platform::load_decals(),
            decal_preset: 0,
            decal_text: "AF 001".into(),
            decal_is_text: false,
            decal_ink: None,
            decal_placement: hangar_core::decal::Placement {
                center: [16, 16],
                width: 32,
                degrees: 0,
                opacity: 100,
                mirror: false,
            },
            decal_draft: None,
            decal_active: false,
            decal_dragging: false,
            libraries: Vec::new(),
            library_id: 0,
            file_backed: false,
            external_model: None,
            next_library_id: 1,
            clipboard: None,
            transfer_plan: None,
            transfer_scroll: 0,
            transfer_note: 0,
            transfer_is_copy: false,
            include_dependencies: true,
            resource_drag: None,
            scroll: 0,
            field_scroll: 0,
            field_selected: 0,
            field_group: None,
            envelope_selected: 0,
            envelope_scroll: 0,
            graft_donor: None,
            graft_library: None,
            graft_mask: 0,
            graft_preview: Default::default(),
            graft_scroll: 0,
            mode: Mode::Model,
            data: vec![],
            brf: None,
            model: None,
            preview: None,
            detail: String::new(),
            prompt: None,
            yaw: 0,
            pitch: -90,
            zoom: 100,
            pan: [0, 0],
            perspective: false,
            mouse: [0, 0],
            drag: false,
            filter: String::new(),
            filter_focus: false,
            quit: false,
            clone_draft: None,
            clone_sources: Vec::new(),
            clone_title: String::new(),
            clone_scroll: 0,
            suggested_output: None,
            variant_shape: Vec::new(),
            variant_id: String::new(),
            variant_draft: None,
            required: Vec::new(),
            collapsed: [true; 9],
            category: None,
            menu: None,
            dock: 0,
            table_scroll: 0,
            inspector_scroll: 0,
            original_brf: None,
            model_entry: None,
            validation: None,
            dependencies: Default::default(),
            dependency_catalogs: Vec::new(),
            aircraft_users: Vec::new(),
            reference_scroll: 0,
            validation_scroll: 0,
            changes_scroll: 0,
            browser: None,
            recent: crate::platform::load_recent(),
            pic: None,
            base_palette: Box::new(core::array::from_fn(|i| [i as u8; 3])),
            palette_loaded: false,
            palette_override: None,
            image_zoom: 100,
            image_pan: [0, 0],
            image_drag: false,
            brush: 150,
            base_color_from: 0,
            brush_radius: 1,
            painting: false,
            last_paint: None,
            paint_enabled: false,
            pick_color: false,
            textures: BTreeMap::new(),
            textured: false,
            stroke: None,
            selected_face: None,
            model_paint: false,
            context_model: None,
            context_entry: None,
        }
    }
    pub fn demo(&mut self) {
        self.finish_stroke();
        if self.doc.dirty() {
            self.status = "Save your changes before loading another LIB".into();
            return;
        }
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("DEMO.SH", model::demo_textured()).unwrap());
        a.entries
            .push(Entry::new("DEMO.PT", hangar_core::brf::demo_with_records()).unwrap());
        for suffix in ["A", "B", "C", "D", "S"] {
            a.entries
                .push(Entry::new(&format!("DEMO_{suffix}.SH"), model::demo_shape()).unwrap());
        }
        a.entries
            .push(Entry::new("DEMO.PIC", picture::demo()).unwrap());
        let sound: Vec<u8> = (0..5512)
            .map(|i| if i % 50 < 25 { 148 } else { 108 })
            .collect();
        a.entries.push(Entry::new("DEMO.5K", sound).unwrap());
        self.suggested_output = None;
        self.doc = Document::new(a);
        self.context_model = None;
        self.context_entry = None;
        self.selected_face = None;
        self.model_paint = false;
        self.required.clear();
        self.path = "Synthetic demo (not a game asset)".into();
        self.file_backed = false;
        self.collapsed = [true; 9];
        self.category = None;
        self.scroll = 0;
        self.table_scroll = 0;
        self.filter.clear();
        self.mode = Mode::Model;
        self.selected = 0;
        self.refresh();
        self.frame();
        self.status =
            "Synthetic demo | G/R/S transforms, X/Y/Z axis, Enter apply, Esc cancel".into();
    }
    pub fn open(&mut self, path: &str) -> Result<()> {
        self.finish_stroke();
        if self.open_existing_library(path)? {
            return Ok(());
        }
        let a = Archive::parse(crate::platform::read(path)?)?;
        self.install_library(Document::new(a), path.into())?;
        self.file_backed = true;
        self.context_model = None;
        self.context_entry = None;
        self.selected_face = None;
        self.model_paint = false;
        self.required.clear();
        self.path = path.into();
        self.selected = 0;
        self.scroll = 0;
        self.filter.clear();
        self.category = None;
        self.collapsed = [true; 9];
        self.table_scroll = 0;
        self.mode = Mode::Browse;
        let aircraft: Vec<_> = self
            .doc
            .archive
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name.ends_with(".PT"))
            .map(|(i, _)| i)
            .collect();
        if aircraft.len() == 1 {
            self.selected = aircraft[0];
            self.mode = Mode::Model;
        }
        self.refresh();
        self.frame();
        self.remember(path);
        self.status = format!(
            "Opened {} | {} entries | {}",
            path,
            self.doc.archive.entries.len(),
            if hangar_core::save::protected_name(path).is_some() {
                "Protected source: extract or save a copy"
            } else {
                "Custom LIB: save with backup"
            }
        );
        Ok(())
    }
    pub fn select_entry(&mut self, index: usize) {
        self.finish_stroke();
        self.selected_face = None;
        self.context_model = None;
        self.context_entry = None;
        self.paint_enabled = false;
        self.model_paint = false;

        self.field_group = None;
        self.envelope_selected = 0;
        self.envelope_scroll = 0;
        self.hp_tool = false;
        self.mesh_edit = false;
        self.mesh_vertices.clear();
        self.decal_active = false;
        self.selected = index.min(self.doc.archive.entries.len().saturating_sub(1));
        self.collapsed[view::category_of(self.name())] = false;
        self.category = Some(view::category_of(self.name()));
        let table_position = self
            .browser_entries()
            .iter()
            .position(|i| *i == self.selected)
            .unwrap_or(0);
        let table_rows = ((self.dock_y() - 80) / 24).max(1) as usize;
        if table_position < self.table_scroll || table_position >= self.table_scroll + table_rows {
            self.table_scroll = table_position.saturating_sub(table_rows / 2);
        }
        let position = self
            .tree_rows()
            .iter()
            .position(|(_, i)| *i == Some(self.selected))
            .unwrap_or(0);
        let rows = ((self.height - self.tree_start() - 54) / 20).max(1) as usize;
        if position < self.scroll || position >= self.scroll + rows {
            self.scroll = position.saturating_sub(rows / 2);
        }
        self.refresh();
        self.frame();
        if matches!(extension(self.name()), "PIC" | "PAL" | "5K" | "11K" | "WAV") {
            self.mode = Mode::Media;
            if self.name().ends_with(".PAL") {
                self.media_tab = 1;
            }
        } else if self.mode == Mode::Media {
            self.mode = Mode::Model;
        }
    }
    fn name(&self) -> &str {
        self.doc
            .archive
            .entries
            .get(self.selected)
            .map_or("No entry", |e| e.name.as_str())
    }
    fn refresh(&mut self) {
        let camera = (self.zoom, self.pan, self.image_zoom, self.image_pan);
        self.refresh_data();
        (self.zoom, self.pan, self.image_zoom, self.image_pan) = camera;
    }
    fn refresh_data(&mut self) {
        self.hp_drag = None;
        self.mesh_drag = None;
        self.decal_draft = None;
        self.decal_dragging = false;
        crate::platform::stop_audio();
        if let Some(i) = self.context_entry {
            self.context_model = self
                .doc
                .archive
                .entries
                .get(i)
                .filter(|e| e.name.ends_with(".SH"))
                .and_then(|e| e.read().ok())
                .and_then(|b| Model::parse(&b).ok());
        }
        self.pic = None;
        self.textures.clear();
        self.painting = false;
        self.last_paint = None;
        self.paint_enabled = false;
        *self.base_palette = self
            .palette_override
            .as_deref()
            .copied()
            .unwrap_or_else(|| core::array::from_fn(|i| [i as u8; 3]));
        self.palette_loaded = self.palette_override.is_some();
        let mut palettes = Vec::new();
        let pts: Vec<_> = self
            .doc
            .archive
            .entries
            .iter()
            .filter(|e| e.name.ends_with(".PT"))
            .collect();
        if pts.len() == 1 {
            palettes.push(format!("{}.PAL", pts[0].name.split('.').next().unwrap()));
        }
        palettes.push("PALETTE.PAL".into());
        if let Some(entry) = palettes
            .iter()
            .find_map(|name| self.resolve_resource(name).map(|(_, _, e)| e))
        {
            if let Ok(b) = entry.read() {
                if let Ok(p) = picture::palette(&b) {
                    *self.base_palette = p;
                    self.palette_loaded = true;
                }
            }
        }

        self.brf = None;
        self.model = None;
        self.model_entry = None;
        self.external_model = None;
        self.original_brf = None;
        self.inspector_scroll = 0;
        self.validation = None;
        self.validation_scroll = 0;
        self.changes_scroll = 0;
        self.reference_scroll = 0;
        let providers = self.dependency_providers();
        let mut names = alloc::collections::BTreeSet::new();
        for name in providers.keys() {
            names.insert(name.clone());
        }
        self.dependencies.update_with(&self.doc.archive, &names);
        for entry in &self.doc.archive.entries {
            names.insert(entry.name.clone());
        }
        for library in &mut self.libraries {
            library
                .dependencies
                .update_with(&library.doc.archive, &names);
        }
        self.preview = None;
        self.data.clear();
        self.field_scroll = 0;
        self.field_selected = 0;
        self.detail.clear();
        self.selected = self
            .selected
            .min(self.doc.archive.entries.len().saturating_sub(1));
        self.aircraft_users = self.dependencies.aircraft_users(self.name());
        if let Some(e) = self.doc.archive.entries.get(self.selected) {
            match e.read() {
                Ok(data) => {
                    let ext = extension(&e.name);
                    if ext == "PIC" {
                        match Pic::parse(&data) {
                            Ok(p) => {
                                self.detail = format!("{} x {} / indexed PIC", p.width, p.height);
                                self.pic = Some(p);
                            }
                            Err(e) => self.detail = e,
                        }
                    } else if matches!(ext, "5K" | "11K" | "WAV") {
                        self.detail = match Pcm::parse(&e.name, &data) {
                            Ok(p) => {
                                format!("{} Hz / {} samples / PCM8 mono", p.rate, p.samples.len())
                            }
                            Err(e) => e,
                        };
                    } else if ext == "SH" {
                        match Model::parse(&data) {
                            Ok(m) => {
                                self.detail = if m.writable {
                                    "Static geometry: editable".into()
                                } else {
                                    m.reason.clone()
                                };
                                self.model = Some(m);
                                self.model_entry = Some(self.selected);
                            }
                            Err(e) => self.detail = e,
                        }
                    } else if data.starts_with(b"[brent's_relocatable_format]") {
                        match Brf::parse(&data, ext) {
                            Ok(b) => {
                                self.detail =
                                    format!("{} fields | original storage units", b.fields.len());
                                self.brf = Some(b);
                            }
                            Err(e) => self.detail = e,
                        }
                    } else {
                        self.detail = "Opaque resource | export or replace entry".into();
                    }
                    self.data = data;
                }
                Err(err) => self.detail = err,
            }
        }
        if let Some(old) = self.doc.saved_entry(self.name()) {
            if let Ok(data) = old.read() {
                self.original_brf = Brf::parse(&data, extension(self.name())).ok();
            }
        }
        let linked_shape = self.brf.as_ref().and_then(|brf| {
            let ptr = brf
                .fields
                .iter()
                .find(|f| f.label == "object.shape" && f.kind == "ptr")?;
            brf.fields
                .iter()
                .find(|f| f.block == ptr.value && f.kind == "string")
                .map(|f| f.value.trim_matches('"').to_string())
        });
        if let Some(name) = linked_shape {
            if let Some((id, index, entry)) = self.resolve_resource(&name) {
                if let Ok(bytes) = entry.read() {
                    if let Ok(model) = Model::parse(&bytes) {
                        self.model = Some(model);
                        if id == self.library_id {
                            self.model_entry = Some(index);
                        } else {
                            self.external_model = Some((id, index));
                        }
                    }
                }
            }
        }
        if !self.doc.archive.entries.is_empty() {
            self.collapsed[view::category_of(self.name())] = false;
        }
        if let Some(model) = self.model.as_ref().or(self.context_model.as_ref()) {
            for name in &model.textures {
                let key = if name.contains('.') {
                    name.clone()
                } else {
                    format!("{name}.PIC")
                };
                if let Some((_, _, entry)) = self.resolve_resource(&key) {
                    if let Ok(bytes) = entry.read() {
                        if let Ok(p) = Pic::parse(&bytes) {
                            self.textures.insert(name.clone(), p);
                        }
                    }
                }
            }
        }
        self.refresh_graft();
        self.refresh_hardpoints();
        self.frame();
    }
    fn frame(&mut self) {
        self.image_zoom = 100;
        self.image_pan = [0, 0];
        self.zoom = 100;
        self.pan = [0, 0];
    }
    fn result(&mut self, r: Result<()>) {
        if let Err(e) = r {
            self.status = format!("Error: {e}");
        }
    }
    pub fn smoke_save_policy(&mut self) {
        *self = Self::new();
        self.demo();
        self.path = "FA_2.LIB".into();
        self.doc.mark_unsaved();
        let before = self.doc.archive.bytes().unwrap();
        self.file_prompt(FileAction::Save);
        assert!(self.prompt.as_ref().unwrap().value.ends_with("HANGAR.LIB"));
        assert!(self
            .perform_file(FileAction::Save, "fa_2.lib")
            .unwrap_err()
            .contains("protected retail LIB"));
        assert!(self.doc.dirty());
        assert_eq!(self.doc.archive.bytes().unwrap(), before);
        let path = format!(
            "{}/HGUI.LIB",
            crate::platform::current_dir().trim_end_matches(['/', '\\'])
        );
        let backup = format!("{path}.bak");
        for p in [&path, &backup, &format!("{path}.tmp")] {
            assert!(!crate::platform::save_exists(p).unwrap());
        }
        self.perform_file(FileAction::Save, &path).unwrap();
        assert!(!self.doc.dirty());
        self.doc
            .replace(0, hangar_core::model::demo_shape())
            .unwrap();
        self.file_prompt(FileAction::Save);
        assert_eq!(self.prompt.as_ref().unwrap().value, path);
        self.perform_file(FileAction::Save, &path).unwrap();
        assert!(self.status.contains("Backup:"));
        assert!(!self.doc.dirty());
        assert_eq!(crate::platform::read(&backup).unwrap(), before);
        self.prompt = None;
        self.browser = None;
        self.open(&path).unwrap();
        for p in [&path, &backup] {
            crate::platform::remove_file(p).unwrap();
        }
    }
    pub fn file_prompt(&mut self, a: FileAction) {
        if self.mesh_drag.take().is_some() {
            self.preview = None;
        }
        self.hp_drag = None;
        self.decal_dragging = false;
        if matches!(a, FileAction::VariantSh) && (self.doc.dirty() || !self.name().ends_with(".PT"))
        {
            self.status = "Select a PT donor in a saved LIB before creating a new aircraft".into();
            return;
        }
        if matches!(a, FileAction::Open) && self.doc.dirty() {
            self.status = "Save your changes before opening another LIB".into();
            return;
        }
        if matches!(a, FileAction::Variant) && self.doc.archive.entries.get(self.selected).is_none()
        {
            self.status = "Select an object to export with its resources".into();
            return;
        }
        if matches!(a, FileAction::Variant) {
            self.begin_clone();
            return;
        }
        if matches!(a, FileAction::CloneSource) {
            if let Some(p) = &self.prompt {
                if matches!(p.kind, PromptKind::CloneTitle) {
                    self.clone_title = p.value.clone();
                }
            }
        }
        let title = match a {
            FileAction::Decal => "Import transparent PNG decal or squadron artwork",
            FileAction::GraftLibrary => "Choose donor LIB, then choose any compatible entry",
            FileAction::ReferenceSource => "Add source LIB catalog for dependency checks",
            FileAction::Report => "Export package report as text",
            FileAction::Png => "Export picture as PNG",
            FileAction::Wav => "Export sound as WAV",
            FileAction::Palette => "Load display palette (.PAL or a LIB containing PALETTE.PAL)",
            FileAction::Open => "Open LIB",
            FileAction::Variant => "Export object",
            FileAction::VariantSh => "New aircraft from loose SH / step 1: select SH file",
            FileAction::CloneSource => "Additional source LIB for object dependencies",
            FileAction::Save => "Save LIB / retail names protected / custom LIBs saved with backup",
            FileAction::Import => "Add entry: path to a resource file",
            FileAction::Replace => "Replace selected entry: resource file path",
            FileAction::Export => "Export selected entry: new file path",
            FileAction::Obj => "Export static geometry: new OBJ path",
            FileAction::Graft => "Copy selected field from same entry in donor LIB: donor path",
        };
        let value = match a {
            FileAction::Save => self.suggested_output.clone().unwrap_or_else(|| {
                if !self.path.is_empty()
                    && !self.path.starts_with("Synthetic")
                    && hangar_core::save::protected_name(&self.path).is_none()
                {
                    self.path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&self.path)
                        .into()
                } else {
                    "HANGAR.LIB".into()
                }
            }),
            FileAction::Report => "HANGAR-REPORT.txt".into(),
            FileAction::Png => format!("{}.png", self.name().split('.').next().unwrap()),
            FileAction::Wav => format!("{}.wav", self.name().split('.').next().unwrap()),
            FileAction::Export => self.name().into(),
            FileAction::Obj => format!("{}.obj", self.name().split('.').next().unwrap_or("model")),
            _ => String::new(),
        };
        let folder = if matches!(a, FileAction::Save) && self.path.contains(['/', '\\']) {
            Self::parent_path(&self.path)
        } else if let Some(p) = self.recent.first() {
            Self::parent_path(p)
        } else {
            crate::platform::current_dir()
        };
        let folder = if crate::platform::is_dir(&folder) {
            folder
        } else {
            crate::platform::current_dir()
        };
        let value = if value.is_empty() {
            format!("{}/", folder.trim_end_matches(['/', '\\']))
        } else {
            format!("{}/{}", folder.trim_end_matches(['/', '\\']), value)
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::File(a),
            title: title.into(),
            value,
            axis: 0,
        });
        let initial = self.prompt.as_ref().unwrap().value.clone();
        self.browse_folder(&folder);
        self.prompt.as_mut().unwrap().value = initial;
        self.filter_focus = false;
    }
    fn perform_file(&mut self, a: FileAction, path: &str) -> Result<()> {
        if path.is_empty() {
            return Err("Enter a file path".into());
        }
        match a {
            FileAction::Decal => self.load_decal(path, true),
            FileAction::GraftLibrary => {
                let a = Archive::parse(crate::platform::read(path)?)?;
                self.graft_library = Some((path.into(), a));
                self.graft_scroll = 0;
                self.mode = Mode::Graft;
                self.status = "Select a donor entry; the current entry remains the target".into();
                Ok(())
            }
            FileAction::Open => self.open(path),
            FileAction::ReferenceSource => {
                let entries = cloning_ui::index(path)?.ok_or("Choose an EALIB source")?;
                if entries.len()
                    + self
                        .dependency_catalogs
                        .iter()
                        .filter(|(p, _)| p != path)
                        .map(|(_, n)| n.len())
                        .sum::<usize>()
                    > 131072
                {
                    return Err("Source catalogs exceed 131072 entries".into());
                }
                if self.dependency_catalogs.len() >= 64 {
                    return Err("At most 64 source catalogs".into());
                }
                self.dependency_catalogs.retain(|(p, _)| p != path);
                self.dependency_catalogs
                    .push((path.into(), entries.into_iter().map(|e| e.name).collect()));
                self.refresh();
                self.status = "Source catalog added; re-run package checks".into();
                Ok(())
            }
            FileAction::Report => {
                let report = self.package_report();
                crate::platform::write_new(
                    path,
                    hangar_core::validation::text(&self.doc, &report).as_bytes(),
                )?;
                self.validation = Some(report);
                self.status = format!("Exported package report to {path}");
                Ok(())
            }
            FileAction::Png => {
                let p = self.pic.as_ref().ok_or("Select a PIC first")?;
                crate::platform::write_new(path, &p.png(&self.base_palette))?;
                self.status = format!("Exported PNG {path}");
                Ok(())
            }
            FileAction::Wav => {
                let pcm = Pcm::parse(self.name(), &self.data)?;
                crate::platform::write_new(path, &pcm.wav())?;
                self.status = format!("Exported WAV {path}");
                Ok(())
            }
            FileAction::Palette => {
                let bytes = crate::platform::read(path)?;
                let palette = if bytes.starts_with(b"EALIB") {
                    let archive = Archive::parse(bytes)?;
                    let at = archive
                        .find("PALETTE.PAL")
                        .ok_or("This LIB has no PALETTE.PAL")?;
                    picture::palette(&archive.entries[at].read()?)?
                } else {
                    picture::palette(&bytes)?
                };
                self.palette_override = Some(Box::new(palette));
                self.refresh();
                self.status =
                    "Display palette loaded; original resource palettes remain unchanged".into();
                Ok(())
            }
            FileAction::Variant => Err("Start Export object from a selected resource".into()),
            FileAction::CloneSource => self.add_clone_source(path),
            FileAction::VariantSh => {
                let shape = crate::platform::read(path)?;
                Model::parse(&shape)?;
                self.variant_shape = shape;
                self.variant_draft = None;
                self.prompt = Some(Prompt {
                    kind: PromptKind::VariantId,
                    title: "New aircraft / step 2: unique ID (1..6 letters/digits)".into(),
                    value: String::new(),
                    axis: 0,
                });
                self.status =
                    "The new aircraft inherits donor flight, equipment, damage and shadow".into();
                Ok(())
            }
            FileAction::Save => {
                if self.other_library_at(path) {
                    return Err("That destination is open in another LIB; switch to it or choose a different path".into());
                }
                let missing: Vec<_> = self
                    .required
                    .iter()
                    .filter(|n| self.doc.archive.find(n).is_none())
                    .cloned()
                    .collect();
                if !missing.is_empty() {
                    return Err(format!(
                        "Add the missing shape textures before packaging: {}",
                        missing.join(", ")
                    ));
                }
                let b = self.doc.archive.bytes()?;
                let backup = crate::saving::library(path, &b)?;
                self.doc.mark_saved();
                let field = self.field_selected;
                let scroll = self.field_scroll;
                self.refresh();
                self.field_selected = field;
                self.field_scroll = scroll;
                self.path = path.into();
                self.file_backed = true;
                self.suggested_output = None;
                self.remember(path);
                self.status = format!(
                    "Saved {} entries into {path}{}",
                    self.doc.archive.entries.len(),
                    backup
                        .map(|p| format!(" | Backup: {p}"))
                        .unwrap_or_default()
                );
                Ok(())
            }
            FileAction::Import => {
                let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
                self.doc.import(name, crate::platform::read(path)?)?;
                self.selected = self.doc.archive.entries.len() - 1;
                self.refresh();
                self.status = "Entry added | Ctrl+Z undo".into();
                Ok(())
            }
            FileAction::Replace => {
                self.doc
                    .replace(self.selected, crate::platform::read(path)?)?;
                self.refresh();
                self.status = "Entry replaced | Ctrl+Z undo".into();
                Ok(())
            }
            FileAction::Export => {
                let e = self
                    .doc
                    .archive
                    .entries
                    .get(self.selected)
                    .ok_or("No selected entry")?;
                crate::platform::write_new(path, &e.read()?)?;
                self.status = format!("Exported {path}");
                Ok(())
            }
            FileAction::Obj => {
                let m = self
                    .preview
                    .as_ref()
                    .or(self.model.as_ref())
                    .ok_or("No decoded shape")?;
                crate::platform::write_new(path, m.obj().as_bytes())?;
                self.status = format!("Exported geometry only: {path}");
                Ok(())
            }
            FileAction::Graft => {
                let brf = self.brf.as_ref().ok_or("Select a BRF field first")?;
                let field = brf
                    .fields
                    .get(self.field_selected)
                    .ok_or("No selected field")?;
                let a = Archive::parse(crate::platform::read(path)?)?;
                let at = a
                    .find(self.name())
                    .ok_or("Donor LIB has no matching entry name")?;
                let bytes = a.entries[at].read()?;
                let donor = Brf::parse(&bytes, extension(self.name()))?;
                let f = donor
                    .fields
                    .iter()
                    .find(|f| f.label == field.label && f.kind == field.kind)
                    .ok_or("Donor schema does not match selected field")?;
                // Populate the normal edit dialog so the exact donor value can be reviewed.
                self.prompt = Some(Prompt {
                    kind: PromptKind::Field(self.field_selected),
                    title: format!("Apply donor {} ({})", field.label, field.kind),
                    value: f.value.clone(),
                    axis: 0,
                });
                Ok(())
            }
        }
    }
    fn edit_field(&mut self, index: usize) {
        if let Some(f) = self.brf.as_ref().and_then(|b| b.fields.get(index)) {
            self.field_selected = index;
            self.prompt = Some(Prompt {
                kind: PromptKind::Field(index),
                title: format!(
                    "{} | {}{}",
                    f.label,
                    f.kind,
                    if f.scaled {
                        " | ^ scaled, raw units"
                    } else {
                        ""
                    }
                ),
                value: f.value.clone(),
                axis: 0,
            });
        }
    }
    fn transform_preview(&mut self) {
        let Some(p) = &self.prompt else {
            return;
        };
        let PromptKind::Transform(op) = p.kind else {
            return;
        };
        let value = if p.value.is_empty() {
            if op == 's' {
                100
            } else {
                0
            }
        } else {
            match p.value.parse::<i32>() {
                Ok(n) => n,
                Err(_) => {
                    self.preview = None;
                    self.status = "Error: enter an integer transform value".into();
                    return;
                }
            }
        };
        let t = match op {
            'g' => Transform::Move(p.axis, value),
            'r' => Transform::Rotate(p.axis, value),
            _ => Transform::Scale(Some(p.axis), value),
        };
        if let Some(m) = &self.model {
            match m.transformed(t) {
                Ok(m) => self.preview = Some(m),
                Err(e) => {
                    self.preview = None;
                    self.status = e;
                }
            }
        }
    }
    pub fn key(&mut self, key: Key, ctrl: bool, shift: bool) {
        if self.mesh_drag.is_some()
            && (matches!(key, Key::Escape) || ctrl && matches!(key, Key::Char('z')))
        {
            self.mesh_drag = None;
            self.preview = None;
            self.status = "Vertex drag cancelled".into();
            return;
        }
        if self.hp_drag.is_some()
            && (matches!(key, Key::Escape) || ctrl && matches!(key, Key::Char('z')))
        {
            self.hp_drag = None;
            self.status = "Station drag cancelled".into();
            return;
        }
        if self.decal_draft.is_some()
            && self.prompt.is_none()
            && (matches!(key, Key::Escape) || ctrl && matches!(key, Key::Char('z')))
        {
            self.decal_draft = None;
            self.decal_dragging = false;
            self.decal_active = false;
            self.status = "Decal preview cancelled".into();
            return;
        }
        if self.painting {
            if matches!(key, Key::Escape) {
                self.painting = false;
                self.stroke = None;
                self.refresh();
                return;
            }
            self.finish_stroke();
        }

        if self.menu.is_some() {
            self.menu = None;
            if matches!(key, Key::Escape) {
                return;
            }
        }

        if self.prompt.is_some() {
            match key {
                Key::Escape => {
                    if self.prompt.as_ref().is_some_and(|p| {
                        matches!(p.kind, PromptKind::File(FileAction::CloneSource))
                    }) {
                        self.browser = None;
                        self.prompt = Some(Prompt {
                            kind: PromptKind::CloneTitle,
                            title: "Export object / step 2: display name".into(),
                            value: self.clone_title.clone(),
                            axis: 0,
                        });
                        return;
                    }
                    if self.prompt.as_ref().is_some_and(|p| {
                        matches!(
                            p.kind,
                            PromptKind::CloneId | PromptKind::CloneTitle | PromptKind::CloneReview
                        )
                    }) {
                        self.clone_draft = None;
                    }
                    self.prompt = None;
                    self.preview = None;
                    self.transfer_plan = None;
                    self.status = "Cancelled".into();
                }
                Key::Enter => {
                    if let Some(p) = &self.prompt {
                        if matches!(p.kind, PromptKind::File(_))
                            && crate::platform::is_dir(&self.browser_path(&p.value))
                        {
                            let folder = self.browser_path(&p.value);
                            self.browse_folder(&folder);
                            return;
                        }
                    }
                    let p = self.prompt.take().unwrap();
                    let r = match p.kind {
                        PromptKind::PaletteColor => self.palette_edit(&p.value),
                        PromptKind::PaletteIndex => (|| {
                            self.brush = p
                                .value
                                .parse()
                                .map_err(|_| "Enter a palette index 0..255")?;
                            Ok(())
                        })(),
                        PromptKind::UvMaterial => self.uv_edit(&p.value),
                        PromptKind::FamilyTexture => self.family_texture(&p.value),
                        PromptKind::DecalText => self.tail_text(&p.value),
                        PromptKind::DecalInk => (|| {
                            let index: u8 = p
                                .value
                                .parse()
                                .map_err(|_| "Enter a palette index 0..255")?;
                            self.decal_ink = Some(index);
                            if self.decal_is_text {
                                let text = self.decal_text.clone();
                                self.tail_text(&text)?;
                            }
                            Ok(())
                        })(),
                        PromptKind::DecalSetting(key) => self.decal_setting(key, &p.value),
                        PromptKind::DecalLibrary => Err("Choose a PNG row or press Esc".into()),
                        PromptKind::StationMove => (|| {
                            let delta: i32 =
                                p.value.parse().map_err(|_| "Enter an integer offset")?;
                            let c = self.hp_context.as_ref().ok_or("No station")?;
                            let station = c.stations.get(self.hp_selected).ok_or("No station")?;
                            let value = station.position[p.axis]
                                .checked_add(delta)
                                .ok_or("Coordinate overflow")?;
                            self.station_value(p.axis + 1, &format!("{value}"))
                        })(),
                        PromptKind::StationValue(column) => self.station_value(column, &p.value),
                        PromptKind::CloseLibrary => {
                            if p.value == "DISCARD" {
                                self.discard_library();
                                Ok(())
                            } else {
                                Err("Type DISCARD or press Esc".into())
                            }
                        }
                        PromptKind::ResourceName(duplicate) => {
                            self.review_resource_name(&p.value, duplicate)
                        }
                        PromptKind::TransferReview => self.apply_transfer(),
                        PromptKind::Isolate => (|| {
                            let to = p.value.trim().to_ascii_uppercase();
                            let shape = self
                                .model_entry
                                .or(self.context_entry)
                                .ok_or("No model context")?;
                            let m = self
                                .model
                                .as_ref()
                                .or(self.context_model.as_ref())
                                .ok_or("No model")?;
                            let from = if self.pic.is_some() {
                                self.name().to_string()
                            } else {
                                let f = self
                                    .selected_face
                                    .and_then(|i| m.faces.get(i))
                                    .ok_or("Select a textured panel")?;
                                if f.texture.contains('.') {
                                    f.texture.clone()
                                } else {
                                    format!("{}.PIC", f.texture)
                                }
                            };
                            let texture =
                                self.doc.archive.find(&from).ok_or("Texture is missing")?;
                            let pixels = self.doc.archive.entries[texture].read()?;
                            let (bytes, n) = Model::retarget_texture(
                                &self.doc.archive.entries[shape].read()?,
                                &from,
                                &to,
                            )?;
                            self.doc.clone_texture(&to, pixels, shape, bytes)?;
                            self.select_entry(shape);
                            self.textured = true;
                            self.status=format!("Cloned {to}; {n} decoded references updated. Other LOD/damage references retain original textures.");
                            Ok(())
                        })(),
                        PromptKind::MeshMove => self.mesh_move(&p.value),
                        PromptKind::BaseColor(face) => self.apply_base_color(face),
                        PromptKind::Recolor => {
                            let r = (|| {
                                let (from, to) = p
                                    .value
                                    .split_once(' ')
                                    .ok_or("Enter two palette indices: FROM TO")?;
                                let from = from
                                    .trim()
                                    .parse::<u8>()
                                    .map_err(|_| "Invalid source color")?;
                                let to = to
                                    .trim()
                                    .parse::<u8>()
                                    .map_err(|_| "Invalid target color")?;
                                let i = self.model_entry.ok_or("Select a shape")?;
                                let bytes = self.doc.archive.entries[i].read()?;
                                let (bytes, n) = Model::recolor(&bytes, from, to)?;
                                self.doc.replace(i, bytes)?;
                                self.refresh();
                                self.textured = true;
                                self.status=format!("Recolored {n} untextured faces in the decoded pose; Ctrl+Z undo");
                                Ok(())
                            })();
                            r
                        }
                        PromptKind::CloneId => match authoring::validate_id(p.value.trim()) {
                            Ok(id) => {
                                if self
                                    .doc
                                    .archive
                                    .find(&format!("{id}.{}", extension(self.name())))
                                    .is_some()
                                {
                                    Err("That object ID already exists; choose a new ID".into())
                                } else {
                                    self.variant_id = id;
                                    self.prompt = Some(Prompt {
                                        kind: PromptKind::CloneTitle,
                                        title: "Export object / step 2: display name".into(),
                                        value: self.clone_title.clone(),
                                        axis: 0,
                                    });
                                    Ok(())
                                }
                            }
                            Err(e) => Err(e),
                        },
                        PromptKind::CloneTitle => {
                            self.clone_title = p.value.clone();
                            match self.build_clone() {
                                Ok(package) => {
                                    self.clone_draft = Some(package);
                                    self.clone_scroll = 0;
                                    self.prompt = Some(Prompt {
                                        kind: PromptKind::CloneReview,
                                        title: "Export object / review private resources".into(),
                                        value: String::new(),
                                        axis: 0,
                                    });
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        PromptKind::CloneReview => (|| {
                            if let Some(package) = self.clone_draft.take() {
                                let count = package.archive.entries.len();
                                self.install_library(
                                    Document::new(package.archive),
                                    format!("{}.LIB", package.id),
                                )?;
                                self.suggested_output = Some(format!("{}.LIB", package.id));
                                self.doc.mark_unsaved();
                                self.required.clear();
                                self.context_model = None;
                                self.context_entry = None;
                                self.selected_face = None;
                                self.selected = self
                                    .doc
                                    .archive
                                    .find(&format!("{}.{}", package.id, extension(&package.donor)))
                                    .unwrap_or(0);
                                self.scroll = 0;
                                self.table_scroll = 0;
                                self.category = None;
                                self.filter.clear();
                                self.collapsed = [true; 9];
                                self.mode = Mode::Model;
                                self.refresh();
                                self.status = format!(
                                    "{count} private resources ready; choose a NEW output LIB path"
                                );
                                self.file_prompt(FileAction::Save);
                                Ok(())
                            } else {
                                Err("No prepared aircraft export".into())
                            }
                        })(),
                        PromptKind::VariantId => match authoring::validate_id(&p.value) {
                            Ok(id) => {
                                self.variant_id = id;
                                self.prompt = Some(Prompt {
                                    kind: PromptKind::VariantTitle,
                                    title: "New aircraft / step 3: display name".into(),
                                    value: String::new(),
                                    axis: 0,
                                });
                                Ok(())
                            }
                            Err(e) => Err(e),
                        },
                        PromptKind::VariantTitle => {
                            match authoring::create(
                                &self.doc.archive,
                                self.name(),
                                self.variant_shape.clone(),
                                &self.variant_id,
                                &p.value,
                            ) {
                                Ok(v) => {
                                    self.status=format!("{} entries | donor damage/shadow retained | {} missing textures",v.archive.entries.len(),v.missing_textures.len());
                                    self.prompt = Some(Prompt {
                                        kind: PromptKind::VariantReview,
                                        title: format!(
                                            "Create {} from {}? Type CREATE to continue",
                                            self.variant_id, v.donor
                                        ),
                                        value: String::new(),
                                        axis: 0,
                                    });
                                    self.variant_draft = Some(v);
                                    Ok(())
                                }
                                Err(e) => Err(e),
                            }
                        }
                        PromptKind::VariantReview => (|| {
                            if p.value == "CREATE" {
                                if let Some(v) = self.variant_draft.take() {
                                    let required = v.missing_textures;
                                    self.status=format!("New aircraft {} | {} shared stock references | {} missing textures",self.variant_id,v.shared.len(),required.len());
                                    let palette =
                                        self.palette_loaded.then(|| self.base_palette.clone());
                                    self.install_library(
                                        Document::new(v.archive),
                                        format!(
                                            "New {}.LIB from {} (not saved)",
                                            self.variant_id, v.donor
                                        ),
                                    )?;
                                    self.doc.mark_unsaved();
                                    self.palette_override = palette;
                                    self.required = required;
                                    self.variant_shape.clear();
                                    self.selected = 0;
                                    self.scroll = 0;
                                    self.filter.clear();
                                    self.mode = Mode::Properties;
                                    self.refresh();
                                    Ok(())
                                } else {
                                    Err("Variant draft is missing".into())
                                }
                            } else {
                                Err("Type CREATE or press Esc".into())
                            }
                        })(),
                        PromptKind::Discard => {
                            if p.value == "DISCARD" {
                                self.quit = true;
                                Ok(())
                            } else {
                                Err("Type DISCARD or press Esc".into())
                            }
                        }
                        PromptKind::File(a) => {
                            let path = self.browser_path(&p.value);
                            self.perform_file(a, &path)
                        }
                        PromptKind::Field(index) => {
                            let r = self
                                .brf
                                .as_ref()
                                .ok_or_else(|| "No BRF fields".to_string())
                                .and_then(|b| {
                                    b.edit(&self.data, index, &p.value, extension(self.name()))
                                })
                                .and_then(|b| self.doc.replace(self.selected, b));
                            if r.is_ok() {
                                let scroll = self.field_scroll;
                                self.refresh();
                                self.field_scroll = scroll;
                                self.field_selected = index;
                                self.status = "Field changed | Ctrl+Z undo".into();
                            }
                            r
                        }
                        PromptKind::Transform(_) => {
                            let r = self
                                .preview
                                .as_ref()
                                .ok_or_else(|| "Enter a valid transform value".to_string())
                                .and_then(|m| m.write(&self.data))
                                .and_then(|b| self.doc.replace(self.selected, b));
                            if r.is_ok() {
                                self.refresh();
                                self.status = "Geometry changed | Ctrl+Z undo".into();
                            }
                            self.preview = None;
                            r
                        }
                    };
                    if let Err(e) = r {
                        self.status = format!("Error: {e}");
                        self.prompt = Some(p);
                    }
                }
                Key::Backspace => {
                    self.prompt.as_mut().unwrap().value.pop();
                    self.transform_preview();
                }
                Key::Char(ch) => {
                    let p = self.prompt.as_mut().unwrap();
                    if matches!(p.kind, PromptKind::Transform(_) | PromptKind::StationMove)
                        && "xyzXYZ".contains(ch)
                    {
                        p.axis = match ch.to_ascii_lowercase() {
                            'x' => 0,
                            'y' => 1,
                            _ => 2,
                        };
                    } else if ctrl && ch.eq_ignore_ascii_case(&'a') {
                        p.value.clear();
                    } else if !ctrl && !ch.is_control() && p.value.len() < 1024 {
                        p.value.push(ch);
                    }
                    self.transform_preview();
                }
                _ => {}
            }
            return;
        }
        if self.filter_focus {
            match key {
                Key::Escape | Key::Enter => self.filter_focus = false,
                Key::Backspace => {
                    self.filter.pop();
                    self.category = None;
                    self.table_scroll = 0;
                    self.scroll = 0;
                }
                Key::Char(c) if !ctrl => {
                    self.filter.push(c);
                    self.category = None;
                    self.table_scroll = 0;
                    self.scroll = 0;
                }
                _ => {}
            }
            return;
        }
        match key {
            Key::Char(ch) if ctrl=>match ch.to_ascii_lowercase(){'c'=>{let r=self.copy_resource();self.result(r);},'v'=>{let r=self.paste_resources();self.result(r);},'d'=>self.rename_prompt(true),'w'=>{let r=self.close_library();self.result(r);},'o'=>self.file_prompt(FileAction::Open),'s'=>self.file_prompt(FileAction::Save),'i'=>self.file_prompt(FileAction::Import),'e'=>self.file_prompt(FileAction::Export),'f'=>self.filter_focus=true,'b'=>{self.mode=Mode::Package;self.file_prompt(FileAction::Save);},'z'=>{if shift{self.doc.redo();}else{self.doc.undo();}self.refresh();self.status=self.doc.summary();},'y'=>{self.doc.redo();self.refresh();},_=>{}},
            Key::Char('h')|Key::Char('H')=>{let r=self.station_add(false,true);self.result(r);},
            Key::Char('a')|Key::Char('A') if self.mesh_edit&&self.mode==Mode::Model=>{self.mesh_vertices=(0..self.model.as_ref().map_or(0,|m|m.vertices.len())).collect();},
            Key::Char('g')|Key::Char('G') if self.mesh_edit&&self.mode==Mode::Model=>self.mesh_move_prompt(),
            Key::Tab if self.mode==Mode::Model=>self.act(view::Action::MeshMode),
            Key::Char('g')|Key::Char('G') if self.hp_tool&&self.mode==Mode::Model => {self.prompt=Some(Prompt{kind:PromptKind::StationMove,title:"Move station / X Y Z axis, source-unit offset".into(),value:"0".into(),axis:0});},
            Key::Char(ch) if "gGrRsS".contains(ch)&&self.model.is_some()=>{
                if self.model_entry!=Some(self.selected) || self.model.as_ref().is_some_and(|m|!m.writable){self.status="Select the linked SH entry to edit supported geometry; animated SH remains read-only".into();return;}
                let op=ch.to_ascii_lowercase();self.prompt=Some(Prompt{kind:PromptKind::Transform(op),title:match op{'g'=>"Move in source units",'r'=>"Rotate in degrees",_=>"Scale in percent"}.into(),value:String::new(),axis:0});self.transform_preview();
            },
            Key::Char('1')|Key::Num(1)=>{self.yaw=0;self.pitch=0;},Key::Char('3')|Key::Num(3)=>{self.yaw=90;self.pitch=0;},Key::Char('7')|Key::Num(7)=>{self.yaw=0;self.pitch = -90;},Key::Char('5')|Key::Num(5)=>{if self.textured{self.perspective=false;self.status="Textured paint preview uses orthographic projection".into();}else{self.perspective = !self.perspective;}},
            Key::Home|Key::Char('.')=>self.frame(),
            Key::Up=>{self.select_entry(self.selected.saturating_sub(1));},
            Key::Down=>{self.select_entry(self.selected+1);},
            Key::Enter=>self.edit_field(self.field_selected),
            Key::Delete=>{let r=self.doc.remove(self.selected);self.result(r);self.refresh();self.status="Entry removed | Ctrl+Z undo".into();},
            Key::Escape=>{self.graft_library=None;self.resource_drag=None;},
            Key::F1=>self.status="Ctrl+O open | Ctrl+S package | Ctrl+I add | Ctrl+E export | Ctrl+Z undo | MMB orbit | Shift+MMB pan | Wheel zoom | G/R/S transform".into(),
            _=>{}
        }
    }
    pub fn close(&mut self) {
        self.finish_stroke();
        crate::platform::stop_audio();
        if self.any_dirty() {
            self.prompt = Some(Prompt {
                kind: PromptKind::Discard,
                title: format!(
                    "Unsaved edits in {} LIBs: type DISCARD to close",
                    usize::from(self.doc.dirty())
                        + self.libraries.iter().filter(|l| l.doc.dirty()).count()
                ),
                value: String::new(),
                axis: 0,
            });
        } else {
            self.quit = true;
        }
    }
    fn left(&self) -> i32 {
        if self.width < 1050 {
            210
        } else {
            280
        }
    }
    fn right(&self) -> i32 {
        self.width - if self.width < 1050 { 270 } else { 322 }
    }
    fn dock_y(&self) -> i32 {
        self.height - if self.height < 700 { 170 } else { 208 }
    }
    pub fn click(&mut self, x: i32, y: i32, button: u8, down: bool) {
        self.mouse = [x, y];
        if button == 1 && !down {
            if self.mesh_drag.is_some() {
                let result = self.finish_mesh_drag();
                self.result(result);
                return;
            }
            if self.hp_drag.is_some() {
                let result = self.finish_station_drag();
                self.result(result);
                return;
            }
            self.decal_dragging = false;
            if let Some((library, entry, start)) = self.resource_drag.take() {
                if self.prompt.is_none()
                    && library == self.library_id
                    && (x - start[0]).abs() + (y - start[1]).abs() > 6
                {
                    let target = self
                        .layout()
                        .hits
                        .into_iter()
                        .rev()
                        .find(|h| h.contains(x, y))
                        .map(|h| h.action);
                    let result = (|| {
                        match target {
                            Some(view::Action::Library(id)) if id != library => {
                                self.selected = entry;
                                self.copy_resource()?;
                                self.switch_library(id)?;
                                self.paste_resources()?;
                            }
                            Some(view::Action::Entry(to))
                                if to != entry
                                    && extension(&self.doc.archive.entries[to].name)
                                        == extension(&self.doc.archive.entries[entry].name) =>
                            {
                                self.selected = entry;
                                self.pin_donor()?;
                                self.select_entry(to);
                                self.mode = Mode::Graft;
                            }
                            _ => {}
                        }
                        Ok(())
                    })();
                    self.result(result);
                }
            }
            self.finish_stroke();
            return;
        }
        if button == 1 && down && self.prompt.is_none() && self.mode == Mode::Model {
            let hp = self
                .layout()
                .hits
                .into_iter()
                .rev()
                .find(|h| h.contains(x, y) && matches!(h.action, view::Action::HardpointSelect(_)))
                .map(|h| h.action);
            if let Some(view::Action::HardpointSelect(i)) = hp {
                self.start_station_drag(i);
                return;
            }
        }
        if button == 1
            && down
            && self.prompt.is_none()
            && self.mode == Mode::Model
            && self.mesh_edit
        {
            let vertex = self
                .layout()
                .hits
                .into_iter()
                .rev()
                .find(|h| h.contains(x, y) && matches!(h.action, view::Action::MeshVertex(_)))
                .map(|h| h.action);
            if let Some(view::Action::MeshVertex(i)) = vertex {
                self.mesh_select(i, true);
                return;
            }
        }
        if button == 1
            && down
            && self.prompt.is_none()
            && self.decal_active
            && x > self.left() + if self.mode == Mode::Media { 12 } else { 40 }
            && x < self.right() - 12
            && y > 84
            && y < self.dock_y() - 20
            && matches!(self.mode, Mode::Model | Mode::Media)
        {
            let result = self.place_decal(x, y);
            self.result(result);
            return;
        }
        if button == 1
            && down
            && self.prompt.is_none()
            && self.mode == Mode::Model
            && self.model.is_some()
            && x > self.left() + 40
            && x < self.right()
            && y > 130
            && y < self.dock_y()
        {
            if let Some((face, uv)) = self.model_hit(x, y) {
                self.hp_tool = false;
                self.selected_face = Some(face);
                self.textured = true;
                self.perspective = false;
                if self.model_paint {
                    self.paint_model_hit(face, uv);
                }
                return;
            }
        }
        if button == 1
            && down
            && self.prompt.is_none()
            && self.mode == Mode::Media
            && self.pic.is_some()
            && self.image_point(x, y).is_some()
            && (self.paint_enabled || self.pick_color)
        {
            self.paint_point(x, y);
            return;
        }
        if button == 2 {
            self.image_drag = down
                && self.mode == Mode::Media
                && self.pic.is_some()
                && x > self.left()
                && x < self.right()
                && y < self.dock_y();
            self.drag = down
                && self.prompt.is_none()
                && ((self.mode == Mode::Model
                    && x > self.left()
                    && x < self.right()
                    && y > 54
                    && y < self.dock_y())
                    || self.image_drag
                    || (self.mode == Mode::Media
                        && self.context_model.is_some()
                        && (x >= self.right() || (self.dock == 3 && y >= self.dock_y()))));
            return;
        }
        if !down {
            return;
        }
        if button == 3 {
            self.menu = None;
            if self.prompt.is_some() {
                self.key(Key::Escape, false, false);
            }
            return;
        }
        if button != 1 {
            return;
        }
        let action = self
            .layout()
            .hits
            .into_iter()
            .rev()
            .find(|h| h.contains(x, y))
            .map(|h| h.action);
        if let Some(action) = action {
            self.resource_drag = if x < self.left() {
                if let view::Action::Entry(i) = action {
                    Some((self.library_id, i, [x, y]))
                } else {
                    None
                }
            } else {
                None
            };
            self.act(action);
        } else {
            self.menu = None;
            self.filter_focus = false;
        }
    }
    pub fn motion(&mut self, x: i32, y: i32, shift: bool) {
        if self.mesh_drag.is_some() {
            let result = self.mesh_motion(x, y);
            self.result(result);
            self.mouse = [x, y];
            return;
        }
        if self.decal_dragging {
            let result = self.place_decal(x, y);
            if let Err(e) = result {
                self.status = e;
            }
            self.mouse = [x, y];
            return;
        }
        if let Some(reference) = self.hp_drag.as_ref().map(|d| d.reference) {
            match self.cursor_station(x, y, reference) {
                Ok(p) => self.hp_drag.as_mut().unwrap().position = p,
                Err(e) => self.status = e,
            }
            self.mouse = [x, y];
            return;
        }
        if let Some((_, _, start)) = self.resource_drag {
            if (x - start[0]).abs() + (y - start[1]).abs() > 6 {
                self.status = "Drop on a LIB to copy / same-type entry to graft".into();
            }
        }

        if self.painting {
            if self.mode == Mode::Model {
                if let Some((face, uv)) = self.model_hit(x, y) {
                    if Some(face) == self.selected_face {
                        self.paint_model_hit(face, uv);
                    }
                }
            } else {
                self.paint_point(x, y);
            }
        }
        if self.drag {
            let dx = x - self.mouse[0];
            let dy = y - self.mouse[1];
            if self.image_drag {
                self.image_pan[0] = (self.image_pan[0] + dx).clamp(-16000, 16000);
                self.image_pan[1] = (self.image_pan[1] + dy).clamp(-16000, 16000);
            } else if shift {
                self.pan[0] = (self.pan[0] + dx).clamp(-10000, 10000);
                self.pan[1] = (self.pan[1] + dy).clamp(-10000, 10000);
            } else {
                self.yaw = (self.yaw + dx).rem_euclid(360);
                self.pitch = (self.pitch + dy).clamp(-90, 90);
            }
        }
        self.mouse = [x, y];
    }
    pub fn wheel(&mut self, delta: i32) {
        if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::TransferReview))
        {
            let len = self.transfer_plan.as_ref().map_or(0, |p| p.items.len());
            self.transfer_scroll = (self.transfer_scroll as i32 - delta * 3)
                .clamp(0, len.saturating_sub(1) as i32) as usize;
            return;
        }

        if self
            .prompt
            .as_ref()
            .is_some_and(|p| matches!(p.kind, PromptKind::CloneReview))
        {
            let len = self.clone_draft.as_ref().map_or(0, |p| p.mapping.len());
            self.clone_scroll = (self.clone_scroll as i32 - delta * 3)
                .clamp(0, len.saturating_sub(1) as i32) as usize;
            return;
        }
        if self.prompt.is_some() {
            if let Some(b) = &mut self.browser {
                b.scroll = (b.scroll as i32 - delta * 3)
                    .clamp(0, b.files.len().saturating_sub(1) as i32)
                    as usize;
            }
            return;
        }
        if self.mode == Mode::Properties
            && self.field_group == Some(hangar_core::definition::Aspect::Envelope)
            && !self.envelope_rows().is_empty()
            && self.mouse[0] > self.left()
            && self.mouse[0] < self.right()
            && self.mouse[1] < self.dock_y()
        {
            let visible = ((self.dock_y() - 26 - 164) / 26).max(1) as usize;
            self.envelope_scroll = (self.envelope_scroll as i32 - delta * 3)
                .clamp(0, 20usize.saturating_sub(visible) as i32)
                as usize;
            return;
        }
        if self.mode == Mode::Graft
            && self.mouse[0] > self.left()
            && self.mouse[0] < self.right()
            && self.mouse[1] < self.dock_y()
        {
            self.graft_scroll = (self.graft_scroll as i32 - delta * 3)
                .clamp(0, self.graft_row_count().saturating_sub(1) as i32)
                as usize;
            return;
        }
        if self.mode == Mode::Package {
            if self.mouse[0] < self.left() {
                let rows = ((self.height - 220) / 24).max(1) as usize;
                self.changes_scroll = (self.changes_scroll as i32 - delta * 3)
                    .clamp(0, self.doc.changes().len().saturating_sub(rows) as i32)
                    as usize;
            } else if self.mouse[0] < self.right() {
                let rows = ((self.height - 192) / 20).max(1) as usize;
                self.validation_scroll = (self.validation_scroll as i32 - delta * 3)
                    .clamp(0, self.validation_lines().len().saturating_sub(rows) as i32)
                    as usize;
            }
            return;
        }
        if self.dock == 4
            && self.mouse[0] > self.left()
            && self.mouse[0] < self.right()
            && self.mouse[1] >= self.dock_y()
        {
            let rows = ((self.height - self.dock_y() - 84) / 22).max(1) as usize;
            self.reference_scroll = (self.reference_scroll as i32 - delta * 3)
                .clamp(0, self.reference_rows().len().saturating_sub(rows) as i32)
                as usize;
            return;
        }
        if self.mode == Mode::Media
            && self.pic.is_some()
            && self.mouse[0] > self.left()
            && self.mouse[0] < self.right()
            && self.mouse[1] < self.dock_y()
        {
            self.image_zoom = if delta > 0 {
                self.image_zoom * 125 / 100
            } else {
                self.image_zoom * 80 / 100
            }
            .clamp(25, 1600);
            return;
        }
        if self.mode == Mode::Media
            && self.context_model.is_some()
            && (self.mouse[0] >= self.right() || self.mouse[1] >= self.dock_y())
        {
            self.zoom = (self.zoom + delta * 10).clamp(10, 1000);
            return;
        }
        if self.mouse[0] < self.left() {
            let count = self.tree_rows().len();
            let rows = ((self.height - self.tree_start() - 54) / 20).max(1) as usize;
            self.scroll = (self.scroll as i32 - delta * 3)
                .clamp(0, count.saturating_sub(rows) as i32) as usize;
        } else if self.mouse[0] >= self.right() {
            self.inspector_scroll =
                (self.inspector_scroll - delta * 3).clamp(0, self.inspector_max_scroll());
        } else if self.mouse[1] > self.dock_y()
            || matches!(self.mode, Mode::Properties | Mode::Graft)
        {
            let n = self.brf.as_ref().map_or(0, |b| {
                b.fields
                    .iter()
                    .filter(|f| {
                        self.mode != Mode::Properties
                            || self.mouse[1] >= self.dock_y()
                            || self.field_group.is_none_or(|group| {
                                hangar_core::definition::aspect(&f.label) == Some(group)
                            })
                    })
                    .count()
            });
            self.field_scroll = (self.field_scroll as i32 - delta * 3)
                .clamp(0, n.saturating_sub(1) as i32) as usize;
        } else if self.mode == Mode::Browse {
            let n = self.browser_entries().len();
            let rows = ((self.dock_y() - 86) / 24).max(1) as usize;
            self.table_scroll = (self.table_scroll as i32 - delta * 3)
                .clamp(0, n.saturating_sub(rows) as i32) as usize;
        } else {
            self.zoom = (self.zoom + delta * 10).clamp(10, 1000);
        }
    }
    fn viewport(&self, d: &mut Canvas, m: &Model, x: i32, y: i32, w: i32, h: i32) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for v in &self.model.as_ref().unwrap_or(m).vertices {
            for j in 0..3 {
                min[j] = min[j].min(v.point[j]);
                max[j] = max[j].max(v.point[j]);
            }
        }
        let center = core::array::from_fn::<_, 3, _>(|i| (min[i] + max[i]) / 2);
        let span = (0..3).map(|i| max[i] - min[i]).max().unwrap_or(1).max(1);
        let project = |p: [i32; 3]| {
            let p = [p[0] - center[0], p[2] - center[2], p[1] - center[1]];
            let p = model::rotate(model::rotate(p, 1, self.yaw), 0, self.pitch);
            let denom = if self.perspective {
                (span * 4 - p[2]).max(span)
            } else {
                span * 4
            };
            [
                x + w / 2
                    + self.pan[0]
                    + (p[0] as i64 * w.min(h) as i64 * self.zoom as i64 * 3 / (denom as i64 * 100))
                        as i32,
                y + h / 2 + self.pan[1]
                    - (p[1] as i64 * w.min(h) as i64 * self.zoom as i64 * 3 / (denom as i64 * 100))
                        as i32,
            ]
        };
        let line = |d: &mut Canvas, a: [i32; 2], b: [i32; 2], color: Rgb| {
            if let Some((a, b)) = clip(a, b, [x, y, x + w - 1, y + h - 1]) {
                d.line(a[0], a[1], b[0], b[1], color);
            }
        };
        for i in -40..=40 {
            let a = i * span / 10;
            line(
                d,
                project([a + center[0], center[1] - span * 4, 0]),
                project([a + center[0], center[1] + span * 4, 0]),
                c::GM_800,
            );
            line(
                d,
                project([center[0] - span * 4, a + center[1], 0]),
                project([center[0] + span * 4, a + center[1], 0]),
                c::GM_800,
            );
        }
        let mut edges = BTreeMap::<([i32; 3], [i32; 3]), (usize, bool, bool)>::new();
        for f in &m.faces {
            let a = project(m.vertices[f.indices[0]].point);
            let b = project(m.vertices[f.indices[1]].point);
            let c = project(m.vertices[f.indices[2]].point);
            let front = (b[0] as i64 - a[0] as i64) * (c[1] as i64 - a[1] as i64)
                - (b[1] as i64 - a[1] as i64) * (c[0] as i64 - a[0] as i64)
                > 0;
            for i in 0..f.indices.len() {
                let a = m.vertices[f.indices[i]].point;
                let b = m.vertices[f.indices[(i + 1) % f.indices.len()]].point;
                let edge = edges
                    .entry(if a < b { (a, b) } else { (b, a) })
                    .or_insert((0, false, false));
                edge.0 += 1;
                edge.1 |= front;
                edge.2 |= !front;
            }
        }
        for ((a, b), (n, front, back)) in edges {
            line(
                d,
                project(a),
                project(b),
                if n == 1 || (front && back) {
                    c::AMBER
                } else {
                    c::GM_500
                },
            );
        }
        for (j, color) in [c::AXIS_X, c::AXIS_Y, c::AXIS_Z].into_iter().enumerate() {
            let mut p = center;
            p[j] += span / 3;
            line(d, project(center), project(p), color);
        }
    }
}
// Integer Cohen-Sutherland clipping prevents models drawing over adjacent editors.
fn clip(mut a: [i32; 2], mut b: [i32; 2], r: [i32; 4]) -> Option<([i32; 2], [i32; 2])> {
    let code = |p: [i32; 2]| {
        u8::from(p[0] < r[0])
            | (u8::from(p[0] > r[2]) * 2)
            | (u8::from(p[1] < r[1]) * 4)
            | (u8::from(p[1] > r[3]) * 8)
    };
    for _ in 0..8 {
        let ca = code(a);
        let cb = code(b);
        if ca | cb == 0 {
            return Some((a, b));
        }
        if ca & cb != 0 {
            return None;
        }
        let c = if ca != 0 { ca } else { cb };
        let dx = b[0] as i64 - a[0] as i64;
        let dy = b[1] as i64 - a[1] as i64;
        let p = if c & 12 != 0 {
            let y = if c & 8 != 0 { r[3] } else { r[1] };
            if dy == 0 {
                return None;
            }
            [a[0] + ((y as i64 - a[1] as i64) * dx / dy) as i32, y]
        } else {
            let x = if c & 2 != 0 { r[2] } else { r[0] };
            if dx == 0 {
                return None;
            }
            [x, a[1] + ((x as i64 - a[0] as i64) * dy / dx) as i32]
        };
        if ca != 0 {
            a = p;
        } else {
            b = p;
        }
    }
    None
}

#[path = "ui_view.rs"]
mod view;

#[path = "ui_browser.rs"]
mod browser_ui;
#[path = "ui_media.rs"]
mod media;

#[path = "ui_clone.rs"]
mod cloning_ui;

#[path = "ui_dependencies.rs"]
mod dependencies_ui;

#[path = "ui_graft.rs"]
mod grafting_ui;

#[path = "ui_libraries.rs"]
mod libraries_ui;

#[path = "ui_hardpoints.rs"]
mod hardpoint_ui;
#[path = "ui_materials.rs"]
mod material_ui;

#[path = "ui_envelope.rs"]
mod envelope_ui;

#[path = "ui_color.rs"]
mod color_ui;

#[path = "ui_mesh.rs"]
mod mesh_ui;
