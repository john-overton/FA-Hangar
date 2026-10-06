//! TORE Hangar theme tokens, generated from tokens.json.
//! Colors are 0x00RRGGBB (GDI COLORREF wants 0x00BBGGRR: use `colorref()`).
//! No alpha by design: every surface is a solid fill.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u32);

impl Rgb {
    pub const fn r(self) -> u8 { (self.0 >> 16) as u8 }
    pub const fn g(self) -> u8 { (self.0 >> 8) as u8 }
    pub const fn b(self) -> u8 { self.0 as u8 }
    /// Win32 COLORREF (0x00BBGGRR) for GDI calls.
    pub const fn colorref(self) -> u32 { (self.b() as u32) << 16 | (self.g() as u32) << 8 | self.r() as u32 }
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

/// Fixed control metrics, in px at 1x.
pub mod metric {
    pub const MENUBAR_H: i32 = 26;
    pub const EDITOR_HEADER_H: i32 = 28;
    pub const STATUSBAR_H: i32 = 22;
    pub const BUTTON_H: i32 = 22;
    pub const FIELD_H: i32 = 20;
    pub const ROW_H: i32 = 20;
    pub const PANEL_HEADER_H: i32 = 24;
    pub const VTAB_STRIP_W: i32 = 30;
    pub const VTAB_SIZE: i32 = 24;
    pub const ICON: i32 = 16;
    pub const TREE_INDENT: i32 = 14;
    pub const PROP_LABEL_FRACTION: f32 = 0.40;
    pub const MIN_WINDOW: (i32, i32) = (800, 600);
}

/// Type styles: (size px, line height px, weight). Family: Ui or Mono.
#[derive(Clone, Copy, Debug)]
pub enum Family { Ui, Mono, Display }

#[derive(Clone, Copy, Debug)]
pub struct TextStyle { pub family: Family, pub size: i32, pub line: i32, pub weight: u16 }

pub mod text {
    use super::{Family, TextStyle};
    /// Dialog titles, the graft target name, workspace tab when active.
    pub const TITLE: TextStyle = TextStyle { family: Family::Ui, size: 14, line: 18, weight: 600 };
    /// Notices and longer help text.
    pub const BODY: TextStyle = TextStyle { family: Family::Ui, size: 13, line: 18, weight: 400 };
    /// Default UI text: buttons, property labels, tree rows, menus.
    pub const LABEL: TextStyle = TextStyle { family: Family::Ui, size: 12, line: 16, weight: 500 };
    /// Uppercase sub-headings inside a panel body and graft slot roles.
    pub const SECTION: TextStyle = TextStyle { family: Family::Ui, size: 11, line: 14, weight: 600 };
    /// Status bar, timeline labels.
    pub const HINT: TextStyle = TextStyle { family: Family::Ui, size: 11, line: 14, weight: 400 };
    /// Every editable number, filename, and hex offset.
    pub const VALUE: TextStyle = TextStyle { family: Family::Mono, size: 12, line: 16, weight: 400 };
    /// Viewport overlays, counts, diff summaries.
    pub const VALUE_SM: TextStyle = TextStyle { family: Family::Mono, size: 11, line: 15, weight: 400 };
    /// Lib entry type badges and keycaps.
    pub const BADGE: TextStyle = TextStyle { family: Family::Mono, size: 10, line: 14, weight: 500 };
    /// Splash and about box only.
    pub const DISPLAY: TextStyle = TextStyle { family: Family::Display, size: 64, line: 60, weight: 600 };
}
