//! Runtime markings panel: the E0 texture slots of the shown shape, each
//! with its faces, Shown/Hidden, Select faces, a slot Select (0 to 4) and
//! Make paintable / Restore runtime marking, optionally applied to the
//! damage family. Each action is one undo step through `shape_markings`;
//! a refusal keeps the reason core gives. The viewport outlines active
//! markings in steel (reference) and hidden ones dim and dashed.
use super::view::{Action, Icon, Layout};
use super::widgets::{pane, Btn, Check, Tone};
use super::*;
use core::cell::RefCell;
use hangar_core::shape_fill::{fill_preview, Fill, FillMode, FillSource};
use hangar_core::shape_markings::{self as mk, Marking};
use theme::{metric as m, space};

pub(super) const MK_TOGGLE: u8 = 0;
pub(super) const MK_SELECT: u8 = 1;
pub(super) const MK_PAINT: u8 = 2;
pub(super) const MK_RESTORE: u8 = 3;
pub(super) const MK_FAMILY: u8 = 4;
/// Fill Select items.
pub(super) const FILL_SURFACE: u8 = 0;
pub(super) const FILL_PANEL: u8 = 1;
pub(super) const FILL_PICK: u8 = 2;
/// The panel's closing note: who decides what.
const NOTE: &str = "The shape decides whether, where and which slot. The game picks the image at run time; Hide and Make paintable apply to every nation.";
/// Damage family suffixes, in order.
const FAMILY: [&str; 4] = ["_A", "_B", "_C", "_D"];

/// One hidden face's corners and stored normal in the shown pose.
type Outline = (Vec<[i32; 3]>, Option<[i32; 3]>);
/// The shown shape's slots, cached by entry storage and pose.
struct Cached {
    entry: Entry,
    pose: model::Pose,
    rows: Vec<Marking>,
    hidden: Vec<Outline>,
}
/// What a slot's Make paintable fills the panel area with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum FillChoice {
    /// The surface under the marking.
    #[default]
    Surface,
    /// The colour the marking faces store.
    Panel,
    /// A palette index.
    Pick(u8),
}
impl FillChoice {
    fn mode(self) -> FillMode {
        match self {
            FillChoice::Surface => FillMode::Surface,
            FillChoice::Panel => FillMode::Panel,
            FillChoice::Pick(i) => FillMode::Index(i),
        }
    }
    fn label(self) -> String {
        match self {
            FillChoice::Surface => "From the surface below".into(),
            FillChoice::Panel => "Panel colour".into(),
            FillChoice::Pick(i) => format!("Pick\u{2026} {i}"),
        }
    }
}
/// The fills each slot would get, cached by the shape and the textures
/// they were read from.
struct FillCache {
    shape: Entry,
    used: Vec<Entry>,
    /// Slot, its surface fill and its panel fill.
    slots: Vec<(u16, Fill, Fill)>,
}
/// Runtime markings panel state (inside the boxed `EditState`).
#[derive(Default)]
pub(super) struct State {
    cache: RefCell<Option<Cached>>,
    fills: RefCell<Option<FillCache>>,
    /// Fill choice per slot (default From the surface below).
    pub fill: Vec<(u16, FillChoice)>,
    /// The open Select is a Fill Select, not a slot Select.
    pub fill_menu: bool,
    /// Apply each action to the damage family too.
    pub family: bool,
    /// Open slot Select: the row's slot and the Select's rect.
    pub slot_menu: Option<(u16, [i32; 4])>,
    /// Package line, cached by the changed SH entries.
    package: RefCell<Option<(Vec<Entry>, String)>>,
}
/// "Wing marking right (slot 4)".
pub(super) fn slot_title(slot: u16) -> String {
    format!("{} (slot {slot})", mk::slot_name(slot))
}
/// "1 face drawn", "1 face hidden", "1 face on F5EVM4.PIC".
fn faces_text(r: &Marking) -> String {
    let mut parts = Vec::new();
    if !r.faces.is_empty() {
        parts.push(format!(
            "{} drawn",
            view::count(r.faces.len(), "face", "faces")
        ));
    }
    if !r.hidden.is_empty() {
        parts.push(format!(
            "{} hidden",
            view::count(r.hidden.len(), "face", "faces")
        ));
    }
    if let Some((_, t)) = r.painted.first() {
        parts.push(format!(
            "{} on {t}",
            view::count(r.painted.len(), "face", "faces")
        ));
    }
    if parts.is_empty() {
        parts.push("No textured face".into());
    }
    parts.join(", ")
}
impl App {
    /// The shape entry the panel edits: the Model workspace shape, else the
    /// Paint workspace's context model.
    fn marking_entry(&self) -> Option<usize> {
        self.model_entry
            .or(self.context_entry)
            .filter(|i| self.doc.archive.entries.get(*i).is_some())
    }
    fn marking_cache(&self) -> core::cell::Ref<'_, Option<Cached>> {
        let pose = self.shown_pose();
        let entry = self.marking_entry().map(|i| &self.doc.archive.entries[i]);
        let fresh = {
            let c = self.ed.markings.cache.borrow();
            match (c.as_ref(), entry) {
                (Some(c), Some(e)) => {
                    c.entry.name == e.name && c.entry.same_storage(e) && c.pose == pose
                }
                (None, None) => true,
                _ => false,
            }
        };
        if !fresh {
            let built = entry.filter(|e| e.name.ends_with(".SH")).map(|e| {
                let bytes = e.read().unwrap_or_default();
                let rows = mk::markings(&bytes).unwrap_or_default();
                let hidden: Vec<usize> = rows.iter().flat_map(|r| r.hidden.clone()).collect();
                let mut outlines = Vec::new();
                if !hidden.is_empty() {
                    // Hidden faces drawn as they would be if shown, in this pose.
                    if let Ok(m) = mk::show_faces(&bytes, &hidden)
                        .and_then(|b| model::Model::with_pose(&b, &pose))
                    {
                        for f in m.faces.iter().filter(|f| hidden.contains(&f.offset)) {
                            let p = f
                                .indices
                                .iter()
                                .filter_map(|i| m.vertices.get(*i).map(|v| v.point));
                            outlines.push((p.collect(), f.normal));
                        }
                    }
                }
                Cached {
                    entry: e.clone(),
                    pose: pose.clone(),
                    rows,
                    hidden: outlines,
                }
            });
            *self.ed.markings.cache.borrow_mut() = built;
        }
        self.ed.markings.cache.borrow()
    }
    /// The shown shape's runtime slots (empty when it selects none).
    pub(super) fn marking_rows(&self) -> Vec<Marking> {
        self.marking_cache()
            .as_ref()
            .map(|c| c.rows.clone())
            .unwrap_or_default()
    }
    /// Damage family members of the shown shape in the active LIB.
    fn marking_family(&self) -> Vec<usize> {
        let Some(i) = self.marking_entry() else {
            return Vec::new();
        };
        let name = &self.doc.archive.entries[i].name;
        let Some(stem) = name.strip_suffix(".SH") else {
            return Vec::new();
        };
        if FAMILY.iter().any(|s| stem.ends_with(s)) {
            return Vec::new();
        }
        FAMILY
            .iter()
            .filter_map(|s| self.doc.archive.find(&format!("{stem}{s}.SH")))
            .collect()
    }
    /// Whether the viewport shows the marking outlines: the panel is open
    /// in an inspector that shows it.
    fn markings_shown(&self) -> bool {
        !self.panel_closed(pane::MARKINGS)
            && match self.mode {
                Mode::Model => !self.hp_tool && !self.animation_tool,
                Mode::Media => self.model_for_paint().is_some(),
                _ => false,
            }
    }
    /// Steel outlines for drawn markings and dim dashed ones for hidden
    /// markings, onto a `render_model` raster.
    pub(super) fn markings_overlay(
        &self,
        pixels: &mut [u32],
        [w, h]: [usize; 2],
        m: &Model,
        view: &hangar_core::model::ViewFrame,
    ) {
        if !self.markings_shown() {
            return;
        }
        let cache = self.marking_cache();
        let Some(c) = cache.as_ref() else {
            return;
        };
        let mut faces: Vec<Outline> = Vec::new();
        for r in &c.rows {
            for o in &r.faces {
                if let Some(f) = m.faces.iter().find(|f| f.offset == *o) {
                    let p = f
                        .indices
                        .iter()
                        .filter_map(|i| m.vertices.get(*i).map(|v| v.point));
                    faces.push((p.collect(), f.normal));
                }
            }
        }
        let n = faces.len();
        for (k, (points, normal)) in faces.iter().chain(&c.hidden).enumerate() {
            if normal.is_some_and(|n| self.camera_point(n)[2] < 0) || points.len() < 2 {
                continue;
            }
            let (color, dashed) = if k < n {
                (c::STEEL.0, false)
            } else {
                (c::INK_FAINT.0, true)
            };
            let q: Vec<[i32; 2]> = points
                .iter()
                .map(|p| {
                    let r = view.raster16(*p, [w, h]);
                    [r[0].div_euclid(16), r[1].div_euclid(16)]
                })
                .collect();
            for i in 0..q.len() {
                line(pixels, [w, h], q[i], q[(i + 1) % q.len()], color, dashed);
            }
        }
    }
    /// The Runtime markings panel, when the shown shape selects runtime
    /// slots.
    pub(super) fn markings_pane(&self, o: &mut Layout, s: &mut widgets::Stack) {
        let rows = self.marking_rows();
        if rows.is_empty() {
            return;
        }
        if self.pane(o, s, pane::MARKINGS, "Runtime markings", Icon::Textured) {
            let family = self.marking_family();
            if !family.is_empty() {
                if let Some(rect) = o.wide(s, m::ROW_H) {
                    let names: Vec<&str> = family
                        .iter()
                        .map(|i| self.doc.archive.entries[*i].name.as_str())
                        .collect();
                    let detail = view::count(names.len(), "shape", "shapes");
                    o.checkbox_row(
                        rect,
                        None,
                        "Apply to damage family",
                        &detail,
                        if self.ed.markings.family {
                            Check::On
                        } else {
                            Check::Off
                        },
                        Action::Marking(0, MK_FAMILY),
                    );
                }
            }
            for r in &rows {
                let slot = r.slot;
                o.stack_subhead(s, &slot_title(slot));
                o.info(s, "Faces", &faces_text(r), "");
                if !r.contested.is_empty() {
                    o.info(
                        s,
                        "Not editable",
                        &format!(
                            "{} also drawn with another texture",
                            view::count(r.contested.len(), "face", "faces")
                        ),
                        "",
                    );
                }
                if let Some(rect) = o.wide(s, m::ROW_H) {
                    let state = match (r.faces.is_empty(), r.hidden.is_empty()) {
                        (false, true) => Check::On,
                        (true, false) => Check::Off,
                        (false, false) => Check::Mixed,
                        (true, true) => {
                            if r.painted.is_empty() {
                                Check::Off
                            } else {
                                Check::On
                            }
                        }
                    };
                    let detail = match state {
                        Check::On => "Shown",
                        Check::Off => "Hidden",
                        Check::Mixed => "Mixed",
                    };
                    o.checkbox_row(
                        rect,
                        None,
                        "Shown",
                        detail,
                        state,
                        Action::Marking(slot, MK_TOGGLE),
                    );
                }
                if let Some(rect) = o.prop(s, "Slot") {
                    o.select(
                        rect,
                        None,
                        &format!("{slot} \u{b7} {}", mk::slot_name(slot)),
                        Action::MarkingMenu(slot),
                        self.menu == Some(chrome::MENU_SLOT)
                            && !self.ed.markings.fill_menu
                            && self.ed.markings.slot_menu.is_some_and(|(n, _)| n == slot),
                    );
                }
                if r.painted.is_empty() && !r.faces.is_empty() && r.contested.is_empty() {
                    self.fill_rows(o, s, slot);
                }
                if let Some(rect) = o.wide(s, m::BUTTON_H) {
                    let half = (rect[2] - space::SPACE_1) / 2;
                    let select = Btn::new("Select faces")
                        .enabled(!r.faces.is_empty() || !r.painted.is_empty());
                    o.button_ex(
                        [rect[0], rect[1], half, rect[3]],
                        select,
                        Action::Marking(slot, MK_SELECT),
                    );
                    let (label, op, enabled) = if r.painted.is_empty() {
                        (
                            "Make paintable",
                            MK_PAINT,
                            !r.faces.is_empty() && r.contested.is_empty(),
                        )
                    } else {
                        ("Restore runtime marking", MK_RESTORE, true)
                    };
                    o.button_ex(
                        [rect[0] + half + space::SPACE_1, rect[1], half, rect[3]],
                        Btn::new(label).enabled(enabled),
                        Action::Marking(slot, op),
                    );
                }
            }
            o.stack_notice(s, Tone::Neutral, NOTE);
        }
        o.panel_end(s);
    }
    /// The Fill row of a slot that can be made paintable: the chosen fill's
    /// swatch and Select, its index and where it came from, and the palette
    /// grid while Pick is chosen.
    fn fill_rows(&self, o: &mut Layout, s: &mut widgets::Stack, slot: u16) {
        let choice = self.fill_choice(slot);
        let fill = self.fill_of(slot, choice);
        if let Some(rect) = o.prop(s, "Fill") {
            o.select(
                rect,
                None,
                &choice.label(),
                Action::MarkingFillMenu(slot),
                self.menu == Some(chrome::MENU_SLOT)
                    && self.ed.markings.fill_menu
                    && self.ed.markings.slot_menu.is_some_and(|(n, _)| n == slot),
            );
        }
        if let Some(f) = fill {
            let why = match f.source {
                FillSource::Surface(_) => "surface below",
                FillSource::Skin => "skin, no surface",
                FillSource::Stored => "stored",
                FillSource::Picked => "picked",
            };
            if let Some([x, y, w, h]) = o.prop(s, "Index") {
                let sw = h - 4;
                let p = self.base_palette[f.color as usize];
                o.canvas.rect(x + 6, y + 2, sw, sw, swatch(p));
                widgets::frame(&mut o.canvas, [x + 6, y + 2, sw, sw], c::LINE_STRONG);
                let tx = x + 6 + sw + space::SPACE_2;
                let text = f.color.to_string();
                let base = widgets::baseline(y, h, Style::Value);
                o.canvas.styled(tx, base, &text, c::INK, Style::Value);
                let ux = tx + text_width(&text, Style::Value) + 4;
                o.canvas.styled(
                    ux,
                    base,
                    &fit(why, x + w - ux, Style::Value),
                    c::INK_MUTED,
                    Style::Value,
                );
            }
        }
        if matches!(choice, FillChoice::Pick(_)) {
            let colors = *self.base_palette;
            let cell = ((s.w - 2 * space::SPACE_2) / 16).max(8);
            for row in 0..16 {
                if s.collapsed() {
                    break;
                }
                let Some([x, y, ..]) = s.take(cell) else {
                    continue;
                };
                for col in 0..16 {
                    let i = row * 16 + col;
                    let xx = x + col * cell;
                    o.canvas
                        .rect(xx, y, cell - 1, cell - 1, swatch(colors[i as usize]));
                    if choice == FillChoice::Pick(i as u8) {
                        widgets::frame(
                            &mut o.canvas,
                            [xx - 1, y - 1, cell + 1, cell + 1],
                            c::AMBER,
                        );
                    }
                    o.hit(
                        [xx, y, cell - 1, cell - 1],
                        Action::MarkingFillColor(slot, i as u8),
                    );
                }
            }
            s.gap(space::SPACE_1);
        }
    }
    /// The fill choice of `slot`.
    pub(super) fn fill_choice(&self, slot: u16) -> FillChoice {
        self.ed
            .markings
            .fill
            .iter()
            .find(|f| f.0 == slot)
            .map_or(FillChoice::Surface, |f| f.1)
    }
    fn set_fill_choice(&mut self, slot: u16, choice: FillChoice) {
        let fill = &mut self.ed.markings.fill;
        match fill.iter_mut().find(|f| f.0 == slot) {
            Some(f) => f.1 = choice,
            None => fill.push((slot, choice)),
        }
    }
    /// The textures of the open LIB, by name.
    fn texture_bytes(&self, name: &str) -> Option<Vec<u8>> {
        let a = &self.doc.archive;
        a.find(name).and_then(|i| a.entries[i].read().ok())
    }
    /// The fill `choice` gives `slot` of the shown shape.
    fn fill_of(&self, slot: u16, choice: FillChoice) -> Option<Fill> {
        if let FillChoice::Pick(color) = choice {
            return Some(Fill {
                color,
                source: FillSource::Picked,
            });
        }
        let entry = self
            .marking_entry()
            .map(|i| &self.doc.archive.entries[i])
            .filter(|e| e.name.ends_with(".SH"))?;
        let a = &self.doc.archive;
        let fresh = self.ed.markings.fills.borrow().as_ref().is_some_and(|c| {
            c.shape.name == entry.name
                && c.shape.same_storage(entry)
                && c.used.iter().all(|u| {
                    a.find(&u.name)
                        .is_some_and(|i| a.entries[i].same_storage(u))
                })
        });
        if !fresh {
            let bytes = entry.read().unwrap_or_default();
            let used = RefCell::new(Vec::new());
            let lookup = |n: &str| {
                let i = a.find(n)?;
                used.borrow_mut().push(a.entries[i].clone());
                a.entries[i].read().ok()
            };
            let slots = mk::markings(&bytes)
                .unwrap_or_default()
                .iter()
                .filter_map(|r| {
                    let of = |mode| fill_preview(&bytes, r.slot, mode, &lookup).ok();
                    Some((r.slot, of(FillMode::Surface)?, of(FillMode::Panel)?))
                })
                .collect();
            *self.ed.markings.fills.borrow_mut() = Some(FillCache {
                shape: entry.clone(),
                used: used.into_inner(),
                slots,
            });
        }
        let cache = self.ed.markings.fills.borrow();
        let (_, surface, panel) = cache.as_ref()?.slots.iter().find(|f| f.0 == slot)?;
        Some(if choice == FillChoice::Panel {
            *panel
        } else {
            *surface
        })
    }
    /// Open the Fill Select of row `slot`, anchored under it.
    pub(super) fn open_fill_menu(&mut self, slot: u16) {
        self.open_slot_menu(slot);
        self.ed.markings.fill_menu = true;
    }
    /// Fill Select item `kind` of row `slot`.
    pub(super) fn marking_fill(&mut self, slot: u16, kind: u8) {
        self.ed.markings.slot_menu = None;
        self.ed.markings.fill_menu = false;
        self.menu = None;
        let choice = match kind {
            FILL_PANEL => FillChoice::Panel,
            FILL_PICK => FillChoice::Pick(
                self.fill_of(slot, FillChoice::Surface)
                    .map_or(0, |f| f.color),
            ),
            _ => FillChoice::Surface,
        };
        self.set_fill_choice(slot, choice);
        self.status = format!(
            "Make paintable will fill {}: {}.",
            slot_title(slot),
            choice.label()
        );
    }
    /// Palette grid cell: pick fill index `i` for `slot`.
    pub(super) fn marking_fill_color(&mut self, slot: u16, i: u8) {
        self.set_fill_choice(slot, FillChoice::Pick(i));
        self.status = format!(
            "Make paintable will fill {} with index {i}.",
            slot_title(slot)
        );
    }
    /// Open the slot Select of row `slot`, anchored under it.
    pub(super) fn open_slot_menu(&mut self, slot: u16) {
        self.ed.markings.fill_menu = false;
        let rect = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::MarkingMenu(n) if n == slot))
            .map(|h| h.rect);
        if let Some(rect) = rect {
            self.ed.markings.slot_menu = Some((slot, rect));
            self.menu = Some(chrome::MENU_SLOT);
        }
    }
    /// Items of the open slot Select: slots 0 to 4.
    pub(super) fn slot_menu_items(&self) -> Vec<(String, Action, bool)> {
        let Some((from, _)) = self.ed.markings.slot_menu else {
            return Vec::new();
        };
        if self.ed.markings.fill_menu {
            let on = self.fill_choice(from);
            return [
                (
                    "From the surface below",
                    FILL_SURFACE,
                    on == FillChoice::Surface,
                ),
                ("Panel colour", FILL_PANEL, on == FillChoice::Panel),
                ("Pick\u{2026}", FILL_PICK, matches!(on, FillChoice::Pick(_))),
            ]
            .into_iter()
            .map(|(label, kind, on)| (label.to_string(), Action::MarkingFill(from, kind), on))
            .collect();
        }
        (0..mk::SLOT_COUNT)
            .map(|to| {
                (
                    format!("{to} \u{b7} {}", mk::slot_name(to)),
                    Action::MarkingSlot(from, to),
                    to == from,
                )
            })
            .collect()
    }
    /// The shapes an action edits: the shown one, then the family members
    /// when Apply to damage family is on.
    fn marking_targets(&self) -> Result<Vec<usize>> {
        let entry = self
            .marking_entry()
            .ok_or("Open the shape owner's LIB to edit runtime markings")?;
        let mut out = vec![entry];
        if self.ed.markings.family {
            out.extend(self.marking_family());
        }
        Ok(out)
    }
    /// A free 8.3 PIC name for a slot's paintable sheet: `<STEM>M<slot>`.
    fn paint_name(&self, slot: u16) -> String {
        let entry = self
            .marking_entry()
            .map(|i| self.doc.archive.entries[i].name.clone());
        let stem: String = entry
            .as_deref()
            .unwrap_or("SHAPE")
            .split('.')
            .next()
            .unwrap_or("SHAPE")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(6)
            .collect();
        let short: String = stem.chars().take(5).collect();
        core::iter::once(format!("{stem}M{slot}.PIC"))
            .chain(
                "ABCDEFGHJKLMNPQRSTUVWXYZ"
                    .chars()
                    .map(|c| format!("{short}M{slot}{c}.PIC")),
            )
            .find(|n| self.doc.archive.find(n).is_none())
            .unwrap_or_else(|| format!("MARK{slot}.PIC"))
    }
    /// Panel action `op` on row `slot`.
    pub(super) fn marking_action(&mut self, slot: u16, op: u8) {
        if op == MK_FAMILY {
            self.ed.markings.family = !self.ed.markings.family;
            self.status = if self.ed.markings.family {
                "Runtime marking actions now apply to the damage family too.".into()
            } else {
                "Runtime marking actions apply to this shape only.".into()
            };
            return;
        }
        if op == MK_SELECT {
            self.select_marking(slot);
            return;
        }
        let r = self.apply_marking(slot, op, None);
        self.result(r);
    }
    /// Select the faces of a slot in the shared panel / Edit Mesh selection.
    fn select_marking(&mut self, slot: u16) {
        let Some(r) = self.marking_rows().into_iter().find(|r| r.slot == slot) else {
            return;
        };
        let faces: Vec<usize> = r
            .faces
            .iter()
            .copied()
            .chain(r.painted.iter().map(|p| p.0))
            .collect();
        if faces.is_empty() {
            self.status = format!("{} has no drawn face to select.", slot_title(slot));
            return;
        }
        self.ed.mesh_faces = faces.clone();
        if self.mesh_edit && self.mode == Mode::Model {
            self.ed.face_select = true;
            self.sync_face_vertices();
            self.status = format!(
                "Selected {} of {}. G, R and S move them.",
                view::count(faces.len(), "face", "faces"),
                slot_title(slot)
            );
        } else {
            self.selected_face = self.model_for_paint().and_then(|m| {
                m.faces
                    .iter()
                    .position(|f| Some(&f.offset) == faces.first())
            });
            self.status = format!(
                "Selected {} of {}. Move them with G, R and S in Edit Mesh (Tab).",
                view::count(faces.len(), "face", "faces"),
                slot_title(slot)
            );
        }
    }
    /// Toggle, reassign (`to`), make paintable or restore one slot on the
    /// target shapes, in one undo step. Family members without the slot are
    /// skipped and named.
    fn apply_marking(&mut self, slot: u16, op: u8, to: Option<u16>) -> Result<()> {
        self.finish_stroke();
        let targets = self.marking_targets()?;
        let main = targets[0];
        let main_rows = mk::markings(&self.doc.archive.entries[main].read()?)?;
        let row = main_rows
            .iter()
            .find(|r| r.slot == slot)
            .ok_or_else(|| format!("No E0 record selects slot {slot}"))?;
        let hide = row.faces.len() >= row.hidden.len() && !row.faces.is_empty();
        let mut entries = Vec::new();
        let mut removals = Vec::new();
        let mut skipped = Vec::new();
        let mut done = Vec::new();
        let paint_name = self.paint_name(slot);
        let mut sheet: Option<([u32; 2], u8, FillSource)> = None;
        let choice = self.fill_choice(slot);
        let mut picture = None;
        for (k, at) in targets.iter().enumerate() {
            let name = self.doc.archive.entries[*at].name.clone();
            let source = self.doc.archive.entries[*at].read()?;
            let rows = mk::markings(&source).map_err(|e| format!("{name}: {e}"))?;
            let Some(r) = rows.iter().find(|r| r.slot == slot) else {
                skipped.push(name);
                continue;
            };
            let edited = match op {
                MK_TOGGLE if hide && r.faces.is_empty() => continue,
                MK_TOGGLE if !hide && r.hidden.is_empty() => continue,
                MK_TOGGLE if hide => mk::hide_slot(&source, slot),
                MK_TOGGLE => mk::show_slot(&source, slot),
                MK_PAINT => mk::make_paintable(
                    &source,
                    slot,
                    &paint_name,
                    sheet.map(|s| s.0),
                    sheet.map_or(choice.mode(), |s| FillMode::Index(s.1)),
                    &|n| self.texture_bytes(n),
                )
                .map(|p| {
                    sheet.get_or_insert((p.size, p.color, p.fill));
                    picture.get_or_insert(p.picture);
                    p.shape
                }),
                MK_RESTORE => mk::restore_slot(&source, slot).map(|(b, names)| {
                    for n in names {
                        if !removals.contains(&n) {
                            removals.push(n);
                        }
                    }
                    b
                }),
                _ => mk::reassign_slot(&source, slot, to.unwrap_or(slot)),
            }
            .map_err(|e| if k == 0 { e } else { format!("{name}: {e}") })?;
            entries.push(Entry::new(&name, edited)?);
            done.push(name);
        }
        if entries.is_empty() {
            self.status = format!(
                "{} is already {}. Nothing changed.",
                slot_title(slot),
                if hide { "hidden" } else { "shown" }
            );
            return Ok(());
        }
        if let Some(p) = picture {
            entries.push(Entry::new(&paint_name, p)?);
        }
        // A restored sheet goes only when no other shape draws from it.
        let edited: Vec<String> = done.clone();
        removals.retain(|n| {
            self.doc.archive.find(n).is_some()
                && self
                    .dependencies
                    .incoming(n)
                    .all(|user| edited.iter().any(|e| e.eq_ignore_ascii_case(user)))
        });
        // A painted sheet takes its stored original along.
        let removals: Vec<String> = removals
            .iter()
            .flat_map(|n| hangar_core::originals::removals(&self.doc.archive, n))
            .collect();
        let selected = self.name().to_string();
        let context = self.context_name();
        self.doc.transaction(entries, &removals)?;
        self.reselect(&selected, context);
        let what = match op {
            MK_TOGGLE if hide => format!("Hid {}", slot_title(slot)),
            MK_TOGGLE => format!("Showed {}", slot_title(slot)),
            MK_PAINT => format!(
                "Made {} paintable on {paint_name}, {} \u{d7} {} px, filled with index {} ({})",
                slot_title(slot),
                sheet.map_or(0, |s| s.0[0]),
                sheet.map_or(0, |s| s.0[1]),
                sheet.map_or(0, |s| s.1),
                match sheet.map(|s| s.2) {
                    Some(FillSource::Surface(_)) => "the surface below",
                    Some(FillSource::Skin) => "the shape skin, no surface found",
                    Some(FillSource::Picked) => "picked",
                    _ => "the panel colour",
                }
            ),
            MK_RESTORE => format!("Restored {} to the game's image", slot_title(slot)),
            _ => format!(
                "Reassigned {} to slot {}",
                slot_title(slot),
                to.unwrap_or(slot)
            ),
        };
        let mut status = format!("{what} in {}.", done.join(", "));
        if !removals.is_empty() {
            status += &format!(" Removed {}.", removals.join(", "));
        }
        if !skipped.is_empty() {
            status += &format!(
                " {} {} no slot {slot}.",
                skipped.join(", "),
                if skipped.len() == 1 { "has" } else { "have" }
            );
        }
        self.status = status + " One undo step.";
        Ok(())
    }
    /// Slot Select item: reassign row `from` to slot `to`.
    pub(super) fn marking_slot(&mut self, from: u16, to: u16) {
        self.ed.markings.slot_menu = None;
        self.menu = None;
        if from == to {
            return;
        }
        let r = self.apply_marking(from, u8::MAX, Some(to));
        self.result(r);
    }
    /// Package line: hidden and paintable markings in the LIB's changed
    /// shapes (retail shapes carry neither).
    pub(super) fn package_markings(&self) -> Option<String> {
        let changed: Vec<Entry> = self
            .doc
            .archive
            .entries
            .iter()
            .filter(|e| e.name.ends_with(".SH") && self.doc.entry_changed(e))
            .cloned()
            .collect();
        let mut cache = self.ed.markings.package.borrow_mut();
        let fresh = cache.as_ref().is_some_and(|(list, _)| {
            list.len() == changed.len()
                && list
                    .iter()
                    .zip(&changed)
                    .all(|(a, b)| a.name == b.name && a.same_storage(b))
        });
        if !fresh {
            let (mut hidden, mut painted, mut shapes) = (0, 0, 0);
            for e in &changed {
                let rows = e
                    .read()
                    .ok()
                    .and_then(|b| mk::markings(&b).ok())
                    .unwrap_or_default();
                let h: usize = rows.iter().map(|r| r.hidden.len()).sum();
                let p: usize = rows.iter().map(|r| r.painted.len()).sum();
                shapes += usize::from(h + p > 0);
                hidden += h;
                painted += p;
            }
            let line = if shapes == 0 {
                String::new()
            } else {
                format!(
                    "Runtime markings: {} hidden, {} paintable in {}",
                    hidden,
                    painted,
                    view::count(shapes, "shape", "shapes")
                )
            };
            *cache = Some((changed, line));
        }
        cache
            .as_ref()
            .map(|(_, l)| l.clone())
            .filter(|l| !l.is_empty())
    }
}
/// A 1px line on a raster, clipped; dashed lines draw 3 of every 6 pixels.
/// A palette colour as a theme colour.
fn swatch(p: [u8; 3]) -> theme::Rgb {
    theme::Rgb((p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32)
}
fn line(
    pixels: &mut [u32],
    [w, h]: [usize; 2],
    a: [i32; 2],
    b: [i32; 2],
    color: u32,
    dashed: bool,
) {
    let (dx, dy) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
    let steps = dx.max(dy).min(4096);
    for t in 0..=steps {
        if dashed && (t / 3) % 2 == 1 {
            continue;
        }
        let (x, y) = if steps == 0 {
            (a[0], a[1])
        } else {
            (
                a[0] + (b[0] - a[0]) * t / steps,
                a[1] + (b[1] - a[1]) * t / steps,
            )
        };
        if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
            pixels[y as usize * w + x as usize] = color;
        }
    }
}

#[cfg(not(windows))]
impl App {
    /// Snapshot workspaces `markings` and `markings-hidden`: the shown SH
    /// (the synthetic markings kit in the demo) from above with the Runtime
    /// markings panel in view; `-hidden` hides the first slot through it.
    pub fn snapshot_markings(&mut self, name: &str) -> Result<()> {
        if self.path.starts_with("Synthetic") {
            self.doc.transaction(
                vec![
                    Entry::new(
                        "MK.SH",
                        hangar_core::shape_testkit::demo_markings_kit(false),
                    )?,
                    Entry::new(
                        "MK_A.SH",
                        hangar_core::shape_testkit::demo_markings_kit(true),
                    )?,
                    Entry::new("KIT.PIC", picture::demo())?,
                ],
                &[],
            )?;
            self.doc.mark_saved();
            let at = self.doc.archive.find("MK.SH").ok_or("Kit missing")?;
            self.select_entry(at);
        }
        self.mode = Mode::Model;
        self.textured = true;
        self.yaw = 0;
        self.pitch = 90;
        let slot = self
            .marking_rows()
            .first()
            .map(|r| r.slot)
            .ok_or("The shape selects no runtime slot")?;
        if name == "markings-hidden" {
            self.marking_action(slot, MK_TOGGLE);
        }
        self.scroll_to_markings();
        Ok(())
    }
}
impl App {
    /// Scroll the inspector until the Runtime markings controls show.
    pub(super) fn scroll_to_markings(&mut self) {
        for _ in 0..80 {
            let shown = self
                .layout()
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::Marking(_, MK_TOGGLE)));
            if shown {
                return;
            }
            self.mouse = [self.right() + 20, 300];
            self.wheel(-1);
        }
    }
}

/// The demo LIB with the synthetic markings kit as MK.SH (slots 2, 3, 4),
/// its damage shape MK_A.SH (slots 3, 4) and KIT.PIC, in the Model
/// workspace at 1280 x 800 seen from above, the panel scrolled into view.
/// The kit goes through Hide and Show once so its relocations are padded
/// as native files pad them, the layout every Hangar edit writes.
#[inline(never)]
fn markings_app() -> Box<App> {
    let native =
        |b: Vec<u8>| -> Vec<u8> { mk::show_slot(&mk::hide_slot(&b, 3).unwrap(), 3).unwrap() };
    let mut a = Box::new(App::new());
    a.demo();
    a.doc
        .transaction(
            vec![
                Entry::new(
                    "MK.SH",
                    native(hangar_core::shape_testkit::demo_markings_kit(false)),
                )
                .unwrap(),
                Entry::new(
                    "MK_A.SH",
                    native(hangar_core::shape_testkit::demo_markings_kit(true)),
                )
                .unwrap(),
                Entry::new("KIT.PIC", picture::demo()).unwrap(),
            ],
            &[],
        )
        .unwrap();
    a.doc.mark_saved();
    a.width = 1280;
    a.height = 800;
    a.select_entry(a.doc.archive.find("MK.SH").unwrap());
    a.mode = Mode::Model;
    a.textured = true;
    a.yaw = 0;
    a.pitch = 90;
    a.scroll_to_markings();
    a
}
impl App {
    fn smoke_row(&self, slot: u16) -> Marking {
        self.marking_rows()
            .into_iter()
            .find(|r| r.slot == slot)
            .unwrap_or_else(|| panic!("slot {slot}: {}", self.status))
    }
    /// Choose a slot's fill through its rendered Select and menu item.
    fn smoke_fill(&mut self, slot: u16, kind: u8) {
        self.scroll_to_markings();
        let select = self.smoke_find(&|x| matches!(x, Action::MarkingFillMenu(s) if s == slot));
        self.chrome_click(select);
        assert_eq!(self.menu, Some(chrome::MENU_SLOT));
        let item =
            self.smoke_find(&|x| matches!(x, Action::MarkingFill(s, k) if s == slot && k == kind));
        self.chrome_click(item);
    }
    fn smoke_mk(&mut self, slot: u16, op: u8) {
        self.scroll_to_markings();
        self.smoke_tap(&|x| matches!(x, Action::Marking(s, o) if s == slot && o == op));
    }
    /// Pixels of `color` in the viewport raster.
    fn smoke_pixels(&self, color: u32) -> usize {
        self.raster_count(color)
    }
    /// Runtime markings end to end through rendered controls: Shown/Hidden,
    /// the viewport outlines, Apply to damage family, Select faces into
    /// Edit Mesh and a G move, the slot Select (and its refusal), Make
    /// paintable and Restore runtime marking, the Package line, undo to the
    /// exact bytes, and hit geometry at both window sizes.
    #[inline(never)]
    pub(super) fn smoke_markings(&mut self) {
        let mut a = markings_app();
        let original = a.doc.archive.bytes().unwrap();
        let slots: Vec<u16> = a.marking_rows().iter().map(|r| r.slot).collect();
        assert_eq!(slots, [2, 3, 4]);
        let f4 = a.smoke_row(4).faces.clone();
        assert_eq!(f4.len(), 1);
        // Drawn markings are outlined in steel while the panel is open.
        assert!(a.smoke_pixels(c::STEEL.0) > 0, "steel outlines");
        // Shown off: one undo step, the face is gone, the outline dashes.
        a.smoke_mk(4, MK_TOGGLE);
        assert!(
            a.status
                .starts_with("Hid Wing marking right (slot 4) in MK.SH."),
            "{}",
            a.status
        );
        assert_eq!(a.smoke_row(4).hidden, f4);
        assert!(!a
            .model
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .any(|f| f.offset == f4[0]));
        assert!(
            a.smoke_pixels(c::INK_FAINT.0) > 0,
            "dashed outline of the hidden face"
        );
        assert_eq!(a.doc.changed_count(), 1);
        // The Package workspace names it.
        assert_eq!(
            a.package_markings().as_deref(),
            Some("Runtime markings: 1 hidden, 0 paintable in 1 shape")
        );
        a.mode = Mode::Package;
        assert!(a.draw().commands.iter().any(
            |d| matches!(d, Draw::Text(_, _, t, ..) if t.starts_with("Runtime markings: 1 hidden"))
        ));
        a.mode = Mode::Model;
        // Shown on again gives the exact bytes back, as does undo.
        a.smoke_mk(4, MK_TOGGLE);
        assert!(a.status.starts_with("Showed"), "{}", a.status);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Apply to damage family: MK.SH and MK_A.SH in one step.
        a.smoke_mk(0, MK_FAMILY);
        assert!(a.ed.markings.family);
        a.smoke_mk(3, MK_TOGGLE);
        assert!(a.status.contains("in MK.SH, MK_A.SH."), "{}", a.status);
        let damaged = a.doc.archive.find("MK_A.SH").unwrap();
        let rows = mk::markings(&a.doc.archive.entries[damaged].read().unwrap()).unwrap();
        assert!(rows
            .iter()
            .any(|r| r.slot == 3 && r.faces.is_empty() && r.hidden.len() == 1));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // A family member without the slot is named, not failed.
        a.smoke_mk(2, MK_TOGGLE);
        assert!(a.status.contains("MK_A.SH has no slot 2."), "{}", a.status);
        a.act(Action::Undo);
        a.smoke_mk(0, MK_FAMILY);
        assert!(!a.ed.markings.family);
        // Select faces feeds Edit Mesh; G moves them; undo is exact.
        a.key(Key::Tab, false, false);
        assert!(a.mesh_edit);
        a.smoke_mk(4, MK_SELECT);
        assert!(a.ed.face_select && a.ed.mesh_faces == f4, "{}", a.status);
        assert_eq!(a.mesh_vertices.len(), 4);
        let before: Vec<[i32; 3]> = a
            .mesh_vertices
            .iter()
            .map(|i| a.model.as_ref().unwrap().vertices[*i].point)
            .collect();
        a.mouse = [600, 300];
        a.key(Key::Char('g'), false, false);
        a.smoke_keys("z2");
        a.key(Key::Enter, false, false);
        let model = a.model.as_ref().unwrap();
        let face = model.faces.iter().find(|f| f.offset == f4[0]).unwrap();
        let moved: Vec<[i32; 3]> = face
            .indices
            .iter()
            .map(|i| model.vertices[*i].point)
            .collect();
        assert!(
            before
                .iter()
                .all(|p| moved.contains(&[p[0], p[1], p[2] + 2])),
            "{}",
            a.status
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.key(Key::Tab, false, false);
        assert!(!a.mesh_edit);
        // Slot Select: reassign 4 to 1 through the menu, refuse 4 to 3.
        a.scroll_to_markings();
        let select = a.smoke_find(&|x| matches!(x, Action::MarkingMenu(4)));
        a.chrome_click(select);
        assert_eq!(a.menu, Some(chrome::MENU_SLOT));
        a.smoke_geometry("slot menu");
        let item = a.smoke_find(&|x| matches!(x, Action::MarkingSlot(4, 1)));
        a.chrome_click(item);
        assert!(
            a.status
                .starts_with("Reassigned Wing marking right (slot 4) to slot 1"),
            "{}",
            a.status
        );
        assert_eq!(a.smoke_row(1).faces, f4);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.scroll_to_markings();
        let select = a.smoke_find(&|x| matches!(x, Action::MarkingMenu(4)));
        a.chrome_click(select);
        let item = a.smoke_find(&|x| matches!(x, Action::MarkingSlot(4, 3)));
        a.chrome_click(item);
        assert!(a.status.contains("would merge"), "{}", a.status);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Fill: the default is the colour the marking sits on (the kit's
        // wing plate, 151), not the marking's stored 150 and not index 0.
        let fill_pixels = |a: &App| -> Vec<u8> {
            let pic = a.doc.archive.find("MKM4.PIC").expect("sheet");
            let bytes = a.doc.archive.entries[pic].read().unwrap();
            let mut px = Pic::parse(&bytes).unwrap().pixels;
            px.sort_unstable();
            px.dedup();
            px
        };
        assert_eq!(a.fill_choice(4), FillChoice::Surface);
        a.scroll_to_markings();
        assert!(a
            .draw()
            .commands
            .iter()
            .any(|d| matches!(d, Draw::Text(_, _, t, ..) if t == "From the surface below")));
        a.smoke_geometry("fill row");
        a.smoke_mk(4, MK_PAINT);
        assert!(
            a.status
                .contains("filled with index 151 (the surface below)"),
            "{}",
            a.status
        );
        assert_eq!(fill_pixels(&a), [151]);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Panel colour: the old fill.
        a.smoke_fill(4, FILL_PANEL);
        assert_eq!(a.fill_choice(4), FillChoice::Panel);
        a.smoke_mk(4, MK_PAINT);
        assert!(
            a.status
                .contains("filled with index 150 (the panel colour)"),
            "{}",
            a.status
        );
        assert_eq!(fill_pixels(&a), [150]);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Pick: starts on the surface's index, then a palette cell.
        a.smoke_fill(4, FILL_PICK);
        assert_eq!(a.fill_choice(4), FillChoice::Pick(151));
        a.scroll_to_markings();
        a.smoke_geometry("fill palette grid");
        a.smoke_tap(&|x| matches!(x, Action::MarkingFillColor(4, 42)));
        assert_eq!(a.fill_choice(4), FillChoice::Pick(42));
        a.smoke_mk(4, MK_PAINT);
        assert!(
            a.status.contains("filled with index 42 (picked)"),
            "{}",
            a.status
        );
        assert_eq!(fill_pixels(&a), [42]);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.smoke_fill(4, FILL_SURFACE);
        assert_eq!(a.fill_choice(4), FillChoice::Surface);
        // Make paintable: a retail-layout PIC, the faces drawn from it.
        a.smoke_mk(4, MK_PAINT);
        assert!(
            a.status
                .starts_with("Made Wing marking right (slot 4) paintable on MKM4.PIC"),
            "{}",
            a.status
        );
        let pic = a.doc.archive.find("MKM4.PIC").expect("new sheet");
        assert!(picture::is_retail_texture(
            &a.doc.archive.entries[pic].read().unwrap()
        ));
        let painted = a.smoke_row(4).painted;
        assert_eq!(painted.len(), 1);
        assert_eq!(painted[0].1, "MKM4.PIC");
        let model = a.model.as_ref().unwrap();
        assert_eq!(
            model
                .faces
                .iter()
                .find(|f| f.offset == painted[0].0)
                .map(|f| f.texture.as_str()),
            Some("MKM4.PIC")
        );
        assert_eq!(a.doc.changed_count(), 2);
        assert_eq!(
            a.package_markings().as_deref(),
            Some("Runtime markings: 0 hidden, 1 paintable in 1 shape")
        );
        // Restore runtime marking: the sheet goes and the bytes come back.
        a.smoke_mk(4, MK_RESTORE);
        assert!(a.status.contains("Removed MKM4.PIC."), "{}", a.status);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.act(Action::Undo);
        assert!(a.doc.archive.find("MKM4.PIC").is_some());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // The Paint workspace shows the same panel for its context model.
        a.mode = Mode::Media;
        a.smoke_mk(3, MK_TOGGLE);
        assert!(
            a.status.starts_with("Hid Wing marking left (slot 3)"),
            "{}",
            a.status
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.mode = Mode::Model;
        // Hit geometry at both sizes, with a hidden row and a painted row.
        a.smoke_mk(4, MK_TOGGLE);
        a.smoke_mk(3, MK_PAINT);
        for (w, h) in [(1280, 800), (800, 600)] {
            a.width = w;
            a.height = h;
            a.scroll_to_markings();
            a.smoke_geometry("runtime markings");
            let r = a.right();
            for hit in a.layout().hits {
                if matches!(
                    hit.action,
                    Action::Marking(..) | Action::MarkingMenu(_) | Action::MarkingFillMenu(_)
                ) {
                    assert!(
                        hit.rect[0] > r && hit.rect[0] + hit.rect[2] <= w,
                        "{w}x{h} {:?}",
                        hit.rect
                    );
                }
            }
        }
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
    }
}

#[cfg(not(windows))]
impl App {
    /// `--markings-check LIB ENTRY.SH NEW_OUTPUT_DIR`: hide every slot of
    /// the shape through the panel, render from above and below, save and
    /// reopen, show every slot (byte-identical to the original entry), make
    /// the last slot paintable, paint a roundel on its sheet with the brush,
    /// render, save and reopen. Writes PNGs and LIBs into the new directory.
    pub fn check_markings(&mut self, entry: &str, out: &str) -> Result<String> {
        use hangar_core::archive::Archive;
        let mut report = String::new();
        let at = self.doc.archive.find(entry).ok_or("SH not found")?;
        let original = self.doc.archive.entries[at].read()?;
        let pick = |a: &mut App| -> Result<()> {
            let at = a.doc.archive.find(entry).ok_or("SH not found")?;
            a.select_entry(at);
            a.mode = Mode::Model;
            a.textured = true;
            a.width = 1280;
            a.height = 800;
            a.frame();
            a.scroll_to_markings();
            Ok(())
        };
        pick(self)?;
        let rows = self.marking_rows();
        if rows.is_empty() {
            return Err(format!("{entry} selects no runtime slot"));
        }
        for r in &rows {
            report += &format!(
                "{entry} {}: E0 at CODE+{:X}, faces {:X?}\n",
                slot_title(r.slot),
                r.records[0],
                r.faces
            );
        }
        report += &format!(
            "  shown, from above: {}\n",
            self.markings_png(out, "shown-top.png", 90)?
        );
        report += &format!(
            "  shown, from below: {}\n",
            self.markings_png(out, "shown-bottom.png", -90)?
        );
        for r in &rows {
            self.scroll_to_markings();
            self.smoke_tap(&|x| matches!(x, Action::Marking(s, MK_TOGGLE) if s == r.slot));
            report += &format!("  {}\n", self.status);
        }
        report += &format!(
            "  hidden, from above: {}\n",
            self.markings_png(out, "hidden-top.png", 90)?
        );
        report += &format!(
            "  hidden, from below: {}\n",
            self.markings_png(out, "hidden-bottom.png", -90)?
        );
        // Save and reopen: the hide blocks are recognised from the bytes.
        let saved = format!("{out}/MKHIDE.LIB");
        crate::platform::write_new(&saved, &self.doc.archive.bytes()?)?;
        self.open(&saved)?;
        pick(self)?;
        let hidden: usize = self.marking_rows().iter().map(|r| r.hidden.len()).sum();
        report += &format!("  saved {saved} and reopened: {hidden} hidden faces recognised\n");
        for r in &rows {
            self.scroll_to_markings();
            self.smoke_tap(&|x| matches!(x, Action::Marking(s, MK_TOGGLE) if s == r.slot));
            report += &format!("  {}\n", self.status);
        }
        let at = self.doc.archive.find(entry).ok_or("SH lost")?;
        let shown = self.doc.archive.entries[at].read()?;
        if shown != original {
            return Err(format!(
                "{entry}: Show after reopening is not byte-identical"
            ));
        }
        report +=
            &format!("  shown again after reopening: byte-identical to the original {entry}\n");
        // Make the last slot paintable and paint a roundel with the brush.
        let slot = rows.last().map(|r| r.slot).unwrap_or(4);
        // The old fill (the marking's stored colour) first, for comparison,
        // then the default (the surface under the marking).
        for (kind, prefix, lib) in [
            (FILL_PANEL, "panel-fill", "MKPANEL"),
            (FILL_SURFACE, "painted", "MKPAINT"),
        ] {
            self.smoke_fill(slot, kind);
            report += &format!("  {prefix}: {}\n", self.status);
            self.scroll_to_markings();
            self.smoke_tap(&|x| matches!(x, Action::Marking(s, MK_PAINT) if s == slot));
            report += &format!("  {}\n", self.status);
            let row = self
                .marking_rows()
                .into_iter()
                .find(|r| r.slot == slot)
                .ok_or("Slot lost")?;
            let name = row
                .painted
                .first()
                .map(|p| p.1.clone())
                .ok_or("Not paintable")?;
            let pic = self.doc.archive.find(&name).ok_or("No sheet")?;
            let size = Pic::parse(&self.doc.archive.entries[pic].read()?)?;
            let near = |a: &App, rgb: [i32; 3]| -> u8 {
                (0..256)
                    .min_by_key(|i| {
                        let p = a.base_palette[*i];
                        (0..3).map(|k| (p[k] as i32 - rgb[k]).pow(2)).sum::<i32>()
                    })
                    .unwrap_or(0) as u8
            };
            let (blue, white, red) = (
                near(self, [30, 50, 140]),
                near(self, [240, 240, 240]),
                near(self, [200, 30, 30]),
            );
            let u = row
                .painted
                .first()
                .and_then(|p| {
                    self.model
                        .as_ref()?
                        .faces
                        .iter()
                        .find(|f| f.offset == p.0)
                        .map(|f| f.uv.clone())
                })
                .unwrap_or_default();
            let (w, h) = (
                u.iter().map(|p| p[0]).max().unwrap_or(1) + 1,
                u.iter().map(|p| p[1]).max().unwrap_or(1) + 1,
            );
            let (cx, cy, radius) = (w / 2, size.height as i32 - 1 - h / 2, w.min(h) / 2);
            self.brush_radius = 0;
            for (color, r) in [(blue, radius), (white, radius * 2 / 3), (red, radius / 3)] {
                self.brush = color;
                for y in cy - r..=cy + r {
                    for x in cx - r..=cx + r {
                        if (x - cx).pow(2) + (y - cy).pow(2) <= r * r && x >= 0 && y >= 0 {
                            self.paint_at(pic, x as usize, y as usize);
                        }
                    }
                }
            }
            self.finish_stroke();
            report += &format!(
                "  painted {name} ({} x {}): roundel of indices {blue}, {white}, {red}, radius {radius} px. {}\n",
                size.width, size.height, self.status
            );
            report += &format!(
                "  {prefix}, from below: {}\n",
                self.markings_png(out, &format!("{prefix}-bottom.png"), -90)?
            );
            report += &format!(
                "  {prefix}, from above: {}\n",
                self.markings_png(out, &format!("{prefix}-top.png"), 90)?
            );
            let saved = format!("{out}/{lib}.LIB");
            crate::platform::write_new(&saved, &self.doc.archive.bytes()?)?;
            let reopened = Archive::parse(crate::platform::read(&saved)?)?;
            let sheet = reopened.find(&name).ok_or("Sheet not saved")?;
            let bytes = reopened.entries[sheet].read()?;
            let shape = reopened.entries[reopened.find(entry).ok_or("SH not saved")?].read()?;
            let painted = mk::markings(&shape)?
                .into_iter()
                .find(|r| r.slot == slot)
                .map_or(0, |r| r.painted.len());
            report += &format!(
                "  saved {saved} and reopened: {name} retail layout {}, {painted} face drawn from it\n",
                picture::is_retail_texture(&bytes)
            );
            // Restore runtime marking: the entry and the entry list as before.
            self.scroll_to_markings();
            self.smoke_tap(&|x| matches!(x, Action::Marking(s, MK_RESTORE) if s == slot));
            report += &format!("  {}\n", self.status);
            let at = self.doc.archive.find(entry).ok_or("SH lost")?;
            let back = self.doc.archive.entries[at].read()?;
            let org = hangar_core::originals::companion(&name).unwrap_or_default();
            if back != original
                || self.doc.archive.find(&name).is_some()
                || self.doc.archive.find(&org).is_some()
            {
                return Err(format!(
                    "{entry}: Restore runtime marking did not return the original bytes"
                ));
            }
            report += &format!(
                "  restored: {entry} byte-identical to the original, {name} and {org} removed\n"
            );
        }
        Ok(report)
    }
    /// The textured viewport from above (pitch 90) or below (-90) as a
    /// 640 x 400 PNG in `out`.
    fn markings_png(&mut self, out: &str, file: &str, pitch: i32) -> Result<String> {
        let (yaw, was) = (self.yaw, self.pitch);
        self.yaw = 0;
        self.pitch = pitch;
        let rgba = self.render_rgba(640, 400);
        (self.yaw, self.pitch) = (yaw, was);
        let path = format!("{out}/{file}");
        crate::platform::write_new(&path, &picture::rgba_png(640, 400, &rgba))?;
        Ok(path)
    }
}
