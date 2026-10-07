use super::view::{border, label_fit, Action, Icon, Layout};
use super::*;
use hangar_core::originals;
struct Frame {
    w: usize,
    h: usize,
    pixels: Vec<u32>,
    faces: Vec<usize>,
    uv: Vec<[i32; 2]>,
}
fn rgb(p: [u8; 3]) -> u32 {
    (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32
}
fn blit(o: &mut Layout, rect: [i32; 4], pixels: &[u32], sw: usize, sh: usize) {
    let [x, y, w, h] = rect;
    if w <= 0 || h <= 0 {
        return;
    }
    let mut out = Vec::with_capacity(w as usize * h as usize);
    for yy in 0..h as usize {
        for xx in 0..w as usize {
            out.push(pixels[(yy * sh / h as usize) * sw + xx * sw / w as usize]);
        }
    }
    o.canvas
        .commands
        .push(Draw::Bitmap(x, y, w as usize, h as usize, out));
}
impl App {
    pub(super) fn current_picture(&self) -> Option<&Pic> {
        self.decal_draft
            .as_ref()
            .filter(|d| d.entry == self.selected)
            .map(|d| &d.pic)
            .or_else(|| {
                self.stroke
                    .as_ref()
                    .filter(|s| s.entry == self.selected)
                    .map(|s| &s.pic)
                    .or(self.pic.as_deref())
            })
    }
    pub(super) fn image_rect(&self) -> Option<[i32; 4]> {
        let p = self.current_picture()?;
        let l = self.left();
        let w = self.right() - l - 36;
        let h = self.dock_y() - 110;
        let scale = (w * 1000 / p.width as i32)
            .min(h * 1000 / p.height as i32)
            .max(1);
        let scale = scale * self.image_zoom / 100;
        let iw = p.width as i32 * scale / 1000;
        let ih = p.height as i32 * scale / 1000;
        Some([
            l + 18 + (w - iw) / 2 + self.image_pan[0],
            84 + (h - ih) / 2 + self.image_pan[1],
            iw.max(1),
            ih.max(1),
        ])
    }
    pub(super) fn image_point(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        if x < self.left() + 12 || x >= self.right() - 12 || y < 84 || y >= self.dock_y() - 20 {
            return None;
        }
        let r = self.image_rect()?;
        let p = self.current_picture()?;
        if x < r[0] || y < r[1] || x >= r[0] + r[2] || y >= r[1] + r[3] {
            return None;
        }
        Some((
            ((x - r[0]) * p.width as i32 / r[2]) as usize,
            ((y - r[1]) * p.height as i32 / r[3]) as usize,
        ))
    }
    fn paint_at(&mut self, entry: usize, x: usize, y: usize) {
        let replace = !self.eraser && self.replace.on;
        if replace && self.replace.from.is_none() {
            self.status = "Alt+click the color to replace first, or use Pick".into();
            return;
        }
        if self.stroke.as_ref().is_none_or(|s| s.entry != entry)
            && !self.stroke_parked.iter().any(|s| s.entry == entry)
            && self.stroke_parked.len() + usize::from(self.stroke.is_some()) >= 64
        {
            self.status = "Stroke limit: release to commit before painting more textures".into();
            return;
        }
        if self.stroke.as_ref().is_some_and(|s| s.entry != entry) {
            let mut previous = self.stroke.take().unwrap();
            previous.last = None;
            self.stroke_parked.push(*previous);
        }
        if self.stroke.is_none() {
            if let Some(at) = self.stroke_parked.iter().position(|s| s.entry == entry) {
                self.stroke = Some(Box::new(self.stroke_parked.remove(at)));
            }
        }
        if self.stroke.is_none() {
            let r: Result<Stroke> = (|| {
                let name = self.doc.archive.entries[entry].name.clone();
                if originals::texture_of(&name).is_some() {
                    return Err(format!(
                        "{name} is a stored original and read-only; paint {} instead",
                        originals::texture_of(&name).unwrap()
                    ));
                }
                let bytes = self.doc.archive.entries[entry].read()?;
                let pic = Pic::parse(&bytes)?;
                if !pic.paintable {
                    return Err("This PIC uses aliased storage; painting disabled".into());
                }
                if !self.palette_loaded && pic.palette.len() != 256 {
                    return Err(
                        "Load the base .PAL before painting this partial-palette PIC".into(),
                    );
                }
                let erase = if self.eraser {
                    Some(self.eraser_source(&name, &pic)?)
                } else {
                    None
                };
                Ok(Stroke {
                    entry,
                    name,
                    original: Some(self.doc.archive.entries[entry].clone()),
                    bytes,
                    pic,
                    last: None,
                    erase,
                    replace: None,
                })
            })();
            match r {
                Ok(s) => self.stroke = Some(Box::new(s)),
                Err(e) => {
                    // Eraser messages ("No stored original for X.PIC") read as status.
                    self.status = if self.eraser {
                        e
                    } else {
                        format!("Error: {e}")
                    };
                    return;
                }
            }
        }
        if self.eraser && self.stroke.as_ref().is_some_and(|s| s.erase.is_none()) {
            let s = self.stroke.as_ref().unwrap();
            match self.eraser_source(&s.name, &s.pic) {
                Ok(source) => self.stroke.as_mut().unwrap().erase = Some(source),
                Err(e) => {
                    self.status = e;
                    return;
                }
            }
        }
        if replace {
            let s = self.stroke.as_ref().unwrap();
            if s.replace.is_none() {
                match self.replace_stroke(&s.pic) {
                    Ok(r) => self.stroke.as_mut().unwrap().replace = Some(r),
                    Err(e) => {
                        self.status = e;
                        return;
                    }
                }
            }
            // Panel lock keeps a Replace stroke inside the locked face's UVs.
            let s = self.stroke.as_ref().unwrap();
            let locked = (self.mode == Mode::Model && self.model_paint && self.paint_lock)
                .then_some(self.selected_face)
                .flatten();
            let current = s
                .replace
                .as_ref()
                .and_then(|r| r.lock.as_ref().map(|l| l.0));
            if locked != current {
                let lock = self.replace_lock(&s.name, &s.pic);
                if let Some(r) = self.stroke.as_mut().unwrap().replace.as_mut() {
                    r.lock = lock;
                }
            }
        }
        self.painting = true;
        let eraser = self.eraser;
        let s = self.stroke.as_mut().unwrap();
        let (px, py) = s.last.unwrap_or((x, y));
        let steps = x.abs_diff(px).max(y.abs_diff(py)).max(1);
        for step in 0..=steps {
            let xx = (px as i64 + (x as i64 - px as i64) * step as i64 / steps as i64) as usize;
            let yy = (py as i64 + (y as i64 - py as i64) * step as i64 / steps as i64) as usize;
            let painted = match (
                s.erase.as_deref().filter(|_| eraser),
                s.replace.as_deref_mut(),
            ) {
                (Some(source), _) => {
                    s.pic
                        .paint_from(&mut s.bytes, xx, yy, self.brush_radius, source)
                }
                (None, Some(r)) if replace => {
                    let region = r.lock.as_ref().map(|l| l.1.as_slice());
                    let n = s.pic.replace(
                        &mut s.bytes,
                        xx,
                        yy,
                        self.brush_radius,
                        &r.set,
                        r.to,
                        region,
                    );
                    if let Ok(n) = n {
                        r.count += n;
                    }
                    n
                }
                _ => s
                    .pic
                    .paint(&mut s.bytes, xx, yy, self.brush_radius, self.brush),
            };
            if let Err(e) = painted {
                s.last = None;
                self.status = format!("Error: {e}");
                return;
            }
        }
        s.last = Some((x, y));
        self.status = if eraser {
            format!(
                "Erasing {} to its original; release to commit one undo step",
                s.name
            )
        } else if let Some(r) = s.replace.as_deref().filter(|_| replace) {
            format!(
                "Replacing index {} with {} in {}; release to commit one undo step",
                r.from, r.to, s.name
            )
        } else {
            format!(
                "Painting index {}; release to commit one undo step",
                self.brush
            )
        };
    }
    /// Eraser target pixels for `name`, matching `current`'s layout: the stored
    /// original first, then a generated panel's face color, then the saved entry.
    pub(super) fn eraser_source(&self, name: &str, current: &Pic) -> Result<Box<Pic>> {
        let mismatch = |from: &str| {
            format!("{from} has a different raster layout than {name}; use Restore texture")
        };
        if let Some(org) = originals::backup(&self.doc.archive, name) {
            let pic = Box::new(Pic::parse(&org.read()?)?);
            if !pic.same_layout(current) {
                return Err(mismatch(&org.name));
            }
            return Ok(pic);
        }
        if let Some(color) = self.panel_color(name) {
            let mut pic = Box::new(current.clone());
            pic.pixels.fill(color);
            return Ok(pic);
        }
        let current_entry = self
            .doc
            .archive
            .find(name)
            .map(|i| &self.doc.archive.entries[i]);
        if let Some(saved) = self
            .doc
            .saved_entry(name)
            .filter(|s| current_entry.is_none_or(|e| !s.same_storage(e)))
        {
            let pic = Box::new(Pic::parse(&saved.read()?)?);
            if !pic.same_layout(current) {
                return Err(mismatch("The saved entry"));
            }
            return Ok(pic);
        }
        Err(format!("No stored original for {name}"))
    }
    pub(super) fn paint_point(&mut self, x: i32, y: i32) {
        let Some((px, py)) = self.image_point(x, y) else {
            if let Some(s) = &mut self.stroke {
                s.last = None;
            }
            return;
        };
        if self.atlas_picks() {
            if let Some(p) = self.current_picture() {
                let i = py * p.width + px;
                let index = p.mask[i].then(|| p.pixels[i]);
                self.color_picked(index);
            }
            return;
        }
        if self.paint_enabled {
            self.paint_at(self.selected, px, py);
        }
    }
    pub(super) fn finish_stroke(&mut self) {
        if let Some(s) = self
            .stroke
            .take()
            .map(|b| *b)
            .or_else(|| self.stroke_parked.pop())
        {
            let enabled = self.paint_enabled;
            let model_paint = self.model_paint;
            let face = self.selected_face;
            let mut strokes = core::mem::take(&mut self.stroke_parked);
            strokes.push(s);
            let plan = self.panel_draft.take();
            let generated = strokes.iter().filter(|s| s.original.is_none()).count();
            // A Replace stroke reports its pixels, indices and textures.
            let replaced = strokes
                .iter()
                .find_map(|s| s.replace.as_ref().map(|r| (r.from, r.to)));
            let counts: Vec<(String, usize)> = strokes
                .iter()
                .filter_map(|s| s.replace.as_ref().map(|r| (s.name.clone(), r.count)))
                .filter(|(_, n)| *n > 0)
                .collect();
            let r: Result<Option<Vec<String>>> = (|| {
                let mut entries = Vec::new();
                let mut removals = Vec::new();
                for s in strokes {
                    if let Some(original) = &s.original {
                        if !self
                            .doc
                            .archive
                            .entries
                            .get(s.entry)
                            .is_some_and(|e| e.same_storage(original))
                        {
                            return Err("Paint source changed; stroke cancelled".into());
                        }
                        // Erasing back to a known state reuses its exact storage.
                        if original.read()? == s.bytes {
                            continue;
                        }
                        // Fully erased back to the saved bytes: a backup added this
                        // session is no longer needed, so the LIB reads as unchanged.
                        if let Some(org) = originals::backup(&self.doc.archive, &s.name) {
                            if self.doc.saved_entry(&org.name).is_none()
                                && self
                                    .doc
                                    .saved_entry(&s.name)
                                    .is_some_and(|saved| saved.same_payload(org))
                                && org.read()? == s.bytes
                            {
                                removals.push(org.name.clone());
                            }
                        }
                    } else if self.doc.archive.find(&s.name).is_some() {
                        return Err("Generated texture name changed; stroke cancelled".into());
                    }
                    entries.push(self.exact_entry(&s.name, s.bytes)?);
                }
                if let Some(plan) = plan {
                    if !self
                        .doc
                        .archive
                        .entries
                        .get(plan.entry)
                        .is_some_and(|e| e.same_storage(&plan.original))
                    {
                        return Err("Panel shape changed; stroke cancelled".into());
                    }
                    entries.push(Entry::new(&plan.original.name, plan.shape)?);
                }
                if entries.is_empty() {
                    return Ok(None);
                }
                let entries = self.with_originals(entries);
                let kept: Vec<_> = entries
                    .iter()
                    .filter(|e| self.doc.archive.find(&e.name).is_none())
                    .filter_map(|e| originals::texture_of(&e.name).map(|_| e.name.clone()))
                    .collect();
                self.doc.transaction(entries, &removals)?;
                Ok(Some(kept))
            })();
            self.painting = false;
            self.refresh();
            self.paint_enabled = enabled;
            self.model_paint = model_paint;
            self.selected_face = face;
            match (r, replaced) {
                (Ok(None), Some((from, _))) => {
                    self.status = format!(
                        "No pixels {} under the stroke; nothing changed",
                        self.replace_source_text(from)
                    )
                }
                (Ok(Some(kept)), Some((from, to))) => {
                    self.status = self.replace_status(&counts, from, to, &kept, generated)
                }
                (Ok(None), None) => self.status = "The stroke changed no pixels".into(),
                (Ok(Some(kept)), None) => {
                    let mut status = String::from("Paint stroke applied");
                    if generated > 0 {
                        status.push_str(&format!(
                            "; {generated} flat panel{} converted to new textures",
                            if generated == 1 { "" } else { "s" }
                        ));
                    }
                    if !kept.is_empty() {
                        status.push_str(&format!("; original kept as {}", kept.join(", ")));
                    }
                    status.push_str(". Ctrl+Z undoes it");
                    self.status = status;
                }
                (Err(e), _) => self.status = format!("Error: {e}"),
            }
        }
    }
    pub(super) fn open_texture(&mut self, index: usize) {
        let m = self
            .model
            .as_ref()
            .or(self.context_model.as_deref())
            .cloned();
        let entry = self.model_entry.or(self.context_entry);
        let face = self.selected_face;
        // The selected panels stay selected on the opened texture's model.
        let panels = core::mem::take(&mut self.ed.mesh_faces);
        self.select_entry(index);
        self.context_model = m.map(Box::new);
        self.context_entry = entry;
        self.selected_face = face;
        self.ed.mesh_faces = panels;
        self.refresh();
        self.mode = Mode::Media;
        self.dock = 3;
    }
    pub(super) fn model_for_paint(&self) -> Option<&Model> {
        self.preview
            .as_deref()
            .or(self.model.as_ref())
            .or(self.context_model.as_deref())
    }
    pub(super) fn texture_for(&self, name: &str) -> Option<&Pic> {
        let full = if name.contains('.') {
            name.to_string()
        } else {
            format!("{name}.PIC")
        };
        self.decal_draft
            .as_ref()
            .filter(|d| {
                self.doc.archive.entries[d.entry]
                    .name
                    .eq_ignore_ascii_case(&full)
            })
            .map(|d| &d.pic)
            .or_else(|| {
                self.stroke
                    .as_ref()
                    .filter(|s| s.name.eq_ignore_ascii_case(&full))
                    .map(|s| &s.pic)
                    .or_else(|| {
                        self.stroke_parked
                            .iter()
                            .find(|s| s.name.eq_ignore_ascii_case(&full))
                            .map(|s| &s.pic)
                    })
                    .or_else(|| self.textures.get(name))
            })
    }
    fn render_model(&self, w: usize, h: usize) -> Frame {
        let mut frame = Frame {
            w,
            h,
            pixels: vec![c::GM_950.0; w * h],
            faces: vec![usize::MAX; w * h],
            uv: vec![[0, 0]; w * h],
        };
        let Some(m) = self.model_for_paint() else {
            return frame;
        };
        // Frame on the committed model, as the wireframe and overlays do, so previews move.
        let (center, span) = self
            .model_bounds()
            .unwrap_or_else(|| super::hardpoint_ui::bounds(m));
        let points: Vec<[i32; 3]> = m
            .vertices
            .iter()
            .map(|v| {
                // Fixed-point camera/depth plus 1/16-pixel raster positions.
                let p = self.camera_point(core::array::from_fn(|i| (v.point[i] - center[i]) * 256));
                [
                    w as i32 * 8
                        + (p[0] as i64 * w.min(h) as i64 * self.zoom as i64 * 48
                            / (span as i64 * 400 * 256)) as i32
                        + self.pan[0] * 16 * w as i32 / (self.right() - self.left()),
                    h as i32 * 8
                        - (p[1] as i64 * w.min(h) as i64 * self.zoom as i64 * 48
                            / (span as i64 * 400 * 256)) as i32
                        + self.pan[1] * 16 * h as i32 / (self.dock_y() - 54),
                    p[2],
                ]
            })
            .collect();
        let mut depth = vec![i64::MIN; w * h];
        let edge = |a: [i32; 3], b: [i32; 3], x: i32, y: i32| {
            (x as i64 - a[0] as i64) * (b[1] as i64 - a[1] as i64)
                - (y as i64 - a[1] as i64) * (b[0] as i64 - a[0] as i64)
        };
        for (fi, f) in m.faces.iter().enumerate() {
            if f.normal.is_some_and(|n| self.camera_point(n)[2] < 0) {
                continue;
            }
            // E0 refers to runtime markings; an unconfigured keyed overlay is blank,
            // never the previous aircraft atlas or an opaque header-color polygon.
            if f.sub & 4 != 0 && f.sub & 3 == 0 && f.texture.is_empty() {
                continue;
            }
            let flat = self.flat && self.mode == Mode::Model && !self.model_paint;
            let texture = if flat {
                None
            } else {
                self.texture_for(&f.texture)
            };
            let colors = texture.map(|p| p.colors(&self.base_palette));
            let light = if flat {
                super::chrome::light(self, m, f)
            } else {
                256
            };
            for j in 1..f.indices.len() - 1 {
                let ids = [0, j, j + 1];
                let tri = ids.map(|i| points[f.indices[i]]);
                let area = edge(tri[0], tri[1], tri[2][0], tri[2][1]);
                if area == 0 {
                    continue;
                }
                let sign = if area < 0 { -1 } else { 1 };
                let area = area.abs();
                let x0 = tri
                    .iter()
                    .map(|p| p[0])
                    .min()
                    .unwrap()
                    .div_euclid(16)
                    .clamp(0, w as i32 - 1);
                let x1 = tri
                    .iter()
                    .map(|p| p[0])
                    .max()
                    .unwrap()
                    .div_euclid(16)
                    .clamp(0, w as i32 - 1);
                let y0 = tri
                    .iter()
                    .map(|p| p[1])
                    .min()
                    .unwrap()
                    .div_euclid(16)
                    .clamp(0, h as i32 - 1);
                let y1 = tri
                    .iter()
                    .map(|p| p[1])
                    .max()
                    .unwrap()
                    .div_euclid(16)
                    .clamp(0, h as i32 - 1);
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let weights = [
                            edge(tri[1], tri[2], x * 16 + 8, y * 16 + 8) * sign,
                            edge(tri[2], tri[0], x * 16 + 8, y * 16 + 8) * sign,
                            edge(tri[0], tri[1], x * 16 + 8, y * 16 + 8) * sign,
                        ];
                        if weights.iter().any(|v| *v < 0) {
                            continue;
                        }
                        let i = y as usize * w + x as usize;
                        let z = (0..3).map(|k| weights[k] * tri[k][2] as i64).sum::<i64>() / area;
                        if z < depth[i] {
                            continue;
                        }
                        if z == depth[i]
                            && frame.faces[i] != usize::MAX
                            && m.faces[frame.faces[i]].sub & 4 != 0
                            && f.sub & 4 == 0
                        {
                            continue;
                        }
                        // Solid mode shows textured panels in neutral ink-muted.
                        let mut color = if flat && !f.texture.is_empty() {
                            c::INK_MUTED.0
                        } else {
                            rgb(self.base_palette[f.color as usize])
                        };
                        if f.sub == 0xee && f.colors.len() == f.indices.len() {
                            let channels: [u8; 3] = core::array::from_fn(|channel| {
                                ((0..3)
                                    .map(|k| {
                                        weights[k]
                                            * self.base_palette[f.colors[ids[k]] as usize][channel]
                                                as i64
                                    })
                                    .sum::<i64>()
                                    / area)
                                    .clamp(0, 255) as u8
                            });
                            color = rgb(channels);
                        }
                        let mut uv = [-1, -1];
                        if let (Some(pic), Some(palette)) = (texture, colors.as_ref()) {
                            if f.uv.len() == f.indices.len() {
                                let u = (0..3)
                                    .map(|k| weights[k] * f.uv[ids[k]][0] as i64)
                                    .sum::<i64>()
                                    / area;
                                let v = (0..3)
                                    .map(|k| weights[k] * f.uv[ids[k]][1] as i64)
                                    .sum::<i64>()
                                    / area;
                                let u = u.clamp(0, pic.width as i64 - 1) as usize;
                                let v = (pic.height as i64 - 1 - v).clamp(0, pic.height as i64 - 1)
                                    as usize;
                                let pi = v * pic.width + u;
                                if !pic.mask[pi] {
                                    continue;
                                }
                                let keyed = f.sub & 8 != 0 && pic.pixels[pi] == 255;
                                if keyed && f.sub & 3 == 0 {
                                    continue;
                                }
                                if !keyed {
                                    color = rgb(palette[pic.pixels[pi] as usize]);
                                }
                                uv = [u as i32, v as i32];
                            }
                        }
                        if light < 256 {
                            color = super::chrome::shade(color, light);
                        }
                        depth[i] = z;
                        frame.pixels[i] = color;
                        frame.faces[i] = fi;
                        frame.uv[i] = uv;
                    }
                }
            }
        }
        // Selected faces: amber edges and an amber-deep fill. Outside Edit
        // Mesh and Parts the selected panels fill only while no paint tool is
        // on, so strokes stay visible.
        let editing = self.mode == Mode::Model && (self.mesh_edit || self.animation_tool);
        let (selection, fill) = if editing {
            (self.selected_faces(), true)
        } else {
            let painting = if self.mode == Mode::Model {
                self.model_paint
            } else {
                self.paint_enabled
            };
            (self.panel_indices(), !painting)
        };
        if !selection.is_empty() {
            let mut picked = vec![false; m.faces.len()];
            for f in selection {
                if let Some(p) = picked.get_mut(f) {
                    *p = true;
                }
            }
            let on = |i: usize| frame.faces[i] != usize::MAX && picked[frame.faces[i]];
            if picked.iter().any(|p| *p) {
                let mut out = frame.pixels.clone();
                for y in 0..h {
                    for x in 0..w {
                        let i = y * w + x;
                        if !on(i) {
                            continue;
                        }
                        let edge = x == 0
                            || y == 0
                            || x + 1 == w
                            || y + 1 == h
                            || [i - 1, i + 1, i - w, i + w]
                                .iter()
                                .any(|j| frame.faces[*j] != frame.faces[i]);
                        if edge {
                            out[i] = c::AMBER.0;
                        } else if fill {
                            out[i] = c::AMBER_DEEP.0;
                        }
                    }
                }
                frame.pixels = out;
            }
        }
        if let Some(selected) = self.selected_face {
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if frame.faces[i] == selected
                        && [i - 1, i + 1, i - w, i + w]
                            .iter()
                            .any(|j| frame.faces[*j] != selected)
                    {
                        frame.pixels[i] = c::AMBER.0;
                    }
                }
            }
        }
        frame
    }
    pub(super) fn draw_model(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        let rw = (w as usize).min(512);
        let rh = (h as usize * rw / w as usize).max(1);
        let frame = self.render_model(rw, rh);
        blit(o, [x, y, w, h], &frame.pixels, rw, rh);
    }
    pub(super) fn model_hit(&self, x: i32, y: i32) -> Option<(usize, [i32; 2])> {
        let rect = [
            self.left() + 1,
            54,
            self.right() - self.left() - 2,
            self.dock_y() - 54,
        ];
        self.preview_hit(rect, x, y)
    }
    /// The face and texel under `(x, y)` in a model view drawn by
    /// `draw_model` into `rect`: the raster's face buffer, as drawn.
    pub(super) fn preview_hit(&self, rect: [i32; 4], x: i32, y: i32) -> Option<(usize, [i32; 2])> {
        let [l, top, w, h] = rect;
        if w <= 0 || h <= 0 || x < l || y < top || x >= l + w || y >= top + h {
            return None;
        }
        let rw = (w as usize).min(512);
        let rh = (h as usize * rw / w as usize).max(1);
        let f = self.render_model(rw, rh);
        let i = ((y - top) as usize * f.h / h as usize) * f.w + (x - l) as usize * f.w / w as usize;
        if f.faces[i] == usize::MAX {
            None
        } else {
            Some((f.faces[i], f.uv[i]))
        }
    }
    /// The textured view as RGBA rows, for real-data checks.
    #[cfg(not(windows))]
    pub(super) fn render_rgba(&self, w: usize, h: usize) -> Vec<u8> {
        self.render_model(w, h)
            .pixels
            .iter()
            .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, 255])
            .collect()
    }
    /// The colour the main viewport draws at `(x, y)`, for smoke checks.
    pub(super) fn model_pixel(&self, x: i32, y: i32) -> Option<u32> {
        let (l, top) = (self.left() + 1, 54);
        let (w, h) = (self.right() - self.left() - 2, self.dock_y() - 54);
        if x < l || y < top || x >= l + w || y >= top + h {
            return None;
        }
        let rw = (w as usize).min(512);
        let rh = (h as usize * rw / w as usize).max(1);
        let f = self.render_model(rw, rh);
        Some(
            f.pixels[((y - top) as usize * f.h / h as usize) * f.w
                + (x - l) as usize * f.w / w as usize],
        )
    }
    /// A click on the Paint workspace's model preview selects panels.
    pub(super) fn preview_click(&mut self, rect: [i32; 4], x: i32, y: i32, shift: bool) {
        let face = self.preview_hit(rect, x, y).map(|(f, _)| f);
        self.panel_click(face, shift);
    }
    pub(super) fn paint_model_hit(&mut self, face: usize, mut uv: [i32; 2]) {
        if self.selected_face != Some(face) {
            if let Some(s) = &mut self.stroke {
                s.last = None;
            }
        }
        self.selected_face = Some(face);
        if uv[0] < 0 || uv[1] < 0 {
            let named = self
                .model_for_paint()
                .and_then(|m| m.faces.get(face))
                .is_some_and(|f| f.sub & 4 != 0 && !f.texture.is_empty() && !f.uv.is_empty());
            if named {
                // Its PIC is not loaded from any open LIB; never try to generate one.
                self.status = "Texture is outside the active LIB; copy/open its owner first".into();
                return;
            }
            if self.eraser {
                self.status = "Flat-color panel; nothing painted to erase".into();
                return;
            }
            if self.replace.on {
                if let Err(error) = self.replace_flat(face) {
                    self.status = error;
                    return;
                }
            }
            if let Err(error) = self.start_generated_stroke(face) {
                self.status = error;
                return;
            }
            if let Some((_, mapped)) = self.model_hit(self.mouse[0], self.mouse[1]) {
                uv = mapped;
            }
        }
        if uv[0] < 0 || uv[1] < 0 {
            return;
        }
        let Some(model) = self.model_for_paint() else {
            return;
        };
        let Some(f) = model.faces.get(face) else {
            return;
        };
        let name = if f.texture.contains('.') {
            f.texture.clone()
        } else {
            format!("{}.PIC", f.texture)
        };
        if let Some(i) = self
            .stroke
            .as_ref()
            .filter(|s| s.name == name)
            .map(|s| s.entry)
            .or_else(|| {
                self.stroke_parked
                    .iter()
                    .find(|s| s.name == name)
                    .map(|s| s.entry)
            })
            .or_else(|| self.doc.archive.find(&name))
        {
            self.paint_at(i, uv[0] as usize, uv[1] as usize);
        } else {
            self.status = "Texture is outside the active LIB; copy/open its owner first".into();
        }
    }
    pub(super) fn media_layout(&self, o: &mut Layout) {
        let l = self.left();
        let w = self.right() - l;
        let h = self.dock_y() - 54;
        {
            use theme::{metric as m, space};
            let top = m::MENUBAR_H;
            let d = &mut o.canvas;
            d.rect(l + 1, top, w - 2, m::EDITOR_HEADER_H, c::GM_800);
            d.rect(l + 1, top + m::EDITOR_HEADER_H - 1, w - 2, 1, c::GM_1000);
            let info = self.current_picture().map(|p| {
                format!(
                    "{} \u{d7} {} \u{b7} {}{}",
                    p.width,
                    p.height,
                    if p.palette.len() == 256 || self.palette_loaded {
                        "indexed"
                    } else {
                        "no palette, grayscale"
                    },
                    if originals::texture_of(self.name()).is_some() {
                        " \u{b7} read-only"
                    } else {
                        ""
                    }
                )
            });
            let mut right = l + w - 1 - space::SPACE_3;
            if let Some(info) = &info {
                let info = fit(info, (w - 24) / 2, Style::ValueSm);
                right -= text_width(&info, Style::ValueSm);
                d.styled(
                    right,
                    widgets::baseline(top, m::EDITOR_HEADER_H, Style::ValueSm),
                    &info,
                    c::INK_MUTED,
                    Style::ValueSm,
                );
            }
            d.styled(
                l + 1 + space::SPACE_3,
                widgets::baseline(top, m::EDITOR_HEADER_H, Style::Value),
                &fit(
                    self.name(),
                    right - space::SPACE_4 - l - space::SPACE_3,
                    Style::Value,
                ),
                c::INK,
                Style::Value,
            );
        }
        if let Some(p) = self.current_picture() {
            let [x, y, iw, ih] = self.image_rect().unwrap();
            let colors = p.colors(&self.base_palette);
            let x0 = x.max(l + 12);
            let y0 = y.max(84);
            let x1 = (x + iw).min(self.right() - 12);
            let y1 = (y + ih).min(self.dock_y() - 20);
            if x1 > x0 && y1 > y0 {
                let mut pixels = Vec::with_capacity(((x1 - x0) * (y1 - y0)) as usize);
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        let px = ((xx - x) * p.width as i32 / iw) as usize;
                        let py = ((yy - y) * p.height as i32 / ih) as usize;
                        let i = py * p.width + px;
                        pixels.push(if p.mask[i] {
                            rgb(colors[p.pixels[i] as usize])
                        } else if (px / 8 + py / 8).is_multiple_of(2) {
                            c::CHECKER_LIGHT.0
                        } else {
                            c::CHECKER_DARK.0
                        });
                    }
                }
                o.canvas.commands.push(Draw::Bitmap(
                    x0,
                    y0,
                    (x1 - x0) as usize,
                    (y1 - y0) as usize,
                    pixels,
                ));
                border(
                    &mut o.canvas,
                    x0 - 1,
                    y0 - 1,
                    x1 - x0 + 2,
                    y1 - y0 + 2,
                    c::LINE_STRONG,
                );
            }
            // selected_face indexes the draft/preview model when one is active.
            if let Some(f) = self
                .selected_face
                .and_then(|i| self.model_for_paint().and_then(|m| m.faces.get(i)))
            {
                if f.uv.len() == f.indices.len() {
                    for j in 0..f.uv.len() {
                        let a = f.uv[j];
                        let b = f.uv[(j + 1) % f.uv.len()];
                        let project = |p2: [i32; 2]| {
                            [
                                x + (p2[0].clamp(0, p.width as i32 - 1) * iw / p.width as i32),
                                y + ((p.height as i32 - 1 - p2[1]).clamp(0, p.height as i32 - 1)
                                    * ih
                                    / p.height as i32),
                            ]
                        };
                        let a = project(a);
                        let b = project(b);
                        if let Some((a, b)) =
                            clip(a, b, [l + 12, 84, self.right() - 13, self.dock_y() - 21])
                        {
                            o.canvas.line(a[0], a[1], b[0], b[1], c::AMBER);
                        }
                    }
                }
            }
        } else if self.name().ends_with(".PAL") {
            let colors = self.active_colors();
            let cell = ((w - 32) / 16).min((h - 60) / 16).max(1);
            let x = l + (w - cell * 16) / 2;
            for (i, color) in colors.iter().enumerate() {
                let xx = x + (i % 16) as i32 * cell;
                let yy = 84 + (i / 16) as i32 * cell;
                o.canvas
                    .rect(xx, yy, cell - 1, cell - 1, theme::Rgb(rgb(*color)));
                o.hit([xx, yy, cell, cell], Action::Brush(i as u8));
                if i == self.brush as usize {
                    border(&mut o.canvas, xx, yy, cell, cell, c::AMBER);
                }
            }
        } else if let Ok(p) = Pcm::parse(self.name(), &self.data) {
            let mid = 54 + h / 2;
            let left = l + 20;
            let width = w - 40;
            o.canvas.line(left, mid, left + width, mid, c::GM_600);
            for x in 0..width {
                let a = x as usize * p.samples.len() / width as usize;
                let b = ((x + 1) as usize * p.samples.len() / width as usize)
                    .max(a + 1)
                    .min(p.samples.len());
                let samples = &p.samples[a..b];
                let low = *samples.iter().min().unwrap() as i32;
                let high = *samples.iter().max().unwrap() as i32;
                o.canvas.line(
                    left + x,
                    mid - (high - 128) * h / 320,
                    left + x,
                    mid - (low - 128) * h / 320,
                    c::STEEL,
                );
            }
            label_fit(
                &mut o.canvas,
                l + 20,
                88,
                w - 40,
                &format!(
                    "{} Hz \u{b7} {} samples \u{b7} PCM8 mono",
                    p.rate,
                    widgets::format_number(p.samples.len() as i64, 0, true)
                ),
                c::INK,
            );
            let by = self.dock_y() - 50;
            o.button_ex(
                [l + 20, by, 100, theme::metric::BUTTON_H],
                widgets::Btn::new("Play").with_icon(Icon::Play).primary(),
                Action::PlayAudio,
            );
            o.button_ex(
                [l + 128, by, 100, theme::metric::BUTTON_H],
                widgets::Btn::new("Stop").with_icon(Icon::Pause),
                Action::StopAudio,
            );
        } else {
            label_fit(
                &mut o.canvas,
                l + 20,
                120,
                w - 40,
                "Select a PIC, 5K, 11K or PCM8 WAV entry.",
                c::INK_MUTED,
            );
        }
    }
    /// The Paint, Materials and Decals tabs above the media inspector panels.
    fn media_tabs(&self, o: &mut Layout) -> i32 {
        use theme::{metric as m, space};
        use widgets::Btn;
        let r = self.right();
        let y = m::MENUBAR_H + m::EDITOR_HEADER_H + space::SPACE_1;
        let items: Vec<(Btn, Action)> = ["Paint", "Materials", "Decals"]
            .iter()
            .enumerate()
            .map(|(i, t)| {
                (
                    Btn::new(t).on(self.media_tab == i as u8),
                    Action::MediaTab(i as u8),
                )
            })
            .collect();
        let x = r + 1 + space::SPACE_1;
        o.segmented(
            [x, y, self.width - space::SPACE_1 - 4 - x, m::BUTTON_H],
            &items,
        );
        y + m::BUTTON_H + space::SPACE_1
    }
    pub(super) fn media_inspector(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{pane, Btn, Check, Tone};
        self.inspector_header(o, None);
        if let Some(pic) = originals::texture_of(self.name()) {
            self.original_inspector(o, &pic);
            return;
        }
        let mut top = m::MENUBAR_H + m::EDITOR_HEADER_H;
        if self.pic.is_some() || self.model_for_paint().is_some() || self.name().ends_with(".PAL") {
            top = self.media_tabs(o);
            if self.media_tab == 1 {
                self.material_inspector(o, top);
                return;
            }
            if self.media_tab == 2 {
                self.decal_inspector(o, top);
                return;
            }
        }
        let mut s = self.inspector_stack(top, 0);
        if self.pic.is_none() && self.model_for_paint().is_none() {
            if self.pane(o, &mut s, pane::AUDIO, "Sound", Icon::Sound) {
                o.info(&mut s, "Format", &self.detail, "");
                if let Some([x, y, w, h]) = o.wide(&mut s, m::BUTTON_H) {
                    let half = (w - space::SPACE_1) / 2;
                    o.button_ex(
                        [x, y, half, h],
                        Btn::new("Play").with_icon(Icon::Play).primary(),
                        Action::PlayAudio,
                    );
                    o.button_ex(
                        [x + half + space::SPACE_1, y, w - half - space::SPACE_1, h],
                        Btn::new("Stop").with_icon(Icon::Pause),
                        Action::StopAudio,
                    );
                }
                for (title, action) in [
                    ("Export WAV", Action::File(FileAction::Wav)),
                    ("Export original", Action::File(FileAction::Export)),
                ] {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(rect, Btn::new(title), action);
                    }
                }
            }
            o.panel_end(&mut s);
            o.stack_end(s);
            return;
        }
        let picture = self.current_picture().or_else(|| {
            self.selected_face
                .and_then(|i| self.model_for_paint().and_then(|m| m.faces.get(i)))
                .and_then(|f| self.texture_for(&f.texture))
        });
        let colors = picture
            .map(|p| p.colors(&self.base_palette))
            .unwrap_or(*self.base_palette);
        if self.pane(o, &mut s, pane::PAINT, "Paint", Icon::Brush) {
            if let Some((i, f)) = self.selected_face.and_then(|i| {
                self.model_for_paint()
                    .and_then(|m| m.faces.get(i))
                    .map(|f| (i, f))
            }) {
                o.info(
                    &mut s,
                    "Panel",
                    &format!("{} \u{b7} color {}", i + 1, f.color),
                    "",
                );
                let full = if f.texture.contains('.') {
                    f.texture.clone()
                } else {
                    format!("{}.PIC", f.texture)
                };
                if let Some(entry) = self.doc.archive.find(&full) {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(
                            rect,
                            Btn::new(&format!("Paint {full}")).with_icon(Icon::Textured),
                            Action::OpenTexture(entry),
                        );
                    }
                }
                if f.uv.is_empty() || f.texture.is_empty() {
                    for (title, action) in [
                        ("Create paintable texture", Action::PanelTexture),
                        ("Panel color", Action::BaseColor(true)),
                    ] {
                        if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                            o.button_ex(rect, Btn::new(title), action);
                        }
                    }
                }
            }
            if self.mode == Mode::Model {
                for (label, on, action) in [
                    ("Paint on the model", self.model_paint, Action::ModelPaint),
                    (
                        "Lock strokes to the panel",
                        self.paint_lock,
                        Action::PaintLock,
                    ),
                ] {
                    if let Some(rect) = o.wide(&mut s, m::ROW_H) {
                        o.checkbox_row(
                            rect,
                            None,
                            label,
                            "",
                            if on { Check::On } else { Check::Off },
                            action,
                        );
                    }
                }
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    let active = self.model_paint;
                    let replace = self.replace.on;
                    o.segmented(
                        rect,
                        &[
                            (
                                Btn::new("Brush")
                                    .with_icon(Icon::Brush)
                                    .on(active && !self.eraser && !replace),
                                Action::PaintToggle,
                            ),
                            (Btn::new("Eraser").on(active && self.eraser), Action::Eraser),
                            (
                                Btn::new("Replace").on(active && replace),
                                Action::ReplaceTool,
                            ),
                        ],
                    );
                }
            }
            if self.pic.is_some() {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    let on = self.paint_enabled;
                    o.segmented(
                        rect,
                        &[
                            (
                                Btn::new("Brush").on(on && !self.eraser && !self.replace.on),
                                Action::PaintToggle,
                            ),
                            (Btn::new("Eraser").on(on && self.eraser), Action::Eraser),
                            (
                                Btn::new("Replace").on(on && self.replace.on),
                                Action::ReplaceTool,
                            ),
                            (Btn::new("Pick").on(self.pick_color), Action::PickColor),
                        ],
                    );
                }
            }
            if let Some(rect) = o.prop(&mut s, "Brush size") {
                let items: Vec<(Btn, Action)> =
                    [(0, "1 px"), (1, "3 px"), (3, "7 px"), (7, "15 px")]
                        .into_iter()
                        .map(|(n, label)| {
                            (
                                Btn::new(label).on(self.brush_radius == n),
                                Action::Radius(n),
                            )
                        })
                        .collect();
                o.segmented(rect_h(rect, m::BUTTON_H), &items);
            }
            let tool = if self.mode == Mode::Model {
                self.model_paint
            } else {
                self.pic.is_some()
            };
            if tool && self.replace.on {
                self.replace_rows(o, &mut s, &colors);
            }
            if self.pic.is_some() || self.texture_target().is_some() {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(
                        rect,
                        Btn::new("Replace color\u{2026}").with_icon(Icon::Palette),
                        Action::ReplaceDialog,
                    );
                }
            }
            let panels = self.panel_offsets().len();
            if panels > 0 {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(
                        rect,
                        Btn::new(&format!(
                            "Remap {} from view\u{2026}",
                            view::count(panels, "panel", "panels")
                        )),
                        Action::FaceTexture(texture_ui::TEX_REMAP),
                    );
                }
            }
            if self.pic.is_some() {
                if let Some(note) = &self.original_note {
                    original_rows(o, &mut s, note);
                    if note.2 {
                        if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                            o.button_ex(
                                rect,
                                Btn::new("Restore texture").with_icon(Icon::Rotate),
                                Action::RestoreTexture,
                            );
                        }
                    }
                }
            }
            o.stack_notice(
                &mut s,
                Tone::Neutral,
                "Shared textures and UVs change every panel that uses them.",
            );
        }
        o.panel_end(&mut s);
        if self.pane(
            o,
            &mut s,
            pane::PALETTE,
            &format!("Palette \u{b7} index {}", self.brush),
            Icon::Palette,
        ) {
            o.info(&mut s, "Source", &self.palette_label(), "");
            if self.palette_gray() && picture.is_none_or(|p| p.palette.len() != 256) {
                o.stack_notice(
                    &mut s,
                    Tone::Warn,
                    "No game palette found; colors are approximate.",
                );
            }
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
                    o.canvas.rect(
                        xx,
                        y,
                        cell - 1,
                        cell - 1,
                        theme::Rgb(rgb(colors[i as usize])),
                    );
                    if i == self.brush as i32 {
                        widgets::frame(&mut o.canvas, [xx - 1, y - 1, cell + 1, cell + 1], c::INK);
                    }
                    o.hit([xx, y, cell - 1, cell - 1], Action::Brush(i as u8));
                }
            }
            s.gap(space::SPACE_1);
            if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                o.button_ex(
                    rect,
                    Btn::new("Load palette\u{2026}").with_icon(Icon::Folder),
                    Action::File(FileAction::Palette),
                );
            }
        }
        o.panel_end(&mut s);
        let mut actions = Vec::new();
        if self.pic.is_some() {
            actions.push(("Export PNG", Action::File(FileAction::Png)));
        }
        if Pcm::parse(self.name(), &self.data).is_ok() {
            actions.push(("Export WAV", Action::File(FileAction::Wav)));
        }
        if self.model.is_some() {
            actions.push(("Remap untextured colors", Action::Recolor));
        }
        if !actions.is_empty() {
            if self.pane(o, &mut s, pane::MEDIA_EXPORT, "Texture", Icon::Image) {
                for (title, action) in actions {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(rect, Btn::new(title), action);
                    }
                }
            }
            o.panel_end(&mut s);
        }
        if self.model_for_paint().is_some() {
            self.face_texture_pane(o, &mut s, true);
        }
        if self.context_model.is_some() && self.dock != 3 {
            if self.pane(o, &mut s, pane::PREVIEW, "Model preview", Icon::Shape) {
                if let Some(rect) = o.wide(&mut s, 150) {
                    self.draw_model(o, rect[0], rect[1], rect[2], rect[3]);
                    o.zoom_rect = Some(rect);
                    o.hit(rect, Action::PanelPick(rect));
                }
            }
            o.panel_end(&mut s);
        }
        o.stack_end(s);
    }
}
/// `rect` with height `h`, centred on the same row.
pub(super) fn rect_h([x, y, w, rh]: [i32; 4], h: i32) -> [i32; 4] {
    [x, y + (rh - h) / 2, w, h]
}
/// The original's rows: what it is, then its stored size when known
/// (notes carry it as "NAME · 1,234 B").
fn original_rows(o: &mut Layout, s: &mut widgets::Stack, note: &(String, String, bool)) {
    let (value, size) = match note.1.split_once(" \u{b7} ") {
        Some((value, size)) => (value, size.strip_suffix(" B")),
        None => (note.1.as_str(), None),
    };
    o.info(s, &note.0, value, "");
    if let Some(size) = size {
        o.info(s, "Stored size", size, "B");
    }
}
fn grouped(n: usize) -> String {
    let digits = format!("{n}");
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
/// Texture originals: `X.ORG` companions, generated panel colors and the
/// session's saved entries. Edits stay entry-granular and one undo step each.
impl App {
    /// New bytes for `name`, reusing the saved entry's or stored original's exact
    /// storage (and compression flag) when the bytes match one of them.
    pub(super) fn exact_entry(&self, name: &str, bytes: Vec<u8>) -> Result<Entry> {
        let candidates =
            self.doc.saved_entry(name).cloned().into_iter().chain(
                originals::backup(&self.doc.archive, name).and_then(|o| o.renamed(name).ok()),
            );
        for entry in candidates {
            if entry.read().is_ok_and(|b| b == bytes) {
                return Ok(entry);
            }
        }
        Entry::new(name, bytes)
    }
    /// Append stored originals for the PICs these entries replace. A return to
    /// the saved entry keeps none, nor does a panel sheet generated this session
    /// (its original is the face color). Sheets from a saved LIB are backed up
    /// like any texture, so a mistaken panel match never loses artwork.
    pub(super) fn with_originals(&self, entries: Vec<Entry>) -> Vec<Entry> {
        let skip: Vec<String> = entries
            .iter()
            .filter(|e| match self.doc.saved_entry(&e.name) {
                Some(saved) => saved.same_storage(e),
                None => self.panel_color(&e.name).is_some(),
            })
            .map(|e| e.name.clone())
            .collect();
        originals::with_originals(&self.doc, entries, &skip)
    }
    /// A sheet Hangar generated for a flat-color panel (named after its SH,
    /// raw square layout) keeps that face's color byte in the SH record.
    pub(super) fn panel_color(&self, name: &str) -> Option<u8> {
        let stem = name.strip_suffix(".PIC")?;
        let at = self.doc.archive.find(name)?;
        let shapes: Vec<_> = self
            .doc
            .archive
            .entries
            .iter()
            .filter(|e| {
                e.name.strip_suffix(".SH").is_some_and(|sh| {
                    let prefix: String = sh.chars().take(6).collect();
                    stem.strip_prefix(prefix.as_str())
                        .is_some_and(|n| n.len() == 2 && n.bytes().all(|b| b.is_ascii_hexdigit()))
                })
            })
            .collect();
        if shapes.is_empty()
            || !self.doc.archive.entries[at]
                .read()
                .is_ok_and(|b| originals::panel_sheet(&b))
        {
            return None;
        }
        let mut color = None;
        for shape in shapes {
            let Some(model) = shape.read().ok().and_then(|b| Model::parse(&b).ok()) else {
                continue;
            };
            for f in model.faces.iter().filter(|f| !f.uv.is_empty()) {
                let texture = f.texture.to_ascii_uppercase();
                if texture == stem || texture == name {
                    if color.is_some_and(|c| c != f.color) {
                        return None;
                    }
                    color = Some(f.color);
                }
            }
        }
        color
    }
    /// The selected PIC or ORG's original for the Paint inspector, computed on
    /// refresh so drawing never decodes payloads.
    pub(super) fn compute_original_note(&self) -> Option<(String, String, bool)> {
        let e = self.doc.archive.entries.get(self.selected)?;
        let note = |label: &str, value: String, restorable| Some((label.into(), value, restorable));
        if let Some(pic) = originals::texture_of(&e.name) {
            return if originals::valid(e) {
                note(
                    "Original of",
                    format!("{pic} \u{b7} {} B", grouped(e.stored_len())),
                    true,
                )
            } else {
                note("Original", "not a PIC payload".into(), false)
            };
        }
        let org = originals::companion(&e.name)?;
        if let Some(backup) = originals::backup(&self.doc.archive, &e.name) {
            return note(
                "Original kept",
                format!("{org} \u{b7} {} B", grouped(backup.stored_len())),
                true,
            );
        }
        if let Some(color) = self.panel_color(&e.name) {
            let solid = self.pic.as_ref().is_some_and(|p| {
                p.pixels
                    .iter()
                    .zip(&p.mask)
                    .all(|(v, m)| !*m || *v == color)
            });
            return note("Original", format!("panel color {color}"), !solid);
        }
        let junk = self.doc.archive.find(&org).is_some();
        if self
            .doc
            .saved_entry(&e.name)
            .is_some_and(|s| !s.same_storage(e))
        {
            return note("Original", "saved entry".into(), true);
        }
        if junk {
            note("Original", format!("{org} not a PIC"), false)
        } else {
            note("Original", "kept on first edit".into(), false)
        }
    }
    /// Restore texture: X.PIC takes X.ORG's exact bytes and flag and X.ORG is
    /// removed, in one undo step. Without one, a generated panel returns to its
    /// face color, otherwise the session's saved entry is used.
    pub(super) fn restore_texture(&mut self) -> Result<()> {
        self.finish_stroke();
        let selected = self
            .doc
            .archive
            .entries
            .get(self.selected)
            .ok_or("Select a PIC")?
            .name
            .clone();
        let name = match originals::texture_of(&selected) {
            Some(pic) => pic,
            None => self
                .texture_target()
                .map(|i| self.doc.archive.entries[i].name.clone())
                .filter(|n| n.ends_with(".PIC"))
                .ok_or("Select a PIC or a textured panel")?,
        };
        let at = self.doc.archive.find(&name);
        let context = self.context_name();
        let status = if originals::backup(&self.doc.archive, &name).is_some() {
            let saved = self.doc.saved_entry(&name).cloned();
            let (entries, removals) = originals::restore(&self.doc.archive, &name, saved.as_ref())?;
            let org = removals[0].clone();
            self.doc.transaction(entries, &removals)?;
            format!("Restored {name} from {org} in one undo step")
        } else if let Some(color) = self.panel_color(&name) {
            let at = at.ok_or("Texture missing")?;
            let mut bytes = self.doc.archive.entries[at].read()?;
            let mut pic = Pic::parse(&bytes)?;
            let solid = vec![color; pic.pixels.len()];
            if pic.patch_indices(&mut bytes, &solid)? == 0 {
                return Err(format!("{name} already shows its panel color {color}"));
            }
            self.doc
                .transaction(vec![self.exact_entry(&name, bytes)?], &[])?;
            format!("Restored {name} to panel color {color} in one undo step")
        } else if let (Some(at), Some(saved)) = (at, self.doc.saved_entry(&name).cloned()) {
            if saved.same_storage(&self.doc.archive.entries[at]) {
                return Err(format!("No stored original for {name}"));
            }
            self.doc.transaction(vec![saved], &[])?;
            format!("Restored {name} from the saved entry in one undo step")
        } else {
            return Err(format!("No stored original for {name}"));
        };
        // Removing X.ORG can shift indices; keep the same entry selected by name.
        if originals::texture_of(&selected).is_some() {
            if let Some(i) = self.doc.archive.find(&name) {
                self.select_entry(i);
            }
        } else {
            self.reselect(&selected, context);
        }
        self.status = status;
        Ok(())
    }
    pub(super) fn context_name(&self) -> Option<String> {
        self.context_entry
            .and_then(|i| self.doc.archive.entries.get(i))
            .map(|e| e.name.clone())
    }
    /// Removals shift indices: keep the selection and model context by name.
    pub(super) fn reselect(&mut self, selected: &str, context: Option<String>) {
        self.selected = self.doc.archive.find(selected).unwrap_or(self.selected);
        self.context_entry = context.and_then(|n| self.doc.archive.find(&n));
        let face = self.selected_face;
        self.refresh();
        self.selected_face = face;
    }
    /// A stored original previews read-only; edits go to its PIC.
    fn original_inspector(&self, o: &mut Layout, pic: &str) {
        use theme::metric as m;
        use widgets::{pane, Btn};
        let mut s = self.inspector_stack(m::MENUBAR_H + m::EDITOR_HEADER_H, 0);
        if self.pane(o, &mut s, pane::ORIGINAL, "Stored original", Icon::Image) {
            if let Some((label, value, restorable)) = &self.original_note {
                original_rows(o, &mut s, &(label.clone(), value.clone(), *restorable));
                o.info(&mut s, "Access", "Read-only", "");
                if let Some(i) = self.doc.archive.find(pic) {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(
                            rect,
                            Btn::new(&format!("Open {pic}")).with_icon(Icon::Image),
                            Action::Entry(i),
                        );
                    }
                }
                if *restorable {
                    if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                        o.button_ex(
                            rect,
                            Btn::new("Restore texture").with_icon(Icon::Rotate),
                            Action::RestoreTexture,
                        );
                    }
                }
            }
            let mut exports = vec![("Export entry", Action::File(FileAction::Export))];
            if self.pic.is_some() {
                exports.insert(0, ("Export PNG", Action::File(FileAction::Png)));
            }
            for (title, action) in exports {
                if let Some(rect) = o.wide(&mut s, m::BUTTON_H) {
                    o.button_ex(rect, Btn::new(title), action);
                }
            }
        }
        o.panel_end(&mut s);
        o.stack_end(s);
    }
    /// Package: drop every stored original for a distribution build, one undo step.
    pub(super) fn remove_originals(&mut self) -> Result<()> {
        self.finish_stroke();
        let names = originals::stored(&self.doc.archive);
        if names.is_empty() {
            self.status = "No stored originals in this LIB".into();
            return Ok(());
        }
        let selected = self.name().to_string();
        let context = self.context_name();
        self.doc.transaction(Vec::new(), &names)?;
        self.reselect(&selected, context);
        self.status = format!(
            "Removed {} stored original{} (.ORG); textures unchanged. Ctrl+Z undoes it",
            names.len(),
            if names.len() == 1 { "" } else { "s" }
        );
        Ok(())
    }
    /// Delete removes a PIC's stored original in the same undo step.
    pub(super) fn delete_entry(&mut self) -> Result<()> {
        self.finish_stroke();
        let name = self
            .doc
            .archive
            .entries
            .get(self.selected)
            .ok_or("No selected entry")?
            .name
            .clone();
        let removals = originals::removals(&self.doc.archive, &name);
        let context = self.context_name();
        self.doc.transaction(Vec::new(), &removals)?;
        self.reselect(&name, context);
        self.status = if removals.len() > 1 {
            format!(
                "Removed {name} and its stored original {}. Ctrl+Z undoes it.",
                removals[1]
            )
        } else {
            "Entry removed. Ctrl+Z undoes it.".into()
        };
        Ok(())
    }
}
impl App {
    /// Integration smoke uses synthetic assets and verifies the live render, not just bytes.
    pub fn smoke_media(&mut self) {
        self.doc = Document::new(Archive::empty());
        self.demo();
        self.select_entry(0);
        self.textured = true;
        let before = self.render_model(192, 144);
        let i = (193..before.faces.len() - 193)
            .find(|i| {
                let f = before.faces[*i];
                f != usize::MAX
                    && before.uv[*i][0] >= 0
                    && [*i - 1, *i + 1, *i - 192, *i + 192]
                        .iter()
                        .all(|n| before.faces[*n] == f)
            })
            .expect("No textured interior pixel");
        let face = before.faces[i];
        let uv = before.uv[i];
        self.selected_face = Some(face);
        let entry = self.doc.archive.find("DEMO.PIC").unwrap();
        let original = self.doc.archive.entries[entry].read().unwrap();
        let picture = Pic::parse(&original).unwrap();
        let pixel = uv[1] as usize * picture.width + uv[0] as usize;
        self.brush = picture.pixels[pixel] ^ 128;
        self.brush_radius = 0;
        self.paint_model_hit(face, uv);
        assert!(self.painting);
        assert!(!self.doc.dirty());
        let live = self.render_model(192, 144);
        assert_ne!(
            before.pixels[i], live.pixels[i],
            "3D preview did not update during stroke"
        );
        self.finish_stroke();
        assert!(self.doc.dirty());
        assert_eq!(
            Pic::parse(&self.doc.archive.entries[entry].read().unwrap())
                .unwrap()
                .pixels[pixel],
            self.brush
        );
        // The 3D eraser uses the same stroke path and the kept DEMO.ORG.
        let painted = self.doc.archive.entries[entry].read().unwrap();
        assert!(self.doc.archive.find("DEMO.ORG").is_some());
        self.eraser = true;
        self.paint_model_hit(face, uv);
        assert!(self.painting);
        self.finish_stroke();
        self.eraser = false;
        assert_eq!(self.doc.archive.entries[entry].read().unwrap(), original);
        self.doc.undo();
        assert_eq!(self.doc.archive.entries[entry].read().unwrap(), painted);
        self.doc.undo();
        self.refresh();
        assert_eq!(self.doc.archive.entries[entry].read().unwrap(), original);
        assert!(self.doc.archive.find("DEMO.ORG").is_none());
        self.selected_face = Some(face);
        self.open_texture(entry);
        assert!(self.context_model.is_some());
        self.paint_enabled = true;
        let rect = self.image_rect().unwrap();
        let x = rect[0] + (uv[0] * 2 + 1) * rect[2] / (picture.width as i32 * 2);
        let y = rect[1] + (uv[1] * 2 + 1) * rect[3] / (picture.height as i32 * 2);
        self.paint_point(x, y);
        assert!(self.painting);
        assert_ne!(self.render_model(192, 144).pixels[i], before.pixels[i]);
        self.finish_stroke();
        let packed = self.doc.archive.bytes().unwrap();
        let reopened = Archive::parse(packed).unwrap();
        assert_eq!(
            reopened.entries[entry].read().unwrap(),
            self.doc.archive.entries[entry].read().unwrap()
        );
        self.doc.undo();
        self.refresh();
        assert_eq!(self.doc.archive.entries[entry].read().unwrap(), original);
        assert!(crate::platform::is_dir(&crate::platform::current_dir()));
        assert!(!crate::platform::roots().is_empty());
        self.file_prompt(FileAction::Open);
        assert!(self.browser.is_some());
        assert!(self
            .browser
            .as_ref()
            .unwrap()
            .files
            .iter()
            .all(|f| f.directory || f.name.to_ascii_uppercase().ends_with(".LIB")));
        self.key(Key::Escape, false, false);
    }
}
impl App {
    #[cfg(not(windows))]
    pub fn check_real_paint(&mut self) -> Result<String> {
        self.textured = true;
        let frame = self.render_model(192, 144);
        let i = (193..frame.faces.len() - 193)
            .find(|i| {
                let face = frame.faces[*i];
                face != usize::MAX
                    && frame.uv[*i][0] >= 0
                    && [*i - 1, *i + 1, *i - 192, *i + 192]
                        .iter()
                        .all(|j| frame.faces[*j] == face)
            })
            .ok_or("No paintable interior pixel")?;
        let face = frame.faces[i];
        let uv = frame.uv[i];
        let m = self.model_for_paint().unwrap();
        let name = m.faces[face].texture.clone();
        let name = if name.contains('.') {
            name
        } else {
            format!("{name}.PIC")
        };
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let original = self.doc.archive.entries[at].read()?;
        let p = Pic::parse(&original)?;
        let point = uv[1] as usize * p.width + uv[0] as usize;
        let colors = p.colors(&self.base_palette);
        let old = colors[p.pixels[point] as usize];
        self.brush = (0..256)
            .find(|n| colors[*n] != old)
            .ok_or("Palette has only one color")? as u8;
        self.brush_radius = 0;
        self.selected_face = Some(face);
        self.paint_model_hit(face, uv);
        if !self.painting {
            return Err(self.status.clone());
        }
        if self.render_model(192, 144).pixels[i] == frame.pixels[i] {
            return Err("Preview failed to change".into());
        }
        self.finish_stroke();
        let after = self.doc.archive.entries[at].read()?;
        let changes = original.iter().zip(&after).filter(|(a, b)| a != b).count();
        if changes != 1 {
            return Err(format!("Unexpected changed byte count {changes}"));
        }
        let archive = Archive::parse(self.doc.archive.bytes()?)?;
        if archive.entries[at].read()? != after {
            return Err("Paint repack mismatch".into());
        }
        self.doc.undo();
        self.refresh();
        if self.doc.archive.entries[at].read()? != original {
            return Err("Paint undo mismatch".into());
        }
        Ok(format!(
            "{name}: 3D brush updated one texture byte and live preview; repack and undo verified"
        ))
    }
    /// Real-data check of stored originals on a user-supplied LIB (never CI).
    #[cfg(not(windows))]
    pub fn check_real_restore(&mut self) -> Result<String> {
        self.textured = true;
        let frame = self.render_model(192, 144);
        let i = (193..frame.faces.len() - 193)
            .find(|i| {
                let face = frame.faces[*i];
                face != usize::MAX
                    && frame.uv[*i][0] >= 0
                    && [*i - 1, *i + 1, *i - 192, *i + 192]
                        .iter()
                        .all(|j| frame.faces[*j] == face)
            })
            .ok_or("No paintable interior pixel")?;
        let (face, uv) = (frame.faces[i], frame.uv[i]);
        let texture = self.model_for_paint().unwrap().faces[face].texture.clone();
        let name = if texture.contains('.') {
            texture.to_ascii_uppercase()
        } else {
            format!("{}.PIC", texture.to_ascii_uppercase())
        };
        let org = originals::companion(&name).ok_or("Texture is not a PIC")?;
        if self.doc.archive.find(&org).is_some() {
            return Err(format!(
                "{org} already exists; use a LIB without stored originals"
            ));
        }
        let at = self.doc.archive.find(&name).ok_or("Texture missing")?;
        let saved = self.doc.archive.entries[at].clone();
        let original = saved.read()?;
        let p = Pic::parse(&original)?;
        let point = uv[1] as usize * p.width + uv[0] as usize;
        self.brush = p.pixels[point] ^ 0x55;
        let count = self.doc.archive.entries.len();
        let stroke = |app: &mut App, radius: usize, eraser: bool| -> Result<()> {
            app.eraser = eraser;
            app.brush_radius = radius;
            app.selected_face = Some(face);
            app.paint_model_hit(face, uv);
            if !app.painting {
                return Err(app.status.clone());
            }
            app.finish_stroke();
            app.eraser = false;
            if app.status.starts_with("Error") {
                return Err(app.status.clone());
            }
            Ok(())
        };
        stroke(self, 0, false)?;
        let backup = originals::backup(&self.doc.archive, &name)
            .ok_or("First paint did not keep a stored original")?
            .clone();
        if self.doc.archive.entries.len() != count + 1 || !backup.same_payload(&saved) {
            return Err("Stored original is not the pre-paint entry".into());
        }
        stroke(self, 1, false)?;
        if !originals::backup(&self.doc.archive, &name).is_some_and(|b| b.same_storage(&backup)) {
            return Err("Second paint changed the stored original".into());
        }
        stroke(self, 3, true)?;
        let at = self.doc.archive.find(&name).unwrap();
        if self.doc.archive.entries[at].read()? != original
            || self.doc.entry_changed(&self.doc.archive.entries[at])
        {
            return Err("Eraser did not return the saved bytes".into());
        }
        stroke(self, 0, false)?;
        self.selected = self.doc.archive.find(&name).unwrap();
        self.refresh();
        self.restore_texture()?;
        let at = self.doc.archive.find(&name).unwrap();
        if !self.doc.archive.entries[at].same_storage(&saved)
            || self.doc.archive.find(&org).is_some()
            || self.doc.changed_count() != 0
        {
            return Err("Restore texture was not byte exact".into());
        }
        self.doc.undo();
        if originals::backup(&self.doc.archive, &name).is_none() {
            return Err("Undo did not bring the stored original back".into());
        }
        self.doc.redo();
        self.doc.undo();
        let report = hangar_core::validation::inspect(&self.doc, &mut Default::default());
        if report.errors > 0 {
            return Err(format!("Validation: {}", report.summary()));
        }
        let reopened = Archive::parse(self.doc.archive.bytes()?)?;
        let kept = &reopened.entries[reopened
            .find(&org)
            .ok_or("Stored original lost on repack")?];
        if kept.read()? != original || kept.flag() != saved.flag() {
            return Err("Stored original changed on repack".into());
        }
        Ok(format!(
            "{name}: {org} kept once (flag {}, {} B stored); eraser returned exact bytes; Restore texture byte exact and clean; undo/redo, validation and repack verified",
            kept.flag(),
            kept.stored_len()
        ))
    }
}

#[inline(never)]
fn brush_test_app() -> Box<App> {
    Box::new(App::new())
}
fn press(a: &mut App, predicate: impl Fn(Action) -> bool) {
    let rect = a.chrome_hit(&predicate).expect("Visible control missing");
    let (x, y) = (rect[0] + rect[2] / 2, rect[1] + rect[3] / 2);
    a.click(x, y, 1, true);
    a.click(x, y, 1, false);
}
/// Whether the rendered layout draws a text containing `text`.
pub(super) fn shows_text(a: &mut App, text: &str) -> bool {
    shows(a, text)
}
fn shows(a: &mut App, text: &str) -> bool {
    a.layout().canvas.commands.iter().any(|d| match d {
        Draw::Text(_, _, s, _, _) => s.contains(text),
        _ => false,
    })
}
/// Drag one atlas stroke through texture pixels; release commits it.
fn drag(a: &mut App, points: &[(i32, i32)]) {
    let r = a.image_rect().unwrap();
    let p = a.current_picture().unwrap();
    let (w, h) = (p.width as i32, p.height as i32);
    let at = |(u, v): (i32, i32)| {
        (
            r[0] + (u * 2 + 1) * r[2] / (w * 2),
            r[1] + (v * 2 + 1) * r[3] / (h * 2),
        )
    };
    let (x, y) = at(points[0]);
    a.click(x, y, 1, true);
    for point in &points[1..] {
        let (x, y) = at(*point);
        a.motion(x, y, false);
    }
    let (x, y) = at(*points.last().unwrap());
    a.click(x, y, 1, false);
}
impl App {
    /// Stored originals end to end: paint keeps X.ORG once, the eraser returns
    /// exact bytes, Restore texture is one undo step, and X.ORG survives a repack.
    #[inline(never)]
    pub(super) fn smoke_originals(&mut self) {
        let mut a = brush_test_app();
        a.demo();
        let entry = a.doc.archive.find("DEMO.PIC").unwrap();
        let original = a.doc.archive.entries[entry].read().unwrap();
        let saved = a.doc.archive.entries[entry].clone();
        let count = a.doc.archive.entries.len();
        a.select_entry(entry);
        assert!(a.mode == Mode::Media && a.pic.is_some());
        assert!(shows(&mut a, "kept on first edit"));
        press(&mut a, |x| matches!(x, Action::PaintToggle));
        assert!(a.paint_enabled && !a.eraser);
        a.brush = 7;
        a.brush_radius = 1;
        drag(&mut a, &[(5, 5), (12, 5)]);
        assert!(
            a.status.contains("original kept as DEMO.ORG"),
            "{}",
            a.status
        );
        assert_eq!(a.doc.archive.entries.len(), count + 1);
        let org = a
            .doc
            .archive
            .find("DEMO.ORG")
            .expect("first paint keeps a backup");
        let backup = a.doc.archive.entries[org].clone();
        assert!(backup.same_payload(&saved));
        assert_ne!(a.doc.archive.entries[entry].read().unwrap(), original);
        // A second stroke leaves the stored original untouched.
        drag(&mut a, &[(20, 20)]);
        assert_eq!(a.doc.archive.entries.len(), count + 1);
        assert!(a.doc.archive.entries[org].same_storage(&backup));
        let painted = a.doc.archive.entries[entry].read().unwrap();
        assert!(
            shows(&mut a, "Original kept") && shows(&mut a, "DEMO.ORG") && shows(&mut a, "1,856")
        );
        // The eraser paints the original back over the same brush circle.
        press(&mut a, |x| matches!(x, Action::Eraser));
        assert!(a.paint_enabled && a.eraser);
        a.brush_radius = 7;
        drag(&mut a, &[(0, 5), (31, 5), (20, 20)]);
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
        assert!(!a.doc.entry_changed(&a.doc.archive.entries[entry]));
        // Fully erased: the session's DEMO.ORG goes too, so nothing reads as changed.
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        assert_eq!(a.doc.changed_count(), 0);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), painted);
        assert!(a.doc.archive.find("DEMO.ORG").is_some());
        a.act(Action::Redo);
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
        // A partial erase keeps the stored original.
        a.brush_radius = 1;
        press(&mut a, |x| matches!(x, Action::PaintToggle));
        drag(&mut a, &[(5, 5), (12, 5)]);
        press(&mut a, |x| matches!(x, Action::Eraser));
        drag(&mut a, &[(5, 5)]);
        assert!(a.doc.archive.find("DEMO.ORG").is_some());
        assert_ne!(a.doc.archive.entries[entry].read().unwrap(), original);
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.changed_count(), 0);
        // Esc discards a stroke and keeps the tool active.
        press(&mut a, |x| matches!(x, Action::PaintToggle));
        let r = a.image_rect().unwrap();
        a.click(r[0] + 4, r[1] + 4, 1, true);
        assert!(a.painting);
        a.key(Key::Escape, false, false);
        assert!(a.paint_enabled && a.stroke.is_none());
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
        // Restore texture: exact bytes and flag, X.ORG removed, one undo step.
        a.brush_radius = 1;
        drag(&mut a, &[(8, 8), (9, 9)]);
        let repainted = a.doc.archive.entries[entry].read().unwrap();
        press(&mut a, |x| matches!(x, Action::RestoreTexture));
        assert_eq!(a.status, "Restored DEMO.PIC from DEMO.ORG in one undo step");
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        assert_eq!(a.doc.changed_count(), 0);
        a.act(Action::Undo);
        assert!(a.doc.archive.find("DEMO.ORG").is_some());
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), repainted);
        a.act(Action::Redo);
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        a.act(Action::Undo);
        // Repack and reopen: X.ORG keeps its bytes and still restores.
        let reopened = Archive::parse(a.doc.archive.bytes().unwrap()).unwrap();
        let org = reopened.find("DEMO.ORG").unwrap();
        assert_eq!(reopened.entries[org].read().unwrap(), original);
        assert_eq!(reopened.entries[org].flag(), saved.flag());
        a.doc = Document::new(reopened);
        a.select_entry(org);
        assert!(a.pic.is_some() && a.mode == Mode::Media);
        assert!(shows(&mut a, "Original of") && shows(&mut a, "1,856"));
        assert!(!a
            .layout()
            .hits
            .iter()
            .any(|h| matches!(h.action, Action::PaintToggle | Action::DecalPlace)));
        a.paint_at(org, 1, 1);
        assert!(a.stroke.is_none() && a.status.contains("read-only"));
        press(&mut a, |x| matches!(x, Action::RestoreTexture));
        let entry = a.doc.archive.find("DEMO.PIC").unwrap();
        assert_eq!(a.selected, entry);
        assert_eq!(a.doc.archive.entries[entry].read().unwrap(), original);
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        // Once saved, with no stored original left, the eraser says so.
        a.doc.mark_saved();
        press(&mut a, |x| matches!(x, Action::Eraser));
        drag(&mut a, &[(3, 3)]);
        assert_eq!(a.status, "No stored original for DEMO.PIC");
        assert!(!a.painting);
        a.act(Action::Undo);
        // Delete takes the stored original along; Package can drop all of them.
        a.select_entry(a.doc.archive.find("DEMO.PIC").unwrap());
        let before = a.doc.archive.bytes().unwrap();
        a.key(Key::Delete, false, false);
        assert!(a.doc.archive.find("DEMO.PIC").is_none());
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
        a.mode = Mode::Package;
        press(&mut a, |x| matches!(x, Action::RemoveOriginals));
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        assert!(a.doc.archive.find("DEMO.PIC").is_some());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), before);
    }
}
impl App {
    /// The Brush / Eraser segmented control in Paint and in the Model
    /// inspector, and the Paint-on-model checkbox, through their hit regions.
    #[inline(never)]
    pub(super) fn smoke_paint_tools(&mut self) {
        let mut a = brush_test_app();
        let rect = |a: &App, predicate: &dyn Fn(Action) -> bool| {
            a.chrome_hit(predicate).expect("Paint control missing")
        };
        for (w, h) in [(1280, 800), (800, 600)] {
            a.demo();
            a.width = w;
            a.height = h;
            for model in [false, true] {
                if model {
                    a.select_entry(0);
                    a.mode = Mode::Model;
                    a.media_tab = 0;
                    a.selected_face = Some(0);
                    press(&mut a, |x| matches!(x, Action::ModelPaint));
                    assert!(a.model_paint, "Paint on the model checkbox");
                } else {
                    a.select_entry(a.doc.archive.find("DEMO.PIC").unwrap());
                }
                let brush = rect(&a, &|x| matches!(x, Action::PaintToggle));
                let eraser = rect(&a, &|x| matches!(x, Action::Eraser));
                assert!(
                    brush[1] == eraser[1]
                        && brush[3] == eraser[3]
                        && brush[0] + brush[2] == eraser[0],
                    "Brush and Eraser are one segmented control"
                );
                press(&mut a, |x| matches!(x, Action::Eraser));
                assert!(a.eraser);
                assert!(
                    a.layout().canvas.commands.iter().any(|d| matches!(d,
                    Draw::Rect(x, _, _, _, color) if *x == eraser[0] && *color == c::AMBER_DEEP.0))
                );
                press(&mut a, |x| matches!(x, Action::PaintToggle));
                assert!(!a.eraser && (a.paint_enabled || a.model_paint));
                assert!(!a.doc.dirty());
            }
        }
    }
    #[inline(never)]
    pub(super) fn smoke_render_and_brush(&mut self) {
        let mut a = brush_test_app();
        a.demo();
        a.select_entry(0);
        a.textured = true;
        a.yaw = 0;
        a.pitch = 90;
        assert!(
            a.camera_point([10, 0, 0])[0] < 0,
            "Front/top camera must not mirror body X"
        );
        assert!(
            a.camera_point([0, 0, 10])[2] > 0,
            "Top must show the upper surface"
        );
        assert_eq!(a.camera_inverse(a.camera_point([12, 23, 34])), [12, 23, 34]);
        let mut m = model::Model::parse(&model::demo_shape()).unwrap();
        m.vertices = [
            [-100, -100, 0],
            [100, -100, 0],
            [0, 100, 0],
            [-100, -100, 1],
            [100, -100, 1],
            [0, 100, 1],
        ]
        .into_iter()
        .map(|point| model::Vertex { point, offset: 0 })
        .collect();
        m.faces.truncate(1);
        m.faces[0].indices = vec![0, 1, 2];
        m.faces[0].color = 60;
        let mut upper = m.faces[0].clone();
        upper.indices = vec![3, 4, 5];
        upper.color = 80;
        m.faces.push(upper);
        a.model = Some(m);
        assert_eq!(a.render_model(128, 128).faces[64 * 128 + 64], 1);
        let pic_entry = a.doc.archive.find("DEMO.PIC").unwrap();
        let mut pic_bytes = a.doc.archive.entries[pic_entry].read().unwrap();
        let mut pic = Pic::parse(&pic_bytes).unwrap();
        pic.patch_indices(&mut pic_bytes, &vec![255; pic.pixels.len()])
            .unwrap();
        a.textures.insert("DEMO.PIC".into(), pic);
        let f = &mut a.model.as_mut().unwrap().faces[1];
        f.texture = "DEMO.PIC".into();
        f.sub = 12;
        f.uv = vec![[0, 0], [63, 0], [0, 63]];
        assert_eq!(
            a.render_model(128, 128).faces[64 * 128 + 64],
            0,
            "Keyed 255 must not hide the base or receive brush hits"
        );
        a.model.as_mut().unwrap().faces[1].sub = 14;
        assert_eq!(
            a.render_model(128, 128).faces[64 * 128 + 64],
            1,
            "Keyed base-fill faces stay opaque"
        );
        assert_eq!(
            a.render_model(128, 128).pixels[64 * 128 + 64],
            rgb(a.base_palette[80])
        );
        a.model.as_mut().unwrap().faces[1].sub = 4;
        assert_eq!(
            a.render_model(128, 128).faces[64 * 128 + 64],
            1,
            "Opaque 255 remains paintable"
        );
        a.model.as_mut().unwrap().faces[1].normal = Some([0, 0, -32765]);
        assert_eq!(
            a.render_model(128, 128).faces[64 * 128 + 64],
            0,
            "Rear-facing artwork cannot cover the front skin"
        );
        a.demo();
        a.select_entry(0);
        a.mode = Mode::Model;
        a.doc
            .transaction(
                vec![Entry::new("SECOND.PIC", picture::demo()).unwrap()],
                &[],
            )
            .unwrap();
        a.doc.mark_saved();
        let original = a.doc.archive.bytes().unwrap();
        let first = a.doc.archive.find("DEMO.PIC").unwrap();
        let second = a.doc.archive.find("SECOND.PIC").unwrap();
        a.act(Action::ModelPaint);
        assert!(a.model_paint);
        assert!(!a.paint_lock);
        a.brush = 213;
        a.brush_radius = 0;
        a.paint_at(first, 2, 3);
        a.paint_at(second, 5, 6);
        a.paint_at(first, 7, 8);
        assert!(!a.doc.dirty());
        assert_eq!(a.stroke_parked.len(), 1);
        let count = a.doc.archive.entries.len();
        a.finish_stroke();
        assert!(a.doc.dirty());
        // Both painted textures keep one stored original each, in the same step.
        assert_eq!(a.doc.archive.entries.len(), count + 2);
        assert!(a.doc.archive.find("DEMO.ORG").is_some());
        assert!(a.doc.archive.find("SECOND.ORG").is_some());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.paint_at(first, 2, 3);
        a.paint_at(second, 5, 6);
        a.key(Key::Escape, false, false);
        assert!(a.stroke_parked.is_empty());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        // Multiple new panel sheets and their shared SH stay one transaction.
        a.doc.replace(0, model::demo_shape()).unwrap();
        a.doc.mark_saved();
        a.select_entry(0);
        // Generated sheets carry no palette: painting them needs the base PAL.
        a.palette_override = Some(Box::new(core::array::from_fn(|i| [i as u8; 3])));
        a.refresh();
        let original = a.doc.archive.bytes().unwrap();
        let count = a.doc.archive.entries.len();
        a.start_generated_stroke(0).unwrap();
        let entry = a.stroke.as_ref().unwrap().entry;
        a.paint_at(entry, 4, 4);
        a.start_generated_stroke(1).unwrap();
        let entry = a.stroke.as_ref().unwrap().entry;
        a.paint_at(entry, 5, 5);
        assert!(!a.doc.dirty());
        a.finish_stroke();
        // Two new panel sheets and no stored originals: their original is the face color.
        assert_eq!(a.doc.archive.entries.len(), count + 2);
        assert!(!a
            .doc
            .archive
            .entries
            .iter()
            .any(|e| e.name.ends_with(".ORG")));
        assert!(a.status.contains("2 flat panels converted"), "{}", a.status);
        assert!(a.model.as_ref().unwrap().faces[..2]
            .iter()
            .all(|f| !f.texture.is_empty()));
        // The eraser and Restore texture return a generated sheet to its face color.
        let sheet = a
            .doc
            .archive
            .entries
            .iter()
            .position(|e| e.name == "DEMO00.PIC")
            .unwrap();
        let face = a.model.as_ref().unwrap().faces[..2]
            .iter()
            .find(|f| f.texture.starts_with("DEMO00"))
            .unwrap()
            .color;
        let solid = |a: &App| {
            Pic::parse(&a.doc.archive.entries[sheet].read().unwrap())
                .unwrap()
                .pixels
                .iter()
                .all(|p| *p == face)
        };
        assert_eq!(a.panel_color("DEMO00.PIC"), Some(face));
        assert!(!solid(&a));
        a.eraser = true;
        a.brush_radius = 7;
        a.paint_at(sheet, 4, 4);
        a.paint_at(sheet, 5, 5);
        a.finish_stroke();
        a.eraser = false;
        assert!(solid(&a));
        assert!(!a
            .doc
            .archive
            .entries
            .iter()
            .any(|e| e.name.ends_with(".ORG")));
        a.paint_at(sheet, 30, 30);
        a.finish_stroke();
        assert!(!solid(&a));
        a.selected = sheet;
        a.refresh();
        a.restore_texture().unwrap();
        assert!(solid(&a), "{}", a.status);
        a.act(Action::Undo);
        a.act(Action::Undo);
        a.act(Action::Undo);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.palette_override = None;
    }
}
