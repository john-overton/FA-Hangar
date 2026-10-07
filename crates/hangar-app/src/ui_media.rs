use super::view::{border, label_fit, text_fit, Action, Layout};
use super::*;
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
                    .or(self.pic.as_ref())
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
            self.stroke_parked.push(previous);
        }
        if self.stroke.is_none() {
            if let Some(at) = self.stroke_parked.iter().position(|s| s.entry == entry) {
                self.stroke = Some(self.stroke_parked.remove(at));
            }
        }
        if self.stroke.is_none() {
            let r: Result<Stroke> = (|| {
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
                Ok(Stroke {
                    entry,
                    name: self.doc.archive.entries[entry].name.clone(),
                    original: Some(self.doc.archive.entries[entry].clone()),
                    bytes,
                    pic,
                    last: None,
                })
            })();
            match r {
                Ok(s) => self.stroke = Some(s),
                Err(e) => {
                    self.status = format!("Error: {e}");
                    return;
                }
            }
        }
        self.painting = true;
        let s = self.stroke.as_mut().unwrap();
        let (px, py) = s.last.unwrap_or((x, y));
        let steps = x.abs_diff(px).max(y.abs_diff(py)).max(1);
        for step in 0..=steps {
            let xx = (px as i64 + (x as i64 - px as i64) * step as i64 / steps as i64) as usize;
            let yy = (py as i64 + (y as i64 - py as i64) * step as i64 / steps as i64) as usize;
            if let Err(e) = s
                .pic
                .paint(&mut s.bytes, xx, yy, self.brush_radius, self.brush)
            {
                self.status = format!("Error: {e}");
                break;
            }
        }
        s.last = Some((x, y));
        self.status = format!(
            "Painting index {} / release to commit one undo step",
            self.brush
        );
    }
    pub(super) fn paint_point(&mut self, x: i32, y: i32) {
        let Some((px, py)) = self.image_point(x, y) else {
            if let Some(s) = &mut self.stroke {
                s.last = None;
            }
            return;
        };
        if self.pick_color {
            if let Some(p) = self.current_picture() {
                self.brush = p.pixels[py * p.width + px];
            }
            self.pick_color = false;
            return;
        }
        if self.paint_enabled {
            self.paint_at(self.selected, px, py);
        }
    }
    pub(super) fn finish_stroke(&mut self) {
        if let Some(s) = self.stroke.take().or_else(|| self.stroke_parked.pop()) {
            let enabled = self.paint_enabled;
            let model_paint = self.model_paint;
            let face = self.selected_face;
            let mut strokes = core::mem::take(&mut self.stroke_parked);
            strokes.push(s);
            let plan = self.panel_draft.take();
            let r = (|| {
                let mut entries = Vec::new();
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
                    } else if self.doc.archive.find(&s.name).is_some() {
                        return Err("Generated texture name changed; stroke cancelled".into());
                    }
                    entries.push(Entry::new(&s.name, s.bytes)?);
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
                self.doc.transaction(entries, &[])
            })();
            self.painting = false;
            self.refresh();
            self.paint_enabled = enabled;
            self.model_paint = model_paint;
            self.selected_face = face;
            match r {
                Ok(()) => {
                    self.status = "Paint stroke applied / Ctrl+Z undo / Package LIB to save".into()
                }
                Err(e) => self.status = format!("Error: {e}"),
            }
        }
    }
    pub(super) fn open_texture(&mut self, index: usize) {
        let m = self.model.as_ref().or(self.context_model.as_ref()).cloned();
        let entry = self.model_entry.or(self.context_entry);
        let face = self.selected_face;
        self.select_entry(index);
        self.context_model = m;
        self.context_entry = entry;
        self.selected_face = face;
        self.refresh();
        self.mode = Mode::Media;
        self.dock = 3;
    }
    pub(super) fn model_for_paint(&self) -> Option<&Model> {
        self.preview
            .as_ref()
            .or(self.model.as_ref())
            .or(self.context_model.as_ref())
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
            let texture = self.texture_for(&f.texture);
            let colors = texture.map(|p| p.colors(&self.base_palette));
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
                        let mut color = rgb(self.base_palette[f.color as usize]);
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
                        depth[i] = z;
                        frame.pixels[i] = color;
                        frame.faces[i] = fi;
                        frame.uv[i] = uv;
                    }
                }
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
        let l = self.left() + 1;
        let top = 54;
        let w = self.right() - self.left() - 2;
        let h = self.dock_y() - 54;
        if x < l || y < top || x >= l + w || y >= top + h {
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
    pub(super) fn paint_model_hit(&mut self, face: usize, mut uv: [i32; 2]) {
        if self.selected_face != Some(face) {
            if let Some(s) = &mut self.stroke {
                s.last = None;
            }
        }
        self.selected_face = Some(face);
        if uv[0] < 0 || uv[1] < 0 {
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
        o.canvas.rect(l + 1, 26, w - 2, 28, c::GM_800);
        text_fit(&mut o.canvas, l + 12, 44, w - 24, self.name(), c::INK);
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
                            0x30363c
                        } else {
                            0x20262c
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
            if let Some(f) = self
                .selected_face
                .and_then(|i| self.context_model.as_ref().and_then(|m| m.faces.get(i)))
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
            label_fit(
                &mut o.canvas,
                l + 16,
                72,
                w - 32,
                &format!(
                    "{} x {} / {}",
                    p.width,
                    p.height,
                    if p.palette.len() == 256 || self.palette_loaded {
                        "Indexed palette"
                    } else {
                        "Palette missing: grayscale preview"
                    }
                ),
                c::INK_MUTED,
            );
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
                &format!("{} Hz / {} samples / PCM8 mono", p.rate, p.samples.len()),
                c::INK,
            );
            o.button(
                [l + 20, self.dock_y() - 50, 100, 26],
                "Play",
                Action::PlayAudio,
                true,
            );
            o.button(
                [l + 130, self.dock_y() - 50, 100, 26],
                "Stop",
                Action::StopAudio,
                false,
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
    pub(super) fn media_inspector(&self, o: &mut Layout) {
        let r = self.right();
        let w = self.width - r;
        let bottom = self.height - 24;
        o.canvas.rect(r + 1, 26, w - 1, 28, c::GM_700);
        o.canvas.label(r + 12, 44, "Livery / media", c::INK);
        if self.pic.is_some() || self.model_for_paint().is_some() || self.name().ends_with(".PAL") {
            let tab = (w - 24) / 3;
            for (i, title) in ["Paint", "Materials", "Decals"].iter().enumerate() {
                o.button(
                    [r + 10 + i as i32 * (tab + 2), 58, tab, 24],
                    title,
                    Action::MediaTab(i as u8),
                    self.media_tab == i as u8,
                );
            }
            if self.media_tab == 1 {
                self.material_inspector(o);
                return;
            }
            if self.media_tab == 2 {
                self.decal_inspector(o);
                return;
            }
        }
        if self.pic.is_none() && self.model_for_paint().is_none() {
            label_fit(
                &mut o.canvas,
                r + 12,
                84,
                w - 24,
                &self.detail,
                c::INK_MUTED,
            );
            o.button(
                [r + 12, 112, w - 24, 26],
                "Play audio",
                Action::PlayAudio,
                true,
            );
            o.button([r + 12, 148, w - 24, 26], "Stop", Action::StopAudio, false);
            o.button(
                [r + 12, 198, w - 24, 26],
                "Export WAV",
                Action::File(FileAction::Wav),
                false,
            );
            o.button(
                [r + 12, 234, w - 24, 26],
                "Export original",
                Action::File(FileAction::Export),
                false,
            );
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
        let mut y = 108;
        if let Some((i, f)) = self.selected_face.and_then(|i| {
            self.model_for_paint()
                .and_then(|m| m.faces.get(i))
                .map(|f| (i, f))
        }) {
            text_fit(
                &mut o.canvas,
                r + 12,
                y,
                w - 24,
                &format!("Panel {} / color index {}", i + 1, f.color),
                c::AMBER,
            );
            y += 25;
            let full = if f.texture.contains('.') {
                f.texture.clone()
            } else {
                format!("{}.PIC", f.texture)
            };
            if let Some(entry) = self.doc.archive.find(&full) {
                o.button(
                    [r + 10, y, w - 20, 24],
                    &format!("UV / paint {full}"),
                    Action::OpenTexture(entry),
                    false,
                );
                y += 32;
            }
            if f.uv.is_empty() || f.texture.is_empty() {
                o.button(
                    [r + 10, y, w - 20, 24],
                    "Create paintable panel texture",
                    Action::PanelTexture,
                    false,
                );
                y += 30;
                o.button(
                    [r + 10, y, w - 20, 24],
                    "Panel color",
                    Action::BaseColor(true),
                    false,
                );
                y += 30;
            }
        }
        if self.mode == Mode::Model {
            o.button(
                [r + 10, y, w - 20, 24],
                "Paint model / auto-create texture",
                Action::ModelPaint,
                self.model_paint,
            );
            y += 30;
            o.button(
                [r + 10, y, w - 20, 24],
                if self.paint_lock {
                    "Panel lock: on"
                } else {
                    "Panel lock: off / cross panels"
                },
                Action::PaintLock,
                self.paint_lock,
            );
            y += 32;
        }
        if self.pic.is_some() {
            o.button(
                [r + 10, y, 90, 24],
                "Brush",
                Action::PaintToggle,
                self.paint_enabled,
            );
            o.button(
                [r + 106, y, w - 116, 24],
                "Pick color",
                Action::PickColor,
                self.pick_color,
            );
            y += 34;
        }
        o.canvas.label(
            r + 12,
            y + 12,
            &format!("PALETTE / index {}", self.brush),
            c::INK_MUTED,
        );
        y += 22;
        let cell = ((w - 24) / 16).max(8);
        for i in 0..256 {
            let x = r + 12 + (i % 16) * cell;
            let yy = y + (i / 16) * cell;
            if yy + cell >= bottom {
                break;
            }
            o.canvas.rect(
                x,
                yy,
                cell - 1,
                cell - 1,
                theme::Rgb(rgb(colors[i as usize])),
            );
            if i == self.brush as i32 {
                border(&mut o.canvas, x - 1, yy - 1, cell + 1, cell + 1, c::INK);
            }
            o.hit([x, yy, cell, cell], Action::Brush(i as u8));
        }
        y += 16 * cell + 12;
        if y + 26 < bottom {
            for (i, n) in [0, 1, 3, 7].into_iter().enumerate() {
                o.button(
                    [r + 10 + i as i32 * 58, y, 52, 23],
                    &format!("{} px", n * 2 + 1),
                    Action::Radius(n),
                    self.brush_radius == n,
                );
            }
            y += 34;
        }
        if y + 24 < bottom {
            o.button(
                [r + 10, y, w - 20, 23],
                "Load palette from PAL / LIB",
                Action::File(FileAction::Palette),
                false,
            );
            y += 31;
        }
        if self.pic.is_some() && y + 24 < bottom {
            o.button(
                [r + 10, y, w - 20, 23],
                "Export PNG",
                Action::File(FileAction::Png),
                false,
            );
            y += 31;
        }
        if Pcm::parse(self.name(), &self.data).is_ok() && y + 24 < bottom {
            o.button(
                [r + 10, y, w - 20, 23],
                "Export WAV",
                Action::File(FileAction::Wav),
                false,
            );
            y += 31;
        }
        if self.model.is_some() && y + 24 < bottom {
            o.button(
                [r + 10, y, w - 20, 23],
                "Remap untextured face colors",
                Action::Recolor,
                false,
            );
            y += 31;
        }
        if self.selected_face.is_some() && y + 24 < bottom {
            o.button(
                [r + 10, y, w - 20, 23],
                "Clone texture for this shape",
                Action::Isolate,
                false,
            );
            y += 31;
        }
        if self.context_model.is_some() && self.dock != 3 && y + 52 < bottom {
            let ph = (bottom - y - 38).min(160);
            self.draw_model(o, r + 12, y, w - 24, ph);
        }
        if y + 28 < bottom {
            label_fit(
                &mut o.canvas,
                r + 12,
                bottom - 8,
                w - 24,
                "Shared PICs / UVs affect other referencing panels.",
                c::INK_FAINT,
            );
        }
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
        self.doc.undo();
        self.refresh();
        assert_eq!(self.doc.archive.entries[entry].read().unwrap(), original);
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
}

#[inline(never)]
fn brush_test_app() -> Box<App> {
    Box::new(App::new())
}
impl App {
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
        a.finish_stroke();
        assert!(a.doc.dirty());
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
        a.palette_loaded = true;
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
        assert_eq!(a.doc.archive.entries.len(), count + 2);
        assert!(a.model.as_ref().unwrap().faces[..2]
            .iter()
            .all(|f| !f.texture.is_empty()));
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
    }
}
