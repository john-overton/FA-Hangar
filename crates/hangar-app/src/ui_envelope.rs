//! An editable point table, retaining exact BRF operands and the raw-fields dock.
use super::view::{Action, Icon, Layout};
use super::widgets::{baseline, Btn, Number, NumberTarget};
use super::*;
use theme::{metric as m, space};
/// Top of the point table below the envelope header, cells and column head.
const TABLE_TOP: i32 = m::EDITOR_HEADER_H + space::SPACE_2 + 2 * m::ROW_H + space::SPACE_1;
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
    /// Point rows that fit between the table head and the hint line.
    pub(super) fn envelope_visible(&self) -> usize {
        let h = self.dock_y() - m::MENUBAR_H;
        ((h - TABLE_TOP - space::SPACE_2 - 2 * m::ROW_H) / m::ROW_H).max(1) as usize
    }
    /// A BRF field cell: NumberField, or a text field for `$hex` operands.
    fn envelope_cell(&self, o: &mut Layout, rect: [i32; 4], index: usize, label: &str) {
        let t = NumberTarget::Field(index);
        let f = &self.brf.as_ref().unwrap().fields[index];
        match self.number_spec(t) {
            Some(spec) => o.number(
                rect,
                &Number {
                    target: t,
                    spec,
                    label,
                    unit: if f.scaled { "scaled" } else { "" },
                    locked: false,
                    axis: None,
                },
            ),
            None => o.text_field(
                rect,
                &format!("{label} {}", f.value),
                self.envelope_field_changed(index),
                Action::Field(index),
            ),
        }
    }
    pub(super) fn envelope_layout(&self, o: &mut Layout, x: i32, y: i32, w: i32, h: i32) {
        let rows = self.envelope_rows();
        let selected = self.envelope_selected.min(rows.len().saturating_sub(1));
        let Some(row) = rows.get(selected) else {
            return;
        };
        let b = self.brf.as_ref().unwrap();
        o.canvas.rect(x, y, w, h, c::GM_800);
        // Header: previous / next envelope around "Envelope N of M".
        let hy = y + (m::EDITOR_HEADER_H - m::ICON_BUTTON) / 2;
        o.canvas
            .rect(x, y + m::EDITOR_HEADER_H - 1, w, 1, c::GM_1000);
        o.icon_button(
            x + space::SPACE_1,
            hy,
            Btn::icon(Icon::ChevronLeft).ghost(),
            Action::EnvelopeStep(-1),
        );
        o.icon_button(
            x + w - space::SPACE_1 - m::ICON_BUTTON,
            hy,
            Btn::icon(Icon::ChevronRight).ghost(),
            Action::EnvelopeStep(1),
        );
        let title = format!("Envelope {} of {}", selected + 1, rows.len());
        let tx = x + space::SPACE_1 + m::ICON_BUTTON + space::SPACE_2;
        o.canvas.styled(
            tx,
            baseline(y, m::EDITOR_HEADER_H, Style::Strong),
            &fit(&title, w - 2 * (tx - x), Style::Strong),
            c::INK,
            Style::Strong,
        );
        // Envelope header values: two rows of two NumberFields.
        let cw = (w - 2 * space::SPACE_3 - space::SPACE_2) / 2;
        for (n, (label, index)) in [
            ("G", row[0]),
            ("Points", row[1]),
            ("Stall lift", row[2]),
            ("Max speed", row[3]),
        ]
        .into_iter()
        .enumerate()
        {
            let cx = x + space::SPACE_3 + (n as i32 % 2) * (cw + space::SPACE_2);
            let cy = y
                + m::EDITOR_HEADER_H
                + space::SPACE_2
                + (n as i32 / 2) * (m::ROW_H + space::SPACE_1);
            self.envelope_cell(o, [cx, cy, cw, m::FIELD_H], index, label);
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
        // Point table: #, speed, altitude; rows past Points are unused.
        let head = y + TABLE_TOP;
        let px = x + space::SPACE_3;
        let sx = x + 52;
        let ax = x + (w + 52) / 2;
        o.canvas.rect(x, head, w, m::ROW_H, c::GM_900);
        for (xx, label) in [(px, "#"), (sx, "SPEED"), (ax, "ALTITUDE")] {
            o.canvas.styled(
                xx,
                baseline(head, m::ROW_H, Style::Section),
                label,
                c::INK_MUTED,
                Style::Section,
            );
        }
        let start = head + m::ROW_H;
        for n in (0..20)
            .skip(self.envelope_scroll)
            .take(self.envelope_visible())
        {
            let yy = start + (n - self.envelope_scroll) as i32 * m::ROW_H;
            let used = n < points;
            o.canvas.rect(
                x,
                yy,
                w,
                m::ROW_H,
                if n % 2 == 0 { c::GM_800 } else { c::GM_900 },
            );
            o.canvas.styled(
                px,
                baseline(yy, m::ROW_H, Style::Value),
                &format!("{}", n + 1),
                if used { c::INK } else { c::INK_MUTED },
                Style::Value,
            );
            for (xx, index, ww) in [
                (sx, row[4 + n * 2], ax - sx - space::SPACE_2),
                (ax, row[5 + n * 2], x + w - space::SPACE_3 - ax),
            ] {
                self.envelope_cell(o, [xx, yy, ww, m::ROW_H], index, "");
            }
        }
        o.canvas.styled(
            x + space::SPACE_3,
            baseline(y + h - m::ROW_H - space::SPACE_1, m::ROW_H, Style::Label),
            &{
                let long = format!("{points} of 20 points used. Drag to scrub, click to type.");
                if text_width(&long, Style::Label) <= w - 2 * space::SPACE_3 {
                    long
                } else {
                    fit(
                        &format!("{points} of 20 points used."),
                        w - 2 * space::SPACE_3,
                        Style::Label,
                    )
                }
            },
            c::INK_MUTED,
            Style::Label,
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
