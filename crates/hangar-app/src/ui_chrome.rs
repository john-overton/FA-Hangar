//! Window chrome: menu bar, dropdown menus, status bar, viewport header and
//! tool strip, viewport overlays and gizmo, dock header. Built from the
//! ui_widgets components; positions come from one place per bar so drawing,
//! hit regions and dropdown anchors always agree.
use super::view::{Action, Icon, Layout};
use super::widgets::{baseline, dot, keycap, keycap_width, notched, Btn, Item};
use super::*;
use alloc::vec;
use theme::{metric as m, space};

const MENUS: [&str; 7] = ["File", "Edit", "Lib", "Entry", "View", "Tools", "Help"];
/// Dropdown ids beyond the seven menu bar menus.
pub(super) const MENU_MODE: usize = 7;
pub(super) const MENU_VIEW: usize = 8;
pub(super) const MENU_SHADING: usize = 9;
/// The Model inspector's Shape Select.
pub(super) const MENU_SHAPE: usize = 10;
/// Edit Mesh header menus.
pub(super) const MENU_SELECT: usize = 11;
pub(super) const MENU_MESH: usize = 12;
/// The options Select of a part setting (`App::part_menu`).
pub(super) const MENU_PART: usize = 13;
const TABS: [(&str, Mode); 6] = [
    ("Browse", Mode::Browse),
    ("Model", Mode::Model),
    ("Flight", Mode::Properties),
    ("Graft", Mode::Graft),
    ("Package", Mode::Package),
    ("Paint", Mode::Media),
];

/// Menu bar geometry for the current window width.
pub(super) struct Bar {
    pub logo: [i32; 4],
    /// The "Hangar" word is dropped first when space is short.
    pub word: bool,
    pub menus: [[i32; 4]; 7],
    pub tabs: [[i32; 4]; 6],
    /// Right-aligned active LIB group: dirty dot, lock, name.
    pub lib: [i32; 4],
    pub lib_text: String,
}

impl App {
    /// The active LIB's name and whether it is a protected retail name.
    fn lib_label(&self) -> (String, bool) {
        (
            self.lib_name().into(),
            hangar_core::save::protected_name(&self.path).is_some(),
        )
    }
    /// Menu bar positions. At narrow widths the app name, then tab and menu
    /// padding give way before the active LIB name is ever truncated.
    pub(super) fn bar(&self) -> Bar {
        let w = self.width;
        let (name, protected) = self.lib_label();
        let lead = m::DIRTY_DOT + 6 + if protected { m::ICON_SM + 4 } else { 0 };
        let full = lead + text_width(&name, Style::Value);
        let mut bar = Bar {
            logo: [0; 4],
            word: true,
            menus: [[0; 4]; 7],
            tabs: [[0; 4]; 6],
            lib: [0; 4],
            lib_text: name.clone(),
        };
        for (word, menu_pad, tab_pad, tab_gap) in [
            (true, 8, 12, 16),
            (false, 8, 12, 16),
            (false, 6, 8, 8),
            (false, 5, 6, 4),
        ] {
            let logo_w = 8
                + m::ICON
                + if word {
                    6 + text_width("Hangar", Style::Strong)
                } else {
                    0
                }
                + 6;
            bar.word = word;
            bar.logo = [
                space::SPACE_1,
                3,
                logo_w - space::SPACE_1,
                m::MENUBAR_ITEM_H,
            ];
            let mut x = logo_w;
            for (i, name) in MENUS.iter().enumerate() {
                let mw = text_width(name, Style::Label) + 2 * menu_pad;
                bar.menus[i] = [x, 3, mw, m::MENUBAR_ITEM_H];
                x += mw + 2;
            }
            x += tab_gap;
            for (i, (name, _)) in TABS.iter().enumerate() {
                let tw = text_width(name, Style::Strong) + 2 * tab_pad;
                bar.tabs[i] = [x, m::MENUBAR_H - m::WSTAB_H, tw, m::WSTAB_H];
                x += tw + 2;
            }
            let room = w - 8 - (x + space::SPACE_2);
            bar.lib = [w - 8 - full.min(room), 3, full.min(room), m::MENUBAR_ITEM_H];
            if full <= room {
                break;
            }
        }
        let room = bar.lib[2] - lead;
        bar.lib_text = fit(&name, room, Style::Value);
        bar
    }
    pub(super) fn menubar(&self, o: &mut Layout) {
        let w = self.width;
        let bar = self.bar();
        let d = &mut o.canvas;
        d.rect(0, 0, w, m::MENUBAR_H, c::GM_950);
        d.rect(0, m::MENUBAR_H - 1, w, 1, c::GM_1000);
        let hover = o.over(bar.logo);
        if hover {
            notched(&mut o.canvas, bar.logo, Some(c::GM_700), None);
        }
        let ground = if hover { c::GM_700 } else { c::GM_950 };
        o.canvas.icon(8, 5, Icon::Hardpoint, c::AMBER, ground);
        if bar.word {
            o.canvas.styled(
                30,
                baseline(0, m::MENUBAR_H, Style::Strong),
                "Hangar",
                c::INK,
                Style::Strong,
            );
        }
        o.hit(bar.logo, Action::Demo);
        for (i, rect) in bar.menus.iter().enumerate() {
            if self.menu == Some(i) || o.over(*rect) {
                notched(&mut o.canvas, *rect, Some(c::GM_700), None);
            }
            o.canvas.styled(
                rect[0] + (rect[2] - text_width(MENUS[i], Style::Label)) / 2,
                baseline(0, m::MENUBAR_H, Style::Label),
                MENUS[i],
                c::INK,
                Style::Label,
            );
            o.hit(*rect, Action::Menu(i));
        }
        for (i, rect) in bar.tabs.iter().enumerate() {
            let (name, mode) = TABS[i];
            let active = self.mode == mode;
            let hover = !active && o.over(*rect);
            let [x, y, tw, th] = *rect;
            if active || hover {
                let fill = if active { c::GM_800 } else { c::GM_900 };
                // Top corners notched; the active tab runs into the editor below.
                o.canvas.rect(x + 1, y, tw - 2, 1, fill);
                o.canvas.rect(x, y + 1, tw, th - 1, fill);
            }
            let style = if active { Style::Strong } else { Style::Label };
            o.canvas.styled(
                x + (tw - text_width(name, style)) / 2,
                baseline(0, m::MENUBAR_H, style) + 1,
                name,
                if active || hover {
                    c::INK
                } else {
                    c::INK_MUTED
                },
                style,
            );
            o.hit(*rect, Action::Mode(mode));
        }
        let (_, protected) = self.lib_label();
        let [mut x, y, _, h] = bar.lib;
        if self.doc.dirty() {
            dot(&mut o.canvas, x, y + (h - m::DIRTY_DOT) / 2, c::AMBER);
        }
        x += m::DIRTY_DOT + 6;
        if protected {
            o.canvas.icon_sm(
                x,
                y + (h - m::ICON_SM) / 2,
                Icon::Lock,
                c::INK_MUTED,
                c::GM_950,
            );
            x += m::ICON_SM + 4;
        }
        o.canvas.styled(
            x,
            baseline(0, m::MENUBAR_H, Style::Value),
            &bar.lib_text,
            c::INK_MUTED,
            Style::Value,
        );
    }
    /// Items for dropdown `menu`. Labels are sentence-case verbs and nouns;
    /// shortcuts are keycaps.
    pub(super) fn menu_items(&self, menu: usize) -> Vec<Item<'static>> {
        match menu {
            0 => vec![
                Item::new("Open LIB", Action::File(FileAction::Open)).key("Ctrl+O"),
                Item::new("New empty LIB", Action::NewLibrary),
                Item::new("Close active LIB", Action::CloseLibrary).key("Ctrl+W"),
                Item::sep(),
                Item::new("Package LIB", Action::File(FileAction::Save)).key("Ctrl+S"),
                Item::sep(),
                Item::new("Load synthetic demo", Action::Demo),
                Item::new("Quit", Action::Close),
            ],
            1 => vec![
                Item::new("Undo", Action::Undo).key("Ctrl+Z"),
                Item::new("Redo", Action::Redo).key("Ctrl+Shift+Z"),
            ],
            2 => vec![
                Item::new("Add entry", Action::File(FileAction::Import)).key("Ctrl+I"),
                Item::new(
                    "Export object with resources",
                    Action::File(FileAction::Variant),
                ),
                Item::new(
                    "New aircraft from SH file",
                    Action::File(FileAction::VariantSh),
                ),
                Item::sep(),
                Item::new("Validate package", Action::Validate),
            ],
            3 => vec![
                Item::new("Copy resource", Action::CopyResource).key("Ctrl+C"),
                Item::new("Paste resources", Action::PasteResources).key("Ctrl+V"),
                Item::new("Rename resource", Action::RenameResource(false)),
                Item::new("Duplicate resource", Action::RenameResource(true)).key("Ctrl+D"),
                Item::new("References and users", Action::Dock(4)),
                Item::sep(),
                Item::new("Export entry", Action::File(FileAction::Export)).key("Ctrl+E"),
                Item::new("Replace entry", Action::File(FileAction::Replace)),
                Item::new("Export geometry as OBJ", Action::File(FileAction::Obj)),
                Item::sep(),
                Item::new("Use as graft donor", Action::PinDonor),
                Item::new("Graft characteristics", Action::Mode(Mode::Graft)),
                Item::new("Paint media", Action::Mode(Mode::Media)),
                Item::new("Hardpoint tools", Action::Hardpoints),
                Item::new("Parts", Action::Animation),
                Item::new("Repair generated panel mappings", Action::RepairPanels),
                Item::new("Decals and markings", Action::MediaTab(2)),
                Item::new("Base color", Action::BaseColor(false)),
                Item::new("Panel color", Action::BaseColor(true)),
                Item::new("Remap color indices", Action::Recolor),
            ],
            4 | MENU_VIEW => {
                let mut items = vec![
                    Item::new("Frame all", Action::View(0)).key("Home"),
                    Item::new("Front", Action::View(1)).key("1"),
                    Item::new("Side", Action::View(3)).key("3"),
                    Item::new("Top", Action::View(7)).key("7"),
                    Item::new("Toggle projection", Action::View(5)).key("5"),
                ];
                if menu == 4 {
                    items.push(Item::sep());
                    items.extend(self.shading_items());
                }
                items
            }
            5 => vec![
                Item::new(
                    "Export object with resources",
                    Action::File(FileAction::Variant),
                ),
                Item::new(
                    "New aircraft from SH file",
                    Action::File(FileAction::VariantSh),
                ),
                Item::new("Graft characteristics", Action::Mode(Mode::Graft)),
                Item::new("Copy one donor field", Action::File(FileAction::Graft)),
            ],
            MENU_MODE => {
                let current = self.viewport_mode();
                VIEWPORT_MODES
                    .iter()
                    .enumerate()
                    .map(|(i, (label, glyph, key))| {
                        let item = Item::new(label, Action::ViewportMode(i as u8))
                            .icon(*glyph)
                            .on(i == current);
                        match key {
                            Some(k) => item.key(k),
                            None => item,
                        }
                    })
                    .collect()
            }
            MENU_SHADING => self.shading_items(),
            MENU_SELECT => vec![
                Item::new("Vertex select", Action::SelectMode(false))
                    .icon(Icon::Vertex)
                    .key("1")
                    .on(!self.ed.face_select),
                Item::new("Face select", Action::SelectMode(true))
                    .icon(Icon::Face)
                    .key("3")
                    .on(self.ed.face_select),
                Item::sep(),
                Item::new("All or none", Action::MeshAll).key("A"),
                Item::new("Invert", Action::MeshOp(edit_ui::OP_INVERT)),
                Item::new("Box select", Action::MeshOp(edit_ui::OP_BOX)).key("B"),
                Item::new("Select linked part", Action::MeshOp(edit_ui::OP_LINKED)),
            ],
            MENU_MESH => vec![
                Item::new("Move", Action::MeshMove).key("G"),
                Item::new("Delete faces", Action::MeshOp(edit_ui::OP_DELETE)).key("X"),
                Item::new("Flip normals", Action::MeshOp(edit_ui::OP_FLIP)).key("Alt+N"),
                Item::new("Duplicate", Action::MeshOp(edit_ui::OP_DUPLICATE)).key("Shift+D"),
                Item::new("Extrude", Action::MeshOp(edit_ui::OP_EXTRUDE)).key("E"),
                Item::new("Make face", Action::MeshOp(edit_ui::OP_FACE)).key("F"),
                Item::new("Add vertex at median", Action::MeshOp(edit_ui::OP_VERTEX)),
                Item::sep(),
                Item::new("Pivot: median", Action::Pivot(false)).on(!self.ed.pivot_individual),
                Item::new("Pivot: individual origins", Action::Pivot(true))
                    .on(self.ed.pivot_individual),
                Item::sep(),
                Item::new("New face colour", Action::MeshOp(edit_ui::OP_FACE_COLOR)),
            ],
            _ => vec![
                Item::new("Controls", Action::Help).key("F1"),
                Item::new("Load synthetic demo", Action::Demo),
            ],
        }
    }
    /// Run `f` over dropdown `menu`'s items; the Shape Select lists entry
    /// names owned here.
    fn with_menu<R>(&self, menu: usize, f: &mut dyn FnMut(&[Item]) -> R) -> R {
        if menu == MENU_PART {
            let options = self.part_menu_items();
            let items: Vec<Item> = options
                .iter()
                .map(|(label, action, on, unseen)| {
                    let item = Item::new(label, *action).on(*on);
                    if *unseen {
                        item.badge("Not seen in retail")
                    } else {
                        item
                    }
                })
                .collect();
            return f(&items);
        }
        if menu != MENU_SHAPE {
            return f(&self.menu_items(menu));
        }
        let shapes = self.shape_items();
        let items: Vec<Item> = shapes
            .iter()
            .map(|(name, action)| {
                Item::new(name, *action)
                    .icon(Icon::Shape)
                    .on(matches!(action, Action::Entry(i) if Some(*i) == self.model_entry))
            })
            .collect();
        f(&items)
    }
    fn shading_items(&self) -> Vec<Item<'static>> {
        let shading = self.shading();
        let mut items: Vec<Item<'static>> = SHADING
            .iter()
            .enumerate()
            .map(|(i, (label, glyph))| {
                Item::new(label, Action::Shading(i as u8))
                    .icon(*glyph)
                    .on(i == shading)
            })
            .collect();
        items.push(
            Item::new("Show hardpoints", Action::HardpointVisibility)
                .icon(Icon::Hardpoint)
                .on(self.hp_visible),
        );
        items
    }
    /// The open dropdown's drawn rect.
    pub(super) fn open_menu_rect(&self) -> Option<[i32; 4]> {
        let menu = self.menu?;
        let (x, y) = self.menu_anchor(menu);
        let (w, h) = self.with_menu(menu, &mut |items| super::widgets::menu_size(items));
        let x = x.min(self.width - w).max(0);
        let y = y.min(self.height - h).max(0);
        Some([x, y, w, h])
    }
    /// Top-left anchor of dropdown `menu`.
    pub(super) fn menu_anchor(&self, menu: usize) -> (i32, i32) {
        match menu {
            0..=6 => (self.bar().menus[menu][0], m::MENUBAR_H),
            MENU_SHAPE => {
                let rect = self.shape_select_rect();
                (rect[0], rect[1] + rect[3] + 1)
            }
            MENU_PART => {
                let rect = self.ed.part_menu.map_or([0; 4], |(_, r)| r);
                (rect[0], rect[1] + rect[3] + 1)
            }
            _ => {
                let header = self.viewport_header_slots();
                let rect = match menu {
                    MENU_MODE => header.mode,
                    MENU_VIEW => header.view,
                    MENU_SELECT => header.select.unwrap_or(header.view),
                    MENU_MESH => header.mesh.unwrap_or(header.view),
                    _ => header.overflow.unwrap_or(header.shading),
                };
                (rect[0], rect[1] + rect[3] + 1)
            }
        }
    }
    pub(super) fn menu_layout(&self, o: &mut Layout, menu: usize) {
        let (x, y) = self.menu_anchor(menu);
        self.with_menu(menu, &mut |items| {
            o.menu(x, y, items, Action::MenuPad);
        });
    }
    /// Status bar: key hints (or the last message) left; active entry · LIB
    /// and the save state right.
    pub(super) fn statusbar(&self, o: &mut Layout) {
        let w = self.width;
        let y = self.height - m::STATUSBAR_H;
        let h = m::STATUSBAR_H;
        let d = &mut o.canvas;
        d.rect(0, y, w, h, c::GM_950);
        d.rect(0, y, w, 1, c::GM_1000);
        let base = baseline(y, h, Style::Hint);
        // Right group, laid out from the right edge.
        let mut right = w - space::SPACE_2;
        let (state, glyph, color): (String, Option<Icon>, Rgb) = if self.doc.dirty() {
            let n = self.doc.changed_count();
            (
                if n == 0 {
                    "Not saved".into()
                } else {
                    view::count(n, "unsaved edit", "unsaved edits")
                },
                None,
                c::INK_MUTED,
            )
        } else if let Some(report) = &self.validation {
            if report.errors > 0 {
                (
                    view::count(report.errors, "validation error", "validation errors"),
                    Some(Icon::Warning),
                    c::DANGER,
                )
            } else {
                ("Validated".into(), Some(Icon::Check), c::OK)
            }
        } else if self.file_backed {
            ("Saved".into(), Some(Icon::Check), c::OK)
        } else if self.doc.archive.entries.is_empty() {
            ("No LIB open".into(), None, c::INK_MUTED)
        } else {
            ("Not on disk".into(), None, c::INK_MUTED)
        };
        let state_w = text_width(&state, Style::Hint);
        right -= state_w;
        d.styled(right, base, &state, color, Style::Hint);
        if let Some(g) = glyph {
            right -= m::ICON_SM + space::SPACE_1;
            d.icon_sm(right, y + (h - m::ICON_SM) / 2, g, color, c::GM_950);
        }
        if self.doc.dirty() {
            right -= m::DIRTY_DOT + space::SPACE_1;
            dot(d, right, y + (h - m::DIRTY_DOT) / 2, c::AMBER);
        }
        right -= space::SPACE_4;
        let where_ = if self.doc.archive.entries.is_empty() {
            String::new()
        } else {
            format!("{} \u{b7} {}", self.name(), self.lib_name())
        };
        let left_limit = w / 2;
        if !where_.is_empty() {
            let text = fit(&where_, right - left_limit, Style::Value);
            right -= text_width(&text, Style::Value);
            d.styled(
                right,
                baseline(y, h, Style::Value),
                &text,
                c::INK,
                Style::Value,
            );
            right -= space::SPACE_4;
        }
        // Left group: the last message, or key hints for the current mode.
        let x = space::SPACE_2;
        let message = !self.status.starts_with("Opened ")
            && !self.status.starts_with("Ready")
            && !self.status.starts_with("Synthetic demo")
            && !self.status.is_empty();
        if message {
            let error = self.status.starts_with("Error:");
            let mut tx = x;
            if error {
                d.icon_sm(
                    tx,
                    y + (h - m::ICON_SM) / 2,
                    Icon::Warning,
                    c::DANGER,
                    c::GM_950,
                );
                tx += m::ICON_SM + space::SPACE_1;
            }
            d.styled(
                tx,
                base,
                &fit(&self.status, right - tx, Style::Hint),
                if error { c::DANGER } else { c::INK },
                Style::Hint,
            );
            return;
        }
        let mut hx = x;
        for (key, label) in self.key_hints() {
            let need = keycap_width(key) + space::SPACE_1 + text_width(label, Style::Hint);
            if hx + need > right {
                break;
            }
            hx += keycap(d, hx, y + (h - m::KEYCAP_H) / 2, key) + space::SPACE_1;
            d.styled(hx, base, label, c::INK_MUTED, Style::Hint);
            hx += text_width(label, Style::Hint) + space::SPACE_4;
        }
    }
    /// Context key hints: the editor and mode decide which keys matter.
    pub(super) fn key_hints(&self) -> &'static [(&'static str, &'static str)] {
        match self.mode {
            Mode::Model if self.mesh_edit => &[
                ("G", "Move"),
                ("S", "Scale"),
                ("1", "Vertex"),
                ("3", "Face"),
                ("B", "Box"),
                ("L", "Part"),
                ("X", "Delete"),
                ("E", "Extrude"),
                ("F", "Make face"),
                ("Tab", "Object mode"),
            ],
            Mode::Model if self.animation_tool => &[
                ("LMB", "Pick part"),
                ("MMB", "Orbit"),
                ("Tab", "Edit mode"),
                ("Ctrl+Z", "Undo setting"),
            ],
            Mode::Model => &[
                ("G", "Move"),
                ("R", "Rotate"),
                ("S", "Scale"),
                ("Tab", "Edit mode"),
                ("MMB", "Orbit"),
                ("H", "Hardpoint"),
            ],
            Mode::Browse => &[
                ("Ctrl+F", "Filter"),
                ("Ctrl+E", "Export"),
                ("Del", "Remove"),
                ("Ctrl+C", "Copy"),
            ],
            Mode::Properties => &[
                ("Enter", "Edit value"),
                ("Wheel", "Scroll"),
                ("Ctrl+Z", "Undo"),
            ],
            Mode::Graft => &[("Wheel", "Scroll"), ("Ctrl+Z", "Undo graft")],
            Mode::Package => &[("Ctrl+B", "Package"), ("Ctrl+S", "Save")],
            Mode::Media => &[
                ("LMB", "Paint"),
                ("MMB", "Pan"),
                ("Wheel", "Zoom"),
                ("Ctrl+Z", "Undo stroke"),
            ],
        }
    }
}

// ---------------------------------------------------------------- dock header

impl App {
    /// Dock tabs that apply right now: (id, label, compact label).
    fn dock_tabs(&self) -> Vec<(u8, &'static str, &'static str)> {
        let mut tabs = vec![
            (0, "Raw fields", "Raw"),
            (1, "Hex", "Hex"),
            (2, "Details", "Details"),
            (4, "References", "Links"),
        ];
        if self.mode == Mode::Media && self.context_model.is_some() {
            tabs.push((3, "3D preview", "3D"));
        }
        tabs
    }
    /// Dock header: 28px, the dock tabs as one segmented control, the entry
    /// name at the right when it fits.
    pub(super) fn dock_header(&self, o: &mut Layout, x: i32, y: i32, w: i32) {
        let h = m::EDITOR_HEADER_H;
        o.canvas.rect(x, y, w, h, c::GM_800);
        o.canvas.rect(x, y, w, 1, c::GM_1000);
        o.canvas.rect(x, y + h - 1, w, 1, c::GM_1000);
        let tabs = self.dock_tabs();
        let full: Vec<(Btn, Action)> = tabs
            .iter()
            .map(|(id, label, _)| (Btn::new(label).on(self.dock == *id), Action::Dock(*id)))
            .collect();
        let items = if Layout::segmented_width(&full) <= w - 2 * space::SPACE_1 {
            full
        } else {
            tabs.iter()
                .map(|(id, _, short)| (Btn::new(short).on(self.dock == *id), Action::Dock(*id)))
                .collect()
        };
        let seg_w = Layout::segmented_width(&items).min(w - 2 * space::SPACE_1);
        let top = y + (h - m::BUTTON_H) / 2;
        o.segmented([x + space::SPACE_1, top, seg_w, m::BUTTON_H], &items);
        let nx = x + space::SPACE_1 + seg_w + space::SPACE_4;
        if x + w - space::SPACE_2 - nx > 60 {
            let name = fit(self.name(), x + w - space::SPACE_2 - nx, Style::Value);
            o.canvas.styled(
                x + w - space::SPACE_2 - text_width(&name, Style::Value),
                baseline(y, h, Style::Value),
                &name,
                c::INK_MUTED,
                Style::Value,
            );
        }
    }
}
// ---------------------------------------------------------------- viewport header

/// Viewport interaction modes offered by the mode Select.
pub(super) const VIEWPORT_MODES: [(&str, Icon, Option<&str>); 5] = [
    ("Object Mode", Icon::Select, Some("Tab")),
    ("Edit Mesh", Icon::Shape, Some("Tab")),
    ("Hardpoints", Icon::Hardpoint, None),
    ("Parts", Icon::Sliders, None),
    ("Texture Paint", Icon::Brush, None),
];
/// Shading modes: wireframe, solid (flat face colors), textured.
pub(super) const SHADING: [(&str, Icon); 3] = [
    ("Wireframe", Icon::Wire),
    ("Solid", Icon::Solid),
    ("Textured", Icon::Textured),
];
pub(super) struct HeaderSlots {
    pub mode: [i32; 4],
    pub view: [i32; 4],
    /// Edit Mesh: the Select and Mesh menus and the vertex/face modes.
    pub select: Option<[i32; 4]>,
    pub mesh: Option<[i32; 4]>,
    pub modes: Option<[i32; 4]>,
    pub visibility: [i32; 4],
    pub shading: [i32; 4],
    /// Set when the right group does not fit: one button opens it as a menu.
    pub overflow: Option<[i32; 4]>,
}
impl App {
    pub(super) fn viewport_mode(&self) -> usize {
        if self.mesh_edit {
            1
        } else if self.hp_tool {
            2
        } else if self.animation_tool {
            3
        } else if self.model_paint {
            4
        } else {
            0
        }
    }
    pub(super) fn shading(&self) -> usize {
        match (self.textured, self.flat) {
            (false, _) => 0,
            (true, true) => 1,
            (true, false) => 2,
        }
    }
    pub(super) fn viewport_header_slots(&self) -> HeaderSlots {
        let l = self.left() + 1;
        let r = self.right() - 1;
        let y = m::MENUBAR_H + (m::EDITOR_HEADER_H - m::BUTTON_H) / 2;
        let h = m::BUTTON_H;
        let mode_w = VIEWPORT_MODES
            .iter()
            .map(|(label, _, _)| Layout::select_width(true, label))
            .max()
            .unwrap_or(116);
        let mode = [l + space::SPACE_1, y, mode_w, h];
        let view_w = Btn::new("View").ghost().width();
        let view = [mode[0] + mode_w + space::SPACE_2, y, view_w, h];
        let seg_w = 3 * m::ICON_BUTTON + 2;
        let shading = [r - space::SPACE_1 - seg_w, y, seg_w, h];
        let visibility = [
            shading[0] - space::SPACE_2 - m::ICON_BUTTON,
            y,
            m::ICON_BUTTON,
            h,
        ];
        let mut end = view[0] + view[2];
        let (mut select, mut mesh) = (None, None);
        if self.mesh_edit {
            let sw = Btn::new("Select").ghost().width();
            let mw = Btn::new("Mesh").ghost().width();
            select = Some([end + space::SPACE_1, y, sw, h]);
            mesh = Some([end + 2 * space::SPACE_1 + sw, y, mw, h]);
            end += 2 * space::SPACE_1 + sw + mw;
        }
        let overflow = (visibility[0] < end + space::SPACE_2).then_some([
            r - space::SPACE_1 - m::ICON_BUTTON,
            y,
            m::ICON_BUTTON,
            h,
        ]);
        let limit = overflow.map_or(visibility[0], |o| o[0]) - space::SPACE_2;
        let seg_modes = 2 * m::ICON_BUTTON + 2;
        let modes = (self.mesh_edit && end + space::SPACE_2 + seg_modes <= limit).then_some([
            end + space::SPACE_2,
            y,
            seg_modes,
            h,
        ]);
        HeaderSlots {
            mode,
            view,
            select,
            mesh,
            modes,
            visibility,
            shading,
            overflow,
        }
    }
    /// Viewport editor header: mode Select, View menu, hardpoint overlay
    /// toggle and the wireframe / solid / textured segmented control.
    pub(super) fn viewport_header(&self, o: &mut Layout) {
        let l = self.left() + 1;
        let width = self.right() - self.left() - 2;
        let top = m::MENUBAR_H;
        o.canvas.rect(l, top, width, m::EDITOR_HEADER_H, c::GM_800);
        o.canvas
            .rect(l, top + m::EDITOR_HEADER_H - 1, width, 1, c::GM_1000);
        let s = self.viewport_header_slots();
        let (label, glyph, _) = VIEWPORT_MODES[self.viewport_mode()];
        o.select(
            [s.mode[0], s.mode[1] + 1, s.mode[2], m::FIELD_H],
            Some(glyph),
            label,
            Action::Menu(MENU_MODE),
            self.menu == Some(MENU_MODE),
        );
        o.button_ex(
            s.view,
            Btn::new("View").ghost().on(self.menu == Some(MENU_VIEW)),
            Action::Menu(MENU_VIEW),
        );
        for (rect, label, menu) in [
            (s.select, "Select", MENU_SELECT),
            (s.mesh, "Mesh", MENU_MESH),
        ] {
            if let Some(rect) = rect {
                o.button_ex(
                    rect,
                    Btn::new(label).ghost().on(self.menu == Some(menu)),
                    Action::Menu(menu),
                );
            }
        }
        if let Some(rect) = s.modes {
            o.segmented(
                rect,
                &[
                    (
                        Btn::icon(Icon::Vertex).on(!self.ed.face_select),
                        Action::SelectMode(false),
                    ),
                    (
                        Btn::icon(Icon::Face).on(self.ed.face_select),
                        Action::SelectMode(true),
                    ),
                ],
            );
        }
        if let Some(rect) = s.overflow {
            o.button_ex(
                rect,
                Btn::icon(SHADING[self.shading()].1)
                    .ghost()
                    .on(self.menu == Some(MENU_SHADING)),
                Action::Menu(MENU_SHADING),
            );
            return;
        }
        o.button_ex(
            s.visibility,
            Btn::icon(Icon::Hardpoint).on(self.hp_visible),
            Action::HardpointVisibility,
        );
        let shading = self.shading();
        let items: Vec<(Btn, Action)> = SHADING
            .iter()
            .enumerate()
            .map(|(i, (_, g))| (Btn::icon(*g).on(i == shading), Action::Shading(i as u8)))
            .collect();
        o.segmented(s.shading, &items);
    }
    /// Floating tool strip: Select, Move, Rotate, Scale | Frame, Paint.
    pub(super) fn tool_strip(&self, o: &mut Layout, writable: bool) {
        let x = self.left() + 1 + space::SPACE_2;
        let y = m::MENUBAR_H + m::EDITOR_HEADER_H + space::SPACE_2;
        let b = m::TOOLSTRIP_BUTTON;
        let tools: [Option<(Icon, Action, bool, bool)>; 7] = [
            Some((Icon::Select, Action::SelectTool, true, !self.model_paint)),
            Some((Icon::Move, Action::Transform('g'), writable, false)),
            Some((Icon::Rotate, Action::Transform('r'), writable, false)),
            Some((Icon::Scale, Action::Transform('s'), writable, false)),
            None,
            Some((Icon::Frame, Action::View(0), true, false)),
            Some((
                Icon::Brush,
                Action::ModelPaint,
                self.model.is_some(),
                self.model_paint,
            )),
        ];
        let h = 3 + 6 * (b + 2) - 2 + 5 + 3;
        let w = b + 6;
        notched(
            &mut o.canvas,
            [x, y, w, h],
            Some(c::GM_800),
            Some(c::GM_1000),
        );
        let mut ty = y + 3;
        for tool in tools {
            match tool {
                None => {
                    o.canvas.rect(x + 3 + 2, ty + 1, b - 4, 1, c::GM_600);
                    ty += 5;
                }
                Some((g, action, enabled, on)) => {
                    o.button_ex(
                        [x + 3, ty, b, b],
                        Btn::icon(g).ghost().on(on).enabled(enabled),
                        action,
                    );
                    ty += b + 2;
                }
            }
        }
    }
}

// ---------------------------------------------------------------- viewport overlays

/// Integer square root (floor).
pub(super) fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
/// Filled disc of radius `r` centred on (cx, cy), drawn as row spans.
pub(super) fn disc(d: &mut Canvas, cx: i32, cy: i32, r: i32, color: Rgb) {
    for dy in -r..=r {
        let dx = isqrt((r * r - dy * dy) as i64 + r as i64 / 2) as i32;
        d.rect(cx - dx, cy + dy, 2 * dx + 1, 1, color);
    }
}
/// Disc with a 1px (or `width`) ring in `edge`.
pub(super) fn ring(d: &mut Canvas, cx: i32, cy: i32, r: i32, edge: Rgb, fill: Rgb) {
    disc(d, cx, cy, r, edge);
    disc(d, cx, cy, r - 1, fill);
}
impl App {
    fn view_name(&self) -> &'static str {
        if self.perspective {
            "User \u{b7} Perspective"
        } else if self.pitch == 90 {
            "Top \u{b7} Orthographic"
        } else if self.yaw == 90 && self.pitch == 0 {
            "Side \u{b7} Orthographic"
        } else if self.yaw == 0 && self.pitch == 0 {
            "Front \u{b7} Orthographic"
        } else {
            "User \u{b7} Orthographic"
        }
    }
    /// Overlay text (view, entry, counts; mode bottom-left, zoom bottom-right)
    /// and the navigation gizmo.
    pub(super) fn viewport_overlay(&self, o: &mut Layout, writable: bool) {
        let l = self.left() + 1;
        let r = self.right() - 1;
        let top = m::MENUBAR_H + m::EDITOR_HEADER_H;
        let bottom = self.dock_y();
        let x = l + space::SPACE_2 + m::TOOLSTRIP_BUTTON + 6 + space::SPACE_2 + space::SPACE_1;
        let room = r - 2 * space::SPACE_2 - m::GIZMO - x;
        let line = Style::ValueSm.spec().line;
        let mut y = top + space::SPACE_2 + 11;
        let d = &mut o.canvas;
        d.styled(x, y, self.view_name(), c::INK, Style::ValueSm);
        y += line;
        if let Some(i) = self.model_entry {
            d.styled(
                x,
                y,
                &fit(&self.doc.archive.entries[i].name, room, Style::ValueSm),
                c::INK_MUTED,
                Style::ValueSm,
            );
            y += line;
        }
        if let Some(model) = &self.model {
            let counts = format!(
                "{} \u{b7} {}",
                view::count(model.vertices.len(), "vert", "verts"),
                view::count(model.faces.len(), "face", "faces")
            );
            d.styled(
                x,
                y,
                &fit(&counts, room, Style::ValueSm),
                c::INK_MUTED,
                Style::ValueSm,
            );
            y += line;
        }
        if let Some(i) = self.model_entry {
            let users = self
                .dependencies
                .incoming(&self.doc.archive.entries[i].name)
                .count();
            if users > 1 {
                d.icon_sm(x, y - 10, Icon::Warning, c::AMBER, c::GM_950);
                d.styled(
                    x + m::ICON_SM + space::SPACE_1,
                    y,
                    &fit(
                        &format!("Shared shape \u{b7} {users} direct users in this LIB"),
                        room - m::ICON_SM - space::SPACE_1,
                        Style::ValueSm,
                    ),
                    c::AMBER,
                    Style::ValueSm,
                );
                y += line;
            }
        }
        if !self.ed.pose.is_empty() {
            let pose: Vec<String> = self
                .ed
                .pose
                .iter()
                .map(|(k, v)| format!("{}={v}", k.trim_start_matches("_PL")))
                .collect();
            d.styled(
                x,
                y,
                &fit(
                    &format!("Pose preview \u{b7} {}", pose.join(" ")),
                    room,
                    Style::ValueSm,
                ),
                c::AMBER,
                Style::ValueSm,
            );
        }
        let zoom = format!("zoom {}%", self.zoom);
        let zoom_w = text_width(&zoom, Style::ValueSm);
        let base = bottom - space::SPACE_2 - 3;
        d.styled(
            r - space::SPACE_3 - zoom_w,
            base,
            &zoom,
            c::INK_MUTED,
            Style::ValueSm,
        );
        let mode = format!(
            "{} \u{b7} {}",
            VIEWPORT_MODES[self.viewport_mode()].0,
            if self.mesh_edit {
                self.selection_text()
            } else if self.animation_tool {
                match self.ed.part_selected {
                    Some(_) => format!(
                        "{} \u{b7} {}",
                        view::count(self.ed.parts.len(), "part", "parts"),
                        view::count(self.ed.mesh_faces.len(), "face", "faces")
                    ),
                    None => view::count(self.ed.parts.len(), "part", "parts"),
                }
            } else if writable {
                "editable static mesh".into()
            } else {
                "static preview".into()
            }
        );
        d.styled(
            x,
            base,
            &fit(
                &mode,
                r - space::SPACE_3 - zoom_w - space::SPACE_4 - x,
                Style::ValueSm,
            ),
            c::INK_MUTED,
            Style::ValueSm,
        );
        if self.model.is_some() || self.preview.is_some() {
            self.gizmo(o, r - space::SPACE_2 - m::GIZMO / 2, top + 6 + m::GIZMO / 2);
        }
    }
    /// Navigation gizmo: the body axes through the current camera rotation,
    /// positive ends as labelled axis-colored caps on 2px stems, negative ends
    /// as rings, drawn back to front. A positive cap views along that axis.
    pub(super) fn gizmo(&self, o: &mut Layout, cx: i32, cy: i32) {
        const LEN: i32 = 28;
        const UNIT: i32 = 1024;
        let colors = [c::AXIS_X, c::AXIS_Y, c::AXIS_Z];
        let views = [3u8, 1, 7];
        let mut caps: Vec<(i32, usize, bool, i32, i32)> = Vec::new();
        for axis in 0..3 {
            let mut p = [0; 3];
            p[axis] = UNIT;
            let v = self.camera_point(p);
            for positive in [true, false] {
                let s = if positive { 1 } else { -1 };
                caps.push((
                    s * v[2],
                    axis,
                    positive,
                    cx + s * v[0] * LEN / UNIT,
                    cy - s * v[1] * LEN / UNIT,
                ));
            }
        }
        // Far caps first; the camera looks down -z, so larger z is nearer.
        caps.sort_unstable_by_key(|cap| (cap.0, cap.1, cap.2));
        for (_, axis, positive, x, y) in caps {
            let color = colors[axis];
            if positive {
                // 2px stem: two 1px lines offset across the stem direction.
                let horizontal = (x - cx).abs() >= (y - cy).abs();
                for k in 0..2 {
                    let (ox, oy) = if horizontal { (0, k) } else { (k, 0) };
                    o.canvas.line(cx + ox, cy + oy, x + ox, y + oy, color);
                }
                disc(&mut o.canvas, x, y, 8, color);
                let label = ["X", "Y", "Z"][axis];
                o.canvas.styled(
                    x - text_width(label, Style::Badge) / 2,
                    y + 4,
                    label,
                    c::GM_1000,
                    Style::Badge,
                );
                o.hit([x - 8, y - 8, 17, 17], Action::View(views[axis]));
            } else {
                ring(&mut o.canvas, x, y, 6, color, c::GM_800);
            }
        }
    }
}

/// Solid-mode light for a face, 96..=256 of 256: faces turned toward the
/// camera are brighter. Uses the stored normal or the first triangle's.
/// Integer only.
pub(super) fn light(app: &App, m: &hangar_core::model::Model, f: &hangar_core::model::Face) -> i32 {
    let n = f.normal.or_else(|| {
        let p = |k: usize| m.vertices.get(*f.indices.get(k)?).map(|v| v.point);
        let (a, b, c) = (p(0)?, p(1)?, p(2)?);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]].map(|v| v as i64);
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]].map(|v| v as i64);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let big = n.iter().map(|c| c.abs()).max().unwrap_or(0);
        let scale = (big / 30000).max(1);
        Some(n.map(|c| (c / scale) as i32))
    });
    let Some(n) = n else {
        return 200;
    };
    let v = app.camera_point(n);
    let len = isqrt(n.iter().map(|c| *c as i64 * *c as i64).sum());
    if len == 0 {
        return 200;
    }
    96 + (160 * (v[2] as i64).abs() / len).min(160) as i32
}
/// `color` (0xRRGGBB) scaled by `light`/256.
pub(super) fn shade(color: u32, light: i32) -> u32 {
    let l = light.clamp(0, 256) as u32;
    let ch = |shift: u32| ((color >> shift & 255) * l / 256) << shift;
    ch(16) | ch(8) | ch(0)
}

// ---------------------------------------------------------------- smoke test

/// Chrome actions whose hit regions must match what they draw.
fn chrome_action(a: Action) -> bool {
    matches!(
        a,
        Action::Menu(_)
            | Action::Mode(_)
            | Action::Demo
            | Action::Dock(_)
            | Action::Shading(_)
            | Action::HardpointVisibility
            | Action::SelectTool
            | Action::Transform(_)
            | Action::ModelPaint
            | Action::View(_)
    )
}
impl App {
    /// The topmost hit region whose action matches (shared smoke helper).
    #[inline(never)]
    pub(super) fn chrome_hit(&self, predicate: &dyn Fn(Action) -> bool) -> Option<[i32; 4]> {
        self.layout()
            .hits
            .into_iter()
            .rev()
            .find(|h| predicate(h.action))
            .map(|h| h.rect)
    }
    pub(super) fn chrome_click(&mut self, rect: [i32; 4]) {
        self.smoke_drag(rect, 0);
    }
    /// Hover, press at the centre of `rect`, move `dx` px and release.
    #[inline(never)]
    pub(super) fn smoke_drag(&mut self, rect: [i32; 4], dx: i32) {
        let (x, y) = (rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
        self.motion(x, y, false);
        self.pointer(x, y, 1, true, false);
        if dx != 0 {
            self.motion(x + dx, y, false);
        }
        self.pointer(x + dx, y, 1, false, false);
    }
    /// Every chrome hit region is covered by what the control draws when
    /// hovered (1px notch tolerance), and stays inside the window.
    fn smoke_chrome_hits(&mut self) {
        let hits: Vec<[i32; 4]> = self
            .layout()
            .hits
            .into_iter()
            .filter(|h| chrome_action(h.action) && h.rect[1] < self.dock_y() + m::EDITOR_HEADER_H)
            .map(|h| h.rect)
            .collect();
        assert!(hits.len() > 20, "Chrome controls missing");
        for [x, y, w, h] in hits {
            assert!(
                x >= 0 && y >= 0 && x + w <= self.width && y + h <= self.height,
                "Chrome control outside the window"
            );
            self.mouse = [x + w / 2, y + h / 2];
            let mut union = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
            for d in self.draw().commands {
                let r = match d {
                    Draw::Rect(rx, ry, rw, rh, _) => [rx, ry, rx + rw, ry + rh],
                    _ => continue,
                };
                if r[0] >= x - 1 && r[1] >= y - 1 && r[2] <= x + w + 1 && r[3] <= y + h + 1 {
                    union = [
                        union[0].min(r[0]),
                        union[1].min(r[1]),
                        union[2].max(r[2]),
                        union[3].max(r[3]),
                    ];
                }
            }
            assert!(
                union[0] <= x + 1
                    && union[1] <= y + 1
                    && union[2] >= x + w - 1
                    && union[3] >= y + h - 1,
                "Hit region {:?} exceeds its drawn control {union:?}",
                [x, y, w, h]
            );
        }
        self.mouse = [0, 0];
    }
    /// Text drawn in the menu bar, editor headers and status bar is never
    /// cut short at 1280 x 800.
    fn smoke_chrome_labels(&self) {
        let l = self.left();
        let r = self.right();
        for d in self.draw().commands {
            if let Draw::Text(x, y, s, _, _) = d {
                let chrome = y < m::MENUBAR_H
                    || y > self.height - m::STATUSBAR_H
                    || (x > l && x < r && y < m::MENUBAR_H + m::EDITOR_HEADER_H)
                    || (x > l
                        && x < r
                        && y > self.dock_y()
                        && y < self.dock_y() + m::EDITOR_HEADER_H);
                assert!(
                    !(chrome && s.contains('\u{2026}')),
                    "Truncated chrome label {s}"
                );
            }
        }
    }
    /// Drive the chrome through its hit regions at 1280 x 800 and 800 x 600.
    pub fn smoke_chrome(&mut self) {
        for (w, h) in [(1280, 800), (800, 600)] {
            self.demo();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            self.select_entry(0);
            self.smoke_chrome_hits();
            if w == 1280 {
                self.smoke_chrome_labels();
            }
            // Menus open from their names, sit inside the window, close on an
            // outside click without touching what is underneath.
            for menu in 0..7 {
                let name = self
                    .chrome_hit(&|a| matches!(a, Action::Menu(n) if n == menu))
                    .unwrap();
                self.chrome_click(name);
                assert_eq!(self.menu, Some(menu));
                let [mx, my, mw, mh] = self.open_menu_rect().unwrap();
                assert!(
                    mx >= 0 && my >= 0 && mx + mw <= w && my + mh <= h,
                    "Menu {menu} off screen"
                );
                let (yaw, pitch, mode) = (self.yaw, self.pitch, self.mode);
                self.chrome_click([self.right() - 40, self.dock_y() - 40, 2, 2]);
                assert!(self.menu.is_none(), "Outside click closes menu {menu}");
                assert!(self.yaw == yaw && self.pitch == pitch && self.mode == mode);
            }
            // Workspace tabs switch workspaces.
            for (_, mode) in TABS {
                let tab = self
                    .chrome_hit(&|a| matches!(a, Action::Mode(m) if m == mode))
                    .unwrap();
                self.chrome_click(tab);
                assert!(self.mode == mode || mode == Mode::Media, "Workspace tab");
            }
            self.select_entry(0);
            self.mode = Mode::Model;
            // Mode Select: Edit Mesh and back to Object Mode through the dropdown.
            for (n, edit) in [(1u8, true), (0, false)] {
                let select = self
                    .chrome_hit(&|a| matches!(a, Action::Menu(MENU_MODE)))
                    .unwrap();
                self.chrome_click(select);
                assert_eq!(self.menu, Some(MENU_MODE));
                let item = self
                    .chrome_hit(&|a| matches!(a, Action::ViewportMode(m) if m == n))
                    .unwrap();
                self.chrome_click(item);
                assert_eq!(self.mesh_edit, edit, "Mode select");
                assert!(self.menu.is_none());
            }
            let label = VIEWPORT_MODES[0].0;
            assert!(self
                .draw()
                .commands
                .iter()
                .any(|d| matches!(d, Draw::Text(_, _, s, _, _) if s == label)));
            // Shading: segmented control, or its overflow menu when narrow.
            for (n, textured, flat) in [(1u8, true, true), (2, true, false), (0, false, false)] {
                if self.viewport_header_slots().overflow.is_some() {
                    let more = self
                        .chrome_hit(&|a| matches!(a, Action::Menu(MENU_SHADING)))
                        .unwrap();
                    self.chrome_click(more);
                }
                let seg = self
                    .chrome_hit(&|a| matches!(a, Action::Shading(m) if m == n))
                    .unwrap();
                self.chrome_click(seg);
                assert_eq!((self.textured, self.flat), (textured, flat), "Shading {n}");
            }
            // The header View menu and the gizmo both change the view.
            let view = self
                .chrome_hit(&|a| matches!(a, Action::Menu(MENU_VIEW)))
                .unwrap();
            self.chrome_click(view);
            let front = self.chrome_hit(&|a| matches!(a, Action::View(1))).unwrap();
            self.chrome_click(front);
            assert_eq!((self.yaw, self.pitch), (0, 0));
            let top = self.chrome_hit(&|a| matches!(a, Action::View(7))).unwrap();
            self.chrome_click(top);
            assert_eq!(self.pitch, 90, "Gizmo Z cap views from the top");
            // Dock tabs.
            let hex = self.chrome_hit(&|a| matches!(a, Action::Dock(1))).unwrap();
            self.chrome_click(hex);
            assert_eq!(self.dock, 1);
            // Status bar: keycap hints, then ENTRY · LIB and the save state.
            let status = |app: &App| -> Vec<String> {
                app.draw()
                    .commands
                    .into_iter()
                    .filter_map(|d| match d {
                        Draw::Text(_, y, s, _, _) if y > app.height - m::STATUSBAR_H => Some(s),
                        _ => None,
                    })
                    .collect()
            };
            self.status.clear();
            let text = status(self);
            assert!(text.iter().any(|s| s == "G") && text.iter().any(|s| s == "Move"));
            assert!(
                text.iter().any(|s| s == "DEMO.SH \u{b7} DEMO.LIB"),
                "{text:?}"
            );
            self.doc
                .replace(0, hangar_core::model::demo_shape())
                .unwrap();
            assert!(status(self).iter().any(|s| s == "1 unsaved edit"));
            self.doc.undo();
            self.refresh();
        }
        self.width = 1280;
        self.height = 800;
    }
}
