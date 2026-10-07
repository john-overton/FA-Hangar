use super::view::{border, label_fit, Action, Layout};
use super::*;
use hangar_core::{
    decal::{self, Image, Placement},
    material,
};
pub(super) struct DecalDraft {
    pub entry: usize,
    pub original: Entry,
    pub bytes: Vec<u8>,
    pub pic: Pic,
    pub changed: usize,
}
impl App {
    pub(super) fn texture_target(&self) -> Option<usize> {
        if self.pic.is_some() {
            // Stored originals are read-only previews.
            return hangar_core::originals::texture_of(self.name())
                .is_none()
                .then_some(self.selected);
        }
        let model = self.model_for_paint()?;
        let face = model.faces.get(self.selected_face?)?;
        let name = if face.texture.contains('.') {
            face.texture.clone()
        } else {
            format!("{}.PIC", face.texture)
        };
        self.doc.archive.find(&name)
    }
    fn palette_target(&self) -> Result<(usize, bool, usize)> {
        let index = self.brush as usize;
        if self.name().ends_with(".PAL") {
            return Ok((self.selected, false, index * 3));
        }
        if let Some(entry) = self.texture_target() {
            let pic = self.current_picture().or_else(|| {
                self.selected_face
                    .and_then(|i| self.model_for_paint().and_then(|m| m.faces.get(i)))
                    .and_then(|f| self.texture_for(&f.texture))
            });
            if pic.is_some_and(|p| index < p.palette.len()) {
                return Ok((entry, true, 0));
            }
        }
        if self.palette_override.is_some() {
            return Err("The loaded display palette is not a saved resource; select/import a PAL to edit it".into());
        }
        let mut names = Vec::new();
        if let Some(owner) = self.station_owner() {
            names.push(format!(
                "{}.PAL",
                self.doc.archive.entries[owner]
                    .name
                    .split('.')
                    .next()
                    .unwrap()
            ));
        }
        names.push("PALETTE.PAL".into());
        for name in names {
            if let Some((id, entry, _)) = self.resolve_resource(&name) {
                if id != self.library_id {
                    return Err("Open the palette owner's LIB to edit its colors".into());
                }
                return Ok((entry, false, index * 3));
            }
        }
        Err("No saved palette owns this index".into())
    }
    fn palette_rgb(&self) -> Result<[u8; 3]> {
        self.palette_target()?;
        Ok(self.active_colors()[self.brush as usize].map(|v| ((v as u16 * 63 + 127) / 255) as u8))
    }
    pub(super) fn material_prompt(&mut self, kind: u8) {
        let (title, value, prompt) = match kind {
            0 => (
                "Palette index / 0..255".into(),
                format!("{}", self.brush),
                PromptKind::PaletteIndex,
            ),
            1 => match self.palette_rgb() {
                Ok(p) => (
                    "RGB / three source components 0..63".into(),
                    format!("{} {} {}", p[0], p[1], p[2]),
                    PromptKind::PaletteColor,
                ),
                Err(e) => {
                    self.status = e;
                    return;
                }
            },
            2 => (
                "UV: U offset, V offset, U %, V %, clockwise degrees".into(),
                "0 0 100 100 0".into(),
                PromptKind::UvMaterial,
            ),
            _ => (
                "Clone texture across aircraft SH family / new PIC name".into(),
                "LIVERY.PIC".into(),
                PromptKind::FamilyTexture,
            ),
        };
        self.prompt = Some(Prompt {
            kind: prompt,
            title,
            value,
            axis: 0,
        });
    }
    pub(super) fn palette_edit(&mut self, value: &str) -> Result<()> {
        let values: Vec<u8> = value
            .split_whitespace()
            .map(str::parse)
            .collect::<core::result::Result<_, _>>()
            .map_err(|_| "Enter R G B as source integers 0..63")?;
        let rgb: [u8; 3] = values
            .try_into()
            .map_err(|_| "Enter exactly three RGB components")?;
        let (entry, pic, _) = self.palette_target()?;
        let source = self.doc.archive.entries[entry].read()?;
        let bytes = material::palette_color(&source, pic, self.brush as usize, rgb)?;
        if pic {
            let name = self.doc.archive.entries[entry].name.clone();
            let entries = self.with_originals(vec![Entry::new(&name, bytes)?]);
            self.doc.transaction(entries, &[])?;
        } else {
            self.doc.replace(entry, bytes)?;
        }
        self.refresh();
        self.status =
            "Palette color changed / all users of this palette see the change / Ctrl+Z undo".into();
        Ok(())
    }
    pub(super) fn uv_edit(&mut self, value: &str) -> Result<()> {
        let n: Vec<i32> = value
            .split_whitespace()
            .map(str::parse)
            .collect::<core::result::Result<_, _>>()
            .map_err(|_| "Enter five UV integers")?;
        if n.len() != 5 {
            return Err("Enter U offset, V offset, U scale %, V scale %, degrees".into());
        }
        let face = self.selected_face.ok_or("Select a textured model face")?;
        let entry = self
            .model_entry
            .or(self.context_entry)
            .ok_or("Open the shape's LIB before editing UVs")?;
        let source = self.doc.archive.entries[entry].read()?;
        let bytes = material::face_uv(
            &source,
            face,
            material::UvTransform {
                shift: [n[0], n[1]],
                scale: [n[2], n[3]],
                degrees: n[4],
            },
        )?;
        if self.context_entry == Some(entry) {
            self.context_model = Some(Model::parse(&bytes)?);
        }
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.selected_face = Some(face);
        self.status = "UV record updated / shared instances use the same UVs / Ctrl+Z undo".into();
        Ok(())
    }
    pub(super) fn family_texture(&mut self, name: &str) -> Result<()> {
        let owner = self
            .station_owner()
            .filter(|i| self.doc.archive.entries[*i].name.ends_with(".PT"))
            .ok_or("Select the owning aircraft PT; shared shapes need an explicit owner")?;
        let picture = self
            .texture_target()
            .ok_or("Select a textured face or its PIC")?;
        let entries = material::clone_family_texture(
            &self.doc.archive,
            &self.doc.archive.entries[owner].name,
            &self.doc.archive.entries[picture].name,
            &name.trim().to_ascii_uppercase(),
        )?;
        let shapes = entries.iter().filter(|e| e.name.ends_with(".SH")).count();
        self.doc.transaction(entries, &[])?;
        if let Some(i) = self.context_entry {
            self.context_model = self.doc.archive.entries[i]
                .read()
                .ok()
                .and_then(|b| Model::parse(&b).ok());
        }
        self.refresh();
        if let Some(i) = self.doc.archive.find(name.trim()) {
            self.open_texture(i);
            self.media_tab = 0;
        }
        self.status = format!(
            "Texture cloned / {shapes} SH modules retargeted across stored states / one undo step"
        );
        Ok(())
    }
    pub(super) fn active_colors(&self) -> [[u8; 3]; 256] {
        if self.name().ends_with(".PAL") {
            return picture::palette(&self.data).unwrap_or(*self.base_palette);
        }
        self.current_picture()
            .or_else(|| {
                self.selected_face
                    .and_then(|i| self.model_for_paint().and_then(|m| m.faces.get(i)))
                    .and_then(|f| self.texture_for(&f.texture))
            })
            .map_or(*self.base_palette, |p| p.colors(&self.base_palette))
    }

    pub(super) fn set_decal(&mut self, image: Image, name: String, text: bool) -> Result<()> {
        self.finish_stroke();
        self.decal_draft = None;
        self.decal_image = Some(image);
        self.decal_name = name;
        self.decal_is_text = text;
        self.decal_active = true;
        self.decal_dragging = false;
        self.media_tab = 2;
        self.hp_tool = false;
        if let Some(entry) = self.texture_target() {
            let pic = Pic::parse(&self.doc.archive.entries[entry].read()?)?;
            self.decal_placement.center = [pic.width as i32 / 2, pic.height as i32 / 2];
            let image = self.decal_image.as_ref().unwrap();
            self.decal_placement.width = if text {
                image.width
            } else {
                (pic.width / 3)
                    .clamp(1, 128)
                    .min((2048 * image.width / image.height).max(1))
            };
            if pic.palette.len() == 256 || self.palette_loaded {
                self.prepare_decal(entry, self.decal_placement)?;
            }
        }
        self.status = "Click the atlas or a model panel to place; adjust, then Apply decal".into();
        Ok(())
    }
    pub(super) fn load_decal(&mut self, path: &str, remember: bool) -> Result<()> {
        if crate::platform::file_size(path)? > 16 * 1024 * 1024 {
            return Err("Decal PNG exceeds 16 MiB".into());
        }
        let image = Image::png(&crate::platform::read(path)?)?;
        self.set_decal(
            image,
            path.rsplit(['/', '\\']).next().unwrap_or(path).into(),
            false,
        )?;
        if remember && !path.contains(['\n', '\r']) {
            let path = if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
                path.into()
            } else {
                format!("{}/{}", crate::platform::current_dir(), path)
            };
            self.decal_paths.retain(|p| p != &path);
            self.decal_paths.insert(0, path);
            self.decal_paths.truncate(16);
            if crate::platform::save_decals(&self.decal_paths).is_err() {
                self.status="Decal loaded; the PNG library list is read-only, so this selection is session-only".into();
            }
        }
        Ok(())
    }
    pub(super) fn text_ink(&self) -> u8 {
        self.decal_ink.unwrap_or_else(|| {
            self.active_colors()
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.iter().map(|v| (255 - *v as i32).pow(2)).sum::<i32>())
                .unwrap()
                .0 as u8
        })
    }
    pub(super) fn tail_text(&mut self, text: &str) -> Result<()> {
        let color = self.active_colors()[self.text_ink() as usize];
        let image = Image::text(text, color)?;
        let previous = if self.decal_is_text {
            self.decal_draft
                .as_ref()
                .map(|d| (d.entry, self.decal_placement))
        } else {
            None
        };
        self.decal_text = text.trim().to_ascii_uppercase();
        self.set_decal(image, format!("Text: {}", self.decal_text), true)?;
        if let Some((entry, placement)) = previous {
            self.prepare_decal(entry, placement)?;
        }
        Ok(())
    }
    pub(super) fn prepare_decal(&mut self, entry: usize, p: Placement) -> Result<()> {
        let image = self
            .decal_image
            .as_ref()
            .ok_or("Import a PNG or choose text/a national marking")?;
        let original = self
            .doc
            .archive
            .entries
            .get(entry)
            .ok_or("Texture missing")?
            .clone();
        if hangar_core::originals::texture_of(&original.name).is_some() {
            return Err("Stored originals are read-only; place decals on the PIC".into());
        }
        let source = original.read()?;
        let pic = Pic::parse(&source)?;
        if pic.palette.len() != 256 && !self.palette_loaded {
            return Err("Load the texture's palette before placing decals".into());
        }
        let (bytes, pic, changed) = decal::composite(&source, &self.base_palette, image, p)?;
        self.decal_placement = p;
        self.decal_draft = Some(Box::new(DecalDraft {
            entry,
            original,
            bytes,
            pic,
            changed,
        }));
        Ok(())
    }
    pub(super) fn place_decal(&mut self, x: i32, y: i32) -> Result<()> {
        let (entry, center) = if self.mode == Mode::Media {
            let (px, py) = self
                .image_point(x, y)
                .ok_or("Click inside the texture atlas")?;
            (self.selected, [px as i32, py as i32])
        } else {
            let (face, uv) = self.model_hit(x, y).ok_or("Click a textured model panel")?;
            if uv[0] < 0 || uv[1] < 0 {
                return Err("This panel has no editable named texture".into());
            }
            self.selected_face = Some(face);
            (
                self.texture_target()
                    .ok_or("Open/copy the texture into the active LIB first")?,
                uv,
            )
        };
        if self.decal_draft.as_ref().is_some_and(|d| d.entry == entry)
            && self.decal_placement.center == center
        {
            return Ok(());
        }
        let mut p = self.decal_placement;
        p.center = center;
        self.prepare_decal(entry, p)?;
        self.decal_dragging = true;
        Ok(())
    }
    pub(super) fn decal_setting(&mut self, key: u8, value: &str) -> Result<()> {
        let n: i32 = value.parse().map_err(|_| "Enter an integer")?;
        let mut p = self.decal_placement;
        match key {
            0 | 1 if (-8192..=8192).contains(&n) => p.center[key as usize] = n,
            2 if (1..=2048).contains(&n) => p.width = n as usize,
            3 => p.degrees = n.rem_euclid(360),
            4 if (0..=100).contains(&n) => p.opacity = n as u8,
            _ => return Err("Value outside decal setting range".into()),
        }
        if let Some(entry) = self
            .decal_draft
            .as_ref()
            .map(|d| d.entry)
            .or_else(|| self.texture_target())
        {
            self.prepare_decal(entry, p)?;
        } else {
            self.decal_placement = p;
        }
        Ok(())
    }
    pub(super) fn decal_setting_prompt(&mut self, key: u8) {
        let p = self.decal_placement;
        let (label, value) = match key {
            0 => ("Texture X", p.center[0]),
            1 => ("Texture Y", p.center[1]),
            2 => ("Width in texture pixels", p.width as i32),
            3 => ("Clockwise rotation in degrees", p.degrees),
            _ => ("Opacity percent", p.opacity as i32),
        };
        self.prompt = Some(Prompt {
            kind: PromptKind::DecalSetting(key),
            title: label.into(),
            value: format!("{value}"),
            axis: 0,
        });
    }
    pub(super) fn apply_decal(&mut self) -> Result<()> {
        let d = self.decal_draft.as_ref().ok_or("Place a decal first")?;
        if d.changed == 0 {
            return Err(
                "No opaque texture pixels changed; move the decal or change its color/size".into(),
            );
        }
        if !self
            .doc
            .archive
            .entries
            .get(d.entry)
            .is_some_and(|e| e.same_storage(&d.original))
        {
            return Err("Texture changed; rebuild the decal preview".into());
        }
        let d = self.decal_draft.take().unwrap();
        let count = d.changed;
        let entries = self.with_originals(vec![Entry::new(&d.original.name, d.bytes)?]);
        self.doc.transaction(entries, &[])?;
        self.refresh();
        self.decal_active = false;
        self.status = format!("Decal baked into {count} indexed pixels / one Ctrl+Z undo step");
        Ok(())
    }
    pub(super) fn material_inspector(&self, o: &mut Layout) {
        let (r, w, h) = (self.right(), self.width - self.right(), self.height);
        let colors = self.active_colors();
        let c = colors[self.brush as usize];
        o.canvas.rect(
            r + 12,
            106,
            40,
            30,
            theme::Rgb((c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32),
        );
        o.button(
            [r + 62, 106, w - 74, 26],
            &format!("Palette index {}", self.brush),
            Action::MaterialPrompt(0),
            false,
        );
        let rgb = self
            .palette_rgb()
            .map(|v| format!("RGB {} {} {} / 0..63", v[0], v[1], v[2]))
            .unwrap_or_else(|_| "Edit saved palette color".into());
        o.button(
            [r + 12, 146, w - 24, 26],
            &rgb,
            Action::MaterialPrompt(1),
            false,
        );
        let palette = self
            .palette_target()
            .ok()
            .map(|(i, pic, _)| (&self.doc.archive.entries[i], pic));
        if let Some((entry, _)) = palette {
            label_fit(&mut o.canvas, r + 12, 192, w - 24, &entry.name, c::STEEL);
        }
        let private = palette.is_some_and(|(entry, pic)| {
            !pic && self.doc.archive.entries.iter().any(|e| {
                e.name.ends_with(".PT")
                    && entry.name == format!("{}.PAL", e.name.split('.').next().unwrap())
            })
        });
        label_fit(
            &mut o.canvas,
            r + 12,
            212,
            w - 24,
            if private {
                "Private PAL: editor preview only"
            } else {
                "Shared palette: affects all users"
            },
            if private { c::AMBER } else { c::INK_MUTED },
        );
        if self.selected_face.is_some() {
            o.button(
                [r + 12, 224, w - 24, 26],
                "Transform selected face UVs",
                Action::MaterialPrompt(2),
                false,
            );
            o.button(
                [r + 12, 260, w - 24, 26],
                "Clone texture across aircraft family",
                Action::MaterialPrompt(3),
                false,
            );
        }
        if self.model.is_some() {
            o.button(
                [r + 12, 300, w - 24, 26],
                "Base color / untextured panels",
                Action::BaseColor(false),
                false,
            );
        }
        o.button(
            [r + 12, 344, w - 24, 26],
            "Load display palette",
            Action::File(FileAction::Palette),
            false,
        );
        for (i, s) in [
            "UV edits keep record sizes intact.",
            "Family cloning includes stored LODs",
            "and damage-shape texture references.",
            "Shared shapes still affect other users.",
            "New aircraft makes private resources.",
        ]
        .iter()
        .enumerate()
        {
            label_fit(
                &mut o.canvas,
                r + 12,
                396 + i as i32 * 22,
                w - 24,
                s,
                c::INK_FAINT,
            );
        }
        if self.pic.is_some() {
            o.button(
                [r + 12, h - 56, w - 24, 26],
                "Export PNG",
                Action::File(FileAction::Png),
                false,
            );
        }
    }
    pub(super) fn decal_inspector(&self, o: &mut Layout) {
        let (r, w, h) = (self.right(), self.width - self.right(), self.height);
        let p = self.decal_placement;
        label_fit(
            &mut o.canvas,
            r + 12,
            107,
            w - 24,
            if self.decal_name.is_empty() {
                "DECAL ARTWORK"
            } else {
                &self.decal_name
            },
            c::STEEL,
        );
        o.button(
            [r + 12, 118, w - 24, 24],
            "Import PNG / squadron artwork",
            Action::File(FileAction::Decal),
            false,
        );
        o.button(
            [r + 12, 148, w - 24, 24],
            &format!("Squadron / PNG library ({})", self.decal_paths.len()),
            Action::DecalLibrary,
            false,
        );
        o.button(
            [r + 12, 178, w - 24, 24],
            "Tail number / text",
            Action::DecalText,
            false,
        );
        o.button(
            [r + 12, 208, w - 66, 24],
            decal::NATIONAL_NAMES[self.decal_preset],
            Action::DecalPreset(false),
            false,
        );
        o.button(
            [self.width - 46, 208, 34, 24],
            ">",
            Action::DecalPreset(true),
            false,
        );
        for (key, y, title) in [
            (0, 242, format!("X {}", p.center[0])),
            (1, 242, format!("Y {}", p.center[1])),
        ] {
            let bw = (w - 30) / 2;
            let x = r + 12 + key as i32 * (bw + 6);
            o.button([x, y, bw, 24], &title, Action::DecalSetting(key), false);
        }
        for (key, y, title) in [
            (2, 272, format!("Width {} px", p.width)),
            (3, 302, format!("Rotation {} deg", p.degrees)),
            (4, 332, format!("Opacity {} %", p.opacity)),
        ] {
            o.button(
                [r + 12, y, w - 24, 24],
                &title,
                Action::DecalSetting(key),
                false,
            );
        }
        o.button(
            [r + 12, 362, w - 24, 24],
            if p.mirror {
                "[x] Mirror horizontally"
            } else {
                "[ ] Mirror horizontally"
            },
            Action::DecalMirror,
            p.mirror,
        );
        o.button(
            [r + 12, 392, w - 24, 24],
            &format!("Text ink / palette index {}", self.text_ink()),
            Action::DecalInk,
            false,
        );
        if let Some(image) = &self.decal_image {
            let height = (h - 570).clamp(24, 120);
            let width = w - 24;
            let mut pixels = Vec::with_capacity((width * height) as usize);
            let scale = (width as usize * 1024 / image.width)
                .min(height as usize * 1024 / image.height)
                .max(1);
            let iw = (image.width * scale / 1024).max(1) as i32;
            let ih = (image.height * scale / 1024).max(1) as i32;
            let ox = (width - iw) / 2;
            let oy = (height - ih) / 2;
            for yy in 0..height {
                for xx in 0..width {
                    let bg = if (xx / 8 + yy / 8) % 2 == 0 { 48 } else { 32 };
                    let src = if xx >= ox && yy >= oy && xx < ox + iw && yy < oy + ih {
                        image.rgba[(yy - oy) as usize * image.height / ih as usize * image.width
                            + (xx - ox) as usize * image.width / iw as usize]
                    } else {
                        [0; 4]
                    };
                    let color: [u8; 3] = core::array::from_fn(|i| {
                        ((src[i] as u32 * src[3] as u32 + bg * (255 - src[3] as u32)) / 255) as u8
                    });
                    pixels.push((color[0] as u32) << 16 | (color[1] as u32) << 8 | color[2] as u32);
                }
            }
            o.canvas.commands.push(Draw::Bitmap(
                r + 12,
                430,
                width as usize,
                height as usize,
                pixels,
            ));
        }
        let label = self
            .decal_draft
            .as_ref()
            .map_or("Click model/atlas to preview".into(), |d| {
                format!("{} opaque pixels / shared UVs", d.changed)
            });
        label_fit(&mut o.canvas, r + 12, h - 116, w - 24, &label, c::AMBER);
        let half = (w - 30) / 2;
        o.button(
            [r + 12, h - 92, half, 24],
            "Place decal",
            Action::DecalPlace,
            self.decal_active,
        );
        o.button(
            [r + 18 + half, h - 92, half, 24],
            "Cancel",
            Action::DecalCancel,
            false,
        );
        if self.decal_draft.as_ref().is_some_and(|d| d.changed > 0) {
            o.button(
                [r + 12, h - 60, w - 24, 28],
                "Apply decal",
                Action::DecalApply,
                true,
            );
        } else {
            label_fit(
                &mut o.canvas,
                r + 12,
                h - 40,
                w - 24,
                "Preview before Apply / Esc cancels",
                c::INK_MUTED,
            );
        }
    }
    pub(super) fn decal_library_layout(&self, o: &mut Layout) {
        let (w, h) = ((self.width - 40).min(720), (self.height - 70).min(500));
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        o.canvas.rect(x, y, w, h, c::GM_800);
        border(&mut o.canvas, x, y, w, h, c::LINE_STRONG);
        o.canvas
            .label(x + 14, y + 24, "SQUADRON / IMPORTED PNG LIBRARY", c::INK);
        if self.decal_paths.is_empty() {
            label_fit(
                &mut o.canvas,
                x + 14,
                y + 64,
                w - 28,
                "Import a PNG first; original files remain on disk.",
                c::INK_MUTED,
            );
        }
        for (i, path) in self
            .decal_paths
            .iter()
            .take(((h - 90) / 24) as usize)
            .enumerate()
        {
            let yy = y + 42 + i as i32 * 24;
            o.button(
                [x + 14, yy, w - 70, 22],
                path.rsplit(['/', '\\']).next().unwrap_or(path),
                Action::DecalSaved(i),
                false,
            );
            o.button([x + w - 46, yy, 32, 22], "x", Action::DecalForget(i), false);
        }
        o.button([x + 14, y + h - 40, 90, 26], "Close", Action::Cancel, false);
    }
}

#[inline(never)]
fn material_app() -> Box<App> {
    Box::new(App::new())
}
impl App {
    #[inline(never)]
    pub(super) fn smoke_material_tools(&mut self) {
        // Heap-allocate the editor so this frame stays below the 4 KiB probe limit.
        let mut a = material_app();
        a.demo();
        let original = a.doc.archive.bytes().unwrap();
        a.select_entry(1);
        a.act(Action::Hardpoints);
        let hit = a
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::HardpointSelect(0)))
            .expect("Station marker missing");
        let x = hit.rect[0] + 9;
        let y = hit.rect[1] + 9;
        a.click(x, y, 1, true);
        a.click(x, y, 1, false);
        assert!(!a.doc.dirty());
        a.click(x, y, 1, true);
        a.motion(x + 22, y - 16, false);
        assert!(!a.doc.dirty());
        a.click(x + 22, y - 16, 1, false);
        assert!(a.doc.dirty());
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.station_add(true, false).unwrap();
        assert_eq!(a.hp_context.as_ref().unwrap().stations.len(), 3);
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.station_value(12, "AIM9.JT").unwrap();
        assert_eq!(
            a.hp_context.as_ref().unwrap().stations[a.hp_selected]
                .store
                .as_deref(),
            Some("AIM9.JT")
        );
        a.act(Action::Undo);
        a.select_entry(0);
        a.selected_face = a
            .model
            .as_ref()
            .unwrap()
            .faces
            .iter()
            .position(|f| !f.uv.is_empty());
        let texture = a.texture_target().unwrap();
        a.open_texture(texture);
        let face = a.selected_face.unwrap();
        let uv = a.context_model.as_ref().unwrap().faces[face].uv.clone();
        a.uv_edit("1 2 100 100 0").unwrap();
        assert_ne!(a.context_model.as_ref().unwrap().faces[face].uv, uv);
        a.act(Action::Undo);
        assert_eq!(a.context_model.as_ref().unwrap().faces[face].uv, uv);
        a.brush = 17;
        a.palette_edit("63 2 0").unwrap();
        assert_eq!(a.palette_rgb().unwrap(), [63, 2, 0]);
        // PIC palette edits keep the texture's original in the same undo step.
        assert!(a.doc.archive.find("DEMO.ORG").is_some());
        a.act(Action::Undo);
        assert!(a.doc.archive.find("DEMO.ORG").is_none());
        let before = a.draw().commands;
        a.set_decal(
            Image::national(0).unwrap(),
            "US stars and bars".into(),
            false,
        )
        .unwrap();
        assert!(a.decal_draft.as_ref().unwrap().changed > 0);
        assert!(!a.doc.dirty());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let after = a.draw().commands;
        assert!(before.iter().zip(&after).any(|(l, r)| match (l, r) {
            (Draw::Bitmap(_, _, _, _, a), Draw::Bitmap(_, _, _, _, b)) => a != b,
            _ => false,
        }));
        a.decal_setting(3, "90").unwrap();
        a.decal_setting(4, "70").unwrap();
        a.apply_decal().unwrap();
        assert!(a.doc.dirty());
        let org = a
            .doc
            .archive
            .find("DEMO.ORG")
            .expect("decal keeps the original");
        assert!(a.doc.archive.entries[org].same_payload(a.doc.saved_entry("DEMO.PIC").unwrap()));
        let encoded = a.doc.archive.bytes().unwrap();
        let reopened = Archive::parse(encoded).unwrap();
        assert_ne!(
            reopened.entries[texture].read().unwrap(),
            a.doc.saved_entry("DEMO.PIC").unwrap().read().unwrap()
        );
        a.act(Action::Undo);
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        a.tail_text("AF 001").unwrap();
        assert!(a.decal_is_text);
        a.key(Key::Escape, false, false);
        assert!(a.decal_draft.is_none());
        assert_eq!(a.doc.archive.bytes().unwrap(), original);
        let png = picture::demo();
        let png = Pic::parse(&png).unwrap().png(&a.base_palette);
        crate::platform::write_new("HG_DECAL.PNG", &png).unwrap();
        let result = a.load_decal("HG_DECAL.PNG", false);
        crate::platform::remove_file("HG_DECAL.PNG").unwrap();
        result.unwrap();
        assert!(a.decal_image.is_some());
        a.decal_paths = (0..16).map(|i| format!("Squadron{i}.png")).collect();
        for (w, h) in [(800, 600), (1280, 800)] {
            a.width = w;
            a.height = h;
            for tab in [0, 1, 2] {
                a.media_tab = tab;
                for hit in a.layout().hits {
                    assert!(
                        hit.rect[0] >= 0
                            && hit.rect[1] >= 0
                            && hit.rect[0] + hit.rect[2] <= w
                            && hit.rect[1] + hit.rect[3] <= h,
                        "Material control outside window"
                    );
                }
            }
            a.act(Action::DecalLibrary);
            assert!(a
                .layout()
                .hits
                .iter()
                .any(|h| matches!(h.action, Action::DecalSaved(15))));
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h
                );
            }
            a.key(Key::Escape, false, false);
            a.select_entry(1);
            a.hp_tool = true;
            a.hp_visible = true;
            for hit in a.layout().hits {
                assert!(
                    hit.rect[0] >= 0
                        && hit.rect[1] >= 0
                        && hit.rect[0] + hit.rect[2] <= w
                        && hit.rect[1] + hit.rect[3] <= h,
                    "Station control outside window"
                );
            }
            a.select_entry(0);
            a.selected_face = a
                .model
                .as_ref()
                .unwrap()
                .faces
                .iter()
                .position(|f| !f.uv.is_empty());
            a.open_texture(texture);
        }
    }
}

#[cfg(not(windows))]
impl App {
    pub fn check_decal_import(&mut self, path: &str, output: &str) -> Result<String> {
        let before = self.doc.archive.bytes()?;
        let source = self.data.clone();
        let original = Pic::parse(&source)?;
        self.load_decal(path, false)?;
        let draft = self.decal_draft.as_ref().ok_or("Choose a PIC target")?;
        let changed = draft.changed;
        let bytes = draft.bytes.clone();
        let after = draft.pic.clone();
        if source[..64] != bytes[..64]
            || original.mask != after.mask
            || original.palette != after.palette
        {
            return Err("Decal changed image metadata".into());
        }
        self.apply_decal()?;
        let packed = Archive::parse(self.doc.archive.bytes()?)?;
        if packed.entries[packed.find(self.name()).ok_or("Texture missing")?].read()? != bytes {
            return Err("Decal package did not round-trip".into());
        }
        crate::platform::write_new(output, &after.png(&self.base_palette))?;
        self.doc.undo();
        self.refresh();
        if self.doc.archive.bytes()? != before {
            return Err("Decal undo did not restore source".into());
        }
        Ok(format!("PASS: {changed} changed pixels; PNG decode, metadata preservation, package reopen and one-step undo"))
    }
}
