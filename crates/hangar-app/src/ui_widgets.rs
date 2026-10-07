//! Design-system components (tore-hangar-design/components/*): drawing plus
//! hit regions on the shared `Layout`, and the NumberField interaction state.
//! Every hit region equals the rect the component draws. Corners use a 1px
//! notch (the ground shows through the corner pixel), the GDI-safe stand-in
//! for `radius-sm`.
use super::view::{Action, Icon, Layout};
use super::*;
use theme::{metric as m, space};

/// Fill `rect` with notched corners and an optional 1px edge.
pub(super) fn notched(d: &mut Canvas, rect: [i32; 4], fill: Option<Rgb>, edge: Option<Rgb>) {
    let [x, y, w, h] = rect;
    if w < 3 || h < 3 {
        if let Some(f) = fill.or(edge) {
            d.rect(x, y, w, h, f);
        }
        return;
    }
    match (fill, edge) {
        (Some(f), Some(e)) => {
            d.rect(x + 1, y + 1, w - 2, h - 2, f);
            frame(d, rect, e);
        }
        (Some(f), None) => {
            d.rect(x + 1, y, w - 2, h, f);
            d.rect(x, y + 1, 1, h - 2, f);
            d.rect(x + w - 1, y + 1, 1, h - 2, f);
        }
        (None, Some(e)) => frame(d, rect, e),
        (None, None) => {}
    }
}
/// A notched 1px outline drawn with fills, exact on every backend.
pub(super) fn frame(d: &mut Canvas, [x, y, w, h]: [i32; 4], color: Rgb) {
    d.rect(x + 1, y, w - 2, 1, color);
    d.rect(x + 1, y + h - 1, w - 2, 1, color);
    d.rect(x, y + 1, 1, h - 2, color);
    d.rect(x + w - 1, y + 1, 1, h - 2, color);
}
/// The focus ring: 1px `focus`, 1px outside the control.
pub(super) fn focus_ring(d: &mut Canvas, [x, y, w, h]: [i32; 4]) {
    frame(d, [x - 2, y - 2, w + 4, h + 4], c::FOCUS);
}
/// Round 6px dot (dirty marker) with its top-left at (x, y).
pub(super) fn dot(d: &mut Canvas, x: i32, y: i32, color: Rgb) {
    let s = m::DIRTY_DOT;
    d.rect(x + 1, y, s - 2, s, color);
    d.rect(x, y + 1, s, s - 2, color);
}
/// Baseline that vertically centers one line of `style` in a box.
pub(super) fn baseline(y: i32, h: i32, style: Style) -> i32 {
    y + (h + style.spec().size * 3 / 4) / 2
}
/// Integer with thousands commas (when `group`); `decimals` digits are
/// fractional.
pub(super) fn format_number(value: i64, decimals: u8, group: bool) -> String {
    let scale = 10i64.pow(decimals as u32);
    let whole = (value / scale).unsigned_abs();
    let digits = format!("{whole}");
    let mut out = String::new();
    if value < 0 {
        out.push('-');
    }
    for (i, ch) in digits.chars().enumerate() {
        if group && i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if decimals > 0 {
        out.push_str(&format!(
            ".{:0width$}",
            (value % scale).unsigned_abs(),
            width = decimals as usize
        ));
    }
    out
}
/// Whether a field's values are quantities (thousands grouped) rather than
/// years, IDs, flags, types, classes or sizes, which read as codes.
pub(super) fn grouped(label: &str) -> bool {
    let name = label
        .rsplit('.')
        .next()
        .unwrap_or(label)
        .split('[')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    !(name.ends_with("id")
        || name.ends_with("ids")
        || ["year", "flag", "type", "class", "size", "code"]
            .iter()
            .any(|w| name.contains(w)))
}
/// Parse typed text ("20,900", "-4.25", "$1F") into the fixed-point value.
pub(super) fn parse_number(text: &str, decimals: u8) -> Result<i64> {
    let t: String = text.chars().filter(|c| *c != ',' && *c != ' ').collect();
    if let Some(hex) = t.strip_prefix('$') {
        return i64::from_str_radix(hex, 16).map_err(|_| "Invalid hex value".into());
    }
    let (negative, t) = t
        .strip_prefix('-')
        .map_or((false, t.as_str()), |r| (true, r));
    let (whole, frac) = t.split_once('.').unwrap_or((t, ""));
    if whole.is_empty() && frac.is_empty()
        || !whole
            .chars()
            .chain(frac.chars())
            .all(|c| c.is_ascii_digit())
        || frac.len() > decimals as usize
    {
        return Err(format!(
            "Enter a number with up to {decimals} decimal places"
        ));
    }
    let mut v: i64 = 0;
    for ch in whole.chars().chain(frac.chars()) {
        v = v
            .checked_mul(10)
            .and_then(|v| v.checked_add(ch as i64 - '0' as i64))
            .ok_or("Number too large")?;
    }
    v = v
        .checked_mul(10i64.pow((decimals as usize - frac.len()) as u32))
        .ok_or("Number too large")?;
    Ok(if negative { -v } else { v })
}

// ---------------------------------------------------------------- buttons

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    Default,
    /// The one action a dialog or panel exists for: amber fill.
    Primary,
    /// Editor header menus and Cancel: no fill until hovered.
    Ghost,
    /// Destructive: danger label, always with the close or warning icon.
    Danger,
}
/// A button description. Build with `Btn::new("Apply graft").primary()` or
/// `Btn::icon(Icon::Play)`, then draw with `Layout::button_ex`.
#[derive(Clone, Copy)]
pub(super) struct Btn<'a> {
    pub label: &'a str,
    pub icon: Option<Icon>,
    pub kind: Kind,
    /// Toggled on: `amber-deep` fill, `amber` label.
    pub on: bool,
    pub enabled: bool,
}
impl<'a> Btn<'a> {
    pub const fn new(label: &'a str) -> Self {
        Btn {
            label,
            icon: None,
            kind: Kind::Default,
            on: false,
            enabled: true,
        }
    }
    /// Icon-only button (22 x 22).
    pub const fn icon(g: Icon) -> Self {
        Btn {
            label: "",
            icon: Some(g),
            kind: Kind::Default,
            on: false,
            enabled: true,
        }
    }
    pub const fn with_icon(mut self, g: Icon) -> Self {
        self.icon = Some(g);
        self
    }
    pub const fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }
    pub const fn ghost(mut self) -> Self {
        self.kind = Kind::Ghost;
        self
    }
    pub const fn danger(mut self) -> Self {
        self.kind = Kind::Danger;
        self
    }
    pub const fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    fn style(&self) -> Style {
        if self.kind == Kind::Primary {
            Style::Strong
        } else {
            Style::Label
        }
    }
    /// Natural width: 10px padding, icon and 4px gap, label.
    pub fn width(&self) -> i32 {
        if self.label.is_empty() {
            return m::ICON_BUTTON;
        }
        20 + self.icon.map_or(0, |_| m::ICON + space::SPACE_1)
            + text_width(self.label, self.style())
    }
}
struct Look {
    fill: Option<Rgb>,
    edge: Option<Rgb>,
    lip: bool,
    ink: Rgb,
    glyph: Rgb,
}
fn look(b: &Btn, hover: bool, pressed: bool) -> Look {
    let ghost = b.kind == Kind::Ghost;
    let edge = (!ghost).then_some(c::GM_1000);
    if !b.enabled {
        return Look {
            fill: (!ghost).then_some(c::GM_800),
            edge,
            lip: false,
            ink: c::INK_FAINT,
            glyph: c::INK_FAINT,
        };
    }
    if b.on {
        return Look {
            fill: Some(c::AMBER_DEEP),
            edge,
            lip: false,
            ink: c::AMBER,
            glyph: c::AMBER,
        };
    }
    if pressed {
        let ink = match b.kind {
            Kind::Primary => c::AMBER,
            Kind::Danger => c::DANGER,
            _ => c::INK,
        };
        return Look {
            fill: Some(c::GM_950),
            edge: Some(c::GM_1000),
            lip: false,
            ink,
            glyph: ink,
        };
    }
    match b.kind {
        Kind::Primary => Look {
            fill: Some(if hover { c::AMBER_BRIGHT } else { c::AMBER }),
            edge,
            lip: false,
            ink: c::ON_AMBER,
            glyph: c::ON_AMBER,
        },
        Kind::Ghost => Look {
            fill: hover.then_some(c::GM_700),
            edge: None,
            lip: false,
            ink: if hover { c::INK } else { c::INK_MUTED },
            glyph: if hover { c::INK } else { c::INK_MUTED },
        },
        Kind::Default | Kind::Danger => {
            let ink = if b.kind == Kind::Danger {
                c::DANGER
            } else {
                c::INK
            };
            Look {
                fill: Some(if hover { c::GM_600 } else { c::GM_700 }),
                edge,
                lip: true,
                ink,
                glyph: ink,
            }
        }
    }
}
/// Paint a button face with `look`; `ground` is the surface behind it.
fn face(d: &mut Canvas, rect: [i32; 4], b: &Btn, l: &Look, ground: Rgb) {
    let [x, y, w, h] = rect;
    notched(d, rect, l.fill, l.edge);
    if l.lip && w > 4 {
        d.rect(x + 1, y + 1, w - 2, 1, c::LIP);
    }
    let bg = l.fill.unwrap_or(ground);
    let style = b.style();
    let label_w = if b.label.is_empty() {
        0
    } else {
        text_width(b.label, style)
    };
    let icon_w = b.icon.map_or(0, |_| m::ICON);
    let gap = if b.icon.is_some() && label_w > 0 {
        space::SPACE_1
    } else {
        0
    };
    let room = w - 8 - icon_w - gap;
    let label = if label_w > room {
        fit(b.label, room, style)
    } else {
        b.label.into()
    };
    let total = icon_w + gap + text_width(&label, style);
    let mut cx = x + (w - total) / 2;
    if let Some(g) = b.icon {
        d.icon(cx, y + (h - m::ICON) / 2, g, l.glyph, bg);
        cx += icon_w + gap;
    }
    if !label.is_empty() {
        d.styled(cx, baseline(y, h, style), &label, l.ink, style);
    }
}

// ---------------------------------------------------------------- menus

/// One dropdown row: label, optional keycap shortcut and icon. `action: None`
/// draws a separator.
#[derive(Clone, Copy)]
pub(super) struct Item<'a> {
    pub label: &'a str,
    pub key: Option<&'a str>,
    pub icon: Option<Icon>,
    pub action: Option<Action>,
    /// The current choice (Select menus): amber label.
    pub on: bool,
    /// A warn badge after the label ("Not seen in retail").
    pub badge: Option<&'a str>,
    /// Disabled items draw in `ink-faint` and get no hit region.
    pub enabled: bool,
    /// Opens a submenu: a chevron instead of a keycap.
    pub sub: bool,
}
impl<'a> Item<'a> {
    pub const fn new(label: &'a str, action: Action) -> Self {
        Item {
            label,
            key: None,
            icon: None,
            action: Some(action),
            on: false,
            badge: None,
            enabled: true,
            sub: false,
        }
    }
    pub const fn sep() -> Self {
        Item {
            label: "",
            key: None,
            icon: None,
            action: None,
            on: false,
            badge: None,
            enabled: true,
            sub: false,
        }
    }
    pub const fn key(mut self, key: &'a str) -> Self {
        self.key = Some(key);
        self
    }
    pub const fn icon(mut self, g: Icon) -> Self {
        self.icon = Some(g);
        self
    }
    pub const fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }
    pub const fn badge(mut self, text: &'a str) -> Self {
        self.badge = Some(text);
        self
    }
    pub const fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub const fn sub(mut self) -> Self {
        self.sub = true;
        self
    }
}
pub(super) const MENU_SEP_H: i32 = 1 + 2 * space::SPACE_1;
/// Top of item `index` in a dropdown whose top edge is `y`.
pub(super) fn menu_item_y(items: &[Item], y: i32, index: usize) -> i32 {
    y + 1
        + space::SPACE_1
        + items[..index.min(items.len())]
            .iter()
            .map(|i| {
                if i.action.is_none() {
                    MENU_SEP_H
                } else {
                    m::MENU_ITEM_H
                }
            })
            .sum::<i32>()
}
/// (width, height) of a dropdown with these items.
pub(super) fn menu_size(items: &[Item]) -> (i32, i32) {
    let icons = items.iter().any(|i| i.icon.is_some());
    let mut w = m::MENU_MIN_W;
    let mut h = 2 * space::SPACE_1 + 2;
    for i in items {
        if i.action.is_none() {
            h += MENU_SEP_H;
            continue;
        }
        h += m::MENU_ITEM_H;
        let key = i.key.map_or(0, |k| keycap_width(k) + space::SPACE_4)
            + i.badge.map_or(0, |b| badge_width(b) + space::SPACE_2)
            + if i.sub {
                m::ICON_SM + space::SPACE_4
            } else {
                0
            };
        let icon = if icons { m::ICON + space::SPACE_2 } else { 0 };
        w = w.max(2 * space::SPACE_3 + icon + text_width(i.label, Style::Label) + key + 2);
    }
    (w, h)
}

// ---------------------------------------------------------------- badges, keycaps, notices

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tone {
    /// Neutral type badge / info notice.
    Neutral,
    Ok,
    Warn,
    Danger,
}
pub(super) fn badge_width(text: &str) -> i32 {
    text_width(text, Style::Badge) + 2 * space::SPACE_1
}
/// TypeBadge: entry extension or a status word, 14px, top-left at (x, y).
/// Returns the width.
pub(super) fn type_badge(d: &mut Canvas, x: i32, y: i32, text: &str, tone: Tone) -> i32 {
    let w = badge_width(text);
    let (fill, ink) = match tone {
        Tone::Neutral => (c::GM_600, c::INK_MUTED),
        Tone::Ok => (c::GM_700, c::OK),
        Tone::Warn => (c::AMBER_DEEP, c::AMBER),
        Tone::Danger => (c::GM_700, c::DANGER),
    };
    notched(d, [x, y, w, m::BADGE_H], Some(fill), None);
    d.styled(
        x + space::SPACE_1,
        baseline(y, m::BADGE_H, Style::Badge),
        text,
        ink,
        Style::Badge,
    );
    w
}
pub(super) fn keycap_width(text: &str) -> i32 {
    (text_width(text, Style::Badge) + 2 * space::SPACE_1 + 2).max(m::KEYCAP_H)
}
/// Keycap (`th-kbd`) with its top-left at (x, y); returns the width.
pub(super) fn keycap(d: &mut Canvas, x: i32, y: i32, text: &str) -> i32 {
    let w = keycap_width(text);
    notched(d, [x, y, w, m::KEYCAP_H], Some(c::GM_700), Some(c::GM_1000));
    d.styled(
        x + (w - text_width(text, Style::Badge)) / 2,
        baseline(y, m::KEYCAP_H, Style::Badge),
        text,
        c::INK,
        Style::Badge,
    );
    w
}
/// Greedy word wrap of `text` into lines at most `width` px in `style`.
pub(super) fn wrap(text: &str, width: i32, style: Style) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let candidate = if line.is_empty() {
            word.into()
        } else {
            format!("{line} {word}")
        };
        if text_width(&candidate, style) <= width || line.is_empty() {
            line = candidate;
        } else {
            lines.push(core::mem::take(&mut line));
            line = word.into();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
        .into_iter()
        .map(|l| {
            if text_width(&l, style) > width {
                fit(&l, width, style)
            } else {
                l
            }
        })
        .collect()
}
const NOTICE_LINE: i32 = 18;
/// Height of a notice `w` px wide.
pub(super) fn notice_height(w: i32, text: &str) -> i32 {
    let lines = wrap(
        text,
        w - 2 * space::SPACE_2 - m::ICON - space::SPACE_2,
        Style::Body,
    )
    .len();
    lines.max(1) as i32 * NOTICE_LINE + 12
}
/// Inline Notice: icon plus a sentence. Returns the drawn height.
pub(super) fn notice(d: &mut Canvas, x: i32, y: i32, w: i32, tone: Tone, text: &str) -> i32 {
    let h = notice_height(w, text);
    let (edge, glyph, ink) = match tone {
        Tone::Neutral => (c::GM_600, Icon::Info, c::INK_MUTED),
        Tone::Ok => (c::GM_600, Icon::Check, c::OK),
        Tone::Warn => (c::AMBER, Icon::Warning, c::AMBER),
        Tone::Danger => (c::DANGER, Icon::Warning, c::DANGER),
    };
    notched(d, [x, y, w, h], Some(c::GM_900), Some(edge));
    d.icon(x + space::SPACE_2, y + 7, glyph, ink, c::GM_900);
    let tx = x + 2 * space::SPACE_2 + m::ICON;
    for (i, line) in wrap(text, x + w - space::SPACE_2 - tx, Style::Body)
        .iter()
        .enumerate()
    {
        d.styled(
            tx,
            y + 6 + i as i32 * NOTICE_LINE + 13,
            line,
            c::INK,
            Style::Body,
        );
    }
    h
}
/// Uppercase `section` sub-head inside a panel body; `y` is the row top.
pub(super) fn subhead(d: &mut Canvas, x: i32, y: i32, w: i32, text: &str) {
    let upper = text.to_ascii_uppercase();
    d.styled(
        x,
        baseline(y, m::ROW_H, Style::Section),
        &fit(&upper, w, Style::Section),
        c::INK_MUTED,
        Style::Section,
    );
}
/// Height of a panel with `rows` property rows and `subheads` sub-heads.
pub(super) fn panel_height(rows: i32, subheads: i32, collapsed: bool) -> i32 {
    if collapsed {
        return m::PANEL_HEADER_H;
    }
    m::PANEL_HEADER_H
        + 2
        + rows * m::ROW_H
        + (rows - 1).max(0) * space::SPACE_1
        + subheads * (m::ROW_H + space::SPACE_1)
        + space::SPACE_2
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Check {
    Off,
    On,
    Mixed,
}

// ---------------------------------------------------------------- number fields

/// What a NumberField edits. Each variant maps to a value, its on-disk value
/// and a commit path in `App::number_spec` / `App::number_commit`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum NumberTarget {
    /// An integer BRF field of the selected entry, by field index. Envelope
    /// table cells are BRF fields too.
    Field(usize),
    /// A numeric field of the selected hardpoint station: column 0..=11 of
    /// `hardpoints::Station::fields`; 1..=3 are the X, Y, Z position.
    Station(usize),
    /// Decal placement setting (`App::decal_setting` key 0..=4): centre X,
    /// centre Y, width, rotation, opacity. Draft-only, no on-disk value.
    Decal(u8),
    /// Parts pose preview variable (index into `App::pose_vars`). Preview
    /// only: never saved, never in undo.
    Pose(u8),
    /// Pivot component (right, forward, up) of the selected part.
    Pivot(u8),
    /// Replace tool and dialog tolerance, in 6-bit palette steps. A tool
    /// setting: never saved, never in undo.
    Tolerance,
}
/// A number in stored units, fixed-point with `decimals` fractional digits.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct NumberSpec {
    pub value: i64,
    /// The value in the file on disk; `None` for values with no saved copy.
    pub disk: Option<i64>,
    /// Arrow and coarse-drag increment.
    pub step: i64,
    pub min: i64,
    pub max: i64,
    pub decimals: u8,
    /// Show the `steel-deep` position bar between min and max.
    pub bounded: bool,
    /// Thousands commas: physical quantities yes, years, IDs and flags no.
    pub group: bool,
}
/// A NumberField to draw: label left (optional), value right with its unit.
#[derive(Clone, Copy)]
pub(super) struct Number<'a> {
    pub target: NumberTarget,
    pub spec: NumberSpec,
    pub label: &'a str,
    pub unit: &'a str,
    /// Derived from another entry: dashed border, muted value, lock icon.
    pub locked: bool,
    /// Vector component: label colored `axis-x/y/z`.
    pub axis: Option<usize>,
}
/// An in-progress scrub drag.
#[derive(Clone, Copy, Debug)]
pub(super) struct Scrub {
    pub target: NumberTarget,
    pub spec: NumberSpec,
    pub x: i32,
    pub value: i64,
    pub moved: bool,
}
/// Pixels of drag per increment: coarse, and with Shift (x0.1).
const SCRUB_PX: i64 = 2;
const SCRUB_FINE_PX: i64 = 20;

// ---------------------------------------------------------------- Layout components

impl Layout {
    /// True when the (uncovered) pointer is inside `rect`.
    pub(super) fn over(&self, [x, y, w, h]: [i32; 4]) -> bool {
        self.mouse[0] >= x && self.mouse[0] < x + w && self.mouse[1] >= y && self.mouse[1] < y + h
    }
    /// Button in any kind/state on `ground`; disabled buttons get no hit region.
    pub(super) fn button_on(&mut self, rect: [i32; 4], b: Btn, action: Action, ground: Rgb) {
        let hover = b.enabled && self.over(rect);
        let l = look(&b, hover, hover && self.pressed);
        face(&mut self.canvas, rect, &b, &l, ground);
        if b.enabled {
            self.hit(rect, action);
        }
    }
    /// Button on the default `gm-800` editor ground.
    pub(super) fn button_ex(&mut self, rect: [i32; 4], b: Btn, action: Action) {
        self.button_on(rect, b, action, c::GM_800);
    }
    /// 22 x 22 icon button with its top-left at (x, y).
    pub(super) fn icon_button(&mut self, x: i32, y: i32, b: Btn, action: Action) {
        let s = m::ICON_BUTTON;
        self.button_ex([x, y, s, s], b, action);
    }
    /// Joined buttons for mutually exclusive modes. Members share `rect`
    /// equally; each gets its own hit region.
    pub(super) fn segmented(&mut self, rect: [i32; 4], items: &[(Btn, Action)]) {
        let [x, y, w, h] = rect;
        // Members share the inner width in proportion to their natural widths.
        let natural: Vec<i32> = items.iter().map(|(b, _)| b.width().max(1)).collect();
        let total: i32 = natural.iter().sum::<i32>().max(1);
        let mut before = 0;
        notched(&mut self.canvas, rect, None, Some(c::GM_1000));
        for (i, (b, action)) in items.iter().enumerate() {
            let mx = x + 1 + (w - 2) * before / total;
            before += natural[i];
            let mw = x + 1 + (w - 2) * before / total - mx;
            let i = i as i32;
            let member = [mx, y + 1, mw, h - 2];
            let hover = b.enabled && self.over(member);
            let mut l = look(b, hover, hover && self.pressed);
            l.edge = None;
            let fill = l.fill.unwrap_or(c::GM_700);
            self.canvas.rect(mx, y + 1, mw, h - 2, fill);
            if l.lip {
                self.canvas.rect(mx, y + 1, mw, 1, c::LIP);
            }
            l.fill = Some(fill);
            l.lip = false;
            // Members are square inside the shared notched keyline.
            face_plain(&mut self.canvas, member, b, &l);
            if i > 0 {
                self.canvas.rect(mx, y + 1, 1, h - 2, c::GM_1000);
            }
            if b.enabled {
                self.hit([mx, y, mw, h], *action);
            }
        }
    }
    /// Natural width of a segmented control.
    pub(super) fn segmented_width(items: &[(Btn, Action)]) -> i32 {
        items.iter().map(|(b, _)| b.width()).sum::<i32>() + 2
    }
    /// Closed Select field: raised face, optional 12px leading icon, value,
    /// chevron. `open` shows it pressed while its menu is up.
    pub(super) fn select(
        &mut self,
        rect: [i32; 4],
        icon: Option<Icon>,
        value: &str,
        action: Action,
        open: bool,
    ) {
        let [x, y, w, h] = rect;
        let hover = self.over(rect);
        let fill = if open {
            c::GM_950
        } else if hover {
            c::GM_600
        } else {
            c::GM_700
        };
        let d = &mut self.canvas;
        notched(d, rect, Some(fill), Some(c::GM_1000));
        if !open {
            d.rect(x + 1, y + 1, w - 2, 1, c::LIP);
        }
        let mut tx = x + 6;
        if let Some(g) = icon {
            d.icon_sm(tx, y + (h - m::ICON_SM) / 2, g, c::INK_MUTED, fill);
            tx += m::ICON_SM + space::SPACE_1;
        }
        let room = x + w - 6 - m::ICON_SM - space::SPACE_1 - tx;
        d.styled(
            tx,
            baseline(y, h, Style::Label),
            &fit(value, room, Style::Label),
            c::INK,
            Style::Label,
        );
        d.icon_sm(
            x + w - 4 - m::ICON_SM,
            y + (h - m::ICON_SM) / 2,
            Icon::ChevronDown,
            c::INK_MUTED,
            fill,
        );
        self.hit(rect, action);
    }
    /// Natural width of a Select showing `value`.
    /// Width that shows `value` whole: the insets `select` draws with.
    pub(super) fn select_width(icon: bool, value: &str) -> i32 {
        6 + if icon { m::ICON_SM + space::SPACE_1 } else { 0 }
            + text_width(value, Style::Label)
            + space::SPACE_1
            + m::ICON_SM
            + 6
    }
    /// Dropdown menu at (x, y), clamped inside the window. The whole surface
    /// is hit-tested as `pad` so clicks on padding never fall through.
    /// Returns the drawn rect.
    pub(super) fn menu(&mut self, x: i32, y: i32, items: &[Item], pad: Action) -> [i32; 4] {
        let (w, h) = menu_size(items);
        let x = x.min(self.size[0] - w).max(0);
        let y = y.min(self.size[1] - h).max(0);
        let rect = [x, y, w, h];
        notched(&mut self.canvas, rect, Some(c::GM_800), Some(c::GM_1000));
        self.hit(rect, pad);
        let icons = items.iter().any(|i| i.icon.is_some());
        let mut iy = y + 1 + space::SPACE_1;
        for item in items {
            let Some(action) = item.action else {
                self.canvas
                    .rect(x + 1, iy + space::SPACE_1, w - 2, 1, c::GM_600);
                iy += MENU_SEP_H;
                continue;
            };
            let row = [x + 1, iy, w - 2, m::MENU_ITEM_H];
            let hot = item.enabled && (self.over(row) || (item.sub && item.on));
            let ground = if hot { c::AMBER_DEEP } else { c::GM_800 };
            if hot {
                self.canvas
                    .rect(row[0], row[1], row[2], row[3], c::AMBER_DEEP);
            }
            let mut tx = x + space::SPACE_3;
            if icons {
                if let Some(g) = item.icon {
                    self.canvas.icon(
                        tx,
                        iy + (m::MENU_ITEM_H - m::ICON) / 2,
                        g,
                        if item.on { c::AMBER } else { c::INK_MUTED },
                        ground,
                    );
                }
                tx += m::ICON + space::SPACE_2;
            }
            let key_w = item
                .key
                .map_or(if item.sub { m::ICON_SM } else { 0 }, keycap_width);
            let badge_w = item.badge.map_or(0, |b| badge_width(b) + space::SPACE_2);
            let room =
                x + w - space::SPACE_3 - key_w - badge_w - tx - if key_w > 0 { 8 } else { 0 };
            if let Some(b) = item.badge {
                type_badge(
                    &mut self.canvas,
                    x + w - space::SPACE_3 - key_w - badge_w + space::SPACE_2,
                    iy + (m::MENU_ITEM_H - m::BADGE_H) / 2,
                    b,
                    Tone::Warn,
                );
            }
            self.canvas.styled(
                tx,
                baseline(iy, m::MENU_ITEM_H, Style::Label),
                &fit(item.label, room, Style::Label),
                if !item.enabled {
                    c::INK_FAINT
                } else if item.on {
                    c::AMBER
                } else {
                    c::INK
                },
                Style::Label,
            );
            if let Some(k) = item.key {
                keycap(
                    &mut self.canvas,
                    x + w - space::SPACE_3 - key_w,
                    iy + (m::MENU_ITEM_H - m::KEYCAP_H) / 2,
                    k,
                );
            } else if item.sub {
                self.canvas.icon_sm(
                    x + w - space::SPACE_3 - key_w,
                    iy + (m::MENU_ITEM_H - m::ICON_SM) / 2,
                    Icon::ChevronRight,
                    if item.enabled {
                        c::INK_MUTED
                    } else {
                        c::INK_FAINT
                    },
                    ground,
                );
            }
            if item.enabled {
                self.hit(row, action);
            }
            iy += m::MENU_ITEM_H;
        }
        rect
    }
    /// Checkbox with its label; the box and the label both toggle. `y` is the
    /// top of a 20px row. Returns the hit rect.
    pub(super) fn checkbox(
        &mut self,
        x: i32,
        y: i32,
        label: &str,
        state: Check,
        action: Action,
        enabled: bool,
    ) -> [i32; 4] {
        let s = m::CHECKBOX;
        let w = s + 6 + text_width(label, Style::Label);
        let rect = [x, y, w, m::ROW_H];
        let by = y + (m::ROW_H - s) / 2;
        let hover = enabled && self.over(rect);
        let d = &mut self.canvas;
        match state {
            Check::On => {
                notched(d, [x, by, s, s], Some(c::AMBER), Some(c::AMBER));
                d.icon_sm(x + 1, by + 1, Icon::Check, c::ON_AMBER, c::AMBER);
            }
            Check::Off | Check::Mixed => {
                let fill = if hover { c::GM_1000 } else { c::GM_950 };
                notched(d, [x, by, s, s], Some(fill), Some(c::LINE_STRONG));
                if state == Check::Mixed {
                    d.rect(x + 3, by + 6, 8, 2, c::AMBER);
                }
            }
        }
        d.styled(
            x + s + 6,
            baseline(y, m::ROW_H, Style::Label),
            label,
            if enabled { c::INK } else { c::INK_FAINT },
            Style::Label,
        );
        if enabled {
            self.hit(rect, action);
        }
        rect
    }
    /// Checkbox row: box, optional icon, label and a right-aligned mono
    /// `detail` in `ink-muted`. The whole row toggles; hover fills `gm-700`.
    pub(super) fn checkbox_row(
        &mut self,
        rect: [i32; 4],
        icon: Option<Icon>,
        label: &str,
        detail: &str,
        state: Check,
        action: Action,
    ) {
        let [x, y, w, h] = rect;
        let hover = self.over(rect);
        let ground = if hover { c::GM_700 } else { c::GM_800 };
        if hover {
            notched(&mut self.canvas, rect, Some(c::GM_700), None);
        }
        let s = m::CHECKBOX;
        let by = y + (h - s) / 2;
        let d = &mut self.canvas;
        match state {
            Check::On => {
                notched(d, [x + 2, by, s, s], Some(c::AMBER), Some(c::AMBER));
                d.icon_sm(x + 3, by + 1, Icon::Check, c::ON_AMBER, c::AMBER);
            }
            Check::Off | Check::Mixed => {
                notched(d, [x + 2, by, s, s], Some(c::GM_950), Some(c::LINE_STRONG));
                if state == Check::Mixed {
                    d.rect(x + 5, by + 6, 8, 2, c::AMBER);
                }
            }
        }
        let mut tx = x + 2 + s + 6;
        if let Some(g) = icon {
            d.icon(tx, y + (h - m::ICON) / 2, g, c::INK_MUTED, ground);
            tx += m::ICON + space::SPACE_1;
        }
        let dw = text_width(detail, Style::ValueSm);
        let dx = x + w - 4 - dw;
        d.styled(
            dx,
            baseline(y, h, Style::ValueSm),
            detail,
            c::INK_MUTED,
            Style::ValueSm,
        );
        d.styled(
            tx,
            baseline(y, h, Style::Label),
            &fit(label, dx - space::SPACE_2 - tx, Style::Label),
            c::INK,
            Style::Label,
        );
        self.hit(rect, action);
    }
    /// Panel header plus surface. Draws the `gm-800` body (unless collapsed)
    /// and keyline over `rect`, the chevron, optional icon and title. Click
    /// the header to `toggle`. Returns the body rect for property rows.
    pub(super) fn panel(
        &mut self,
        rect: [i32; 4],
        title: &str,
        icon: Option<Icon>,
        collapsed: bool,
        toggle: Action,
    ) -> [i32; 4] {
        let [x, y, w, _] = rect;
        let h = if collapsed {
            m::PANEL_HEADER_H
        } else {
            rect[3]
        };
        let header = [x, y, w, m::PANEL_HEADER_H];
        let d = &mut self.canvas;
        notched(d, [x, y, w, h], Some(c::GM_800), Some(c::GM_1000));
        let mut tx = x + 6;
        d.icon_sm(
            tx,
            y + (m::PANEL_HEADER_H - m::ICON_SM) / 2,
            if collapsed {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            },
            c::INK_MUTED,
            c::GM_800,
        );
        tx += m::ICON_SM + space::SPACE_1;
        if let Some(g) = icon {
            d.icon(
                tx,
                y + (m::PANEL_HEADER_H - m::ICON) / 2,
                g,
                c::INK_MUTED,
                c::GM_800,
            );
            tx += m::ICON + space::SPACE_1;
        }
        d.styled(
            tx,
            baseline(y, m::PANEL_HEADER_H, Style::Strong),
            &fit(title, x + w - 8 - tx, Style::Strong),
            c::INK,
            Style::Strong,
        );
        self.hit(header, toggle);
        [
            x + space::SPACE_2,
            y + m::PANEL_HEADER_H + 2,
            w - 2 * space::SPACE_2,
            (h - m::PANEL_HEADER_H - 2 - space::SPACE_2).max(0),
        ]
    }
    /// Ghost icon tool in a panel header; `slot` 0 is the rightmost.
    pub(super) fn panel_tool(&mut self, panel: [i32; 4], slot: i32, g: Icon, action: Action) {
        let s = m::PANEL_HEADER_H - 4;
        let x = panel[0] + panel[2] - 4 - s - slot * (s + 2);
        self.button_ex([x, panel[1] + 2, s, s], Btn::icon(g).ghost(), action);
    }
    /// Property row: label right-aligned in the `PROP_LABEL_PCT` column,
    /// returns the control rect (the rest of the row, 20px tall).
    pub(super) fn prop_row(&mut self, rect: [i32; 4], label: &str) -> [i32; 4] {
        let [x, y, w, _] = rect;
        let col = w * m::PROP_LABEL_PCT / 100;
        let text = fit(label, col, Style::Label);
        self.canvas.styled(
            x + col - text_width(&text, Style::Label),
            baseline(y, m::ROW_H, Style::Label),
            &text,
            c::INK_MUTED,
            Style::Label,
        );
        let cx = x + col + space::SPACE_2;
        [cx, y, x + w - cx, m::ROW_H]
    }
    /// NumberField. Drag scrubs, the hover arrows step, double-click types,
    /// Backspace over it resets to the on-disk value (see `App::number_*`).
    pub(super) fn number(&mut self, rect: [i32; 4], n: &Number) {
        let [x, y, w, h] = rect;
        let value = match self.scrub {
            Some((t, v)) if t == n.target => v,
            _ => n.spec.value,
        };
        let hover = !n.locked && self.over(rect);
        let fill = if hover { c::GM_1000 } else { c::GM_950 };
        let d = &mut self.canvas;
        notched(d, rect, Some(fill), None);
        if n.spec.bounded && n.spec.max > n.spec.min && w > 2 {
            let span = (n.spec.max - n.spec.min) as i128;
            let bar = ((value.clamp(n.spec.min, n.spec.max) - n.spec.min) as i128 * (w - 2) as i128
                / span) as i32;
            d.rect(x + 1, y + 1, bar, h - 2, c::STEEL_DEEP);
        }
        if n.locked {
            let mut i = 1;
            while i < w - 1 {
                d.rect(x + i, y, 2.min(w - 1 - i), 1, c::LINE_STRONG);
                d.rect(x + i, y + h - 1, 2.min(w - 1 - i), 1, c::LINE_STRONG);
                i += 4;
            }
            let mut j = 1;
            while j < h - 1 {
                d.rect(x, y + j, 1, 2.min(h - 1 - j), c::LINE_STRONG);
                d.rect(x + w - 1, y + j, 1, 2.min(h - 1 - j), c::LINE_STRONG);
                j += 4;
            }
        } else {
            frame(d, rect, c::LINE_STRONG);
        }
        let arrow = m::NUM_ARROW_W;
        let mut left = x + 6;
        let mut right = x + w - 6;
        if hover {
            d.icon_sm(
                x + 1 + (arrow - m::ICON_SM) / 2,
                y + (h - m::ICON_SM) / 2,
                Icon::ChevronLeft,
                c::INK_MUTED,
                fill,
            );
            d.icon_sm(
                x + w - 1 - arrow + (arrow - m::ICON_SM) / 2,
                y + (h - m::ICON_SM) / 2,
                Icon::ChevronRight,
                c::INK_MUTED,
                fill,
            );
            left = x + 1 + arrow;
            right = x + w - 1 - arrow;
        }
        if n.locked {
            d.icon_sm(
                right - m::ICON_SM,
                y + (h - m::ICON_SM) / 2,
                Icon::Lock,
                c::INK_MUTED,
                fill,
            );
            right -= m::ICON_SM + space::SPACE_1;
        }
        let changed = n.spec.disk.is_some_and(|disk| disk != value);
        let text = format_number(value, n.spec.decimals, n.spec.group);
        let unit_w = if n.unit.is_empty() {
            0
        } else {
            text_width(n.unit, Style::Value) + 4
        };
        let value_w = text_width(&text, Style::Value);
        let vx = right - unit_w - value_w;
        let base = baseline(y, h, Style::Value);
        d.styled(
            vx,
            base,
            &text,
            if n.locked {
                c::INK_MUTED
            } else if changed {
                c::AMBER
            } else {
                c::INK
            },
            Style::Value,
        );
        if !n.unit.is_empty() {
            d.styled(right - unit_w + 4, base, n.unit, c::INK_MUTED, Style::Value);
        }
        if !n.label.is_empty() {
            let color = match n.axis {
                Some(0) => c::AXIS_X,
                Some(1) => c::AXIS_Y,
                Some(2) => c::AXIS_Z,
                _ => c::INK_MUTED,
            };
            d.styled(
                left,
                baseline(y, h, Style::Label),
                &fit(n.label, vx - 6 - left, Style::Label),
                color,
                Style::Label,
            );
        }
        if !n.locked {
            if hover {
                self.hit([x, y, arrow + 1, h], Action::NumberStep(n.target, -1));
                self.hit(
                    [x + w - arrow - 1, y, arrow + 1, h],
                    Action::NumberStep(n.target, 1),
                );
                self.hit(
                    [x + arrow + 1, y, w - 2 * arrow - 2, h],
                    Action::Number(n.target),
                );
            } else {
                self.hit(rect, Action::Number(n.target));
            }
        }
    }
}
// ---------------------------------------------------------------- scrolling panel stacks

/// Collapsible panel ids of the right editor: bits of `App::panels`.
/// Collapsed state is kept per panel, so each tab remembers its own.
pub(super) mod pane {
    pub const LIB_ENTRY: u8 = 0;
    pub const RELATIONS: u8 = 1;
    pub const ENTRY_ACTIONS: u8 = 2;
    pub const GEOMETRY: u8 = 3;
    pub const TEXTURES: u8 = 4;
    pub const ENVELOPE: u8 = 5;
    pub const PROPULSION: u8 = 6;
    pub const WEIGHTS: u8 = 7;
    pub const HANDLING: u8 = 8;
    pub const STRUCTURE: u8 = 9;
    pub const GROUPS: u8 = 10;
    pub const ISSUES: u8 = 11;
    pub const GRAFT: u8 = 12;
    pub const STATION: u8 = 13;
    pub const STATION_DATA: u8 = 14;
    pub const STATIONS: u8 = 15;
    pub const PAINT: u8 = 16;
    pub const PALETTE: u8 = 17;
    pub const ORIGINAL: u8 = 18;
    pub const MEDIA_EXPORT: u8 = 19;
    pub const PANEL: u8 = 20;
    pub const MATERIAL: u8 = 21;
    pub const MATERIAL_NOTES: u8 = 22;
    pub const ARTWORK: u8 = 23;
    pub const PLACEMENT: u8 = 24;
    pub const AUDIO: u8 = 25;
    pub const PREVIEW: u8 = 26;
    pub const DONOR: u8 = 27;
    pub const MESH_SELECTION: u8 = 28;
    pub const MESH_OPS: u8 = 29;
    pub const PARTS: u8 = 30;
    pub const POSE: u8 = 31;
    pub const PART_SETTINGS: u8 = 32;
    pub const IDENTITY: u8 = 33;
    pub const MESH_TEXTURE: u8 = 34;
    pub const MARKINGS: u8 = 35;
}

/// A panel between `panel_begin` and `panel_end`.
struct OpenPanel {
    /// Draw-list position the frame is inserted at.
    mark: usize,
    y: i32,
    title: String,
    icon: Option<Icon>,
    collapsed: bool,
    tools: i32,
}
/// Strip below a panel stack for its more-below chevron.
const CUE_H: i32 = 16;
/// Pitch of one property row: a 20px row and the 4px gap below it.
pub(super) const ROW_PITCH: i32 = m::ROW_H + space::SPACE_1;
/// A vertical flow of panels and rows inside a scrolling editor region.
/// Only rows that fit entirely inside `top..bottom` are drawn or hit-tested,
/// so nothing is clipped mid-control; panel frames are cut at the edges.
pub(super) struct Stack {
    pub x: i32,
    pub w: i32,
    pub top: i32,
    pub bottom: i32,
    /// Next row top in window coordinates (scroll already applied).
    pub y: i32,
    scroll: i32,
    /// Row column inside the open panel (x, w).
    inner: (i32, i32),
    open: Option<OpenPanel>,
}
impl Stack {
    /// `rect` is the editor area; content scrolls up by `scroll` px. The
    /// bottom `CUE_H` px are kept for the more-below cue.
    pub fn new(rect: [i32; 4], scroll: i32) -> Self {
        let [x, y, w, h] = rect;
        let (cx, cw) = (x + space::SPACE_1, w - 2 * space::SPACE_1 - 4);
        Stack {
            x: cx,
            w: cw,
            top: y,
            bottom: y + h - CUE_H,
            y: y + space::SPACE_1 - scroll.max(0),
            scroll: scroll.max(0),
            inner: (cx, cw),
            open: None,
        }
    }
    pub fn visible(&self, y: i32, h: i32) -> bool {
        y >= self.top && y + h <= self.bottom
    }
    /// Reserve a row `h` px tall in the current column; returns its rect when
    /// it is entirely visible.
    pub fn row(&mut self, h: i32) -> Option<[i32; 4]> {
        let rect = [self.inner.0, self.y, self.inner.1, h];
        self.y += h + space::SPACE_1;
        self.visible(rect[1], h).then_some(rect)
    }
    /// Reserve `h` px with no gap after it (swatch grids); `None` unless
    /// entirely visible.
    pub fn take(&mut self, h: i32) -> Option<[i32; 4]> {
        let rect = [self.inner.0, self.y, self.inner.1, h];
        self.y += h;
        self.visible(rect[1], h).then_some(rect)
    }
    /// Extra vertical space.
    pub fn gap(&mut self, h: i32) {
        self.y += h;
    }
    /// True while the open panel is collapsed (its rows are skipped).
    pub fn collapsed(&self) -> bool {
        self.open.as_ref().is_some_and(|p| p.collapsed)
    }
}
impl Layout {
    /// Start a collapsible panel in `s`. Its header toggles with `toggle`;
    /// `tools` header tool slots (rightmost first) are left out of the toggle
    /// region. Rows follow until `panel_end`.
    pub(super) fn panel_begin(
        &mut self,
        s: &mut Stack,
        title: &str,
        icon: Option<Icon>,
        collapsed: bool,
        toggle: Action,
        tools: i32,
    ) {
        let header = [
            s.x,
            s.y,
            s.w - tools * (m::PANEL_HEADER_H - 2),
            m::PANEL_HEADER_H,
        ];
        if s.visible(s.y, m::PANEL_HEADER_H) {
            self.hit(header, toggle);
        }
        s.open = Some(OpenPanel {
            mark: self.canvas.commands.len(),
            y: s.y,
            title: title.into(),
            icon,
            collapsed,
            tools,
        });
        s.y += m::PANEL_HEADER_H;
        if !collapsed {
            s.y += 2;
        }
        s.inner = (s.x + space::SPACE_2, s.w - 2 * space::SPACE_2);
    }
    /// Header tool of the open panel; `slot` 0 is the rightmost.
    pub(super) fn panel_header_tool(&mut self, s: &Stack, slot: i32, b: Btn, action: Action) {
        let Some(p) = &s.open else {
            return;
        };
        let size = m::PANEL_HEADER_H - 4;
        let x = s.x + s.w - 4 - size - slot * (size + 2);
        if s.visible(p.y + 2, size) {
            self.button_ex([x, p.y + 2, size, size], b, action);
        }
    }
    /// Close the open panel: its frame and header are drawn beneath the rows
    /// already emitted, cut to the visible region.
    pub(super) fn panel_end(&mut self, s: &mut Stack) {
        let Some(p) = s.open.take() else {
            return;
        };
        let (mark, y) = (p.mark, p.y);
        if !p.collapsed {
            s.y += space::SPACE_2 - space::SPACE_1;
        }
        let rect = [s.x, y, s.w, s.y - y];
        let hover = self.over([s.x, y, s.w, m::PANEL_HEADER_H]) && s.visible(y, m::PANEL_HEADER_H);
        let tail = self.canvas.commands.split_off(mark);
        panel_frame(&mut self.canvas, rect, (s.top, s.bottom), &p, hover);
        self.canvas.commands.extend(tail);
        s.y += space::SPACE_1;
        s.inner = (s.x, s.w);
    }
    /// Finish the stack: record the scroll range and draw the scroll cue (a
    /// thumb at the right edge, a chevron when more content is below).
    pub(super) fn stack_end(&mut self, s: Stack) {
        let content = s.y + s.scroll - s.top;
        let view = s.bottom - s.top;
        let max = (content - view).max(0);
        self.inspector_max = max;
        if max == 0 || view < 24 {
            return;
        }
        let scroll = s.scroll.min(max);
        let track = view - 4;
        let thumb = (track * view / content).max(16).min(track);
        let ty = s.top + 2 + (track - thumb) * scroll / max;
        let tx = s.x + s.w + 1;
        self.canvas.rect(tx, ty, 3, thumb, c::GM_600);
        if scroll < max {
            self.canvas.rect(s.x, s.bottom, s.w, 1, c::GM_1000);
            self.canvas.icon_sm(
                s.x + (s.w - m::ICON_SM) / 2,
                s.bottom + 2,
                Icon::ChevronDown,
                c::INK_MUTED,
                c::GM_800,
            );
        }
    }
    /// Property row in the stack: label right-aligned in the label column;
    /// returns the visible control rect.
    pub(super) fn prop(&mut self, s: &mut Stack, label: &str) -> Option<[i32; 4]> {
        if s.collapsed() {
            return None;
        }
        s.row(m::ROW_H).map(|r| self.prop_row(r, label))
    }
    /// Read-only value row: label and a mono value in `ink` (`unit` muted).
    pub(super) fn info(&mut self, s: &mut Stack, label: &str, value: &str, unit: &str) {
        if let Some([x, y, w, h]) = self.prop(s, label) {
            let unit_w = if unit.is_empty() {
                0
            } else {
                text_width(unit, Style::Value) + 4
            };
            let text = fit(value, w - unit_w - 6, Style::Value);
            let base = baseline(y, h, Style::Value);
            self.canvas.styled(x + 6, base, &text, c::INK, Style::Value);
            if !unit.is_empty() {
                self.canvas.styled(
                    x + 6 + text_width(&text, Style::Value) + 4,
                    base,
                    unit,
                    c::INK_MUTED,
                    Style::Value,
                );
            }
        }
    }
    /// Full-width row of the stack (buttons, notices, lists); `None` when
    /// collapsed or not entirely visible.
    pub(super) fn wide(&mut self, s: &mut Stack, h: i32) -> Option<[i32; 4]> {
        if s.collapsed() {
            return None;
        }
        s.row(h)
    }
    /// Sub-head inside the open panel.
    pub(super) fn stack_subhead(&mut self, s: &mut Stack, text: &str) {
        if let Some([x, y, w, _]) = self.wide(s, m::ROW_H) {
            subhead(&mut self.canvas, x, y, w, text);
        }
    }
    /// Notice inside the stack, wrapped to the column.
    pub(super) fn stack_notice(&mut self, s: &mut Stack, tone: Tone, text: &str) {
        if s.collapsed() {
            return;
        }
        let h = notice_height(s.inner.1, text);
        if let Some([x, y, w, _]) = s.row(h) {
            notice(&mut self.canvas, x, y, w, tone, text);
        }
    }
    /// Sunken text field for values a NumberField cannot scrub (strings,
    /// pointers, `$hex` operands): click opens the type prompt.
    pub(super) fn text_field(&mut self, rect: [i32; 4], text: &str, changed: bool, action: Action) {
        let [x, y, w, h] = rect;
        let hover = self.over(rect);
        let fill = if hover { c::GM_1000 } else { c::GM_950 };
        notched(&mut self.canvas, rect, Some(fill), Some(c::LINE_STRONG));
        self.canvas.styled(
            x + 6,
            baseline(y, h, Style::Value),
            &fit(text, w - 12, Style::Value),
            if changed { c::AMBER } else { c::INK },
            Style::Value,
        );
        self.hit(rect, action);
    }
}
/// Panel surface, keyline, chevron, icon and title over `rect`, cut to the
/// visible `clip` rows. The header shows `gm-700` when hovered.
fn panel_frame(d: &mut Canvas, rect: [i32; 4], clip: (i32, i32), p: &OpenPanel, hover: bool) {
    let (title, icon, collapsed, tools) = (p.title.as_str(), p.icon, p.collapsed, p.tools);
    let [x, y, w, h] = rect;
    let (top, bottom) = clip;
    let y0 = y.max(top);
    let y1 = (y + h).min(bottom);
    if y1 <= y0 || w < 3 {
        return;
    }
    // Body and side keylines, with notched corners where the edge shows.
    let inner0 = y0.max(y + 1);
    let inner1 = y1.min(y + h - 1);
    d.rect(x + 1, y0, w - 2, y1 - y0, c::GM_800);
    if inner1 > inner0 {
        d.rect(x, inner0, 1, inner1 - inner0, c::GM_1000);
        d.rect(x + w - 1, inner0, 1, inner1 - inner0, c::GM_1000);
    }
    if y >= top {
        d.rect(x + 1, y, w - 2, 1, c::GM_1000);
    }
    if y + h <= bottom {
        d.rect(x + 1, y + h - 1, w - 2, 1, c::GM_1000);
    }
    let hh = m::PANEL_HEADER_H;
    if y < top || y + hh > bottom {
        return;
    }
    let ground = if hover { c::GM_700 } else { c::GM_800 };
    if hover {
        d.rect(x + 1, y + 1, w - 2, hh - 2, c::GM_700);
    }
    let mut tx = x + 6;
    d.icon_sm(
        tx,
        y + (hh - m::ICON_SM) / 2,
        if collapsed {
            Icon::ChevronRight
        } else {
            Icon::ChevronDown
        },
        c::INK_MUTED,
        ground,
    );
    tx += m::ICON_SM + space::SPACE_1;
    if let Some(g) = icon {
        d.icon(tx, y + (hh - m::ICON) / 2, g, c::INK_MUTED, ground);
        tx += m::ICON + space::SPACE_1;
    }
    d.styled(
        tx,
        baseline(y, hh, Style::Strong),
        &fit(title, x + w - 8 - tools * (hh - 2) - tx, Style::Strong),
        c::INK,
        Style::Strong,
    );
}
/// Like `face` but for members of a segmented control (square corners).
fn face_plain(d: &mut Canvas, rect: [i32; 4], b: &Btn, l: &Look) {
    let plain = Look {
        fill: None,
        edge: None,
        lip: false,
        ink: l.ink,
        glyph: l.glyph,
    };
    face(d, rect, b, &plain, l.fill.unwrap_or(c::GM_700));
}

// ---------------------------------------------------------------- NumberField plumbing

impl App {
    /// Current value, on-disk value and limits for a NumberField target.
    pub(super) fn number_spec(&self, t: NumberTarget) -> Option<NumberSpec> {
        let integer = |f: &hangar_core::brf::Field, disk: Option<&hangar_core::brf::Field>| {
            let (min, max) = match f.kind.as_str() {
                "byte" => (-128, 255),
                "word" => (-32768, 65535),
                "dword" => (i32::MIN as i64, u32::MAX as i64),
                _ => return None,
            };
            // `$hex` operands keep their notation: they open the type prompt instead.
            let value = f.value.parse::<i64>().ok()?;
            Some(NumberSpec {
                value,
                disk: disk.and_then(|o| o.value.parse::<i64>().ok()),
                step: 1,
                min,
                max,
                decimals: 0,
                bounded: false,
                group: grouped(&f.label),
            })
        };
        match t {
            NumberTarget::Field(i) => {
                let f = self.brf.as_ref()?.fields.get(i)?;
                integer(f, self.original_brf.as_ref().and_then(|b| b.fields.get(i)))
            }
            NumberTarget::Station(column) => {
                let c = self.hp_context.as_ref()?;
                let s = c.stations.get(self.hp_selected)?;
                let f = c.brf.fields.get(*s.fields.get(column)?)?;
                let saved = c
                    .saved
                    .as_ref()
                    .and_then(|b| b.fields.iter().find(|old| old.label == f.label));
                if !(1..=3).contains(&column) {
                    return integer(f, saved);
                }
                // Positions are signed source coordinates, `$hex` words sign-extended.
                let signed = |f: &hangar_core::brf::Field| match f.value.strip_prefix('$') {
                    Some(h) => u32::from_str_radix(h, 16).ok().map(|n| {
                        if f.kind == "word" {
                            n as u16 as i16 as i64
                        } else {
                            n as i32 as i64
                        }
                    }),
                    None => f.value.parse::<i64>().ok(),
                };
                Some(NumberSpec {
                    value: self
                        .hp_drag
                        .as_ref()
                        .map_or(s.position[column - 1], |d| d.position[column - 1])
                        as i64,
                    disk: saved.and_then(signed),
                    step: 1,
                    min: -32768,
                    max: 32767,
                    decimals: 0,
                    bounded: false,
                    group: true,
                })
            }
            NumberTarget::Pose(var) => self.pose_spec(var),
            NumberTarget::Pivot(axis) => self.pivot_spec(axis),
            NumberTarget::Tolerance => Some(self.tolerance_spec()),
            NumberTarget::Decal(key) => {
                let p = self.decal_placement;
                let (value, min, max) = match key {
                    0 => (p.center[0], -8192, 8192),
                    1 => (p.center[1], -8192, 8192),
                    2 => (p.width as i32, 1, 2048),
                    3 => (p.degrees, 0, 359),
                    _ => (p.opacity as i32, 0, 100),
                };
                Some(NumberSpec {
                    value: value as i64,
                    disk: None,
                    step: 1,
                    min: min as i64,
                    max: max as i64,
                    decimals: 0,
                    bounded: key == 4,
                    group: false,
                })
            }
        }
    }
    /// Write `value` to the target as one undo step.
    pub(super) fn number_commit(&mut self, t: NumberTarget, value: i64) -> Result<()> {
        let spec = self.number_spec(t).ok_or("Value is not editable")?;
        if value < spec.min || value > spec.max {
            return Err(format!(
                "Value outside {}..{}",
                format_number(spec.min, spec.decimals, spec.group),
                format_number(spec.max, spec.decimals, spec.group)
            ));
        }
        if value == spec.value {
            return Ok(());
        }
        match t {
            NumberTarget::Field(i) => {
                let bytes = self.brf.as_ref().ok_or("No fields")?.edit(
                    &self.data,
                    i,
                    &format!("{value}"),
                    extension(self.name()),
                )?;
                self.doc.replace(self.selected, bytes)?;
                let scroll = self.field_scroll;
                self.refresh();
                self.field_scroll = scroll;
                self.field_selected = i;
                self.status = "Field changed. Ctrl+Z undoes it.".into();
            }
            NumberTarget::Station(column) => self.station_value(column, &format!("{value}"))?,
            NumberTarget::Decal(key) => self.decal_setting(key, &format!("{value}"))?,
            NumberTarget::Pose(var) => self.pose_commit(var, value),
            NumberTarget::Pivot(axis) => self.pivot_commit(axis, value)?,
            NumberTarget::Tolerance => self.tolerance_commit(value),
        }
        Ok(())
    }
    /// The exact saved operand text of a BRF-backed target, so a reset
    /// restores the file's bytes (including `$hex` notation), not a reformat.
    fn saved_text(&self, t: NumberTarget) -> Option<String> {
        match t {
            NumberTarget::Field(i) => self
                .original_brf
                .as_ref()
                .and_then(|b| b.fields.get(i))
                .map(|f| f.value.clone()),
            NumberTarget::Station(column) => {
                let c = self.hp_context.as_ref()?;
                let f = &c.brf.fields[*c.stations.get(self.hp_selected)?.fields.get(column)?];
                c.saved
                    .as_ref()?
                    .fields
                    .iter()
                    .find(|old| old.label == f.label)
                    .map(|old| old.value.clone())
            }
            NumberTarget::Pivot(axis) => self.pivot_spec(axis)?.disk.map(|v| v.to_string()),
            NumberTarget::Decal(_) | NumberTarget::Pose(_) | NumberTarget::Tolerance => None,
        }
    }
    /// Write the saved operand text back to a BRF-backed target, one undo step.
    fn restore_saved(&mut self, t: NumberTarget, text: &str) -> Result<()> {
        match t {
            NumberTarget::Field(i) => {
                let bytes = self.brf.as_ref().ok_or("No fields")?.edit(
                    &self.data,
                    i,
                    text,
                    extension(self.name()),
                )?;
                if bytes != self.data {
                    self.doc.replace(self.selected, bytes)?;
                }
                let scroll = self.field_scroll;
                self.refresh();
                self.field_scroll = scroll;
                self.field_selected = i;
            }
            NumberTarget::Station(column) => {
                let c = self.hp_context.as_ref().ok_or("No station owner")?;
                let entry = c.entry;
                let field = *c
                    .stations
                    .get(self.hp_selected)
                    .and_then(|s| s.fields.get(column))
                    .ok_or("No station")?;
                let old = self.doc.archive.entries[entry].read()?;
                let bytes = c.brf.edit(
                    &old,
                    field,
                    text,
                    extension(&self.doc.archive.entries[entry].name),
                )?;
                if bytes != old {
                    self.doc.replace(entry, bytes)?;
                }
                self.refresh();
            }
            NumberTarget::Pivot(axis) => {
                let v = text.parse::<i64>().map_err(|_| "Saved pivot")?;
                self.pivot_commit(axis, v)?;
            }
            NumberTarget::Decal(_) | NumberTarget::Pose(_) | NumberTarget::Tolerance => {
                return Err("No saved value to reset to".into())
            }
        }
        Ok(())
    }
    /// Left press on a NumberField starts a scrub.
    pub(super) fn number_press(&mut self, t: NumberTarget, x: i32) {
        if let Some(spec) = self.number_spec(t) {
            self.scrub = Some(Scrub {
                target: t,
                spec,
                x,
                value: spec.value,
                moved: false,
            });
        }
    }
    /// Drag: coarse one step per 2px, Shift fine (x0.1), Ctrl snaps to ten steps.
    pub(super) fn number_motion(&mut self, x: i32, shift: bool) {
        let ctrl = self.ctrl;
        let Some(s) = self.scrub.as_mut() else {
            return;
        };
        let dx = (x - s.x) as i64;
        if dx.abs() > 2 {
            s.moved = true;
        }
        if !s.moved {
            return;
        }
        let px = if shift { SCRUB_FINE_PX } else { SCRUB_PX };
        let mut v = s.spec.value.saturating_add(dx / px * s.spec.step);
        if ctrl {
            let snap = s.spec.step * 10;
            v = (v + snap / 2).div_euclid(snap) * snap;
        }
        s.value = v.clamp(s.spec.min, s.spec.max);
    }
    /// Release commits a moved scrub as one undo step; a click without a drag
    /// types an exact value, as in Blender.
    pub(super) fn number_release(&mut self) {
        if let Some(s) = self.scrub.take() {
            if s.moved {
                let result = self.number_commit(s.target, s.value);
                self.result(result);
            } else {
                self.number_prompt(s.target);
            }
        }
    }
    pub(super) fn number_cancel(&mut self) {
        if self.scrub.take().is_some() {
            self.status = "Scrub cancelled".into();
        }
    }
    pub(super) fn number_step(&mut self, t: NumberTarget, direction: i32) {
        if let Some(spec) = self.number_spec(t) {
            let v = (spec.value + spec.step * direction as i64).clamp(spec.min, spec.max);
            let result = self.number_commit(t, v);
            self.result(result);
        }
    }
    /// Backspace: back to the value in the file on disk.
    pub(super) fn number_reset(&mut self, t: NumberTarget) {
        if let NumberTarget::Pose(var) = t {
            // A pose input resets to "not set" (the neutral preview).
            self.pose_clear(var);
            return;
        }
        match self.saved_text(t) {
            Some(text) => {
                self.status.clear();
                let result = self.restore_saved(t, &text);
                self.result(result);
                if result_ok(&self.status) {
                    self.status = "Value reset to the file on disk. Ctrl+Z undoes it.".into();
                }
            }
            None => self.status = "No saved value to reset to".into(),
        }
    }
    /// Double-click: type an exact value in the prompt.
    pub(super) fn number_prompt(&mut self, t: NumberTarget) {
        if let Some(spec) = self.number_spec(t) {
            self.scrub = None;
            self.prompt = Some(Prompt {
                kind: PromptKind::Number(t),
                title: match t {
                    NumberTarget::Field(i) => self
                        .brf
                        .as_ref()
                        .and_then(|b| b.fields.get(i))
                        .map_or("Value".into(), |f| f.label.clone()),
                    NumberTarget::Station(_) => format!("HP{} value", self.hp_selected + 1),
                    NumberTarget::Decal(_) => "Decal placement".into(),
                    NumberTarget::Pose(_) => "Pose preview value (never saved)".into(),
                    NumberTarget::Pivot(_) => "Part pivot in source units".into(),
                    NumberTarget::Tolerance => {
                        "Replace tolerance: RGB distance in 6-bit palette steps (0..64)".into()
                    }
                },
                value: format_number(spec.value, spec.decimals, false),
                axis: 0,
            });
        }
    }
    pub(super) fn number_typed(&mut self, t: NumberTarget, text: &str) -> Result<()> {
        let spec = self.number_spec(t).ok_or("Value is not editable")?;
        let v = parse_number(text, spec.decimals)?;
        self.number_commit(t, v)
    }
    /// The NumberField under the pointer, if any.
    pub(super) fn number_under_mouse(&self) -> Option<NumberTarget> {
        let [x, y] = self.mouse;
        self.layout()
            .hits
            .into_iter()
            .rev()
            .find(|h| h.contains(x, y))
            .and_then(|h| match h.action {
                Action::Number(t) | Action::NumberStep(t, _) => Some(t),
                _ => None,
            })
    }
    /// Backends report a double-click of the left button after its press.
    pub fn double_click(&mut self, x: i32, y: i32) {
        self.mouse = [x, y];
        if self.prompt.is_some() {
            return;
        }
        if let Some(t) = self.layout().hits.into_iter().rev().find_map(|h| {
            if h.contains(x, y) {
                if let Action::Number(t) = h.action {
                    return Some(t);
                }
            }
            None
        }) {
            self.number_prompt(t);
        }
    }
    /// Backends report Ctrl before pointer and motion events (scrub snapping).
    pub fn modifiers(&mut self, ctrl: bool) {
        self.ctrl = ctrl;
    }
}
fn result_ok(status: &str) -> bool {
    !status.starts_with("Error:")
}

// ---------------------------------------------------------------- smoke test

impl App {
    /// Components draw inside their hit regions, and the NumberField
    /// plumbing scrubs, snaps, steps, resets, types and undoes through the
    /// real pointer paths.
    pub fn smoke_widgets(&mut self) {
        self.demo();
        self.width = 1280;
        self.height = 800;
        self.select_entry(1);
        let weight = self
            .brf
            .as_ref()
            .unwrap()
            .fields
            .iter()
            .position(|f| f.label == "object.weight")
            .unwrap();
        let t = NumberTarget::Field(weight);
        let spec = self.number_spec(t).expect("Integer field");
        assert_eq!(spec.disk, Some(spec.value));
        // Drawn components keep every hit region inside what they draw.
        let mut o = self.layout();
        let start = o.hits.len();
        let before = o.canvas.commands.len();
        o.mouse = [12, 12];
        o.button_ex(
            [10, 10, 90, 22],
            Btn::new("Apply graft").primary(),
            Action::Apply,
        );
        o.icon_button(110, 10, Btn::icon(Icon::Play), Action::Apply);
        o.segmented(
            [140, 10, 66, 22],
            &[
                (Btn::icon(Icon::Wire).on(true), Action::Apply),
                (Btn::icon(Icon::Solid), Action::Apply),
                (Btn::icon(Icon::Textured), Action::Apply),
            ],
        );
        o.select(
            [220, 10, 116, 20],
            Some(Icon::Select),
            "Object Mode",
            Action::Apply,
            false,
        );
        o.checkbox(350, 10, "Show on AB", Check::On, Action::Apply, true);
        o.number(
            [10, 40, 200, 20],
            &Number {
                target: t,
                spec,
                label: "Weight",
                unit: "lb",
                locked: false,
                axis: None,
            },
        );
        let body = o.panel(
            [10, 70, 300, panel_height(2, 0, false)],
            "Weights",
            None,
            false,
            Action::Apply,
        );
        let row = o.prop_row([body[0], body[1], body[2], 20], "Empty");
        assert!(row[0] > body[0] && row[0] + row[2] == body[0] + body[2]);
        let menu = o.menu(
            400,
            40,
            &[
                Item::new("Open LIB", Action::Apply).key("Ctrl+O"),
                Item::sep(),
                Item::new("Close", Action::Cancel),
            ],
            Action::MenuPad,
        );
        assert_eq!(menu[2], m::MENU_MIN_W);
        let mut drawn = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for d in &o.canvas.commands[before..] {
            let r = match d {
                Draw::Rect(x, y, w, h, _) => [*x, *y, x + w, y + h],
                Draw::Icon(x, y, _, small, _, _) => {
                    let s = if *small { 12 } else { 16 };
                    [*x, *y, x + s, y + s]
                }
                _ => continue,
            };
            drawn = [
                drawn[0].min(r[0]),
                drawn[1].min(r[1]),
                drawn[2].max(r[2]),
                drawn[3].max(r[3]),
            ];
        }
        for h in &o.hits[start..] {
            assert!(
                h.rect[0] >= drawn[0]
                    && h.rect[1] >= drawn[1]
                    && h.rect[0] + h.rect[2] <= drawn[2]
                    && h.rect[1] + h.rect[3] <= drawn[3],
                "Component hit region outside its drawing"
            );
        }
        assert_eq!(format_number(20900, 0, true), "20,900");
        assert_eq!(format_number(1997, 0, false), "1997");
        assert_eq!(format_number(-4250, 3, true), "-4.250");
        assert!(!grouped("object.year") && !grouped("object.objId") && !grouped("flags"));
        assert!(!grouped("hardpoint[0].typeId") && grouped("object.weight"));
        assert_eq!(parse_number("20,900", 0), Ok(20900));
        assert_eq!(parse_number("-4.25", 3), Ok(-4250));
        assert!(parse_number("1.2345", 3).is_err());
        assert_eq!(
            type_badge(&mut o.canvas, 0, 0, "11K", Tone::Neutral),
            badge_width("11K")
        );
        assert!(
            notice(
                &mut o.canvas,
                0,
                0,
                200,
                Tone::Warn,
                "F14.SH is referenced by 3 aircraft. Edits apply to all of them."
            ) > 30
        );
        // Scrub: press, drag 20px right (+10), Shift (fine) and Ctrl (snap).
        let before = self.doc.archive.bytes().unwrap();
        self.number_press(t, 100);
        self.number_motion(101, false);
        assert!(!self.scrub.unwrap().moved, "Small jitter is a click");
        self.number_motion(120, false);
        assert_eq!(self.scrub.unwrap().value, spec.value + 10);
        self.number_motion(140, true);
        assert_eq!(self.scrub.unwrap().value, spec.value + 2);
        self.modifiers(true);
        self.number_motion(134, false);
        assert_eq!(self.scrub.unwrap().value % 10, 0);
        self.modifiers(false);
        self.key(Key::Escape, false, false);
        assert!(
            self.scrub.is_none() && !self.doc.dirty(),
            "Esc reverts a scrub"
        );
        self.number_press(t, 100);
        self.motion(120, 300, false);
        self.pointer(120, 300, 1, false, false);
        assert_eq!(self.number_spec(t).unwrap().value, spec.value + 10);
        assert_eq!(self.doc.changed_count(), 1);
        self.doc.undo();
        self.refresh();
        assert_eq!(self.doc.archive.bytes().unwrap(), before, "One undo step");
        // Arrows step; Backspace resets to the on-disk value.
        self.number_step(t, 1);
        assert_eq!(self.number_spec(t).unwrap().value, spec.value + 1);
        self.number_reset(t);
        assert_eq!(self.number_spec(t).unwrap().value, spec.value);
        // Double-click types an exact value through the prompt.
        self.number_prompt(t);
        self.key(Key::Char('a'), true, false);
        for ch in "12,345".chars() {
            self.key(Key::Char(ch), false, false);
        }
        self.key(Key::Enter, false, false);
        assert!(self.prompt.is_none(), "{}", self.status);
        assert_eq!(self.number_spec(t).unwrap().value, 12345);
        self.doc.undo();
        self.doc.undo();
        self.doc.undo();
        self.refresh();
        assert_eq!(self.doc.archive.bytes().unwrap(), before);
    }
}
