//! Window chrome: menu bar, dropdown menus, status bar, viewport header and
//! tool strip, viewport overlays and gizmo, dock header. Built from the
//! ui_widgets components; positions come from one place per bar so drawing,
//! hit regions and dropdown anchors always agree.
use super::view::{Action, Icon, Layout};
use super::widgets::{baseline, dot, keycap, keycap_width, notched, Btn, Item};
use super::*;
use theme::{metric as m, space};

const MENUS: [&str; 7] = ["File", "Edit", "Lib", "Entry", "View", "Tools", "Help"];
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
            4 => {
                let mut items = vec![
                    Item::new("Frame all", Action::View(0)).key("Home"),
                    Item::new("Front", Action::View(1)).key("1"),
                    Item::new("Side", Action::View(3)).key("3"),
                    Item::new("Top", Action::View(7)).key("7"),
                    Item::new("Toggle projection", Action::View(5)).key("5"),
                ];
                if menu == 4 {
                    items.push(Item::sep());
                    items.push(Item::new("Toggle textured", Action::Textured));
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
            _ => vec![
                Item::new("Controls", Action::Help).key("F1"),
                Item::new("Load synthetic demo", Action::Demo),
            ],
        }
    }
    /// Top-left anchor of dropdown `menu`.
    pub(super) fn menu_anchor(&self, menu: usize) -> (i32, i32) {
        let bar = self.bar();
        (bar.menus[menu.min(6)][0], m::MENUBAR_H)
    }
    pub(super) fn menu_layout(&self, o: &mut Layout, menu: usize) {
        let (x, y) = self.menu_anchor(menu);
        o.menu(x, y, &self.menu_items(menu), Action::MenuPad);
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
                c::INK,
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
                ("R", "Rotate"),
                ("S", "Scale"),
                ("A", "Select all"),
                ("Shift", "Extend"),
                ("Tab", "Object mode"),
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
