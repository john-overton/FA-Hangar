use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use hangar_core::{
    archive::{Archive, Entry},
    authoring::{self, Variant},
    brf::Brf,
    document::Document,
    model::{self, Model, Transform},
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
    pub fn text(&mut self, x: i32, y: i32, s: &str, color: Rgb) {
        self.commands.push(Draw::Text(x, y, s.into(), color.0));
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Browse,
    Model,
    Properties,
    Package,
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
            yaw: -25,
            pitch: 22,
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
        }
    }
    pub fn demo(&mut self) {
        if self.doc.dirty() {
            self.status = "Save your changes before loading another LIB".into();
            return;
        }
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("DEMO.SH", model::demo_shape()).unwrap());
        a.entries
            .push(Entry::new("DEMO.PT", hangar_core::brf::demo()).unwrap());
        for suffix in ["A", "B", "C", "D", "S"] {
            a.entries
                .push(Entry::new(&format!("DEMO_{suffix}.SH"), model::demo_shape()).unwrap());
        }
        self.doc = Document::new(a);
        self.required.clear();
        self.path = "Synthetic demo (not a game asset)".into();
        self.selected = 0;
        self.refresh();
        self.status =
            "Synthetic demo | G/R/S transforms, X/Y/Z axis, Enter apply, Esc cancel".into();
    }
    pub fn open(&mut self, path: &str) -> Result<()> {
        if self.doc.dirty() {
            return Err("Save your changes before opening another LIB".into());
        }
        let a = Archive::parse(crate::platform::read(path)?)?;
        self.doc = Document::new(a);
        self.required.clear();
        self.path = path.into();
        self.selected = 0;
        self.scroll = 0;
        self.filter.clear();
        self.refresh();
        self.status = format!(
            "Opened {} | {} entries",
            path,
            self.doc.archive.entries.len()
        );
        Ok(())
    }
    pub fn select_entry(&mut self, index: usize) {
        self.selected = index.min(self.doc.archive.entries.len().saturating_sub(1));
        let rows = ((self.height - 142) / 20).max(1) as usize;
        if self.selected < self.scroll || self.selected >= self.scroll + rows {
            self.scroll = self.selected.saturating_sub(rows / 2);
        }
        self.refresh();
    }
    fn name(&self) -> &str {
        self.doc
            .archive
            .entries
            .get(self.selected)
            .map_or("No entry", |e| e.name.as_str())
    }
    fn refresh(&mut self) {
        self.brf = None;
        self.model = None;
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
                    if ext == "SH" {
                        match Model::parse(&data) {
                            Ok(m) => {
                                self.detail = if m.writable {
                                    "Static geometry: editable".into()
                                } else {
                                    m.reason.clone()
                                };
                                self.model = Some(m);
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
        self.frame();
    }
    fn frame(&mut self) {
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
            FileAction::Open => "Open LIB: full path",
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
            FileAction::Export => self.name().into(),
            FileAction::Obj => format!("{}.obj", self.name().split('.').next().unwrap_or("model")),
            _ => String::new(),
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::File(a),
            title: title.into(),
            value,
            axis: 0,
        });
        self.filter_focus = false;
    }
    fn perform_file(&mut self, a: FileAction, path: &str) -> Result<()> {
        if path.is_empty() {
            return Err("Enter a file path".into());
        }
        match a {
            FileAction::Open => self.open(path),
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
                self.path = path.into();
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
        if self.prompt.is_some() {
            match key {
                Key::Escape => {
                    self.prompt = None;
                    self.preview = None;
                    self.status = "Cancelled".into();
                }
                Key::Enter => {
                    let p = self.prompt.take().unwrap();
                    let r = match p.kind {
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
                    self.scroll = 0;
                }
                Key::Char(c) if !ctrl => {
                    self.filter.push(c);
                    self.scroll = 0;
                }
                _ => {}
            }
            return;
        }
        match key {
            Key::Char(ch) if ctrl=>match ch.to_ascii_lowercase(){'o'=>self.file_prompt(FileAction::Open),'s'=>self.file_prompt(FileAction::Save),'i'=>self.file_prompt(FileAction::Import),'e'=>self.file_prompt(FileAction::Export),'f'=>self.filter_focus=true,'z'=>{if shift{self.doc.redo();}else{self.doc.undo();}self.refresh();self.status=self.doc.summary();},'y'=>{self.doc.redo();self.refresh();},_=>{}},
            Key::Char(ch) if "gGrRsS".contains(ch)&&self.model.is_some()=>{
                if self.model.as_ref().is_some_and(|m|!m.writable){self.status=self.model.as_ref().unwrap().reason.clone();return;}
                let op=ch.to_ascii_lowercase();self.prompt=Some(Prompt{kind:PromptKind::Transform(op),title:match op{'g'=>"Move in source units",'r'=>"Rotate in degrees",_=>"Scale in percent"}.into(),value:String::new(),axis:0});self.transform_preview();
            },
            Key::Char('1')|Key::Num(1)=>{self.yaw=0;self.pitch=0;},Key::Char('3')|Key::Num(3)=>{self.yaw=90;self.pitch=0;},Key::Char('7')|Key::Num(7)=>{self.yaw=0;self.pitch=90;},Key::Char('5')|Key::Num(5)=>self.perspective = !self.perspective,
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
            260
        }
    }
    fn right(&self) -> i32 {
        self.width - if self.width < 1050 { 260 } else { 322 }
    }
    fn visible(&self) -> Vec<usize> {
        let f = self.filter.to_ascii_uppercase();
        self.doc
            .archive
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.name.contains(&f).then_some(i))
            .collect()
    }
    pub fn click(&mut self, x: i32, y: i32, button: u8, down: bool) {
        self.mouse = [x, y];
        if button == 2 {
            self.drag =
                down && x > self.left() && x < self.right() && y > 82 && y < self.height - 165;
            return;
        }
        if !down {
            return;
        }
        if button == 3 && self.prompt.is_some() {
            self.key(Key::Escape, false, false);
            return;
        }
        if button != 1 {
            return;
        }
        if self.prompt.is_some() {
            return;
        }
        self.filter_focus = false;
        if y < 26 {
            match x {
                0..=164 => self.file_prompt(FileAction::Open),
                165..=294 => self.file_prompt(FileAction::Save),
                295..=370 => {
                    self.doc.undo();
                    self.refresh();
                }
                371..=445 => {
                    self.doc.redo();
                    self.refresh();
                }
                446..=530 => self.demo(),
                531..=700 => self.file_prompt(FileAction::Variant),
                _ => {}
            }
            return;
        }
        if (30..=54).contains(&y) {
            self.mode = match x {
                0..=130 => Mode::Browse,
                131..=245 => Mode::Model,
                246..=390 => Mode::Properties,
                _ => Mode::Package,
            };
            return;
        }
        if x < self.left() {
            if y < 110 {
                self.filter_focus = true;
                return;
            }
            let row = ((y - 116) / 20) as usize + self.scroll;
            if let Some(i) = self.visible().get(row) {
                self.selected = *i;
                self.refresh();
            }
            return;
        }
        if x >= self.right() {
            if (110..138).contains(&y) {
                self.file_prompt(FileAction::Export);
                return;
            }
            if (142..170).contains(&y) {
                self.file_prompt(FileAction::Replace);
                return;
            }
            if (174..202).contains(&y) {
                self.file_prompt(FileAction::Import);
                return;
            }
            if (206..234).contains(&y) && self.model.is_some() {
                self.file_prompt(FileAction::Obj);
                return;
            }
            if y >= 300 && self.brf.is_some() {
                let row = ((y - 300) / 36) as usize + self.field_scroll;
                self.edit_field(row);
                return;
            }
        }
        if self.mode == Mode::Package
            && x > self.left() + 24
            && x < self.left() + 220
            && (250..282).contains(&y)
        {
            self.file_prompt(FileAction::Save);
        }
        if self.mode == Mode::Properties && self.brf.is_some() && y >= 140 && y < self.height - 165
        {
            let row = ((y - 140) / 28) as usize + self.field_scroll;
            self.edit_field(row);
        }
        if y > self.height - 145 && y < self.height - 110 {
            if x < self.left() + 140 {
                self.file_prompt(FileAction::Import);
            } else if x < self.left() + 320 {
                self.file_prompt(FileAction::Graft);
            }
        }
    }
    pub fn motion(&mut self, x: i32, y: i32, shift: bool) {
        if self.drag {
            let dx = x - self.mouse[0];
            let dy = y - self.mouse[1];
            if shift {
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
            return;
        }
        if self.mouse[0] < self.left() {
            let count = self.visible().len();
            let rows = ((self.height - 140) / 20).max(1) as usize;
            self.scroll = (self.scroll as i32 - delta * 3)
                .clamp(0, count.saturating_sub(rows) as i32) as usize;
        } else if self.mouse[0] >= self.right() || self.mode == Mode::Properties {
            let n = self.brf.as_ref().map_or(0, |b| b.fields.len());
            self.field_scroll = (self.field_scroll as i32 - delta * 3)
                .clamp(0, n.saturating_sub(1) as i32) as usize;
        } else {
            self.zoom = (self.zoom + delta * 10).clamp(10, 1000);
        }
    }
    fn button(canvas: &mut Canvas, x: i32, y: i32, w: i32, label: &str) {
        canvas.rect(x, y, w, 22, c::GM_700);
        canvas.line(x, y, x + w, y, c::GM_600);
        canvas.text(x + 8, y + 15, label, c::INK_MUTED);
    }
    pub fn draw(&self) -> Canvas {
        let mut d = Canvas {
            commands: Vec::new(),
        };
        let w = self.width;
        let h = self.height;
        let l = self.left();
        let r = self.right();
        let vw = r - l;
        d.rect(0, 0, w, h, c::GM_900);
        d.rect(0, 0, w, 26, c::GM_950);
        d.text(12, 18, "TORE / Open LIB", c::INK);
        d.text(175, 18, "Package LIB", c::AMBER);
        d.text(308, 18, "Undo", c::INK_MUTED);
        d.text(383, 18, "Redo", c::INK_MUTED);
        d.text(458, 18, "Demo", c::INK_MUTED);
        d.text(544, 18, "New aircraft", c::STEEL);
        d.text(w - 170, 18, "HANGAR  /  0.1", c::INK_FAINT);
        for (x, size, label, mode) in [
            (8, 122, "Browse", Mode::Browse),
            (132, 110, "Model", Mode::Model),
            (244, 144, "Properties", Mode::Properties),
            (390, 120, "Package", Mode::Package),
        ] {
            if self.mode == mode {
                d.rect(x, 30, size, 24, c::AMBER_DEEP);
                d.line(x, 53, x + size, 53, c::AMBER);
            }
            d.text(
                x + 14,
                47,
                label,
                if self.mode == mode {
                    c::AMBER
                } else {
                    c::INK_MUTED
                },
            );
        }
        d.rect(0, 56, l, h - 78, c::GM_800);
        d.rect(r, 56, w - r, h - 78, c::GM_800);
        d.rect(l + 1, 56, vw - 2, h - 78, c::GM_950);
        d.rect(0, 56, l, 28, c::GM_700);
        d.text(12, 75, "LIB ENTRIES", c::INK_MUTED);
        d.text(
            l - 55,
            75,
            &format!("{}", self.doc.archive.entries.len()),
            c::INK_FAINT,
        );
        d.rect(8, 90, l - 16, 20, c::GM_950);
        d.text(
            14,
            104,
            &short(
                &if self.filter.is_empty() {
                    "Search entries...  Ctrl+F".into()
                } else {
                    self.filter.clone()
                },
                ((l - 26) / 7) as usize,
            ),
            if self.filter_focus {
                c::AMBER
            } else {
                c::INK_FAINT
            },
        );
        if self.filter_focus {
            d.line(8, 110, l - 8, 110, c::FOCUS);
        }
        let rows = ((h - 142) / 20).max(0) as usize;
        for (row, i) in self
            .visible()
            .iter()
            .skip(self.scroll)
            .take(rows)
            .enumerate()
        {
            let e = &self.doc.archive.entries[*i];
            let y = 116 + row as i32 * 20;
            if *i == self.selected {
                d.rect(4, y, l - 8, 20, c::AMBER_DEEP);
                d.rect(4, y, 2, 20, c::AMBER);
            } else if row % 2 == 1 {
                d.rect(4, y, l - 8, 20, c::GM_900);
            }
            d.text(
                16,
                y + 14,
                &short(&e.name, 18),
                if *i == self.selected {
                    c::AMBER_BRIGHT
                } else {
                    c::INK
                },
            );
            d.text(l - 42, y + 14, extension(&e.name), c::INK_FAINT);
        }
        d.rect(l + 1, 56, vw - 2, 28, c::GM_800);
        d.text(l + 14, 75, &short(self.name(), 20), c::INK);
        d.text(
            (r - 165).max(l + 175),
            75,
            if self.perspective {
                "Perspective"
            } else {
                "Orthographic"
            },
            c::INK_FAINT,
        );
        d.rect(r, 56, w - r, 28, c::GM_700);
        d.text(r + 14, 75, "ENTRY / INSPECTOR", c::INK_MUTED);
        d.text(r + 14, 102, &short(self.name(), 32), c::AMBER);
        Self::button(&mut d, r + 12, 114, w - r - 24, "Export entry   Ctrl+E");
        Self::button(&mut d, r + 12, 146, w - r - 24, "Replace entry");
        Self::button(&mut d, r + 12, 178, w - r - 24, "Add entry      Ctrl+I");
        if self.model.is_some() {
            Self::button(&mut d, r + 12, 210, w - r - 24, "Export geometry as OBJ");
        }
        d.text(
            r + 14,
            260,
            &format!("{} bytes decoded", self.data.len()),
            c::INK_MUTED,
        );
        d.text(r + 14, 282, "FIELDS / SOURCE UNITS", c::INK_FAINT);
        if let Some(b) = &self.brf {
            for (row, (i, f)) in b
                .fields
                .iter()
                .enumerate()
                .skip(self.field_scroll)
                .take(((h - 332) / 36).max(0) as usize)
                .enumerate()
            {
                let y = 300 + row as i32 * 36;
                d.text(
                    r + 14,
                    y + 12,
                    &short(&f.label, ((w - r - 28) / 7) as usize),
                    c::INK_MUTED,
                );
                d.rect(
                    r + 12,
                    y + 16,
                    w - r - 24,
                    18,
                    if i == self.field_selected {
                        c::AMBER_DEEP
                    } else {
                        c::GM_950
                    },
                );
                d.text(
                    r + 20,
                    y + 29,
                    &short(&f.value, ((w - r - 40) / 7) as usize),
                    c::INK,
                );
            }
        } else if let Some(m) = &self.model {
            d.text(
                r + 14,
                312,
                &format!("{} vertices", m.vertices.len()),
                c::INK,
            );
            d.text(r + 14, 338, &format!("{} faces", m.faces.len()), c::INK);
            d.text(
                r + 14,
                376,
                if m.writable {
                    "G  Move   R  Rotate   S  Scale"
                } else {
                    "Read-only static pose"
                },
                if m.writable { c::AMBER } else { c::STEEL },
            );
            d.text(r + 14, 400, "MMB orbit / Shift+MMB pan", c::INK_MUTED);
            d.text(r + 14, 422, "Wheel zoom / Home frame", c::INK_MUTED);
            d.text(r + 14, 444, "1 front / 3 side / 7 top", c::INK_MUTED);
        }
        match self.mode {
            Mode::Package => {
                d.text(l + 24, 130, "PACKAGE LIB", c::AMBER);
                d.text(
                    l + 24,
                    165,
                    &format!("{} entries ready", self.doc.archive.entries.len()),
                    c::INK,
                );
                d.text(
                    l + 24,
                    194,
                    "Unchanged compression is preserved.",
                    c::INK_MUTED,
                );
                d.text(
                    l + 24,
                    218,
                    "Writes a new file; existing files are protected.",
                    c::INK_MUTED,
                );
                Self::button(&mut d, l + 24, 254, 196, "Choose output path");
            }
            Mode::Properties if self.brf.is_some() => {
                d.text(
                    l + 18,
                    112,
                    "FIELD                             VALUE",
                    c::INK_FAINT,
                );
                for (row, f) in self
                    .brf
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .skip(self.field_scroll)
                    .take(((h - 315) / 28).max(0) as usize)
                    .enumerate()
                {
                    let y = 140 + row as i32 * 28;
                    if row % 2 == 0 {
                        d.rect(l + 8, y, vw - 16, 26, c::GM_900);
                    }
                    d.text(
                        l + 18,
                        y + 18,
                        &short(&f.label, ((vw * 2 / 3 - 32) / 7).max(3) as usize),
                        c::INK_MUTED,
                    );
                    d.text(
                        l + vw * 2 / 3,
                        y + 18,
                        &short(&f.value, ((vw / 3 - 16) / 7).max(3) as usize),
                        c::INK,
                    );
                }
            }
            Mode::Browse => {
                d.text(l + 18, 116, "RESOURCE BYTES / HEX", c::INK_FAINT);
                for (row, bytes) in self
                    .data
                    .chunks(12)
                    .take(((h - 300) / 20).max(0) as usize)
                    .enumerate()
                {
                    let mut s = format!("{:06X}  ", row * 12);
                    for b in bytes {
                        s.push_str(&format!("{b:02X} "));
                    }
                    d.text(
                        l + 18,
                        148 + row as i32 * 20,
                        &short(&s, ((vw - 32) / 7).max(1) as usize),
                        c::INK_MUTED,
                    );
                }
            }
            _ => {
                if let Some(m) = self.preview.as_ref().or(self.model.as_ref()) {
                    self.viewport(&mut d, m, l + 1, 84, vw - 2, h - 250);
                } else {
                    d.text(l + 28, 150, "TORE HANGAR", c::INK);
                    d.text(
                        l + 28,
                        184,
                        "A workshop for Fighters Anthology LIBs",
                        c::INK_MUTED,
                    );
                    d.text(l + 28, 230, "Open LIB  /  Ctrl+O", c::AMBER);
                    d.text(
                        l + 28,
                        260,
                        "Select SH for a model; PT/JT/OT for fields.",
                        c::INK_MUTED,
                    );
                    d.text(
                        l + 28,
                        292,
                        "Click Properties to edit definition values.",
                        c::INK_MUTED,
                    );
                    d.text(
                        l + 28,
                        334,
                        "Demo contains synthetic resources only.",
                        c::INK_FAINT,
                    );
                }
            }
        }
        let dock = h - 164;
        d.rect(l + 1, dock, vw - 2, 142, c::GM_800);
        d.line(l, dock, r, dock, c::GM_1000);
        Self::button(&mut d, l + 14, dock + 18, 120, "Add entry");
        Self::button(&mut d, l + 146, dock + 18, 164, "Copy donor field");
        d.text(
            l + 14,
            dock + 66,
            &short(&self.detail, ((vw - 28) / 7).max(1) as usize),
            c::INK_MUTED,
        );
        d.text(
            l + 14,
            dock + 92,
            "Edits stay in memory until Package LIB.",
            c::INK_FAINT,
        );
        d.text(
            l + 14,
            dock + 117,
            &short(&self.path, ((vw - 28) / 7).max(1) as usize),
            c::INK_FAINT,
        );
        d.line(l, 56, l, h - 22, c::GM_1000);
        d.line(r, 56, r, h - 22, c::GM_1000);
        d.rect(0, h - 22, w, 22, c::GM_950);
        d.text(
            12,
            h - 7,
            &short(&self.status, ((w - 135) / 7).max(1) as usize),
            if self.status.starts_with("Error:") {
                c::DANGER
            } else {
                c::INK_MUTED
            },
        );
        d.text(
            w - 112,
            h - 7,
            if self.doc.dirty() {
                "* MODIFIED"
            } else {
                "  SAVED"
            },
            if self.doc.dirty() {
                c::AMBER
            } else {
                c::INK_FAINT
            },
        );
        if let Some(p) = &self.prompt {
            let pw = (w - 48).min(700);
            let x = (w - pw) / 2;
            let y = h / 2 - 66;
            d.rect(x - 2, y - 2, pw + 4, 144, c::GM_1000);
            d.rect(x, y, pw, 140, c::GM_800);
            d.text(
                x + 16,
                y + 26,
                &short(&p.title, ((pw - 32) / 7) as usize),
                c::INK,
            );
            d.rect(x + 14, y + 42, pw - 28, 30, c::GM_950);
            d.line(x + 14, y + 72, x + pw - 14, y + 72, c::FOCUS);
            let chars = ((pw - 48) / 7) as usize;
            let value: String = p
                .value
                .chars()
                .rev()
                .take(chars)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            d.text(x + 22, y + 63, &format!("{value}_"), c::AMBER);
            let hint = if matches!(p.kind, PromptKind::Transform(_)) {
                format!(
                    "Axis {} | X/Y/Z constrain | Enter apply | Esc cancel",
                    ['X', 'Y', 'Z'][p.axis]
                )
            } else {
                "Enter apply / open | Esc cancel | Ctrl+A clear".into()
            };
            d.text(x + 16, y + 99, &hint, c::INK_MUTED);
            d.text(
                x + 16,
                y + 122,
                &short(&self.status, ((pw - 32) / 7) as usize),
                if self.status.starts_with("Error:") {
                    c::DANGER
                } else {
                    c::INK_FAINT
                },
            );
        }
        d
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
        for i in -10..=10 {
            let a = i * span / 10;
            line(
                d,
                project([a + center[0], center[1] - span, min[2]]),
                project([a + center[0], center[1] + span, min[2]]),
                c::GM_800,
            );
            line(
                d,
                project([center[0] - span, a + center[1], min[2]]),
                project([center[0] + span, a + center[1], min[2]]),
                c::GM_800,
            );
        }
        for f in &m.faces {
            for i in 0..f.indices.len() {
                let a = m.vertices[f.indices[i]].point;
                let b = m.vertices[f.indices[(i + 1) % f.indices.len()]].point;
                line(d, project(a), project(b), c::AMBER);
            }
        }
        for (j, color) in [c::AXIS_X, c::AXIS_Y, c::AXIS_Z].into_iter().enumerate() {
            let mut p = center;
            p[j] += span / 3;
            line(d, project(center), project(p), color);
        }
        d.text(x + 16, y + 26, "STATIC POSE / WIREFRAME", c::INK_FAINT);
        d.text(
            x + 16,
            y + h - 18,
            &format!(
                "{} vertices   {} faces   {}%",
                m.vertices.len(),
                m.faces.len(),
                self.zoom
            ),
            c::INK_MUTED,
        );
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
