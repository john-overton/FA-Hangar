//! F.A. Hangar theme tokens, generated from tokens.json by tools/gen/theme.py.
//! Do not edit by hand: change tokens.json and rerun the generator.
//! Colors are 0x00RRGGBB (GDI COLORREF wants 0x00BBGGRR: use `colorref()`).
//! No alpha and no floating point by design: every surface is a solid fill.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    pub const fn r(self) -> u8 { (self.0 >> 16) as u8 }
    pub const fn g(self) -> u8 { (self.0 >> 8) as u8 }
    pub const fn b(self) -> u8 { self.0 as u8 }
    /// Win32 COLORREF (0x00BBGGRR) for GDI calls.
    pub const fn colorref(self) -> u32 { (self.b() as u32) << 16 | (self.g() as u32) << 8 | self.r() as u32 }
    /// Integer mix: `weight` of 256 parts `other`. Used for icon edge pixels, never for alpha.
    pub const fn mix(self, other: Rgb, weight: u32) -> Rgb {
        const fn c(a: u8, b: u8, w: u32) -> u32 { (a as u32 * (256 - w) + b as u32 * w) / 256 }
        let w = if weight > 256 { 256 } else { weight };
        Rgb(c(self.r(), other.r(), w) << 16 | c(self.g(), other.g(), w) << 8 | c(self.b(), other.b(), w))
    }
}

pub mod color {
    use super::Rgb;
    /// Keylines: the 1px dark edge around buttons, panels, and editor seams. Never a fill for content.
    pub const GM_1000: Rgb = Rgb(0x0F1113);
    /// Viewport background, field and NumberField fill, menubar, status bar, vertical tab strip.
    pub const GM_950: Rgb = Rgb(0x15181B);
    /// App window background behind editors; zebra rows in the outliner; timeline ruler.
    pub const GM_900: Rgb = Rgb(0x1B1F23);
    /// Editor and panel surface (outliner, properties, timeline). The default ground for text.
    pub const GM_800: Rgb = Rgb(0x23282D);
    /// Raised controls: buttons, selects, keycaps; row hover.
    pub const GM_700: Rgb = Rgb(0x2C3238);
    /// Button hover; type badge fill; separators inside menus.
    pub const GM_600: Rgb = Rgb(0x383F46);
    /// Non-selected inner mesh edges in the viewport; timeline spans. Decorative only, never text.
    pub const GM_500: Rgb = Rgb(0x48515A);
    /// Border on fields, NumberFields, checkboxes, and graft slots. 3:1 on gm-800 and gm-700.
    pub const LINE_STRONG: Rgb = Rgb(0x727B84);
    /// Primary text and values on every gm surface (10:1 or better) and on amber-deep.
    pub const INK: Rgb = Rgb(0xE4E8EB);
    /// Property labels, secondary text, idle icons, viewport overlay text. 5:1 or better on gm-600 through gm-1000.
    pub const INK_MUTED: Rgb = Rgb(0xAAB3BB);
    /// Disabled text and placeholders only. 4.2:1 on gm-800; never for live labels.
    pub const INK_FAINT: Rgb = Rgb(0x808A93);
    /// Selection and the active state: selected outline in the viewport, active tab icon, changed values, primary button fill, dirty dot. 6:1 on gm-800.
    pub const AMBER: Rgb = Rgb(0xE9A23B);
    /// The ACTIVE item inside a selection (active outliner row text, object origin); primary button hover.
    pub const AMBER_BRIGHT: Rgb = Rgb(0xF6C066);
    /// Solid selection fill behind selected rows, toggled buttons, and hot menu items. Takes ink, amber, or amber-bright text.
    pub const AMBER_DEEP: Rgb = Rgb(0x3A2F1C);
    /// Text and checkmarks on amber and amber-bright fills.
    pub const ON_AMBER: Rgb = Rgb(0x1B1307);
    /// Time and reference: timeline playhead, hardpoint markers, graft ghost geometry, links. 6:1 on gm-800.
    pub const STEEL: Rgb = Rgb(0x66AEDB);
    /// Slider fill inside a NumberField that has a bounded range.
    pub const STEEL_DEEP: Rgb = Rgb(0x1D3140);
    /// Keyboard focus ring: 1px solid, 1px offset. 6:1 on gm-800, 7:1 on gm-950.
    pub const FOCUS: Rgb = Rgb(0x66AEDB);
    /// Validated / packaged / round-trip clean. Always paired with a check icon or word.
    pub const OK: Rgb = Rgb(0x7CCC8A);
    /// Errors, destructive actions, failed validation. 4.8:1 on gm-800; always paired with a warning icon or word.
    pub const DANGER: Rgb = Rgb(0xEC6A58);
    /// X axis: gizmo, grid axis line, X labels in vector fields. 4.9:1 on gm-950.
    pub const AXIS_X: Rgb = Rgb(0xE2595E);
    /// Y axis: gizmo, grid axis line, Y labels in vector fields. 8:1 on gm-950.
    pub const AXIS_Y: Rgb = Rgb(0x86C24A);
    /// Z axis: gizmo, Z labels in vector fields. 5.4:1 on gm-950.
    pub const AXIS_Z: Rgb = Rgb(0x4F8FE3);
    /// Transparency checkerboard behind masked PIC pixels, light squares. Never a UI surface.
    pub const CHECKER_LIGHT: Rgb = Rgb(0x30363C);
    /// Transparency checkerboard behind masked PIC pixels, dark squares. Never a UI surface.
    pub const CHECKER_DARK: Rgb = Rgb(0x20262C);
    /// 1px top highlight on raised controls (buttons, selects). Drawn as a single line, not a blur.
    pub const LIP: Rgb = Rgb(0x3D444B);
}

/// Spacing, in px at 1x.
pub mod space {
    /// Icon to label gap; gap between stacked panels.
    pub const SPACE_1: i32 = 4;
    /// Panel body inset; label to control gap in a property row.
    pub const SPACE_2: i32 = 8;
    /// Menu item side padding.
    pub const SPACE_3: i32 = 12;
    /// Status bar group gap; dialog padding.
    pub const SPACE_4: i32 = 16;
    /// Gap between dialog sections.
    pub const SPACE_6: i32 = 24;
}

/// Corner radii, in px at 1x.
pub mod radius {
    /// Badges, keycaps, timeline playhead cap.
    pub const RADIUS_XS: i32 = 2;
    /// Buttons, fields, panels, rows. The default.
    pub const RADIUS_SM: i32 = 3;
    /// Floating viewport tool strip.
    pub const RADIUS_MD: i32 = 4;
}

/// Fixed control metrics, in px at 1x (percent where named PCT).
pub mod metric {
    /// Menu bar height.
    pub const MENUBAR_H: i32 = 26;
    /// Menu names inside the menu bar.
    pub const MENUBAR_ITEM_H: i32 = 20;
    /// Workspace tabs, bottom-aligned in the menu bar so the active tab joins the editor below.
    pub const WSTAB_H: i32 = 22;
    /// Every editor header, including the dock tab header.
    pub const EDITOR_HEADER_H: i32 = 28;
    /// Status bar height.
    pub const STATUSBAR_H: i32 = 22;
    /// Buttons and icon buttons.
    pub const BUTTON_H: i32 = 22;
    /// Square icon button.
    pub const ICON_BUTTON: i32 = 22;
    /// Icon buttons inside the floating viewport tool strip.
    pub const TOOLSTRIP_BUTTON: i32 = 26;
    /// Fields, NumberFields and Selects.
    pub const FIELD_H: i32 = 20;
    /// Outliner rows and property rows.
    pub const ROW_H: i32 = 20;
    /// Dropdown menu items.
    pub const MENU_ITEM_H: i32 = 22;
    /// Minimum dropdown menu width.
    pub const MENU_MIN_W: i32 = 160;
    /// Panel header.
    pub const PANEL_HEADER_H: i32 = 24;
    /// Vertical property tab strip.
    pub const VTAB_STRIP_W: i32 = 30;
    /// Vertical property tab.
    pub const VTAB_SIZE: i32 = 24;
    /// Icon size.
    pub const ICON: i32 = 16;
    /// Small icon inside selects, fields and twisties.
    pub const ICON_SM: i32 = 12;
    /// Checkbox box.
    pub const CHECKBOX: i32 = 14;
    /// Type badge height.
    pub const BADGE_H: i32 = 14;
    /// Keycap height in menus and the status bar.
    pub const KEYCAP_H: i32 = 16;
    /// Round amber dirty dot.
    pub const DIRTY_DOT: i32 = 6;
    /// NumberField step arrow, shown on hover.
    pub const NUM_ARROW_W: i32 = 14;
    /// Outliner indent per depth.
    pub const TREE_INDENT: i32 = 14;
    /// Property label column, percent of the row width.
    pub const PROP_LABEL_PCT: i32 = 40;
    /// Viewport navigation gizmo box.
    pub const GIZMO: i32 = 84;
    /// Every Nth viewport grid line is gm-700.
    pub const GRID_MAJOR: i32 = 5;
    /// Outliner width in the Model workspace.
    pub const OUTLINER_W: i32 = 280;
    /// Properties editor width in the Model workspace.
    pub const PROPERTIES_W: i32 = 322;
    /// Animation dock height below the viewport.
    pub const DOCK_H: i32 = 186;
    /// Minimum window width.
    pub const MIN_WINDOW_W: i32 = 800;
    /// Minimum window height.
    pub const MIN_WINDOW_H: i32 = 600;
    /// Minimum window (width, height).
    pub const MIN_WINDOW: (i32, i32) = (800, 600);
}

/// Type styles: (size px, line height px, weight). Family: Ui or Mono.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family { Ui, Mono, Display }

/// `tracking` is letter spacing in 1/100 em (0 for none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextStyle { pub family: Family, pub size: i32, pub line: i32, pub weight: u16, pub tracking: i32 }

pub mod text {
    use super::{Family, TextStyle};
    /// Dialog titles, the graft target name, workspace tab when active.
    pub const TITLE: TextStyle = TextStyle { family: Family::Ui, size: 14, line: 18, weight: 600, tracking: 0 };
    /// Notices and longer help text.
    pub const BODY: TextStyle = TextStyle { family: Family::Ui, size: 13, line: 18, weight: 400, tracking: 0 };
    /// Default UI text: buttons, property labels, tree rows, menus.
    pub const LABEL: TextStyle = TextStyle { family: Family::Ui, size: 12, line: 16, weight: 500, tracking: 0 };
    /// Uppercase sub-headings inside a panel body and graft slot roles.
    pub const SECTION: TextStyle = TextStyle { family: Family::Ui, size: 11, line: 14, weight: 600, tracking: 6 };
    /// Status bar, timeline labels.
    pub const HINT: TextStyle = TextStyle { family: Family::Ui, size: 11, line: 14, weight: 400, tracking: 0 };
    /// Every editable number, filename, and hex offset.
    pub const VALUE: TextStyle = TextStyle { family: Family::Mono, size: 12, line: 16, weight: 400, tracking: 0 };
    /// Viewport overlays, counts, diff summaries.
    pub const VALUE_SM: TextStyle = TextStyle { family: Family::Mono, size: 11, line: 15, weight: 400, tracking: 0 };
    /// Lib entry type badges and keycaps.
    pub const BADGE: TextStyle = TextStyle { family: Family::Mono, size: 10, line: 14, weight: 500, tracking: 0 };
    /// Splash and about box only.
    pub const DISPLAY: TextStyle = TextStyle { family: Family::Display, size: 64, line: 60, weight: 600, tracking: 0 };
}
