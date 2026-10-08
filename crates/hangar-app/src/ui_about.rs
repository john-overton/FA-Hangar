//! Help -> About: the app icon, name, version and build, shown as a prompt
//! dialog (`PromptKind::About`) so it is modal like the others.
use super::view::{Action, Layout, DIALOG_HEAD};
use super::*;

const ICO: &[u8] = include_bytes!("../../../fa-hangar-design/icons/app/fa-hangar.ico");
const ICON: usize = 48;
const SITE: &str = "https://github.com/john-overton/FA-Hangar";
const BLURB: &str = "Edit Fighters Anthology LIBs and the shapes, textures and definitions inside them. Nothing is written until you save.";

/// The platform this executable was built for.
pub(super) fn build_target() -> &'static str {
    if cfg!(windows) {
        if cfg!(target_arch = "x86") {
            "Windows 98/ME (i686)"
        } else if cfg!(target_arch = "x86_64") {
            "Windows x64"
        } else {
            "Windows"
        }
    } else {
        "Linux"
    }
}

/// The 48px 32-bit entry of the committed app icon as 0xRRGGBB rows, top
/// down, with transparent pixels replaced by `ground`.
fn app_icon(ground: Rgb) -> Option<Vec<u32>> {
    let int = |at: usize, len: usize| -> Option<usize> {
        let b = ICO.get(at..at.checked_add(len)?)?;
        Some(b.iter().rev().fold(0, |v, &x| v << 8 | x as usize))
    };
    for i in 0..int(4, 2)?.min(32) {
        let entry = 6 + i * 16;
        // Width byte, 32 bits per pixel, DIB header of 40 bytes.
        let (n, offset) = (int(entry, 1)?, int(entry + 12, 4)?);
        if n != ICON || int(entry + 6, 2)? != 32 || int(offset, 4)? != 40 {
            continue;
        }
        let pixels = ICO.get(offset + 40..offset + 40 + n * n * 4)?;
        let mut out = Vec::with_capacity(n * n);
        // DIB rows run bottom-up; pixels are BGRA.
        for row in pixels.chunks_exact(n * 4).rev() {
            for p in row.chunks_exact(4) {
                out.push(if p[3] < 128 {
                    ground.0
                } else {
                    (p[2] as u32) << 16 | (p[1] as u32) << 8 | p[0] as u32
                });
            }
        }
        return Some(out);
    }
    None
}

impl App {
    pub(super) fn about_open(&mut self) {
        self.menu = None;
        self.prompt = Some(Prompt {
            kind: PromptKind::About,
            title: "About F.A. Hangar".into(),
            value: String::new(),
            axis: 0,
        });
    }
    pub(super) fn about_dialog(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, wrap, Btn};
        let w = (self.width - 48).min(480);
        let inner = w - 2 * space::SPACE_4;
        let blurb = wrap(BLURB, inner, Style::Body);
        let rows = [
            ("Build", build_target()),
            ("License", env!("CARGO_PKG_LICENSE")),
            ("Project", SITE),
        ];
        let h = DIALOG_HEAD
            + space::SPACE_3
            + ICON as i32
            + space::SPACE_4
            + rows.len() as i32 * m::ROW_H
            + space::SPACE_3
            + blurb.len() as i32 * 18
            + space::SPACE_4
            + m::BUTTON_H
            + space::SPACE_4;
        let rect = [(self.width - w) / 2, (self.height - h) / 2, w, h];
        o.hits.clear();
        let title = self.prompt.as_ref().map_or("", |p| p.title.as_str());
        let [bx, by, bw, _] = self.dialog_frame(o, rect, title);
        if let Some(pixels) = app_icon(c::GM_800) {
            o.canvas
                .commands
                .push(Draw::Bitmap(bx, by, ICON, ICON, pixels));
        }
        let tx = bx + ICON as i32 + space::SPACE_4;
        o.canvas.styled(
            tx,
            by + 22,
            &fit("F.A. Hangar", bx + bw - tx, Style::Display),
            c::INK,
            Style::Display,
        );
        o.canvas.styled(
            tx,
            by + 42,
            &format!("Version {}", env!("CARGO_PKG_VERSION")),
            c::INK,
            Style::Value,
        );
        let mut y = by + ICON as i32 + space::SPACE_4;
        let col = 72;
        for (label, value) in rows {
            o.canvas.styled(
                bx,
                baseline(y, m::ROW_H, Style::Label),
                label,
                c::INK_MUTED,
                Style::Label,
            );
            o.canvas.styled(
                bx + col,
                baseline(y, m::ROW_H, Style::Value),
                &fit(value, bw - col, Style::Value),
                c::INK,
                Style::Value,
            );
            y += m::ROW_H;
        }
        y += space::SPACE_3;
        for line in &blurb {
            o.canvas.styled(bx, y + 13, line, c::INK_MUTED, Style::Body);
            y += 18;
        }
        self.dialog_actions(
            o,
            rect,
            &[],
            None,
            Some(Btn::new("Close").primary()),
            Action::Apply,
        );
    }
}

impl App {
    /// Help -> About through the menu and hit regions at both minimum and
    /// large window sizes; Close, Esc and Enter dismiss it.
    pub(super) fn smoke_about(&mut self) {
        for (w, h) in [(800, 600), (1280, 800)] {
            self.demo();
            self.width = w;
            self.height = h;
            self.mode = Mode::Model;
            let help = self
                .chrome_hit(&|a| matches!(a, Action::Menu(6)))
                .expect("Help menu");
            for close in 0..3 {
                self.chrome_click(help);
                assert_eq!(self.menu, Some(6));
                let item = self
                    .chrome_hit(&|a| matches!(a, Action::About))
                    .expect("About item in the Help menu");
                self.chrome_click(item);
                assert!(
                    self.prompt
                        .as_ref()
                        .is_some_and(|p| matches!(p.kind, PromptKind::About))
                        && self.menu.is_none(),
                    "About opens from Help"
                );
                self.smoke_geometry("about");
                let texts: Vec<(i32, i32, String, Style)> = self
                    .draw()
                    .commands
                    .iter()
                    .filter_map(|d| match d {
                        Draw::Text(x, y, s, _, st) => Some((*x, *y, s.clone(), *st)),
                        _ => None,
                    })
                    .collect();
                let has = |s: &str| texts.iter().any(|t| t.2 == s);
                assert!(has(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
                assert!(has(build_target()) && has("GPL-3.0-only") && has(SITE));
                assert!(texts
                    .iter()
                    .any(|t| t.2 == "F.A. Hangar" && t.3 == Style::Display));
                assert!(
                    self.draw().commands.iter().any(|d| matches!(
                        d,
                        Draw::Bitmap(_, _, bw, bh, p)
                            if *bw == ICON && *bh == ICON && p.len() == ICON * ICON
                    )),
                    "App icon missing"
                );
                for (x, y, s, st) in &texts {
                    assert!(
                        *x >= 0 && x + text_width(s, *st) <= w && *y >= 0 && *y <= h,
                        "About text {s} outside {w}x{h}"
                    );
                }
                match close {
                    0 => {
                        let b = self
                            .chrome_hit(&|a| matches!(a, Action::Apply))
                            .expect("Close button");
                        self.chrome_click(b);
                    }
                    1 => self.key(Key::Escape, false, false),
                    _ => self.key(Key::Enter, false, false),
                }
                assert!(self.prompt.is_none(), "About closes");
                assert_ne!(self.status, "Cancelled");
            }
        }
    }
}
