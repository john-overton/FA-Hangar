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
#[path = "ui_glyphs.rs"]
#[allow(dead_code)]
#[rustfmt::skip]
pub mod glyphs;
use theme::{color as c, Family, Rgb};
/// A `theme::text` style. Backends pick the font, size and weight from it;
/// `y` in a text command is the baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Title,
    Body,
    Label,
    /// `label` at weight 600: panel headers, the active workspace tab, the app name.
    Strong,
    /// Uppercase sub-heads; callers pass uppercase text.
    Section,
    Hint,
    Value,
    ValueSm,
    Badge,
}
impl Style {
    pub const ALL: [Style; 9] = [
        Style::Title,
        Style::Body,
        Style::Label,
        Style::Strong,
        Style::Section,
        Style::Hint,
        Style::Value,
        Style::ValueSm,
        Style::Badge,
    ];
    pub const fn spec(self) -> theme::TextStyle {
        match self {
            Style::Title => theme::text::TITLE,
            Style::Body => theme::text::BODY,
            Style::Label => theme::text::LABEL,
            Style::Strong => theme::TextStyle {
                weight: 600,
                ..theme::text::LABEL
            },
            Style::Section => theme::text::SECTION,
            Style::Hint => theme::text::HINT,
            Style::Value => theme::text::VALUE,
            Style::ValueSm => theme::text::VALUE_SM,
            Style::Badge => theme::text::BADGE,
        }
    }
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn mono(self) -> bool {
        matches!(self.spec().family, Family::Mono)
    }
}
/// Estimated advance of one character in `style`, in px. Proportional styles
/// use Arial/Tahoma-class advances (Tahoma Bold runs about 8% wider than
/// Arial Bold); mono styles use Lucida Console's 0.6 em cell. The Windows
/// fonts are the reference and the SVG snapshot fonts match them; the Linux
/// development fallback (`fixed`, 6px cells) can run up to ~10% wider.
pub fn char_width(ch: char, style: Style) -> i32 {
    let t = style.spec();
    if t.family == Family::Mono {
        return (t.size * 1234 + 1024) / 2048;
    }
    let bold = t.weight >= 600;
    let table = if bold {
        &glyphs::UI_ADVANCE_BOLD
    } else {
        &glyphs::UI_ADVANCE
    };
    let units = match ch {
        ' '..='~' => table[ch as usize - 32] as i32,
        '\u{b7}' => 21,
        '\u{2026}' => 64,
        _ => 36,
    } * if bold { 108 } else { 100 };
    (units * t.size + 3200) / 6400
}
pub fn text_width(s: &str, style: Style) -> i32 {
    s.chars().map(|ch| char_width(ch, style)).sum()
}
/// `s` cut to `width` px in `style` with a trailing ellipsis.
pub fn fit(s: &str, width: i32, style: Style) -> String {
    if text_width(s, style) <= width {
        return s.into();
    }
    let room = width - char_width('\u{2026}', style);
    let mut used = 0;
    let mut out = String::new();
    for ch in s.chars() {
        used += char_width(ch, style);
        if used > room {
            break;
        }
        out.push(ch);
    }
    if room > 0 {
        out.push('\u{2026}');
    }
    out
}
/// Single-byte text for the native ANSI/ISO-8859-1 font APIs: Latin-1 passes
/// through, the ellipsis becomes cp1252 0x85 (or "..." where `ansi` is false).
pub fn native_text(s: &str, ansi: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{2026}' if ansi => out.push(0x85),
            '\u{2026}' => out.extend_from_slice(b"..."),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push(b'-'),
            '\u{20}'..='\u{7e}' | '\u{a0}'..='\u{ff}' => out.push(ch as u32 as u8),
            _ => out.push(b'?'),
        }
    }
    out
}
#[derive(Clone, Debug)]
pub enum Draw {
    Rect(i32, i32, i32, i32, u32),
    Line(i32, i32, i32, i32, u32),
    /// Baseline-positioned text in a theme style.
    Text(i32, i32, String, u32, Style),
    Bitmap(i32, i32, usize, usize, Vec<u32>),
    /// A generated icon at its top-left corner, 12px when the flag is set and
    /// 16px otherwise: full pixels in the first color, half-coverage edge
    /// pixels in the second (the icon color mixed into its ground).
    Icon(i32, i32, Glyph, bool, u32, u32),
}
pub use glyphs::Glyph;
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
    /// UI text in the default `label` style.
    pub fn label(&mut self, x: i32, y: i32, s: &str, color: Rgb) {
        self.styled(x, y, s, color, Style::Label);
    }
    /// Data text (numbers, names, offsets) in the default mono `value` style.
    pub fn text(&mut self, x: i32, y: i32, s: &str, color: Rgb) {
        self.styled(x, y, s, color, Style::Value);
    }
    /// Icon `g` at (x, y) in `color` over a `ground` fill (for its edge pixels).
    pub fn icon(&mut self, x: i32, y: i32, g: Glyph, color: Rgb, ground: Rgb) {
        self.commands.push(Draw::Icon(
            x,
            y,
            g,
            false,
            color.0,
            color.mix(ground, 128).0,
        ));
    }
    /// The 12px (`th-ic-sm`) version of `icon`.
    pub fn icon_sm(&mut self, x: i32, y: i32, g: Glyph, color: Rgb, ground: Rgb) {
        self.commands
            .push(Draw::Icon(x, y, g, true, color.0, color.mix(ground, 128).0));
    }
    pub fn styled(&mut self, x: i32, y: i32, s: &str, color: Rgb, style: Style) {
        self.commands
            .push(Draw::Text(x, y, s.into(), color.0, style));
    }
    /// The commands as an SVG document, matching the native fonts by style.
    #[cfg(not(windows))]
    pub fn svg(&self, width: i32, height: i32) -> String {
        fn escape(s: &str) -> String {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        }
        let mut s = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xml:space=\"preserve\" width=\"{width}\" height=\"{height}\" shape-rendering=\"crispEdges\">"
        );
        for d in &self.commands {
            match d {
                Draw::Bitmap(x, y, w, h, pixels) => {
                    for yy in 0..*h {
                        let mut xx = 0;
                        while xx < *w {
                            let color = pixels[yy * w + xx];
                            let mut end = xx + 1;
                            while end < *w && pixels[yy * w + end] == color {
                                end += 1;
                            }
                            s.push_str(&format!(
                                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1\" fill=\"#{color:06x}\"/>",
                                x + xx as i32,
                                y + yy as i32,
                                end - xx
                            ));
                            xx = end;
                        }
                    }
                }
                Draw::Icon(x, y, g, small, full, half) => {
                    let mask = g.mask(*small);
                    for (runs, c) in [(mask.full, full), (mask.half, half)] {
                        for (row, at, len) in runs {
                            s.push_str(&format!(
                                "<rect x=\"{}\" y=\"{}\" width=\"{len}\" height=\"1\" fill=\"#{c:06x}\"/>",
                                x + *at as i32,
                                y + *row as i32
                            ));
                        }
                    }
                }
                Draw::Rect(x, y, w, h, c) => s.push_str(&format!(
                    "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"#{c:06x}\"/>"
                )),
                Draw::Line(x, y, a, b, c) => s.push_str(&format!(
                    "<path d=\"M{}.5 {}.5 L{}.5 {}.5\" stroke=\"#{c:06x}\" stroke-linecap=\"square\"/>",
                    x, y, a, b
                )),
                Draw::Text(x, y, t, c, style) => {
                    let spec = style.spec();
                    let family = if style.mono() {
                        "'Lucida Console','Liberation Mono',monospace"
                    } else {
                        "Tahoma,'Liberation Sans',sans-serif"
                    };
                    s.push_str(&format!(
                        "<text x=\"{x}\" y=\"{y}\" font-family=\"{family}\" font-size=\"{}\" font-weight=\"{}\" fill=\"#{c:06x}\">{}</text>",
                        spec.size,
                        if spec.weight >= 600 { 700 } else { 400 },
                        escape(t)
                    ))
                }
            }
        }
        s.push_str("</svg>");
        s
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
    PartPosition(usize),
    AnimationState(usize),
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
    Number(widgets::NumberTarget),
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
    original: Option<Entry>,
    bytes: Vec<u8>,
    pic: Pic,
    last: Option<(usize, usize)>,
    /// Eraser target: the texture's original pixels, same layout as `pic`.
    erase: Option<Box<Pic>>,
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
    hp_slew: bool,
    mesh_edit: bool,
    animation_tool: bool,
    animation_state: BTreeMap<usize, i32>,
    /// Import symbols when the state keys were chosen; keys are absolute addresses.
    animation_symbols: BTreeMap<usize, String>,
    animation_scroll: usize,
    animation_part: usize,
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
    transfer_source: Option<u64>,
    transfer_move: bool,
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
    collapsed: [bool; 10],
    root_collapsed: bool,
    category: Option<usize>,
    /// Outliner type filter (aircraft, shapes, images): only that group shows.
    type_filter: Option<usize>,
    menu: Option<usize>,
    dock: u8,
    table_scroll: usize,
    /// Right editor panel scroll in px, for `inspector_kind`.
    inspector_scroll: i32,
    inspector_kind: u8,
    /// Collapsed panels: bit `widgets::pane::*`.
    panels: u64,
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
    eraser: bool,
    original_note: Option<(String, bool)>,
    pick_color: bool,
    textures: BTreeMap<String, Pic>,
    textured: bool,
    /// Solid shading: the textured raster with flat, lit face colors.
    flat: bool,
    stroke: Option<Stroke>,
    stroke_parked: Vec<Stroke>,
    panel_draft: Option<Box<mesh_ui::PanelPlan>>,
    selected_face: Option<usize>,
    model_paint: bool,
    paint_lock: bool,
    context_model: Option<Model>,
    context_entry: Option<usize>,
    /// Left button held (pressed button faces).
    pressed: bool,
    /// Ctrl held, reported by the backend (NumberField snapping).
    ctrl: bool,
    scrub: Option<widgets::Scrub>,
}
fn extension(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or("")
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
            hp_slew: false,
            mesh_edit: false,
            animation_tool: false,
            animation_state: BTreeMap::new(),
            animation_symbols: BTreeMap::new(),
            animation_scroll: 0,
            animation_part: 0,
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
            transfer_source: None,
            transfer_move: false,
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
            pitch: 90,
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
            collapsed: [true; 10],
            root_collapsed: false,
            type_filter: None,
            category: None,
            menu: None,
            dock: 0,
            table_scroll: 0,
            inspector_scroll: 0,
            inspector_kind: 0,
            panels: 0,
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
            eraser: false,
            original_note: None,
            pick_color: false,
            textures: BTreeMap::new(),
            textured: false,
            flat: false,
            stroke: None,
            stroke_parked: Vec::new(),
            panel_draft: None,
            selected_face: None,
            model_paint: false,
            paint_lock: false,
            context_model: None,
            context_entry: None,
            pressed: false,
            ctrl: false,
            scrub: None,
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
        self.collapsed = [true; 10];
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
        self.collapsed = [true; 10];
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
        self.scroll = self
            .library_rows()
            .iter()
            .position(|(id, cat, _)| *id == self.library_id && cat.is_none())
            .unwrap_or(0);
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

        self.root_collapsed = false;
        self.inspector_scroll = 0;
        self.field_group = None;
        self.envelope_selected = 0;
        self.envelope_scroll = 0;
        self.hp_tool = false;
        self.mesh_edit = false;
        self.animation_tool = false;
        self.animation_state.clear();
        self.animation_scroll = 0;
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
        let table_rows = self.browse_rows();
        if table_position < self.table_scroll || table_position >= self.table_scroll + table_rows {
            self.table_scroll = table_position.saturating_sub(table_rows / 2);
        }
        let position = self
            .library_rows()
            .iter()
            .position(|(id, _, i)| *id == self.library_id && *i == Some(self.selected))
            .unwrap_or(0);
        let rows = self.outliner_rows();
        if position < self.scroll || position >= self.scroll + rows {
            self.scroll = position.saturating_sub(rows / 2);
        }
        self.refresh();
        self.frame();
        if matches!(
            extension(self.name()),
            "PIC" | "ORG" | "PAL" | "5K" | "11K" | "WAV"
        ) {
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
        self.original_note = self.compute_original_note();
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
                    if ext == "PIC" || ext == "ORG" {
                        // A stored original previews like its PIC but is never painted.
                        match Pic::parse(&data) {
                            Ok(p) => {
                                self.detail = if ext == "ORG" {
                                    format!(
                                        "{} x {} / stored original, read-only",
                                        p.width, p.height
                                    )
                                } else {
                                    format!("{} x {} / indexed PIC", p.width, p.height)
                                };
                                self.pic = Some(p);
                            }
                            Err(_) if ext == "ORG" => {
                                self.detail = "Not a PIC payload; not a stored original".into();
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
        let vertices = self.model.as_ref().map_or(0, |m| m.vertices.len());
        self.mesh_vertices.retain(|i| *i < vertices);
        self.refresh_graft();
        self.refresh_hardpoints();
        let reset = self.revalidate_animation_state();
        if self.animation_tool {
            if let Err(error) = self.animation_preview() {
                self.preview = None;
                self.status = error;
            } else if reset {
                self.status = "Preview states reset: the shape's import addresses changed".into();
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
                let bytes = crate::platform::read(path)?;
                let old = self
                    .doc
                    .archive
                    .entries
                    .get(self.selected)
                    .ok_or("No selected entry")?;
                if old.read().ok().as_deref() == Some(bytes.as_slice()) {
                    self.status = "Replacement is identical / nothing changed".into();
                    return Ok(());
                }
                let entry = Entry::new(&old.name.clone(), bytes)?;
                let entries = self.with_originals(vec![entry]);
                let kept = entries.get(1).map(|e| e.name.clone());
                self.doc.transaction(entries, &[])?;
                self.refresh();
                self.status = match kept {
                    Some(org) => format!("Entry replaced / original kept as {org} | Ctrl+Z undo"),
                    None => "Entry replaced | Ctrl+Z undo".into(),
                };
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
    /// Principal axis closest to the view direction, for unconstrained rotation.
    pub(super) fn view_axis(&self) -> usize {
        let d = self.camera_inverse([0, 0, 1024]);
        (0..3).max_by_key(|k| d[*k].abs()).unwrap_or(2)
    }
    /// Axis 3 means unconstrained: G takes one X value or X Y Z, R uses the view
    /// axis and S scales uniformly, as in Blender.
    fn transform_value(&self, op: char, axis: usize, value: &str) -> Result<Transform> {
        let n = value
            .split_whitespace()
            .map(str::parse::<i32>)
            .collect::<core::result::Result<Vec<_>, _>>()
            .map_err(|_| "Enter an integer transform value")?;
        let lock = (axis < 3).then_some(axis);
        Ok(match (op, n.as_slice()) {
            ('g', []) => Transform::Translate([0; 3]),
            ('g', [v]) => Transform::Move(lock.unwrap_or(0), *v),
            ('g', [x, y, z]) if lock.is_none() => Transform::Translate([*x, *y, *z]),
            ('r', []) => Transform::Rotate(lock.unwrap_or(self.view_axis()), 0),
            ('r', [v]) => Transform::Rotate(lock.unwrap_or(self.view_axis()), *v),
            ('s', []) => Transform::Scale(lock, 100),
            ('s', [v]) => Transform::Scale(lock, *v),
            ('g', _) => return Err("Enter one value, or X Y Z offsets with no axis lock".into()),
            _ => return Err("Enter one integer value".into()),
        })
    }
    fn transform_preview(&mut self) {
        let Some(p) = &self.prompt else {
            return;
        };
        let PromptKind::Transform(op) = p.kind else {
            return;
        };
        let t = match self.transform_value(op, p.axis, &p.value) {
            Ok(t) => t,
            Err(e) => {
                self.preview = None;
                self.status = format!("Error: {e}");
                return;
            }
        };
        if let Some(m) = &self.model {
            let result = if self.mesh_edit {
                m.median(&self.mesh_vertices)
                    .ok_or_else(|| "Select vertices first".to_string())
                    .and_then(|pivot| m.transform_selection(t, Some(&self.mesh_vertices), pivot))
            } else {
                m.transformed(t)
            };
            match result {
                Ok(m) => self.preview = Some(m),
                Err(e) => {
                    self.preview = None;
                    self.status = e;
                }
            }
        }
    }
    fn transform_prompt(&mut self, op: char) {
        let title = match (self.mesh_edit, op) {
            (false, 'g') => "Move in source units",
            (false, 'r') => "Rotate in degrees",
            (false, _) => "Scale in percent",
            (true, 'g') => "Move selected vertices in source units",
            (true, 'r') => "Rotate selection about its median point in degrees",
            (true, _) => "Scale selection about its median point in percent",
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::Transform(op),
            title: title.into(),
            value: String::new(),
            axis: 3,
        });
        self.transform_preview();
    }
    fn apply_transform(&mut self) -> Result<()> {
        let preview = self
            .preview
            .take()
            .ok_or_else(|| "Enter a valid transform value".to_string())?;
        if self.mesh_edit {
            let entry = self.model_entry.ok_or("Open the SH owner before editing")?;
            let bytes = preview.write(&self.doc.archive.entries[entry].read()?)?;
            self.doc.replace(entry, bytes)?;
            self.refresh();
            self.status = "Selected vertices transformed | Ctrl+Z undo".into();
        } else {
            let bytes = preview.write(&self.data)?;
            self.doc.replace(self.selected, bytes)?;
            self.refresh();
            self.status = "Geometry changed | Ctrl+Z undo".into();
        }
        Ok(())
    }
    pub fn key(&mut self, key: Key, ctrl: bool, shift: bool) {
        if self.scrub.is_some() {
            if matches!(key, Key::Escape) {
                self.number_cancel();
            }
            return;
        }
        if self.mesh_drag.is_some()
            && (matches!(key, Key::Escape) || ctrl && matches!(key, Key::Char('z')))
        {
            self.mesh_drag = None;
            self.preview = None;
            self.status = "Vertex drag cancelled".into();
            return;
        }
        if let (Some(_), Key::Char(ch), false) = (&self.mesh_drag, key, ctrl) {
            if let Some(axis) = "xyz".find(ch.to_ascii_lowercase()) {
                self.mesh_drag_axis(axis);
                return;
            }
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
                // Discard the stroke but stay in the active paint tool.
                let tool = (self.paint_enabled, self.model_paint);
                self.painting = false;
                self.stroke = None;
                self.stroke_parked.clear();
                self.panel_draft = None;
                self.refresh();
                (self.paint_enabled, self.model_paint) = tool;
                self.status = "Stroke discarded / nothing changed".into();
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
                    self.transfer_source = None;
                    self.transfer_move = false;
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
                                Err("Click Discard changes or Cancel".into())
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
                            if !from.to_ascii_uppercase().ends_with(".PIC") {
                                return Err("Select a PIC texture to clone".into());
                            }
                            let texture =
                                self.doc.archive.find(&from).ok_or("Texture is missing")?;
                            if !to.ends_with(".PIC") || self.doc.archive.find(&to).is_some() {
                                return Err("Choose an unused PIC name".into());
                            }
                            let (bytes, n) = Model::retarget_texture(
                                &self.doc.archive.entries[shape].read()?,
                                &from,
                                &to,
                            )?;
                            // Stored bytes and compression are copied as-is, with any stored original.
                            let mut entries = vec![
                                Entry::new(&self.doc.archive.entries[shape].name.clone(), bytes)?,
                                self.doc.archive.entries[texture].renamed(&to)?,
                            ];
                            entries.extend(hangar_core::originals::cloned(
                                &self.doc.archive,
                                &from,
                                &to,
                            )?);
                            self.doc.transaction(entries, &[])?;
                            self.select_entry(shape);
                            self.textured = true;
                            self.status=format!("Cloned {to}; {n} decoded references updated. Other LOD/damage references retain original textures.");
                            Ok(())
                        })(),
                        PromptKind::PartPosition(axis) => self.part_position(axis, &p.value),
                        PromptKind::AnimationState(address) => {
                            self.set_animation_state(address, &p.value)
                        }
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
                                self.collapsed = [true; 10];
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
                                Err("Click Discard changes or Cancel".into())
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
                        PromptKind::Transform(_) => self.apply_transform(),
                        PromptKind::Number(t) => self.number_typed(t, &p.value),
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
                        let axis = match ch.to_ascii_lowercase() {
                            'x' => 0,
                            'y' => 1,
                            _ => 2,
                        };
                        // Transform locks toggle; a station move always needs an axis.
                        p.axis = if p.axis == axis && matches!(p.kind, PromptKind::Transform(_)) {
                            3
                        } else {
                            axis
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
            Key::Char('a')|Key::Char('A') if self.mesh_edit&&self.mode==Mode::Model=>self.mesh_toggle_all(),
            Key::Char(ch) if "gGrRsS".contains(ch)&&self.mesh_edit&&self.mode==Mode::Model=>self.mesh_transform_prompt(ch.to_ascii_lowercase()),
            Key::Tab if self.mode==Mode::Model=>self.act(view::Action::MeshMode),
            Key::Char('g')|Key::Char('G') if self.hp_tool&&self.mode==Mode::Model => {self.prompt=Some(Prompt{kind:PromptKind::StationMove,title:"Move station / X Y Z axis, source-unit offset".into(),value:"0".into(),axis:0});},
            Key::Char(ch) if "gGrRsS".contains(ch)&&self.model.is_some()=>{
                if self.model_entry!=Some(self.selected) || self.model.as_ref().is_some_and(|m|!m.writable){self.status="Select the linked SH entry to edit supported geometry; animated SH remains read-only".into();return;}
                self.transform_prompt(ch.to_ascii_lowercase());
            },
            Key::Char('1')|Key::Num(1)=>{self.yaw=0;self.pitch=0;},Key::Char('3')|Key::Num(3)=>{self.yaw=90;self.pitch=0;},Key::Char('7')|Key::Num(7)=>{self.yaw=0;self.pitch = 90;},Key::Char('5')|Key::Num(5)=>{if self.textured{self.perspective=false;self.status="Textured paint preview uses orthographic projection".into();}else{self.perspective = !self.perspective;}},
            Key::Home|Key::Char('.')=>self.frame(),
            Key::Up=>{self.select_entry(self.selected.saturating_sub(1));},
            Key::Down=>{self.select_entry(self.selected+1);},
            Key::Enter=>self.edit_field(self.field_selected),
            Key::Backspace=>{if let Some(t)=self.number_under_mouse(){self.number_reset(t);}},
            Key::Delete=>{let r=self.delete_entry();self.result(r);},
            Key::Escape=>{self.graft_library=None;self.resource_drag=None;},
            Key::F1=>self.status="Ctrl+O open | Ctrl+S package | Ctrl+I add | Ctrl+E export | Ctrl+Z undo | MMB orbit | Shift+MMB pan | Wheel zoom | G/R/S transform, X/Y/Z toggles axis lock | Tab edit mode: G/R/S act on selected vertices, Shift+click extends, A all/none".into(),
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
                    "Unsaved edits in {} LIBs / close without saving?",
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
        self.pointer(x, y, button, down, false);
    }
    /// Mouse button event with the Shift state, which extends vertex selections.
    pub fn pointer(&mut self, x: i32, y: i32, button: u8, down: bool, shift: bool) {
        self.mouse = [x, y];
        if button == 1 {
            self.pressed = down;
        }
        if button == 1 && !down {
            if self.scrub.is_some() {
                self.number_release();
                return;
            }
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
                            Some(
                                view::Action::Library(id)
                                | view::Action::LibraryToggle(id)
                                | view::Action::LibraryCategory(id, _)
                                | view::Action::LibraryEntry(id, _),
                            ) if id != library => {
                                self.selected = entry;
                                self.prepare_drop(id)?;
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
        if button == 1 && down && self.prompt.is_none() && self.menu.is_some() {
            let inside = self
                .open_menu_rect()
                .is_some_and(|[mx, my, mw, mh]| x >= mx && y >= my && x < mx + mw && y < my + mh);
            let action = self
                .layout()
                .hits
                .into_iter()
                .rev()
                .find(|h| h.contains(x, y))
                .map(|h| h.action);
            match action {
                Some(a) if inside || matches!(a, view::Action::Menu(_)) => self.act(a),
                _ => self.menu = None,
            }
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
                self.mesh_press(i, shift);
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
            && !self.animation_tool
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
        if let Some(view::Action::Number(t)) = action {
            self.number_press(t, x);
            return;
        }
        if let Some(action) = action {
            self.resource_drag = if x < self.left() {
                match action {
                    view::Action::Entry(i) => Some((self.library_id, i, [x, y])),
                    view::Action::LibraryEntry(id, i) => Some((id, i, [x, y])),
                    _ => None,
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
        if self.scrub.is_some() {
            self.number_motion(x, shift);
            self.mouse = [x, y];
            return;
        }
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
                self.status = "Drop on a LIB to review copy/move / same-type entry to graft".into();
            }
        }

        if self.painting {
            if self.mode == Mode::Model {
                if let Some((face, uv)) = self.model_hit(x, y) {
                    if !self.paint_lock || Some(face) == self.selected_face {
                        self.mouse = [x, y];
                        self.paint_model_hit(face, uv);
                    } else if let Some(s) = &mut self.stroke {
                        s.last = None;
                    }
                } else if let Some(s) = &mut self.stroke {
                    s.last = None;
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
        if self.animation_tool
            && self.mode == Mode::Model
            && self.mouse[0] >= self.right()
            && self.mouse[1] >= 321
        {
            let hidden = self
                .animation_addresses()
                .len()
                .saturating_sub(self.animation_rows());
            self.animation_scroll =
                (self.animation_scroll as i32 - delta * 3).clamp(0, hidden as i32) as usize;
            return;
        }
        if self.mouse[0] < self.left() {
            let count = self.library_rows().len();
            let rows = self.outliner_rows();
            self.scroll = (self.scroll as i32 - delta * 3)
                .clamp(0, count.saturating_sub(rows) as i32) as usize;
        } else if self.mouse[0] >= self.right() {
            let kind = self.inspector_kind();
            if kind != self.inspector_kind {
                self.inspector_kind = kind;
                self.inspector_scroll = 0;
            }
            let max = self.layout().inspector_max;
            self.inspector_scroll =
                (self.inspector_scroll - delta * 3 * widgets::ROW_PITCH).clamp(0, max);
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
            let rows = self.browse_rows();
            self.table_scroll = (self.table_scroll as i32 - delta * 3)
                .clamp(0, n.saturating_sub(rows) as i32) as usize;
        } else {
            self.zoom = (self.zoom + delta * 10).clamp(10, 1000);
        }
    }
    /// A proper camera basis: front sees +forward, screen right is -body X.
    /// The old X/up/forward swap had determinant -1 and mirrored every view.
    pub(super) fn camera_point(&self, p: [i32; 3]) -> [i32; 3] {
        model::rotate(
            model::rotate([-p[0], p[2], p[1]], 1, self.yaw),
            0,
            self.pitch,
        )
    }
    pub(super) fn camera_inverse(&self, p: [i32; 3]) -> [i32; 3] {
        let p = model::rotate(model::rotate(p, 0, -self.pitch), 1, -self.yaw);
        [-p[0], p[2], p[1]]
    }
    fn viewport(&self, d: &mut Canvas, m: &Model, x: i32, y: i32, w: i32, h: i32) {
        let (center, span) = self
            .model_bounds()
            .unwrap_or_else(|| hardpoint_ui::bounds(m));
        let project = |p: [i32; 3]| {
            let p = self.camera_point(core::array::from_fn(|i| p[i] - center[i]));
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
        // Grid on the ground plane, aligned to the origin; every fifth line gm-700.
        let step = (span / 10).max(1);
        let reach = span * 4;
        for axis in 0..2 {
            let from = (center[axis] - reach).div_euclid(step);
            let to = (center[axis] + reach).div_euclid(step);
            for k in from..=to {
                if k == 0 {
                    continue;
                }
                let mut a = [0; 3];
                let mut b = [0; 3];
                a[axis] = k * step;
                b[axis] = k * step;
                a[1 - axis] = center[1 - axis] - reach;
                b[1 - axis] = center[1 - axis] + reach;
                let major = k.rem_euclid(theme::metric::GRID_MAJOR) == 0;
                line(
                    d,
                    project(a),
                    project(b),
                    if major { c::GM_700 } else { c::GM_800 },
                );
            }
        }
        // Full-length X and Y axis lines through the origin.
        for (axis, color) in [(0, c::AXIS_X), (1, c::AXIS_Y)] {
            let mut a = [0; 3];
            let mut b = [0; 3];
            a[axis] = center[axis] - reach;
            b[axis] = center[axis] + reach;
            line(d, project(a), project(b), color);
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
        let origin = project([0, 0, 0]);
        if origin[0] > x + 4 && origin[1] > y + 4 && origin[0] < x + w - 4 && origin[1] < y + h - 4
        {
            chrome::ring(d, origin[0], origin[1], 3, c::GM_1000, c::AMBER_BRIGHT);
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

#[path = "ui_chrome.rs"]
mod chrome;
#[path = "ui_view.rs"]
mod view;
// Components not yet adopted by every editor stay available for later passes.
#[path = "ui_widgets.rs"]
#[allow(dead_code)]
mod widgets;

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

#[path = "ui_animation.rs"]
mod animation_ui;
