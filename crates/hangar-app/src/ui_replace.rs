//! Color replacement in PIC textures: the Replace paint tool beside Brush
//! and Eraser, and the Replace color dialog. Both work on palette indices
//! through `Pic::replace` and `Pic::replace_all`; a stroke or an apply is one
//! undo step and keeps stored originals exactly as the brush does.
use super::view::{Action, Icon, Layout, DIALOG_HEAD};
use super::widgets::{self, baseline, Btn, Number, NumberTarget, Tone};
use super::*;
use hangar_core::{
    originals,
    picture::{footprint, matching, IndexSet, MAX_TOLERANCE},
};
use theme::{metric as m, space};

pub(super) const SCOPE_WHOLE: u8 = 0;
pub(super) const SCOPE_PANELS: u8 = 1;
pub(super) const SCOPE_FACES: u8 = 2;
const SCOPES: [&str; 3] = ["Whole texture", "Selected panels", "Faces' textures"];
const SLOTS: [&str; 2] = ["From", "To"];

/// Replace tool and dialog state, boxed to keep `App` small.
#[derive(Default)]
pub(super) struct ReplaceState {
    /// Replace is the active paint tool.
    pub on: bool,
    /// The palette index to replace; the paint color (`App::brush`) replaces it.
    pub from: Option<u8>,
    /// RGB distance in 6-bit palette steps (`picture::matching`).
    pub tolerance: u8,
    /// Alt held, reported by the backend: Alt+click picks `from`.
    pub alt: bool,
    pub dialog: Option<Dialog>,
}
/// The Replace color dialog; From and To are `ReplaceState::from` and the
/// paint color, so the dialog and the brush share them.
#[derive(Default)]
pub(super) struct Dialog {
    pub scope: u8,
    /// The swatch the palette grid and Pick from image set: 0 From, 1 To.
    pub slot: u8,
    /// Hidden while the next click on the texture or model picks a color.
    pub picking: bool,
    /// Pixels each PIC would change, or why the scope cannot apply.
    pub plan: Option<Result<Vec<(String, usize)>>>,
    /// Why each scope is unavailable.
    pub refusals: [Option<String>; 3],
}
/// A Replace stroke's matching indices and running count.
pub(super) struct StrokeReplace {
    pub set: IndexSet,
    pub from: u8,
    pub to: u8,
    pub count: usize,
    /// Panel lock: the face and its UV footprint the stroke stays inside.
    pub lock: Option<(usize, Vec<bool>)>,
}
fn rgb(p: [u8; 3]) -> theme::Rgb {
    theme::Rgb((p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32)
}
fn grouped(n: usize) -> String {
    widgets::format_number(n as i64, 0, true)
}
/// "_F18.PIC", "_F18.PIC and KIT.PIC", "_F18.PIC and 2 more".
fn names(list: &[String]) -> String {
    match list {
        [] => "nothing".into(),
        [one] => one.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, rest @ ..] => format!("{a} and {} more", rest.len()),
    }
}
/// A swatch with a keyline at (x, y).
fn swatch(o: &mut Layout, x: i32, y: i32, size: i32, color: [u8; 3]) {
    o.canvas.rect(x, y, size, size, rgb(color));
    widgets::frame(&mut o.canvas, [x, y, size, size], c::LINE_STRONG);
}
impl App {
    /// "of index 34" or "of index 34 (tolerance 6)".
    pub(super) fn replace_source_text(&self, from: u8) -> String {
        match self.replace.tolerance {
            0 => format!("of index {from}"),
            t => format!("of index {from} (tolerance {t})"),
        }
    }
    /// What a finished replacement says: count, indices, textures, kept
    /// originals and converted panels.
    pub(super) fn replace_status(
        &self,
        counts: &[(String, usize)],
        from: u8,
        to: u8,
        kept: &[String],
        generated: usize,
    ) -> String {
        let total: usize = counts.iter().map(|(_, n)| n).sum();
        let list: Vec<String> = counts.iter().map(|(n, _)| n.clone()).collect();
        let mut s = format!(
            "Replaced {} pixel{} {} with {to} in {}",
            grouped(total),
            if total == 1 { "" } else { "s" },
            self.replace_source_text(from),
            names(&list)
        );
        if generated > 0 {
            s.push_str(&format!(
                "; {generated} flat panel{} converted to new textures",
                if generated == 1 { "" } else { "s" }
            ));
        }
        if !kept.is_empty() {
            s.push_str(&format!("; original kept as {}", kept.join(", ")));
        }
        s.push_str(". One undo step.");
        s
    }
    /// The Replace tool: a third paint mode beside Brush and Eraser.
    pub(super) fn replace_tool(&mut self) {
        self.finish_stroke();
        if self.mode == Mode::Model {
            let on = !self.replace.on;
            if on && !self.model_paint {
                self.act(Action::ModelPaint);
            }
            self.replace.on = on;
        } else {
            self.paint_enabled = !self.paint_enabled || !self.replace.on;
            self.replace.on = self.paint_enabled;
        }
        self.eraser = false;
        self.pick_color = false;
        if self.replace.on {
            self.status = match self.replace.from {
                None => "Replace: Alt+click the color to replace, or use Pick; strokes paint the current color over it".into(),
                Some(from) => format!(
                    "Replace: strokes paint index {} over index {from}",
                    self.brush
                ),
            };
        }
    }
    /// Indices of `pic` that match the Replace source within the tolerance.
    pub(super) fn replace_set(&self, pic: &Pic, from: u8) -> IndexSet {
        matching(
            &pic.colors(&self.base_palette),
            from,
            self.replace.tolerance,
        )
    }
    /// A new Replace stroke on `pic`, or why it cannot start.
    pub(super) fn replace_stroke(&self, pic: &Pic) -> Result<Box<StrokeReplace>> {
        let from = self
            .replace
            .from
            .ok_or("Alt+click the color to replace first, or use Pick")?;
        Ok(Box::new(StrokeReplace {
            set: self.replace_set(pic, from),
            from,
            to: self.brush,
            count: 0,
            lock: None,
        }))
    }
    /// Panel lock on the model: the UV footprint of the locked face in
    /// texture `name`, kept with the stroke until the face changes.
    pub(super) fn replace_lock(&self, name: &str, pic: &Pic) -> Option<(usize, Vec<bool>)> {
        if self.mode != Mode::Model || !self.model_paint || !self.paint_lock {
            return None;
        }
        let face = self.selected_face?;
        let f = self.model_for_paint()?.faces.get(face)?;
        if !texture_ui::full(&f.texture).eq_ignore_ascii_case(name) {
            return None;
        }
        footprint(pic.width, pic.height, &[f.uv.as_slice()])
            .ok()
            .map(|mask| (face, mask))
    }
    /// A flat panel converts to a sheet for Replace only when its color
    /// matches, so a stroke never creates a texture it leaves unchanged.
    pub(super) fn replace_flat(&self, face: usize) -> Result<()> {
        let from = self
            .replace
            .from
            .ok_or("Alt+click the color to replace first, or use Pick")?;
        let color = self
            .model_for_paint()
            .and_then(|m| m.faces.get(face))
            .map(|f| f.color)
            .ok_or("Panel missing")?;
        if matching(&self.base_palette, from, self.replace.tolerance)[color as usize] {
            Ok(())
        } else {
            Err(format!(
                "Panel color {color} is not {}; nothing to replace",
                self.replace_source_text(from).trim_start_matches("of ")
            ))
        }
    }
    /// The color under a model hit: the texture pixel, or a flat panel's color.
    fn model_color(&self, face: usize, uv: [i32; 2]) -> Option<u8> {
        let f = self.model_for_paint()?.faces.get(face)?;
        if uv[0] < 0 || uv[1] < 0 {
            return (f.sub & 4 == 0 || f.texture.is_empty()).then_some(f.color);
        }
        let p = self.texture_for(&f.texture)?;
        let i = uv[1] as usize * p.width + uv[0] as usize;
        p.mask.get(i).copied().unwrap_or(false).then(|| p.pixels[i])
    }
    /// Whether the next click picks a color for the hidden dialog.
    pub(super) fn replace_picking(&self) -> bool {
        self.replace.dialog.as_ref().is_some_and(|d| d.picking)
    }
    /// Alt+click on the model with Replace, or a dialog pick: take the color
    /// under the cursor instead of painting. Returns whether it picked.
    pub(super) fn model_pick(&mut self, face: usize, uv: [i32; 2]) -> bool {
        let alt = self.replace.alt && self.replace.on && self.model_paint;
        if !alt && !self.replace_picking() {
            return false;
        }
        let index = self.model_color(face, uv);
        self.color_picked(index);
        true
    }
    /// Whether an atlas click picks rather than paints.
    pub(super) fn atlas_picks(&self) -> bool {
        self.pick_color
            || self.replace_picking()
            || (self.replace.alt && self.replace.on && !self.painting)
    }
    /// A picked color goes to the dialog's slot, the Replace source, or the
    /// paint color.
    pub(super) fn color_picked(&mut self, index: Option<u8>) {
        let Some(index) = index else {
            self.status = "Transparent pixel; no color picked".into();
            return;
        };
        self.pick_color = false;
        if let Some(d) = self.replace.dialog.as_mut().filter(|d| d.picking) {
            d.picking = false;
            let slot = d.slot;
            if slot == 0 {
                self.replace.from = Some(index);
            } else {
                self.brush = index;
            }
            self.replace_refresh();
            self.replace_show();
            self.status = format!("{} color: index {index}", SLOTS[slot as usize]);
            return;
        }
        if self.replace.on {
            self.replace.from = Some(index);
            // Back to the Replace tool after Pick.
            if self.mode == Mode::Media {
                self.paint_enabled = true;
            }
            self.status = format!(
                "Replace source: index {index}; strokes paint index {} over it",
                self.brush
            );
        } else {
            self.brush = index;
            self.status = format!("Picked palette index {index}");
        }
    }
    /// Backends report Alt before pointer events (Alt+click picks for Replace).
    pub fn alt_modifier(&mut self, held: bool) {
        let changed = self.replace.alt != held;
        self.replace.alt = held;
        if changed && self.add_tool_on() {
            let shift = self.ed.add_vertex.as_ref().is_some_and(|t| t.shift);
            self.add_vertex_hover(self.mouse[0], self.mouse[1], shift);
        }
    }
    /// The PIC entry `name` decoded for replacement, or why not.
    fn replace_source(&self, name: &str) -> Result<(usize, Vec<u8>, Pic)> {
        if let Some(pic) = originals::texture_of(name) {
            return Err(format!(
                "{name} is a stored original and read-only; replace in {pic} instead"
            ));
        }
        let at = self.doc.archive.find(name).ok_or_else(|| {
            format!("{name} is outside the active LIB; copy or open its owner first")
        })?;
        let bytes = self.doc.archive.entries[at].read()?;
        let pic = Pic::parse(&bytes)?;
        if !pic.paintable {
            return Err(format!("{name} uses aliased storage; painting disabled"));
        }
        if !self.palette_loaded && pic.palette.len() != 256 {
            return Err(
                "Load the base .PAL before replacing colors in this partial-palette PIC".into(),
            );
        }
        Ok((at, bytes, pic))
    }
    /// The PICs a scope acts on, each with its region (`None`: whole image).
    fn replace_targets(&self, scope: u8) -> Result<Vec<(String, Option<Vec<bool>>)>> {
        let faces = self.texture_faces();
        match scope {
            SCOPE_WHOLE => {
                let name = if self.pic.is_some() {
                    self.name().to_string()
                } else {
                    let at = self
                        .texture_target()
                        .ok_or("Select a PIC or a textured panel")?;
                    self.doc.archive.entries[at].name.clone()
                };
                self.replace_source(&name)?;
                Ok(vec![(name, None)])
            }
            SCOPE_PANELS => {
                let model = self
                    .model_for_paint()
                    .ok_or("Select faces in Edit Mesh or pick a textured panel")?;
                let mut groups: Vec<(String, Vec<&[[i32; 2]]>)> = Vec::new();
                for offset in &faces {
                    let Some(f) = model.faces.iter().find(|f| f.offset == *offset) else {
                        continue;
                    };
                    if f.sub & 4 == 0 || f.texture.is_empty() || f.uv.len() != f.indices.len() {
                        continue;
                    }
                    let name = texture_ui::full(&f.texture);
                    match groups.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, polys)) => polys.push(&f.uv),
                        None => groups.push((name, vec![&f.uv])),
                    }
                }
                if groups.is_empty() {
                    return Err(
                        "Select textured faces in Edit Mesh or pick a textured panel".into(),
                    );
                }
                let mut out = Vec::new();
                for (name, polys) in groups {
                    let (_, _, pic) = self.replace_source(&name)?;
                    out.push((name, Some(footprint(pic.width, pic.height, &polys)?)));
                }
                Ok(out)
            }
            SCOPE_FACES => {
                let list: Vec<String> = self
                    .face_texture_names(&faces)
                    .into_iter()
                    .map(|(n, _)| n)
                    .filter(|n| !n.is_empty())
                    .collect();
                if list.len() < 2 {
                    return Err(
                        "The selected faces draw from one texture; use Whole texture".into(),
                    );
                }
                for name in &list {
                    self.replace_source(name)?;
                }
                Ok(list.into_iter().map(|n| (n, None)).collect())
            }
            _ => Err("Unknown replace scope".into()),
        }
    }
    /// Pixels the dialog would change in each PIC.
    fn replace_plan(&self, scope: u8) -> Result<Vec<(String, usize)>> {
        let targets = self.replace_targets(scope)?;
        let from = self
            .replace
            .from
            .ok_or("Choose the From color: click a palette cell or Pick from image")?;
        let mut out = Vec::new();
        for (name, region) in targets {
            let (_, _, pic) = self.replace_source(&name)?;
            let set = self.replace_set(&pic, from);
            out.push((
                name,
                pic.replace_count(&set, self.brush, region.as_deref())?,
            ));
        }
        Ok(out)
    }
    /// Recount after a change of scope, colors or tolerance.
    pub(super) fn replace_refresh(&mut self) {
        let Some(scope) = self.replace.dialog.as_ref().map(|d| d.scope) else {
            return;
        };
        let refusals = core::array::from_fn(|k| self.replace_targets(k as u8).err());
        let plan = self.replace_plan(scope);
        if let Some(d) = self.replace.dialog.as_mut() {
            d.refusals = refusals;
            d.plan = Some(plan);
        }
    }
    pub(super) fn replace_show(&mut self) {
        self.prompt = Some(Prompt {
            kind: PromptKind::ReplaceColor,
            title: "Replace color".into(),
            value: String::new(),
            axis: 0,
        });
    }
    /// Paint, Mesh menu and Face textures action: open the dialog on the
    /// selected faces' panels in Edit Mesh or with panels selected, else on
    /// the whole texture.
    pub(super) fn open_replace_dialog(&mut self) {
        self.finish_stroke();
        let panels = if self.mesh_edit && self.mode == Mode::Model {
            !self.texture_faces().is_empty()
        } else {
            !self.panel_offsets().is_empty()
        };
        self.replace.dialog = Some(Dialog {
            scope: if panels { SCOPE_PANELS } else { SCOPE_WHOLE },
            ..Default::default()
        });
        self.replace_refresh();
        let d = self.replace.dialog.as_mut().unwrap();
        if d.refusals[d.scope as usize].is_some() {
            match (0..3).find(|k| d.refusals[*k].is_none()) {
                Some(k) => d.scope = k as u8,
                None => {
                    let why = d.refusals[d.scope as usize].take().unwrap_or_default();
                    self.replace.dialog = None;
                    self.status = format!("Error: {why}");
                    return;
                }
            }
            self.replace_refresh();
        }
        self.replace_show();
    }
    pub(super) fn replace_scope(&mut self, scope: u8) {
        if let Some(d) = self.replace.dialog.as_mut() {
            if scope < 3 && d.refusals[scope as usize].is_none() {
                d.scope = scope;
            }
        }
        self.replace_refresh();
    }
    pub(super) fn replace_slot(&mut self, slot: u8) {
        if let Some(d) = self.replace.dialog.as_mut() {
            d.slot = slot.min(1);
        }
    }
    /// Palette cell in the dialog: sets the active slot.
    pub(super) fn replace_swatch(&mut self, index: u8) {
        let Some(slot) = self.replace.dialog.as_ref().map(|d| d.slot) else {
            return;
        };
        if slot == 0 {
            self.replace.from = Some(index);
        } else {
            self.brush = index;
        }
        self.replace_refresh();
    }
    /// Hide the dialog until the next click on the texture or model.
    pub(super) fn replace_pick_image(&mut self) {
        let Some(d) = self.replace.dialog.as_mut() else {
            return;
        };
        d.picking = true;
        let slot = SLOTS[d.slot as usize];
        self.prompt = None;
        if self.mode == Mode::Media {
            self.pick_color = true;
            self.paint_enabled = false;
        }
        self.status = format!(
            "Click the {} to pick the {slot} color; Esc returns to Replace color",
            if self.mode == Mode::Media {
                "texture"
            } else {
                "model"
            }
        );
    }
    /// Esc while picking: back to the dialog unchanged.
    pub(super) fn replace_pick_cancel(&mut self) {
        if let Some(d) = self.replace.dialog.as_mut() {
            d.picking = false;
        }
        self.pick_color = false;
        self.replace_show();
        self.status = "Pick cancelled".into();
    }
    /// Apply the dialog: every PIC of the scope in one transaction, with
    /// stored originals for first edits.
    pub(super) fn apply_replace(&mut self) -> Result<()> {
        let scope = self
            .replace
            .dialog
            .as_ref()
            .map(|d| d.scope)
            .ok_or("Replace color is not open")?;
        let from = self.replace.from.ok_or("Choose the From color")?;
        let to = self.brush;
        let mut entries = Vec::new();
        let mut counts = Vec::new();
        for (name, region) in self.replace_targets(scope)? {
            let (_, mut bytes, mut pic) = self.replace_source(&name)?;
            let set = self.replace_set(&pic, from);
            let n = pic.replace_all(&mut bytes, &set, to, region.as_deref())?;
            if n > 0 {
                entries.push(self.exact_entry(&name, bytes)?);
                counts.push((name, n));
            }
        }
        if entries.is_empty() {
            return Err(format!(
                "No pixels {} to replace with {to}; nothing changed",
                self.replace_source_text(from)
            ));
        }
        let entries = self.with_originals(entries);
        let kept: Vec<String> = entries
            .iter()
            .filter(|e| self.doc.archive.find(&e.name).is_none())
            .filter_map(|e| originals::texture_of(&e.name).map(|_| e.name.clone()))
            .collect();
        self.doc.transaction(entries, &[])?;
        self.replace.dialog = None;
        let tool = (self.paint_enabled, self.model_paint, self.selected_face);
        self.refresh();
        (self.paint_enabled, self.model_paint, self.selected_face) = tool;
        self.status = self.replace_status(&counts, from, to, &kept, 0);
        Ok(())
    }
    /// Paint inspector rows of the Replace tool: the two swatches and the
    /// tolerance.
    pub(super) fn replace_rows(
        &self,
        o: &mut Layout,
        s: &mut widgets::Stack,
        colors: &[[u8; 3]; 256],
    ) {
        if let Some([x, y, _, h]) = o.prop(s, "Replace") {
            let size = m::ICON_SM;
            let sy = y + (h - size) / 2;
            let base = baseline(y, h, Style::Value);
            let mut cx = x + 6;
            match self.replace.from {
                Some(i) => {
                    swatch(o, cx, sy, size, colors[i as usize]);
                    cx += size + space::SPACE_1;
                    let t = format!("{i}");
                    o.canvas.styled(cx, base, &t, c::INK, Style::Value);
                    cx += text_width(&t, Style::Value) + space::SPACE_2;
                }
                None => {
                    o.canvas
                        .styled(cx, base, "Alt+click", c::INK_MUTED, Style::Value);
                    cx += text_width("Alt+click", Style::Value) + space::SPACE_2;
                }
            }
            o.canvas
                .styled(cx, base, "with", c::INK_MUTED, Style::Label);
            cx += text_width("with", Style::Label) + space::SPACE_2;
            swatch(o, cx, sy, size, colors[self.brush as usize]);
            cx += size + space::SPACE_1;
            o.canvas
                .styled(cx, base, &format!("{}", self.brush), c::INK, Style::Value);
        }
        if let Some(rect) = o.prop(s, "Tolerance") {
            let t = NumberTarget::Tolerance;
            if let Some(spec) = self.number_spec(t) {
                o.number(
                    rect,
                    &Number {
                        target: t,
                        spec,
                        label: "",
                        unit: "steps",
                        locked: false,
                        axis: None,
                    },
                );
            }
        }
    }
    /// Spec of the tolerance NumberField.
    pub(super) fn tolerance_spec(&self) -> widgets::NumberSpec {
        widgets::NumberSpec {
            value: self.replace.tolerance as i64,
            disk: None,
            step: 1,
            min: 0,
            max: MAX_TOLERANCE as i64,
            decimals: 0,
            bounded: true,
            group: false,
        }
    }
    pub(super) fn tolerance_commit(&mut self, value: i64) {
        self.replace.tolerance = value.clamp(0, MAX_TOLERANCE as i64) as u8;
        self.replace_refresh();
        // A typed value returns to the dialog it was opened from.
        if self.prompt.is_none() && self.replace.dialog.is_some() {
            self.replace_show();
        }
        self.status = format!("Replace tolerance {} palette steps", self.replace.tolerance);
    }
    /// The Replace color dialog: From and To swatches, palette grid, Pick
    /// from image, tolerance, scope and the live pixel count.
    pub(super) fn replace_dialog(&self, o: &mut Layout) {
        let (Some(p), Some(d)) = (self.prompt.as_ref(), self.replace.dialog.as_ref()) else {
            return;
        };
        let colors = self
            .current_picture()
            .or_else(|| {
                self.selected_face
                    .and_then(|i| self.model_for_paint().and_then(|m| m.faces.get(i)))
                    .and_then(|f| self.texture_for(&f.texture))
            })
            .map(|p| p.colors(&self.base_palette))
            .unwrap_or(*self.base_palette);
        let w = (self.width - 48).min(560);
        let cell = ((self.height - 360) / 16).clamp(10, 18);
        let h = DIALOG_HEAD
            + space::SPACE_3
            + m::BUTTON_H
            + space::SPACE_2
            + 16 * cell
            + 3 * (space::SPACE_2 + m::BUTTON_H)
            + space::SPACE_2
            + space::SPACE_4
            + m::BUTTON_H
            + space::SPACE_4;
        let h = h.min(self.height - 24);
        let rect = [(self.width - w) / 2, (self.height - h) / 2, w, h];
        o.hits.clear();
        let [bx, mut y, bw, _] = self.dialog_frame(o, rect, &p.title);
        let label = |o: &mut Layout, y: i32, text: &str| {
            o.canvas.styled(
                bx,
                baseline(y, m::BUTTON_H, Style::Label),
                text,
                c::INK_MUTED,
                Style::Label,
            );
        };
        // From and To slots, then Pick from image.
        let pick = Btn::new("Pick from image").with_icon(Icon::Brush);
        let pw = pick.width();
        let sw = (bw - pw - 2 * space::SPACE_2) / 2;
        for k in 0..2u8 {
            let r = [bx + k as i32 * (sw + space::SPACE_2), y, sw, m::BUTTON_H];
            let on = d.slot == k;
            widgets::notched(
                &mut o.canvas,
                r,
                Some(if on { c::AMBER_DEEP } else { c::GM_950 }),
                Some(if on { c::AMBER } else { c::LINE_STRONG }),
            );
            let index = if k == 0 {
                self.replace.from
            } else {
                Some(self.brush)
            };
            let size = m::ICON_SM;
            let mut tx = r[0] + space::SPACE_2;
            if let Some(i) = index {
                swatch(
                    o,
                    tx,
                    y + (m::BUTTON_H - size) / 2,
                    size,
                    colors[i as usize],
                );
                tx += size + space::SPACE_2;
            }
            let text = match index {
                Some(i) => format!("{} \u{b7} {i}", SLOTS[k as usize]),
                None => format!("{} \u{b7} pick", SLOTS[k as usize]),
            };
            o.canvas.styled(
                tx,
                baseline(y, m::BUTTON_H, Style::Value),
                &fit(&text, r[0] + sw - tx - space::SPACE_1, Style::Value),
                if on { c::AMBER } else { c::INK },
                Style::Value,
            );
            o.hit(r, Action::ReplaceSlot(k));
        }
        o.button_ex(
            [bx + bw - pw, y, pw, m::BUTTON_H],
            pick,
            Action::ReplacePickImage,
        );
        y += m::BUTTON_H + space::SPACE_2;
        // Palette grid: From framed amber, To framed ink.
        let gx = bx + (bw - 16 * cell) / 2;
        for i in 0..256i32 {
            let xx = gx + (i % 16) * cell;
            let yy = y + (i / 16) * cell;
            o.canvas
                .rect(xx, yy, cell - 1, cell - 1, rgb(colors[i as usize]));
            if i == self.brush as i32 {
                widgets::frame(&mut o.canvas, [xx - 1, yy - 1, cell + 1, cell + 1], c::INK);
            }
            if Some(i as u8) == self.replace.from {
                widgets::frame(
                    &mut o.canvas,
                    [xx - 1, yy - 1, cell + 1, cell + 1],
                    c::AMBER,
                );
            }
            o.hit([xx, yy, cell - 1, cell - 1], Action::ReplaceSwatch(i as u8));
        }
        y += 16 * cell + space::SPACE_2;
        let third = bw / 3;
        label(o, y, "Tolerance");
        let t = NumberTarget::Tolerance;
        o.number(
            [bx + third, y + 1, bw - third, m::FIELD_H],
            &Number {
                target: t,
                spec: self.number_spec(t).unwrap_or_else(|| self.tolerance_spec()),
                label: "",
                unit: "steps",
                locked: false,
                axis: None,
            },
        );
        y += m::BUTTON_H + space::SPACE_2;
        label(o, y, "Scope");
        let scopes: Vec<(Btn, Action)> = SCOPES
            .iter()
            .enumerate()
            .map(|(k, s)| {
                (
                    Btn::new(s)
                        .on(d.scope as usize == k)
                        .enabled(d.refusals[k].is_none()),
                    Action::ReplaceScope(k as u8),
                )
            })
            .collect();
        o.segmented([bx + third, y, bw - third, m::BUTTON_H], &scopes);
        y += m::BUTTON_H + space::SPACE_2;
        let (tone, text) = match &d.plan {
            Some(Ok(list)) => {
                let total: usize = list.iter().map(|(_, n)| n).sum();
                let names: Vec<String> = list.iter().map(|(n, _)| n.clone()).collect();
                if total == 0 {
                    (
                        Tone::Neutral,
                        format!(
                            "No pixels {} in {}; nothing to replace.",
                            self.replace_source_text(self.replace.from.unwrap_or(0)),
                            names_short(&names)
                        ),
                    )
                } else {
                    (
                        Tone::Neutral,
                        format!(
                            "{} pixel{} will change in {}.",
                            grouped(total),
                            if total == 1 { "" } else { "s" },
                            names_short(&names)
                        ),
                    )
                }
            }
            Some(Err(why)) => (Tone::Warn, format!("{why}.")),
            None => (Tone::Neutral, String::new()),
        };
        widgets::notice(
            &mut o.canvas,
            bx,
            y,
            bw,
            tone,
            &fit(&text, bw - 32, Style::Label),
        );
        if let Some(error) = self.status.strip_prefix("Error: ") {
            let ey = y + m::BUTTON_H + space::SPACE_1;
            o.canvas.styled(
                bx,
                baseline(ey, m::ROW_H, Style::Label),
                &fit(error, bw / 2, Style::Label),
                c::DANGER,
                Style::Label,
            );
        }
        let count = match &d.plan {
            Some(Ok(list)) => list.iter().map(|(_, n)| n).sum(),
            _ => 0,
        };
        self.dialog_actions(
            o,
            rect,
            &[],
            Some("Cancel"),
            Some(Btn::new("Replace pixels").primary().enabled(count > 0)),
            Action::Apply,
        );
    }
}
/// Texture names for the dialog's count, kept short.
fn names_short(list: &[String]) -> String {
    match list.len() {
        0..=2 => names(list),
        n => format!("{n} textures"),
    }
}
#[cfg(not(windows))]
impl App {
    /// Snapshot workspaces: `replace` (the atlas of the first textured face
    /// with the Replace tool on its most used index), `replace-model` (the
    /// 3D brush row) and `replace-dialog` (Replace color over the atlas).
    pub fn snapshot_replace(&mut self, name: &str) -> Result<()> {
        if name == "replace-model" {
            self.mode = Mode::Model;
            self.act(Action::ModelPaint);
            self.act(Action::ReplaceTool);
            self.replace.from = Some(self.brush ^ 0x40);
            return Ok(());
        }
        self.workspace("uv")?;
        self.act(Action::ReplaceTool);
        let p = self.current_picture().ok_or("No texture to replace in")?;
        let mut counts = [0usize; 256];
        for (v, opaque) in p.pixels.iter().zip(&p.mask) {
            if *opaque {
                counts[*v as usize] += 1;
            }
        }
        let most = (0..256).max_by_key(|i| counts[*i]).unwrap_or(0) as u8;
        self.replace.from = Some(most);
        self.replace.tolerance = 2;
        if name == "replace-dialog" {
            self.act(Action::ReplaceDialog);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- smoke test

#[inline(never)]
fn replace_app() -> Box<App> {
    let mut a = Box::new(App::new());
    a.demo();
    a.width = 1280;
    a.height = 800;
    a
}
/// Screen point of atlas pixel (u, v) in the Paint workspace.
fn atlas_point(a: &App, u: i32, v: i32) -> (i32, i32) {
    let r = a.image_rect().unwrap();
    let p = a.current_picture().unwrap();
    let (w, h) = (p.width as i32, p.height as i32);
    (
        r[0] + (u * 2 + 1) * r[2] / (w * 2),
        r[1] + (v * 2 + 1) * r[3] / (h * 2),
    )
}
/// Press, drag through atlas pixels and release: one stroke.
fn atlas_drag(a: &mut App, points: &[(i32, i32)]) {
    let (x, y) = atlas_point(a, points[0].0, points[0].1);
    a.motion(x, y, false);
    a.pointer(x, y, 1, true, false);
    for (u, v) in &points[1..] {
        let (x, y) = atlas_point(a, *u, *v);
        a.motion(x, y, false);
    }
    let (u, v) = *points.last().unwrap();
    let (x, y) = atlas_point(a, u, v);
    a.pointer(x, y, 1, false, false);
}
fn alt_click(a: &mut App, x: i32, y: i32) {
    a.alt_modifier(true);
    a.smoke_press(x, y, false);
    a.alt_modifier(false);
}
fn pixels_of(a: &App, name: &str) -> Vec<u8> {
    let at = a.doc.archive.find(name).unwrap();
    Pic::parse(&a.doc.archive.entries[at].read().unwrap())
        .unwrap()
        .pixels
}
/// Every changed pixel had an index in `from` and now holds `to`; returns
/// how many changed.
fn only_matching(before: &[u8], after: &[u8], from: &[u8], to: u8) -> usize {
    let mut n = 0;
    for (b, a) in before.iter().zip(after) {
        if b != a {
            assert!(
                from.contains(b) && *a == to,
                "pixel {b} became {a}, expected only {from:?} to {to}"
            );
            n += 1;
        }
    }
    n
}
fn press(a: &mut App, predicate: &dyn Fn(Action) -> bool) {
    let r = a.smoke_find(predicate);
    a.chrome_click(r);
}
/// Type into the open prompt, replacing its value, and press Enter.
fn type_value(a: &mut App, text: &str) {
    a.key(Key::Char('a'), true, false);
    for ch in text.chars() {
        a.key(Key::Char(ch), false, false);
    }
    a.key(Key::Enter, false, false);
}
/// Hit regions inside the window and apart from each other.
fn hits_apart(a: &App, what: &str) {
    let hits = a.layout().hits;
    for (i, x) in hits.iter().enumerate() {
        let [hx, hy, hw, hh] = x.rect;
        assert!(
            hx >= 0 && hy >= 0 && hx + hw <= a.width && hy + hh <= a.height,
            "{what} control outside the window at {}x{}",
            a.width,
            a.height
        );
        for y in &hits[i + 1..] {
            let [yx, yy, yw, yh] = y.rect;
            assert!(
                hx >= yx + yw || yx >= hx + hw || hy >= yy + yh || yy >= hy + hh,
                "overlapping {what} controls at {}x{}",
                a.width,
                a.height
            );
        }
    }
}
fn dialog_open(a: &App) -> bool {
    matches!(
        a.prompt.as_ref().map(|p| &p.kind),
        Some(PromptKind::ReplaceColor)
    )
}
impl App {
    /// Replace end to end through rendered controls: the tool beside Brush
    /// and Eraser, Alt+click and Pick for the source, strokes that change
    /// only matching pixels, tolerance, the stored original, the eraser and
    /// Restore texture after a replace, the 3D brush with Panel lock and a
    /// generated panel, and Replace color in each scope with its count,
    /// apply and undo to the exact bytes, at both window sizes.
    #[inline(never)]
    pub(super) fn smoke_replace(&mut self) {
        smoke_replace_atlas();
        smoke_replace_dialog();
        smoke_replace_model();
        smoke_replace_scopes();
    }
}
#[inline(never)]
fn smoke_replace_atlas() {
    let mut a = replace_app();
    let entry = a.doc.archive.find("DEMO.PIC").unwrap();
    let saved = a.doc.archive.entries[entry].clone();
    let original = saved.read().unwrap();
    let before = Pic::parse(&original).unwrap().pixels;
    a.select_entry(entry);
    press(&mut a, &|x| matches!(x, Action::ReplaceTool));
    assert!(a.replace.on && a.paint_enabled && !a.eraser, "{}", a.status);
    assert!(a.status.contains("Alt+click"), "{}", a.status);
    // No source yet: a stroke changes nothing and says how to pick one.
    atlas_drag(&mut a, &[(2, 2), (6, 2)]);
    assert!(a.stroke.is_none() && !a.doc.dirty());
    assert!(a.status.starts_with("Alt+click the color"), "{}", a.status);
    // Alt+click picks index 16 at (5, 1) without painting.
    let (x, y) = atlas_point(&a, 5, 1);
    alt_click(&mut a, x, y);
    assert_eq!(a.replace.from, Some(16), "{}", a.status);
    assert!(!a.doc.dirty() && a.stroke.is_none());
    assert!(super::media::shows_text(&mut a, "with"));
    // A stroke over mixed pixels changes only index 16.
    a.brush = 200;
    press(&mut a, &|x| matches!(x, Action::Radius(3)));
    atlas_drag(&mut a, &[(2, 2), (8, 2), (14, 2)]);
    let after = pixels_of(&a, "DEMO.PIC");
    let n = only_matching(&before, &after, &[16], 200);
    assert!(n > 10, "{n}");
    assert_eq!(
        after[32 + 1],
        before[32 + 1],
        "index 0 under the stroke kept"
    );
    assert_eq!(
        a.status,
        format!(
            "Replaced {n} pixels of index 16 with 200 in DEMO.PIC; original kept as DEMO.ORG. One undo step."
        )
    );
    // The first edit keeps DEMO.ORG, byte for byte the saved entry.
    let org = a.doc.archive.find("DEMO.ORG").expect("stored original");
    assert!(a.doc.archive.entries[org].same_payload(&saved));
    assert_eq!(a.doc.archive.entries[org].read().unwrap(), original);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert!(a.doc.archive.find("DEMO.ORG").is_none());
    // Undo refreshes the editor and parks the tool; Replace takes it up again.
    press(&mut a, &|x| matches!(x, Action::ReplaceTool));
    assert!(a.replace.on && a.paint_enabled);
    // Tolerance through its NumberField: 18 steps adds indices 0 and 32.
    let field = a.smoke_find(&|x| matches!(x, Action::Number(NumberTarget::Tolerance)));
    a.chrome_click(field);
    assert!(matches!(
        a.prompt.as_ref().map(|p| &p.kind),
        Some(PromptKind::Number(NumberTarget::Tolerance))
    ));
    type_value(&mut a, "18");
    assert_eq!(a.replace.tolerance, 18, "{}", a.status);
    assert!(a.prompt.is_none());
    atlas_drag(&mut a, &[(2, 2), (8, 2), (14, 2)]);
    let after = pixels_of(&a, "DEMO.PIC");
    let wide = only_matching(&before, &after, &[0, 16, 32], 200);
    assert!(wide > n, "{wide} > {n}");
    assert_eq!(after[32 + 1], 200, "index 0 now within tolerance");
    assert_eq!(after[2 * 32 + 13], before[2 * 32 + 13], "index 48 kept");
    assert!(
        a.status.contains("of index 16 (tolerance 18)"),
        "{}",
        a.status
    );
    // The eraser paints the replaced pixels back from DEMO.ORG.
    press(&mut a, &|x| matches!(x, Action::Eraser));
    assert!(a.eraser && !a.replace.on);
    press(&mut a, &|x| matches!(x, Action::Radius(7)));
    atlas_drag(&mut a, &[(0, 2), (16, 2)]);
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert!(a.doc.archive.find("DEMO.ORG").is_none());
    assert_eq!(a.doc.changed_count(), 0);
    // Pick while in Replace sets the source and returns to Replace.
    press(&mut a, &|x| matches!(x, Action::ReplaceTool));
    press(&mut a, &|x| matches!(x, Action::PickColor));
    assert!(a.pick_color && !a.paint_enabled);
    let (x, y) = atlas_point(&a, 13, 1);
    a.smoke_press(x, y, false);
    assert_eq!(a.replace.from, Some(48), "{}", a.status);
    assert!(a.paint_enabled && a.replace.on && !a.pick_color);
    assert_eq!(a.doc.changed_count(), 0);
    // Restore texture after a replace: exact bytes, DEMO.ORG removed.
    press(&mut a, &|x| matches!(x, Action::Radius(3)));
    atlas_drag(&mut a, &[(13, 2), (13, 6)]);
    assert!(a.status.starts_with("Replaced"), "{}", a.status);
    press(&mut a, &|x| matches!(x, Action::RestoreTexture));
    assert_eq!(a.status, "Restored DEMO.PIC from DEMO.ORG in one undo step");
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert!(a.doc.archive.find("DEMO.ORG").is_none());
    a.act(Action::Undo);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert_eq!(a.doc.changed_count(), 0);
    // A Replace stroke Esc discards like the brush.
    press(&mut a, &|x| matches!(x, Action::ReplaceTool));
    let (x, y) = atlas_point(&a, 13, 2);
    a.pointer(x, y, 1, true, false);
    assert!(a.painting);
    a.key(Key::Escape, false, false);
    assert!(a.replace.on && a.paint_enabled && a.stroke.is_none());
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    // Brush, Eraser, Replace and Pick are one segmented control; the
    // Replace rows and the dialog button sit inside the inspector.
    for (w, h) in [(800, 600), (1280, 800)] {
        a.width = w;
        a.height = h;
        a.inspector_scroll = 0;
        assert!(super::media::shows_text(&mut a, "steps"));
        let members: [&dyn Fn(Action) -> bool; 4] = [
            &|x| matches!(x, Action::PaintToggle),
            &|x| matches!(x, Action::Eraser),
            &|x| matches!(x, Action::ReplaceTool),
            &|x| matches!(x, Action::PickColor),
        ];
        let r: Vec<[i32; 4]> = members
            .iter()
            .map(|p| a.chrome_hit(*p).expect("tool member"))
            .collect();
        for k in 1..4 {
            assert!(
                r[k][1] == r[0][1] && r[k][3] == r[0][3] && r[k - 1][0] + r[k - 1][2] == r[k][0],
                "Brush, Eraser, Replace and Pick are one control at {w}x{h}"
            );
        }
        let right = a.right();
        let controls: [&dyn Fn(Action) -> bool; 3] = [
            &|x| matches!(x, Action::Number(NumberTarget::Tolerance)),
            &|x| matches!(x, Action::ReplaceDialog),
            &|x| matches!(x, Action::PickColor),
        ];
        for p in controls {
            let [x, _, cw, _] = a.smoke_find(p);
            assert!(x > right && x + cw <= w, "inside the inspector at {w}x{h}");
        }
    }
}
#[inline(never)]
fn smoke_replace_dialog() {
    let mut a = replace_app();
    let entry = a.doc.archive.find("DEMO.PIC").unwrap();
    let original = a.doc.archive.entries[entry].read().unwrap();
    let before = Pic::parse(&original).unwrap().pixels;
    a.select_entry(entry);
    // Whole texture from the Paint button; From through the grid, To the
    // paint color.
    press(&mut a, &|x| matches!(x, Action::ReplaceDialog));
    assert!(dialog_open(&a), "{}", a.status);
    assert_eq!(a.replace.dialog.as_ref().unwrap().scope, SCOPE_WHOLE);
    assert!(super::media::shows_text(&mut a, "Choose the From color"));
    assert!(a.chrome_hit(&|x| matches!(x, Action::Apply)).is_none());
    press(&mut a, &|x| matches!(x, Action::ReplaceSwatch(32)));
    assert_eq!(a.replace.from, Some(32));
    press(&mut a, &|x| matches!(x, Action::ReplaceSlot(1)));
    press(&mut a, &|x| matches!(x, Action::ReplaceSwatch(201)));
    assert_eq!(a.brush, 201);
    assert!(super::media::shows_text(
        &mut a,
        "48 pixels will change in DEMO.PIC."
    ));
    // Selected panels needs faces; Faces' textures needs several PICs.
    assert!(a
        .chrome_hit(&|x| matches!(x, Action::ReplaceScope(1)))
        .is_none());
    assert!(a
        .chrome_hit(&|x| matches!(x, Action::ReplaceScope(2)))
        .is_none());
    // Tolerance typed from the dialog returns to it with a new count.
    press(&mut a, &|x| {
        matches!(x, Action::Number(NumberTarget::Tolerance))
    });
    type_value(&mut a, "18");
    assert!(dialog_open(&a) && a.replace.tolerance == 18, "{}", a.status);
    assert!(super::media::shows_text(
        &mut a,
        "144 pixels will change in DEMO.PIC."
    ));
    press(&mut a, &|x| {
        matches!(x, Action::Number(NumberTarget::Tolerance))
    });
    a.key(Key::Escape, false, false);
    assert!(dialog_open(&a) && a.replace.tolerance == 18);
    press(&mut a, &|x| {
        matches!(x, Action::Number(NumberTarget::Tolerance))
    });
    type_value(&mut a, "0");
    assert!(dialog_open(&a) && a.replace.tolerance == 0);
    // Pick from image: the dialog hides, Esc returns to it, a click picks.
    press(&mut a, &|x| matches!(x, Action::ReplaceSlot(0)));
    press(&mut a, &|x| matches!(x, Action::ReplacePickImage));
    assert!(a.prompt.is_none() && a.replace_picking());
    a.key(Key::Escape, false, false);
    assert!(dialog_open(&a) && !a.replace_picking());
    press(&mut a, &|x| matches!(x, Action::ReplacePickImage));
    let (x, y) = atlas_point(&a, 13, 1);
    a.smoke_press(x, y, false);
    assert!(dialog_open(&a), "{}", a.status);
    assert_eq!(a.replace.from, Some(48));
    assert!(!a.doc.dirty());
    press(&mut a, &|x| matches!(x, Action::ReplaceSwatch(32)));
    // The dialog's controls stay inside it and apart at both sizes.
    for (w, h) in [(800, 600), (1280, 800)] {
        a.width = w;
        a.height = h;
        assert!(
            a.layout().hits.len() >= 256 + 7,
            "grid, slots, pick, field, scopes, buttons"
        );
        hits_apart(&a, "Replace color");
    }
    // Apply: one undo step, the original kept, only index 32 changed.
    press(&mut a, &|x| matches!(x, Action::Apply));
    assert!(
        a.prompt.is_none() && a.replace.dialog.is_none(),
        "{}",
        a.status
    );
    assert_eq!(
        a.status,
        "Replaced 48 pixels of index 32 with 201 in DEMO.PIC; original kept as DEMO.ORG. One undo step."
    );
    assert_eq!(
        only_matching(&before, &pixels_of(&a, "DEMO.PIC"), &[32], 201),
        48
    );
    assert!(a.doc.archive.find("DEMO.ORG").is_some());
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert!(a.doc.archive.find("DEMO.ORG").is_none());
    // Selected panels: the UV footprint of the face picked for painting.
    a.select_entry(0);
    let m = a.model.clone().unwrap();
    let face = m
        .faces
        .iter()
        .position(|f| f.sub & 4 != 0 && f.uv.len() == f.indices.len() && f.uv.len() > 2)
        .unwrap();
    a.selected_face = Some(face);
    a.open_texture(entry);
    press(&mut a, &|x| matches!(x, Action::ReplaceDialog));
    press(&mut a, &|x| matches!(x, Action::ReplaceScope(1)));
    assert_eq!(a.replace.dialog.as_ref().unwrap().scope, SCOPE_PANELS);
    // The index with the most pixels inside the face's footprint.
    let region = footprint(32, 32, &[m.faces[face].uv.as_slice()]).unwrap();
    let inside_of = |v: u8| (0..1024).filter(|i| region[*i] && before[*i] == v).count();
    let target = (0..=255u8).max_by_key(|v| inside_of(*v)).unwrap();
    let inside = inside_of(target);
    assert!(inside > 0);
    press(&mut a, &|x| matches!(x, Action::ReplaceSlot(0)));
    press(
        &mut a,
        &move |x| matches!(x, Action::ReplaceSwatch(i) if i == target),
    );
    assert!(super::media::shows_text(
        &mut a,
        &format!("{inside} pixels will change in DEMO.PIC.")
    ));
    a.key(Key::Enter, false, false);
    assert!(
        a.status.starts_with(&format!("Replaced {inside} pixels")),
        "{}",
        a.status
    );
    let after = pixels_of(&a, "DEMO.PIC");
    assert_eq!(only_matching(&before, &after, &[target], 201), inside);
    assert!((0..1024).all(|i| after[i] == before[i] || region[i]));
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
    assert_eq!(a.doc.changed_count(), 0);
    // Cancel closes without changes.
    press(&mut a, &|x| matches!(x, Action::ReplaceDialog));
    press(&mut a, &|x| matches!(x, Action::Cancel));
    assert!(a.prompt.is_none() && a.replace.dialog.is_none() && !a.doc.dirty());
}
/// A root face of the shown model drawing from `texture` (empty: a flat
/// face), with its projected centre.
fn face_on(a: &App, texture: &str) -> (usize, [i32; 2]) {
    let m = a.model_for_paint().unwrap();
    (0..m.faces.len())
        .filter(|f| {
            let face = &m.faces[*f];
            face.group.is_none()
                && if texture.is_empty() {
                    face.sub & 4 == 0
                } else {
                    face.sub & 4 != 0 && texture_ui::full(&face.texture) == texture
                }
        })
        .find_map(|f| {
            let face = &m.faces[f];
            let n = face.indices.len() as i32;
            let c: [i32; 3] = core::array::from_fn(|k| {
                face.indices
                    .iter()
                    .map(|i| m.vertices[*i].point[k])
                    .sum::<i32>()
                    / n
            });
            let p = a.hp_project(c)?;
            (a.in_viewport(p[0], p[1])
                && p[1] > 130
                && a.model_hit(p[0], p[1]).map(|(g, _)| g) == Some(f))
            .then_some((f, p))
        })
        .expect("A pickable face")
}
#[inline(never)]
fn smoke_replace_model() {
    let mut a = texture_ui::texture_app();
    let original = a.doc.archive.bytes().unwrap();
    let kit = pixels_of(&a, "KIT.PIC");
    // The 3D brush row: Brush, Eraser, Replace.
    press(&mut a, &|x| matches!(x, Action::ModelPaint));
    assert!(a.model_paint);
    press(&mut a, &|x| matches!(x, Action::ReplaceTool));
    assert!(a.model_paint && a.replace.on && !a.eraser);
    let brush = a.chrome_hit(&|x| matches!(x, Action::PaintToggle)).unwrap();
    let eraser = a.chrome_hit(&|x| matches!(x, Action::Eraser)).unwrap();
    let replace = a.chrome_hit(&|x| matches!(x, Action::ReplaceTool)).unwrap();
    assert!(brush[0] + brush[2] == eraser[0] && eraser[0] + eraser[2] == replace[0]);
    // Alt+click on the model picks the texture index under the cursor.
    let (f, p) = face_on(&a, "KIT.PIC");
    let (_, uv) = a.model_hit(p[0], p[1]).unwrap();
    alt_click(&mut a, p[0], p[1]);
    let from = kit[uv[1] as usize * 32 + uv[0] as usize];
    assert_eq!(a.replace.from, Some(from), "{}", a.status);
    assert!(!a.doc.dirty());
    a.brush = from ^ 0x80;
    press(&mut a, &|x| matches!(x, Action::Radius(7)));
    let stroke = |a: &mut App, dx: &[i32]| {
        a.motion(p[0], p[1], false);
        a.pointer(p[0], p[1], 1, true, false);
        for d in dx {
            a.motion(p[0] + d, p[1], false);
        }
        a.pointer(p[0] + dx[dx.len() - 1], p[1], 1, false, false);
    };
    stroke(&mut a, &[4, 8, 60, 120]);
    assert!(a.status.starts_with("Replaced"), "{}", a.status);
    let free = only_matching(&kit, &pixels_of(&a, "KIT.PIC"), &[from], from ^ 0x80);
    assert!(free > 0);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Panel lock keeps the stroke on the face and inside its UV footprint.
    press(&mut a, &|x| matches!(x, Action::PaintLock));
    assert!(a.paint_lock);
    stroke(&mut a, &[4, 8, 60, 120]);
    let after = pixels_of(&a, "KIT.PIC");
    let locked = only_matching(&kit, &after, &[from], from ^ 0x80);
    assert!(locked > 0, "{}", a.status);
    let m = a.model_for_paint().unwrap();
    let region = footprint(32, 32, &[m.faces[f].uv.as_slice()]).unwrap();
    assert!((0..1024).all(|i| after[i] == kit[i] || region[i]));
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    press(&mut a, &|x| matches!(x, Action::PaintLock));
    // A flat panel converts only when its color matches the source.
    a.palette_override = Some(Box::new(core::array::from_fn(|i| [i as u8; 3])));
    a.refresh();
    let count = a.doc.archive.entries.len();
    let (g, q) = face_on(&a, "");
    let color = a.model_for_paint().unwrap().faces[g].color;
    a.replace.from = Some(color ^ 1);
    a.smoke_press(q[0], q[1], false);
    assert!(a.status.starts_with("Panel color"), "{}", a.status);
    assert!(!a.doc.dirty() && a.doc.archive.entries.len() == count);
    alt_click(&mut a, q[0], q[1]);
    assert_eq!(a.replace.from, Some(color));
    a.brush = color ^ 0x40;
    a.smoke_press(q[0], q[1], false);
    assert!(
        a.status.contains("1 flat panel converted to new textures")
            && a.status.ends_with("One undo step."),
        "{}",
        a.status
    );
    assert_eq!(a.doc.archive.entries.len(), count + 1, "a sheet, no .ORG");
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
#[inline(never)]
fn smoke_replace_scopes() {
    let mut a = texture_ui::texture_app();
    // Clone one face's texture so the selection spans two PICs.
    a.key(Key::Tab, false, false);
    a.key(Key::Char('3'), false, false);
    let (_, p) = face_on(&a, "KIT.PIC");
    a.smoke_press(p[0], p[1], false);
    a.smoke_menu_pick(chrome::MENU_MESH, &|x| {
        matches!(x, Action::FaceTexture(texture_ui::TEX_CLONE))
    });
    a.key(Key::Enter, false, false);
    assert!(
        a.status.starts_with("Cloned KIT.PIC to KITT1.PIC"),
        "{}",
        a.status
    );
    let (_, q) = face_on(&a, "KIT.PIC");
    a.smoke_press(q[0], q[1], true);
    let faces = a.texture_faces();
    assert_eq!(faces.len(), 2);
    assert_eq!(a.face_texture_names(&faces).len(), 2);
    let cloned = a.doc.archive.bytes().unwrap();
    let kit = pixels_of(&a, "KIT.PIC");
    let copy = pixels_of(&a, "KITT1.PIC");
    // Mesh menu: the dialog opens on the selected panels.
    a.smoke_menu_pick(chrome::MENU_MESH, &|x| matches!(x, Action::ReplaceDialog));
    assert!(dialog_open(&a), "{}", a.status);
    assert_eq!(a.replace.dialog.as_ref().unwrap().scope, SCOPE_PANELS);
    // Pick from image in Edit Mesh picks from the model, keeping the selection.
    press(&mut a, &|x| matches!(x, Action::ReplacePickImage));
    assert!(a.prompt.is_none() && a.replace_picking());
    let (_, uv) = a.model_hit(q[0], q[1]).unwrap();
    a.smoke_press(q[0], q[1], false);
    assert!(dialog_open(&a), "{}", a.status);
    assert_eq!(
        a.replace.from,
        Some(kit[uv[1] as usize * 32 + uv[0] as usize])
    );
    assert_eq!(a.texture_faces(), faces, "a pick keeps the selection");
    press(&mut a, &|x| matches!(x, Action::ReplaceSlot(0)));
    press(&mut a, &|x| matches!(x, Action::ReplaceSwatch(16)));
    press(&mut a, &|x| matches!(x, Action::ReplaceSlot(1)));
    press(&mut a, &|x| matches!(x, Action::ReplaceSwatch(201)));
    let plan = |a: &App| match &a.replace.dialog.as_ref().unwrap().plan {
        Some(Ok(list)) => list.clone(),
        other => panic!("{:?}", other.as_ref().map(|r| r.as_ref().err())),
    };
    let panels = plan(&a);
    assert_eq!(panels.len(), 2, "one footprint per PIC");
    // Faces' textures: every index-16 pixel of both PICs, 32 each.
    press(&mut a, &|x| matches!(x, Action::ReplaceScope(2)));
    assert_eq!(plan(&a).iter().map(|(_, n)| n).sum::<usize>(), 64);
    assert!(super::media::shows_text(&mut a, "64 pixels will change in"));
    for (w, h) in [(800, 600), (1280, 800)] {
        a.width = w;
        a.height = h;
        hits_apart(&a, "Replace color");
    }
    press(&mut a, &|x| matches!(x, Action::Apply));
    assert!(
        a.status
            .starts_with("Replaced 64 pixels of index 16 with 201 in"),
        "{}",
        a.status
    );
    assert_eq!(
        only_matching(&kit, &pixels_of(&a, "KIT.PIC"), &[16], 201),
        32
    );
    assert_eq!(
        only_matching(&copy, &pixels_of(&a, "KITT1.PIC"), &[16], 201),
        32
    );
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), cloned, "one undo step");
    // Selected panels: each PIC changes only inside its faces' footprint.
    a.smoke_menu_pick(chrome::MENU_MESH, &|x| matches!(x, Action::ReplaceDialog));
    let expected: usize = panels.iter().map(|(_, n)| n).sum();
    a.key(Key::Enter, false, false);
    if expected == 0 {
        assert!(a.status.contains("nothing changed"), "{}", a.status);
        a.key(Key::Escape, false, false);
    } else {
        assert!(a.status.starts_with("Replaced"), "{}", a.status);
        let changed = only_matching(&kit, &pixels_of(&a, "KIT.PIC"), &[16], 201)
            + only_matching(&copy, &pixels_of(&a, "KITT1.PIC"), &[16], 201);
        assert_eq!(changed, expected);
        a.act(Action::Undo);
    }
    assert_eq!(a.doc.archive.bytes().unwrap(), cloned);
    assert!(a.prompt.is_none());
}

// ---------------------------------------------------------------- real-data check

#[cfg(not(windows))]
impl App {
    /// Manual real-data check (`--replace-check`): on the shape's main PIC,
    /// Replace color in the whole texture and then in the UV footprint of
    /// the vertical tail's faces (Selected panels, tolerance 4). Each apply
    /// must change only matching pixels (raster bytes only), keep X.ORG byte
    /// for byte, undo and redo exactly, and Restore texture must return the
    /// saved bytes. Writes before/after PNGs and two create-new LIBs to `out`.
    pub fn check_real_replace(&mut self, shape: &str, out: &str) -> Result<String> {
        let mut report = String::new();
        let sh = self.doc.archive.find(shape).ok_or("SH not found")?;
        self.select_entry(sh);
        let model = self.model.clone().ok_or("Not a decodable SH")?;
        let mut uses: Vec<(String, usize)> = Vec::new();
        for f in model
            .faces
            .iter()
            .filter(|f| f.sub & 4 != 0 && !f.texture.is_empty())
        {
            let n = texture_ui::full(&f.texture);
            match uses.iter_mut().find(|(x, _)| *x == n) {
                Some((_, k)) => *k += 1,
                None => uses.push((n, 1)),
            }
        }
        uses.sort_unstable_by(|a, b| b.1.cmp(&a.1));
        let name = uses.first().ok_or("No textured faces")?.0.clone();
        let org = originals::companion(&name).ok_or("Not a PIC")?;
        if self.doc.archive.find(&org).is_some() {
            return Err(format!(
                "{org} already exists; use a LIB without stored originals"
            ));
        }
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let saved = self.doc.archive.entries[at].clone();
        let original = saved.read()?;
        let p0 = Pic::parse(&original)?;
        let colors = p0.colors(&self.base_palette);
        let png = |app: &App, file: &str| -> Result<String> {
            let at = app.doc.archive.find(&name).ok_or("Texture missing")?;
            let pic = Pic::parse(&app.doc.archive.entries[at].read()?)?;
            let path = format!("{out}/{file}");
            crate::platform::write_new(&path, &pic.png(&app.base_palette))?;
            Ok(path)
        };
        report += &format!(
            "{name}: {} x {}, {}, palette {}; {} of {} faces of {shape} draw from it\n",
            p0.width,
            p0.height,
            if p0.mask.iter().all(|m| *m) {
                "raw"
            } else {
                "span-coded"
            },
            if self.palette_loaded {
                "PALETTE.PAL"
            } else {
                "missing"
            },
            uses[0].1,
            model.faces.len()
        );
        report += &format!("  before: {}\n", png(self, "before.png")?);
        // The farthest color from `from`, so the change is visible.
        let vivid = |from: u8| -> u8 {
            let d = |i: usize| -> i32 {
                (0..3)
                    .map(|k| (colors[i][k] as i32 - colors[from as usize][k] as i32).pow(2))
                    .sum()
            };
            (0..256).max_by_key(|i| d(*i)).unwrap_or(0) as u8
        };
        // Only raster bytes change, each from a matching index to `to`.
        let verify = |before: &[u8],
                      after: &[u8],
                      set: &IndexSet,
                      to: u8,
                      region: Option<&[bool]>|
         -> Result<usize> {
            let (a, b) = (Pic::parse(before)?, Pic::parse(after)?);
            if before.len() != after.len() || before[..64] != after[..64] || a.mask != b.mask {
                return Err("Header, size or transparency changed".into());
            }
            let mut pixels = 0;
            for i in 0..a.pixels.len() {
                if a.pixels[i] != b.pixels[i] {
                    if !set[a.pixels[i] as usize]
                        || b.pixels[i] != to
                        || region.is_some_and(|r| !r[i])
                    {
                        return Err(format!("Pixel {i} changed outside the match"));
                    }
                    pixels += 1;
                } else if set[a.pixels[i] as usize]
                    && a.mask[i]
                    && region.is_none_or(|r| r[i])
                    && a.pixels[i] != to
                {
                    return Err(format!("Matching pixel {i} was left"));
                }
            }
            let bytes = before.iter().zip(after).filter(|(x, y)| x != y).count();
            if bytes != pixels {
                return Err(format!("{bytes} bytes changed for {pixels} pixels"));
            }
            Ok(pixels)
        };
        let plan_total = |app: &App| -> usize {
            match app.replace.dialog.as_ref().and_then(|d| d.plan.as_ref()) {
                Some(Ok(list)) => list.iter().map(|(_, n)| n).sum(),
                _ => 0,
            }
        };
        // 1. Whole texture, tolerance 0: the most used opaque index.
        let mut counts = [0usize; 256];
        for (v, m) in p0.pixels.iter().zip(&p0.mask) {
            if *m {
                counts[*v as usize] += 1;
            }
        }
        let from = (0..256).max_by_key(|i| counts[*i]).unwrap_or(0) as u8;
        let to = vivid(from);
        self.select_entry(at);
        self.replace.tolerance = 0;
        self.act(Action::ReplaceDialog);
        if self.replace.dialog.is_none() {
            return Err(self.status.clone());
        }
        self.act(Action::ReplaceSlot(0));
        self.act(Action::ReplaceSwatch(from));
        self.act(Action::ReplaceSlot(1));
        self.act(Action::ReplaceSwatch(to));
        let planned = plan_total(self);
        self.key(Key::Enter, false, false);
        if !self.status.starts_with("Replaced") {
            return Err(self.status.clone());
        }
        let status = self.status.clone();
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let whole = self.doc.archive.entries[at].read()?;
        let set = matching(&colors, from, 0);
        let changed = verify(&original, &whole, &set, to, None)?;
        let kept = originals::backup(&self.doc.archive, &name)
            .ok_or("No stored original kept")?
            .clone();
        let org_exact =
            kept.read()? == original && kept.same_payload(&saved) && kept.flag() == saved.flag();
        report += &format!(
            "  whole texture: index {from} -> {to}, tolerance 0: {changed} pixels ({} counted in the dialog, {} of that index); raster bytes only\n  status: {status}\n  {org}: byte-identical to the saved {name} (flag {}, {} B stored): {org_exact}\n",
            planned,
            counts[from as usize],
            kept.flag(),
            kept.stored_len()
        );
        if changed != planned || changed != counts[from as usize] || !org_exact {
            return Err(format!("Whole-texture replace failed\n{report}"));
        }
        report += &format!("  after: {}\n", png(self, "whole.png")?);
        let whole_lib = self.doc.archive.bytes()?;
        self.act(Action::Undo);
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let undone = self.doc.archive.entries[at].read()? == original
            && self.doc.archive.find(&org).is_none()
            && self.doc.changed_count() == 0;
        self.act(Action::Redo);
        let redone = self.doc.archive.bytes()? == whole_lib;
        self.select_entry(self.doc.archive.find(&name).ok_or("Texture missing")?);
        self.act(Action::RestoreTexture);
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let restored = self.doc.archive.entries[at].same_storage(&saved)
            && self.doc.archive.find(&org).is_none()
            && self.doc.changed_count() == 0;
        let restore_status = self.status.clone();
        self.act(Action::Undo);
        let reapplied = self.doc.archive.bytes()? == whole_lib;
        report += &format!(
            "  undo exact and {org} removed: {undone}; redo exact: {redone}; Restore texture ({restore_status}) exact and clean: {restored}; its undo exact: {reapplied}\n"
        );
        if !undone || !redone || !restored || !reapplied {
            return Err(format!("Undo or Restore failed\n{report}"));
        }
        let path = format!("{out}/WHOLE.LIB");
        crate::platform::write_new(&path, &whole_lib)?;
        report += &format!("  wrote {path}\n");
        self.act(Action::Undo);
        if self.doc.changed_count() != 0 {
            return Err("Undo did not return to the opened LIB".into());
        }
        // 2. Selected panels: the vertical tail's faces, tolerance 4.
        let sh = self.doc.archive.find(shape).ok_or("SH not found")?;
        self.select_entry(sh);
        self.mode = Mode::Model;
        self.textured = true;
        if !self.mesh_edit {
            self.act(Action::MeshMode);
        }
        self.select_mode(true);
        let m = self.model.clone().ok_or("No model")?;
        let (aft, fore) = m.vertices.iter().fold((i32::MAX, i32::MIN), |(a, b), v| {
            (a.min(v.point[1]), b.max(v.point[1]))
        });
        let tail = aft + (fore - aft) * 3 / 10;
        let centre = |f: &model::Face, k: usize| -> i32 {
            f.indices
                .iter()
                .map(|i| m.vertices[*i].point[k])
                .sum::<i32>()
                / f.indices.len().max(1) as i32
        };
        let top = |f: &model::Face| -> i32 {
            f.indices
                .iter()
                .map(|i| m.vertices[*i].point[2])
                .max()
                .unwrap_or(0)
        };
        // Tail: side-facing faces in the rear 30%, the eight reaching highest.
        let mut ranked: Vec<(i32, usize)> = m
            .faces
            .iter()
            .filter(|f| f.sub & 4 != 0 && texture_ui::full(&f.texture) == name)
            .filter(|f| f.uv.len() == f.indices.len())
            .filter(|f| f.normal.is_some_and(|n| n[0].abs() >= 30000))
            .filter(|f| centre(f, 1) <= tail)
            .map(|f| (top(f), f.offset))
            .collect();
        ranked.sort_unstable_by(|a, b| b.cmp(a));
        let fins: Vec<usize> = ranked.iter().take(8).map(|(_, o)| *o).collect();
        if fins.is_empty() {
            return Err("No side-facing textured tail faces".into());
        }
        self.ed.mesh_faces = fins.clone();
        self.sync_face_vertices();
        let polys: Vec<&[[i32; 2]]> = m
            .faces
            .iter()
            .filter(|f| fins.contains(&f.offset))
            .map(|f| f.uv.as_slice())
            .collect();
        let region = footprint(p0.width, p0.height, &polys)?;
        let mut inside = [0usize; 256];
        for i in 0..region.len() {
            if region[i] && p0.mask[i] {
                inside[p0.pixels[i] as usize] += 1;
            }
        }
        let from = (0..256).max_by_key(|i| inside[*i]).unwrap_or(0) as u8;
        let to = vivid(from);
        let tolerance = 4;
        let set = matching(&colors, from, tolerance);
        let similar = (0..256).filter(|i| set[*i]).count();
        let covered = region.iter().filter(|r| **r).count();
        self.replace.tolerance = tolerance;
        self.act(Action::ReplaceDialog);
        if self.replace.dialog.as_ref().map(|d| d.scope) != Some(SCOPE_PANELS) {
            return Err(format!(
                "Dialog did not open on the selected panels: {}",
                self.status
            ));
        }
        self.act(Action::ReplaceSlot(0));
        self.act(Action::ReplaceSwatch(from));
        self.act(Action::ReplaceSlot(1));
        self.act(Action::ReplaceSwatch(to));
        let planned = plan_total(self);
        self.key(Key::Enter, false, false);
        if !self.status.starts_with("Replaced") {
            return Err(self.status.clone());
        }
        let status = self.status.clone();
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let fin = self.doc.archive.entries[at].read()?;
        let changed = verify(&original, &fin, &set, to, Some(&region))?;
        let outside = (0..region.len())
            .filter(|i| !region[*i] && set[p0.pixels[*i] as usize] && p0.mask[*i])
            .count();
        let kept = originals::backup(&self.doc.archive, &name)
            .ok_or("No stored original kept")?
            .clone();
        let org_exact = kept.read()? == original && kept.flag() == saved.flag();
        report += &format!(
            "  tail panels: {} faces, footprint {covered} pixels; index {from} -> {to}, tolerance {tolerance} ({similar} indices): {changed} pixels ({planned} counted), {outside} matching pixels outside the footprint untouched\n  status: {status}\n  {org} byte-identical: {org_exact}\n",
            fins.len()
        );
        if changed != planned || changed == 0 || !org_exact {
            return Err(format!("Panel replace failed\n{report}"));
        }
        report += &format!("  after: {}\n", png(self, "fin.png")?);
        let fin_lib = self.doc.archive.bytes()?;
        self.act(Action::Undo);
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let undone = self.doc.archive.entries[at].read()? == original
            && self.doc.archive.find(&org).is_none()
            && self.doc.changed_count() == 0;
        self.act(Action::Redo);
        let redone = self.doc.archive.bytes()? == fin_lib;
        report += &format!("  undo exact: {undone}; redo exact: {redone}\n");
        if !undone || !redone {
            return Err(format!("Panel undo failed\n{report}"));
        }
        let reopened = Archive::parse(fin_lib.clone())?;
        let r = reopened
            .find(&org)
            .ok_or("Stored original lost on repack")?;
        if reopened.entries[r].read()? != original {
            return Err("Stored original changed on repack".into());
        }
        let path = format!("{out}/FIN.LIB");
        crate::platform::write_new(&path, &fin_lib)?;
        report += &format!("  wrote {path}; reopened, {org} intact\n");
        // 3. The 3D Replace brush on a tail face, then the eraser over it.
        self.act(Action::Undo);
        if self.doc.changed_count() != 0 {
            return Err("Undo did not return to the opened LIB".into());
        }
        if self.mesh_edit {
            self.act(Action::MeshMode);
        }
        self.act(Action::ModelPaint);
        self.act(Action::ReplaceTool);
        let face = m
            .faces
            .iter()
            .position(|f| f.offset == fins[0])
            .ok_or("Tail face lost")?;
        let uv = m.faces[face].uv.clone();
        let n = uv.len().max(1) as i32;
        let point = [
            uv.iter().map(|q| q[0]).sum::<i32>() / n,
            p0.height as i32 - 1 - uv.iter().map(|q| q[1]).sum::<i32>() / n,
        ];
        let from = p0.pixels[point[1] as usize * p0.width + point[0] as usize];
        self.replace.from = Some(from);
        self.replace.tolerance = 0;
        self.brush = vivid(from);
        self.brush_radius = 7;
        self.selected_face = Some(face);
        self.paint_model_hit(face, point);
        if !self.painting {
            return Err(self.status.clone());
        }
        self.finish_stroke();
        let status = self.status.clone();
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let brushed = self.doc.archive.entries[at].read()?;
        let set = matching(&colors, from, 0);
        let (a, b) = (Pic::parse(&original)?, Pic::parse(&brushed)?);
        let mut dab = 0;
        for i in 0..a.pixels.len() {
            if a.pixels[i] != b.pixels[i] {
                if !set[a.pixels[i] as usize] || b.pixels[i] != self.brush {
                    return Err(format!("3D Replace changed pixel {i} outside the match"));
                }
                dab += 1;
            }
        }
        self.eraser = true;
        self.replace.on = false;
        self.selected_face = Some(face);
        self.paint_model_hit(face, point);
        self.finish_stroke();
        self.eraser = false;
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let erased = self.doc.archive.entries[at].read()? == original
            && self.doc.archive.find(&org).is_none()
            && self.doc.changed_count() == 0;
        report += &format!(
            "  3D Replace brush (15 px) on a tail face: {dab} pixels of index {from}, only matching pixels\n  status: {status}\n  Eraser over the same dab returned the saved bytes and dropped {org}: {erased}\n"
        );
        if dab == 0 || !erased {
            return Err(format!("3D Replace or eraser failed\n{report}"));
        }
        Ok(report)
    }
}
