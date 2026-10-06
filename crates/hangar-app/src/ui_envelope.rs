//! An editable point table, retaining exact BRF operands and the raw-fields dock.
use super::view::{label_fit, text_fit, Action, Layout};
use super::*;
impl App {
    pub(super) fn envelope_rows(&self) -> Vec<[usize; 44]> {
        let mut rows = Vec::new();
        let Some(brf) = &self.brf else {
            return rows;
        };
        for n in 0..51 {
            let prefix = format!("envelope[{n}].");
            let fields: Vec<_> = brf
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.label.starts_with(&prefix))
                .map(|(i, _)| i)
                .collect();
            if let Ok(row) = fields.try_into() {
                rows.push(row);
            }
        }
        rows
    }
    pub(super) fn envelope_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        let rows = self.envelope_rows();
        let Some(row) = rows.get(self.envelope_selected.min(rows.len().saturating_sub(1))) else {
            return;
        };
        let b = self.brf.as_ref().unwrap();
        let val = |i: usize| {
            format!(
                "{}{}",
                if b.fields[i].scaled { "^" } else { "" },
                b.fields[i].value
            )
        };
        o.canvas.rect(x, y, w, h, c::GM_800);
        o.canvas
            .label(x + 12, y + 19, "Flight envelope / source values", c::INK);
        o.button(
            [x + 10, y + 29, 36, 24],
            "<",
            Action::EnvelopeStep(-1),
            false,
        );
        text_fit(
            &mut o.canvas,
            x + 58,
            y + 46,
            w - 116,
            &format!(
                "G {} / row {} of {}",
                val(row[0]),
                self.envelope_selected.min(rows.len() - 1) + 1,
                rows.len()
            ),
            c::STEEL,
        );
        o.button(
            [x + w - 46, y + 29, 36, 24],
            ">",
            Action::EnvelopeStep(1),
            false,
        );
        let cell = (w - 24) / 3;
        for (n, (label, index)) in [
            ("Points", row[1]),
            ("Stall lift", row[2]),
            ("Max speed", row[3]),
        ]
        .into_iter()
        .enumerate()
        {
            let xx = x + 8 + n as i32 * (cell + 4);
            label_fit(&mut o.canvas, xx, y + 72, cell, label, c::INK_MUTED);
            o.button(
                [xx, y + 80, cell, 24],
                &val(index),
                Action::Field(index),
                self.envelope_field_changed(index),
            );
        }
        let points = b.fields[row[1]]
            .value
            .parse::<usize>()
            .ok()
            .or_else(|| {
                b.fields[row[1]]
                    .value
                    .strip_prefix('$')
                    .and_then(|v| usize::from_str_radix(v, 16).ok())
            })
            .unwrap_or(0)
            .min(20);
        let start = y + 138;
        let px = x + 8;
        let sx = x + 52;
        let ax = x + w / 2 + 14;
        o.canvas.rect(x, y + 112, w, 24, c::GM_900);
        for (xx, label) in [(px, "#"), (sx, "SPEED"), (ax, "ALTITUDE")] {
            o.canvas.label(xx, y + 129, label, c::INK_MUTED);
        }
        let visible = ((h - 164) / 26).max(1) as usize;
        for n in (0..20).skip(self.envelope_scroll).take(visible) {
            let yy = start + (n - self.envelope_scroll) as i32 * 26;
            let active = n < points;
            o.canvas
                .rect(x, yy, w, 25, if n % 2 == 0 { c::GM_900 } else { c::GM_800 });
            o.canvas.text(
                px,
                yy + 17,
                &format!("{}", n + 1),
                if active { c::INK } else { c::INK_FAINT },
            );
            for (xx, index, ww) in [
                (sx, row[4 + n * 2], ax - sx - 8),
                (ax, row[5 + n * 2], x + w - ax - 10),
            ] {
                text_fit(
                    &mut o.canvas,
                    xx + 5,
                    yy + 17,
                    ww - 10,
                    &val(index),
                    if self.envelope_field_changed(index) {
                        c::AMBER
                    } else if active {
                        c::INK
                    } else {
                        c::INK_FAINT
                    },
                );
                o.hit([xx, yy, ww, 25], Action::Field(index));
            }
        }
        label_fit(
            &mut o.canvas,
            x + 10,
            y + h - 10,
            w - 20,
            "Dim rows are unused / wheel scroll / click to edit",
            c::INK_FAINT,
        );
    }
    fn envelope_field_changed(&self, index: usize) -> bool {
        let Some(f) = self.brf.as_ref().and_then(|b| b.fields.get(index)) else {
            return false;
        };
        self.original_brf
            .as_ref()
            .and_then(|b| b.fields.iter().find(|old| old.label == f.label))
            .is_none_or(|old| old.value != f.value || old.scaled != f.scaled)
    }
}
