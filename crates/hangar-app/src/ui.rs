use alloc::{
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
    Png,
    Wav,
    Palette,
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
    Recolor,
    Isolate,
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
    scroll: usize,
    field_scroll: usize,
    field_selected: usize,
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
    validation: Option<String>,
    browser: Option<Browser>,
    recent: Vec<String>,
    pic: Option<Pic>,
    base_palette: [[u8; 3]; 256],
    palette_loaded: bool,
    palette_override: Option<[[u8; 3]; 256]>,
    image_zoom: i32,
    image_pan: [i32; 2],
    image_drag: bool,
    brush: u8,
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
            scroll: 0,
            field_scroll: 0,
            field_selected: 0,
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
            browser: None,
            recent: crate::platform::load_recent(),
            pic: None,
            base_palette: core::array::from_fn(|i| [i as u8; 3]),
            palette_loaded: false,
            palette_override: None,
            image_zoom: 100,
            image_pan: [0, 0],
            image_drag: false,
            brush: 150,
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
            .push(Entry::new("DEMO.PT", hangar_core::brf::demo()).unwrap());
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
        self.doc = Document::new(a);
        self.context_model = None;
        self.context_entry = None;
        self.selected_face = None;
        self.model_paint = false;
        self.required.clear();
        self.path = "Synthetic demo (not a game asset)".into();
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
        if self.doc.dirty() {
            return Err("Save your changes before opening another LIB".into());
        }
        let a = Archive::parse(crate::platform::read(path)?)?;
        self.doc = Document::new(a);
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
        self.refresh();
        self.frame();
        self.remember(path);
        self.status = format!(
            "Opened {} | {} entries",
            path,
            self.doc.archive.entries.len()
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
        let rows = ((self.height - 132) / 20).max(1) as usize;
        if position < self.scroll || position >= self.scroll + rows {
            self.scroll = position.saturating_sub(rows / 2);
        }
        self.refresh();
        self.frame();
        if matches!(extension(self.name()), "PIC" | "5K" | "11K" | "WAV") {
            self.mode = Mode::Media;
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
        crate::platform::stop_audio();
        self.pic = None;
        self.textures.clear();
        self.painting = false;
        self.last_paint = None;
        self.paint_enabled = false;
        self.base_palette = self
            .palette_override
            .unwrap_or_else(|| core::array::from_fn(|i| [i as u8; 3]));
        self.palette_loaded = self.palette_override.is_some();
        if let Some(i) = self.doc.archive.find("PALETTE.PAL") {
            if let Ok(b) = self.doc.archive.entries[i].read() {
                if let Ok(p) = picture::palette(&b) {
                    self.base_palette = p;
                    self.palette_loaded = true;
                }
            }
        }

        self.brf = None;
        self.model = None;
        self.model_entry = None;
        self.original_brf = None;
        self.inspector_scroll = 0;
        self.validation = None;
        self.preview = None;
        self.data.clear();
        self.field_scroll = 0;
        self.field_selected = 0;
        self.detail.clear();
        self.selected = self
            .selected
            .min(self.doc.archive.entries.len().saturating_sub(1));
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
        if let Some(brf) = &self.brf {
            if let Some(ptr) = brf
                .fields
                .iter()
                .find(|f| f.label == "object.shape" && f.kind == "ptr")
            {
                if let Some(field) = brf
                    .fields
                    .iter()
                    .find(|f| f.block == ptr.value && f.kind == "string")
                {
                    let name = field.value.trim_matches('"');
                    if let Some(index) = self.doc.archive.find(name) {
                        if let Ok(bytes) = self.doc.archive.entries[index].read() {
                            if let Ok(model) = Model::parse(&bytes) {
                                self.model = Some(model);
                                self.model_entry = Some(index);
                            }
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
                if let Some(i) = self.doc.archive.find(&key) {
                    if let Ok(bytes) = self.doc.archive.entries[i].read() {
                        if let Ok(p) = Pic::parse(&bytes) {
                            self.textures.insert(name.clone(), p);
                        }
                    }
                }
            }
        }
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
    pub fn file_prompt(&mut self, a: FileAction) {
        if matches!(a, FileAction::Variant) && (self.doc.dirty() || !self.name().ends_with(".PT")) {
            self.status = "Select a PT donor in a saved LIB before creating a new aircraft".into();
            return;
        }
        if matches!(a, FileAction::Open) && self.doc.dirty() {
            self.status = "Save your changes before opening another LIB".into();
            return;
        }
        let title = match a {
            FileAction::Png => "Export picture as PNG",
            FileAction::Wav => "Export sound as WAV",
            FileAction::Palette => "Load display palette (.PAL)",
            FileAction::Open => "Open LIB",
            FileAction::Variant => "New aircraft / step 1: imported main SH path",
            FileAction::Save => "Package LIB: new output path",
            FileAction::Import => "Add entry: path to a resource file",
            FileAction::Replace => "Replace selected entry: resource file path",
            FileAction::Export => "Export selected entry: new file path",
            FileAction::Obj => "Export static geometry: new OBJ path",
            FileAction::Graft => "Copy selected field from same entry in donor LIB: donor path",
        };
        let value = match a {
            FileAction::Save => "HANGAR.LIB".into(),
            FileAction::Png => format!("{}.png", self.name().split('.').next().unwrap()),
            FileAction::Wav => format!("{}.wav", self.name().split('.').next().unwrap()),
            FileAction::Export => self.name().into(),
            FileAction::Obj => format!("{}.obj", self.name().split('.').next().unwrap_or("model")),
            _ => String::new(),
        };
        let folder = if let Some(p) = self.recent.first() {
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
            FileAction::Open => self.open(path),
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
                self.palette_override = Some(picture::palette(&crate::platform::read(path)?)?);
                self.refresh();
                self.status =
                    "Display palette loaded; original resource palettes remain unchanged".into();
                Ok(())
            }
            FileAction::Variant => {
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
                Archive::parse(b.clone())?;
                crate::platform::write_new(path, &b)?;
                self.doc.mark_saved();
                let field = self.field_selected;
                let scroll = self.field_scroll;
                self.refresh();
                self.field_selected = field;
                self.field_scroll = scroll;
                self.path = path.into();
                self.remember(path);
                self.status = format!(
                    "Packaged {} entries into {path}",
                    self.doc.archive.entries.len()
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
                    self.prompt = None;
                    self.preview = None;
                    self.status = "Cancelled".into();
                }
                Key::Enter => {
                    if let Some(p) = &self.prompt {
                        if matches!(p.kind, PromptKind::File(_))
                            && crate::platform::is_dir(p.value.trim())
                        {
                            let folder = p.value.trim().to_string();
                            self.browse_folder(&folder);
                            return;
                        }
                    }
                    let p = self.prompt.take().unwrap();
                    let r = match p.kind {
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
                        PromptKind::VariantReview => {
                            if p.value == "CREATE" {
                                if let Some(v) = self.variant_draft.take() {
                                    self.required = v.missing_textures;
                                    self.status=format!("New aircraft {} | {} shared stock references | {} missing textures",self.variant_id,v.shared.len(),self.required.len());
                                    self.doc = Document::new(v.archive);
                                    self.doc.mark_unsaved();
                                    self.path = format!(
                                        "New {}.LIB from {} (not saved)",
                                        self.variant_id, v.donor
                                    );
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
                        }
                        PromptKind::Discard => {
                            if p.value == "DISCARD" {
                                self.quit = true;
                                Ok(())
                            } else {
                                Err("Type DISCARD or press Esc".into())
                            }
                        }
                        PromptKind::File(a) => self.perform_file(a, p.value.trim()),
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
                    if matches!(p.kind, PromptKind::Transform(_)) && "xyzXYZ".contains(ch) {
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
            Key::Char(ch) if ctrl=>match ch.to_ascii_lowercase(){'o'=>self.file_prompt(FileAction::Open),'s'=>self.file_prompt(FileAction::Save),'i'=>self.file_prompt(FileAction::Import),'e'=>self.file_prompt(FileAction::Export),'f'=>self.filter_focus=true,'b'=>{self.mode=Mode::Package;self.file_prompt(FileAction::Save);},'z'=>{if shift{self.doc.redo();}else{self.doc.undo();}self.refresh();self.status=self.doc.summary();},'y'=>{self.doc.redo();self.refresh();},_=>{}},
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
            Key::F1=>self.status="Ctrl+O open | Ctrl+S package | Ctrl+I add | Ctrl+E export | Ctrl+Z undo | MMB orbit | Shift+MMB pan | Wheel zoom | G/R/S transform".into(),
            _=>{}
        }
    }
    pub fn close(&mut self) {
        self.finish_stroke();
        crate::platform::stop_audio();
        if self.doc.dirty() {
            self.prompt = Some(Prompt {
                kind: PromptKind::Discard,
                title: "Unsaved changes: type DISCARD to close, or Esc to keep editing".into(),
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
            self.finish_stroke();
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
                        && (x >= self.right() || y >= self.dock_y())));
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
            self.act(action);
        } else {
            self.menu = None;
            self.filter_focus = false;
        }
    }
    pub fn motion(&mut self, x: i32, y: i32, shift: bool) {
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
        if self.prompt.is_some() {
            if let Some(b) = &mut self.browser {
                b.scroll = (b.scroll as i32 - delta * 3)
                    .clamp(0, b.files.len().saturating_sub(1) as i32)
                    as usize;
            }
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
            let rows = ((self.height - 132) / 20).max(1) as usize;
            self.scroll = (self.scroll as i32 - delta * 3)
                .clamp(0, count.saturating_sub(rows) as i32) as usize;
        } else if self.mouse[0] >= self.right() {
            self.inspector_scroll =
                (self.inspector_scroll - delta * 3).clamp(0, self.inspector_max_scroll());
        } else if self.mouse[1] > self.dock_y()
            || matches!(self.mode, Mode::Properties | Mode::Graft)
        {
            let n = self.brf.as_ref().map_or(0, |b| b.fields.len());
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
