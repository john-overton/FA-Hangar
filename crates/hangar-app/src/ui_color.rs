use super::view::{Action, Layout};
use super::*;
impl App {
    pub(super) fn dominant_color(&self) -> Option<u8> {
        let mut counts = BTreeMap::<u8, usize>::new();
        for f in &self.model.as_ref().or(self.context_model.as_deref())?.faces {
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
                        .or(self.context_model.as_deref())
                        .and_then(|m| m.faces.get(i))
                })
                .filter(|f| f.sub & 4 == 0)
                .map(|f| f.color)
        } else {
            self.dominant_color()
        };
        let Some(color) = color else {
            self.media_tab = 1;
            self.status = "This surface uses a texture; edit its PIC in Paint or Materials".into();
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
                "Base color of the visible flat-color faces".into()
            },
            value: String::new(),
            axis: 0,
            caret: super::Caret::END,
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
        self.status = "Model color changed in one undo step; textures are unchanged".into();
        Ok(())
    }
    pub(super) fn color_dialog(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, Btn};
        let (w, h) = ((self.width - 40).min(600), (self.height - 60).min(610));
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        let title = self
            .prompt
            .as_ref()
            .map_or("Base color", |p| p.title.as_str());
        let [bx, by, bw, bh] = self.dialog_frame(o, [x, y, w, h], title);
        o.canvas.styled(
            bx,
            baseline(by, m::ROW_H, Style::Value),
            &format!("Index {} \u{2192} {}", self.base_color_from, self.brush),
            c::INK,
            Style::Value,
        );
        let grid_top = by + m::ROW_H + space::SPACE_2;
        let room = bh - m::ROW_H - space::SPACE_2 - 2 * m::ROW_H - m::BUTTON_H;
        let cell = (bw / 16).min(room / 16);
        let gx = x + (w - cell * 16) / 2;
        for i in 0..256 {
            let xx = gx + (i % 16) * cell;
            let yy = grid_top + (i / 16) * cell;
            let rgb = self.base_palette[i as usize];
            o.canvas.rect(
                xx,
                yy,
                cell - 1,
                cell - 1,
                theme::Rgb((rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32),
            );
            if i == self.brush as i32 {
                widgets::frame(&mut o.canvas, [xx - 1, yy - 1, cell + 1, cell + 1], c::INK);
            }
            o.hit([xx, yy, cell - 1, cell - 1], Action::Brush(i as u8));
        }
        o.canvas.styled(
            bx,
            baseline(
                grid_top + 16 * cell + space::SPACE_1,
                m::ROW_H,
                Style::Label,
            ),
            &fit(
                if self.palette_loaded {
                    "Every user of a shared SH sees this color change."
                } else {
                    "No palette loaded: indices show in grayscale."
                },
                bw,
                Style::Label,
            ),
            c::INK_MUTED,
            Style::Label,
        );
        self.dialog_actions(
            o,
            [x, y, w, h],
            &[],
            Some("Cancel"),
            Some(Btn::new("Apply color").primary()),
            Action::Apply,
        );
    }
}
