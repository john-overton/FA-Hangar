use super::view::{border, label_fit, Action, Layout};
use super::*;
impl App {
    pub(super) fn dominant_color(&self) -> Option<u8> {
        let mut counts = BTreeMap::<u8, usize>::new();
        for f in &self.model.as_ref().or(self.context_model.as_ref())?.faces {
            if f.sub & 4 == 0 {
                *counts.entry(f.color).or_default() += 1;
            }
        }
        counts.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c)
    }
    pub(super) fn base_color_prompt(&mut self, face: bool) {
        let color = if face {
            self.selected_face
                .and_then(|i| {
                    self.model
                        .as_ref()
                        .or(self.context_model.as_ref())
                        .and_then(|m| m.faces.get(i))
                })
                .filter(|f| f.sub & 4 == 0)
                .map(|f| f.color)
        } else {
            self.dominant_color()
        };
        let Some(color) = color else {
            self.media_tab = 1;
            self.status = "This surface uses a texture; edit its PIC in Paint / Materials".into();
            return;
        };
        self.base_color_from = color;
        self.brush = color;
        self.textured = true;
        self.perspective = false;
        self.prompt = Some(Prompt {
            kind: PromptKind::BaseColor(face),
            title: if face {
                "Panel color".into()
            } else {
                "Base color / matching visible flat-color faces".into()
            },
            value: String::new(),
            axis: 0,
        });
    }
    pub(super) fn apply_base_color(&mut self, face: bool) -> Result<()> {
        let entry = self
            .model_entry
            .or(self.context_entry)
            .ok_or("Open the model owner's LIB before editing colors")?;
        let source = self.doc.archive.entries[entry].read()?;
        let bytes = if face {
            Model::recolor_face(
                &source,
                self.selected_face.ok_or("Select a face")?,
                self.brush,
            )?
        } else {
            Model::recolor(&source, self.base_color_from, self.brush)?.0
        };
        self.doc.replace(entry, bytes)?;
        self.refresh();
        self.status = "Model color changed / one undo step / textures remain separate".into();
        Ok(())
    }
    pub(super) fn color_dialog(&self, o: &mut Layout) {
        let (w, h) = ((self.width - 40).min(600), (self.height - 60).min(610));
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        let cell = ((w - 28) / 16).min((h - 128) / 16);
        let gx = x + (w - cell * 16) / 2;
        o.hits.clear();
        o.canvas.rect(x, y, w, h, c::GM_800);
        border(&mut o.canvas, x, y, w, h, c::LINE_STRONG);
        label_fit(
            &mut o.canvas,
            x + 14,
            y + 24,
            w - 28,
            self.prompt
                .as_ref()
                .map_or("Base color", |p| p.title.as_str()),
            c::INK,
        );
        label_fit(
            &mut o.canvas,
            x + 14,
            y + 49,
            w - 28,
            &format!("Palette index {} -> {}", self.base_color_from, self.brush),
            c::AMBER,
        );
        for i in 0..256 {
            let xx = gx + (i % 16) * cell;
            let yy = y + 66 + (i / 16) * cell;
            let rgb = self.base_palette[i as usize];
            o.canvas.rect(
                xx,
                yy,
                cell - 1,
                cell - 1,
                theme::Rgb((rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32),
            );
            if i == self.brush as i32 {
                border(&mut o.canvas, xx - 1, yy - 1, cell + 1, cell + 1, c::INK);
            }
            o.hit([xx, yy, cell, cell], Action::Brush(i as u8));
        }
        label_fit(
            &mut o.canvas,
            x + 14,
            y + h - 49,
            w - 28,
            if self.palette_loaded {
                "Shared SH users see this color change."
            } else {
                "Palette missing: indices shown in grayscale."
            },
            c::INK_MUTED,
        );
        o.button(
            [x + 14, y + h - 37, 92, 26],
            "Cancel",
            Action::Cancel,
            false,
        );
        o.button(
            [x + w - 150, y + h - 37, 136, 26],
            "Apply color",
            Action::Apply,
            true,
        );
    }
}
