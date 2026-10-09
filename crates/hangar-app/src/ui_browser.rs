use super::view::{Action, Icon, Layout};
use super::*;
impl App {
    pub(super) fn browser_path(&self, value: &str) -> String {
        let value = value.trim();
        if !value.is_empty()
            && !value.contains(['/', '\\'])
            && value.as_bytes().get(1) != Some(&b':')
        {
            if let Some(b) = &self.browser {
                return format!("{}/{}", b.folder.trim_end_matches(['/', '\\']), value);
            }
        }
        value.into()
    }
    pub(super) fn parent_path(path: &str) -> String {
        let p = path.trim_end_matches(['/', '\\']);
        match p.rsplit_once(['/', '\\']) {
            Some(("", _)) => "/".into(),
            Some((dir, _)) if dir.ends_with(':') => format!("{dir}\\"),
            Some((dir, _)) => dir.into(),
            None => crate::platform::current_dir(),
        }
    }
    pub(super) fn remember(&mut self, path: &str) {
        let absolute = if path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
            path.into()
        } else {
            format!(
                "{}/{}",
                crate::platform::current_dir().trim_end_matches(['/', '\\']),
                path
            )
        };
        self.recent.retain(|p| p != &absolute);
        self.recent.insert(0, absolute);
        self.recent.truncate(8);
        let _ = crate::platform::save_recent(&self.recent);
    }
    pub(super) fn browse_folder(&mut self, path: &str) {
        match crate::platform::list_dir(path) {
            Ok(mut files) => {
                if let Some(p) = &self.prompt {
                    if let PromptKind::File(action) = p.kind {
                        let ext = match action {
                            FileAction::Open
                            | FileAction::Save
                            | FileAction::Graft
                            | FileAction::GraftLibrary
                            | FileAction::CloneSource
                            | FileAction::ReferenceSource => Some("LIB"),
                            FileAction::Decal => Some("PNG"),
                            FileAction::VariantSh => Some("SH"),
                            FileAction::Palette => Some("PAL"),
                            _ => None,
                        };
                        if let Some(ext) = ext {
                            files.retain(|f| {
                                f.directory
                                    || (matches!(action, FileAction::Palette)
                                        && f.name.to_ascii_uppercase().ends_with(".LIB"))
                                    || f.name
                                        .rsplit('.')
                                        .next()
                                        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
                            });
                        }
                    }
                }
                self.browser = Some(Browser {
                    folder: path.into(),
                    files,
                    scroll: 0,
                });
                if let Some(p) = &mut self.prompt {
                    if matches!(p.kind, PromptKind::File(_)) {
                        let save = matches!(
                            p.kind,
                            PromptKind::File(
                                FileAction::Save
                                    | FileAction::Png
                                    | FileAction::Wav
                                    | FileAction::Export
                                    | FileAction::Obj
                                    | FileAction::Report
                            )
                        );
                        let leaf = if save && !crate::platform::is_dir(&p.value) {
                            p.value.rsplit(['/', '\\']).next().unwrap_or("").to_string()
                        } else {
                            String::new()
                        };
                        p.value = format!("{}/{}", path.trim_end_matches(['/', '\\']), leaf);
                    }
                }
            }
            Err(e) => self.status = format!("Error: {e}"),
        }
    }
    pub(super) fn browse_pick(&mut self, i: usize) {
        let Some(item) = self.browser.as_ref().and_then(|b| b.files.get(i)).cloned() else {
            return;
        };
        if item.directory {
            self.browse_folder(&item.path);
        } else if let Some(p) = &mut self.prompt {
            p.value = item.path;
        }
    }
    pub(super) fn recent_open(&mut self, i: usize) {
        if let Some(path) = self.recent.get(i).cloned() {
            let action = self.prompt.as_ref().and_then(|p| {
                if let PromptKind::File(a) = p.kind {
                    Some(a)
                } else {
                    None
                }
            });
            if matches!(action, Some(FileAction::Open)) {
                let r = self.open(&path);
                self.result(r);
                if !self.status.starts_with("Error:") {
                    self.prompt = None;
                    self.browser = None;
                }
            } else {
                let folder = Self::parent_path(&path);
                self.browse_folder(&folder);
                if matches!(
                    action,
                    Some(FileAction::Palette | FileAction::Graft | FileAction::CloneSource)
                ) {
                    if let Some(p) = &mut self.prompt {
                        p.value = path;
                    }
                }
            }
        }
    }
    pub(super) fn browser_layout(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, notched, subhead, Btn};
        let Some(p) = &self.prompt else {
            return;
        };
        let Some(b) = &self.browser else {
            return;
        };
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(620);
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        let [bx, by, bw, _] = self.dialog_frame(o, [x, y, w, h], &p.title);
        let side = 172;
        let lx = bx + side;
        let lw = bw - side;
        // Navigation: Up, Drives, then the current folder.
        let up = Btn::new("Up").with_icon(Icon::ChevronLeft);
        let uw = up.width();
        o.button_ex([bx, by, uw, m::BUTTON_H], up, Action::BrowserUp);
        let drives = Btn::new("Drives").with_icon(Icon::Folder);
        o.button_ex(
            [
                bx + uw + space::SPACE_1,
                by,
                side - uw - space::SPACE_1 - space::SPACE_2,
                m::BUTTON_H,
            ],
            drives,
            Action::BrowserRoots,
        );
        let d = &mut o.canvas;
        notched(
            d,
            [lx, by, lw, m::BUTTON_H],
            Some(c::GM_950),
            Some(c::LINE_STRONG),
        );
        d.styled(
            lx + 8,
            baseline(by, m::BUTTON_H, Style::Value),
            &fit(&b.folder, lw - 16, Style::Value),
            c::INK_MUTED,
            Style::Value,
        );
        // Recent LIBs.
        let ry = by + m::BUTTON_H + space::SPACE_2;
        subhead(d, bx, ry, side - space::SPACE_2, "Recent LIBs");
        let foot = y + h - 150;
        for (i, path) in self.recent.iter().enumerate() {
            let yy = ry + m::ROW_H + i as i32 * 38;
            if yy + 36 > foot {
                break;
            }
            let rect = [bx, yy, side - space::SPACE_2, 36];
            let fill = if o.over(rect) { c::GM_700 } else { c::GM_900 };
            let d = &mut o.canvas;
            notched(d, rect, Some(fill), None);
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            d.styled(
                bx + 6,
                baseline(yy + 2, 16, Style::Value),
                &fit(name, side - 22, Style::Value),
                c::INK,
                Style::Value,
            );
            d.styled(
                bx + 6,
                baseline(yy + 18, 16, Style::ValueSm),
                &fit(path, side - 22, Style::ValueSm),
                c::INK_MUTED,
                Style::ValueSm,
            );
            o.hit(rect, Action::Recent(i));
        }
        // Folder contents.
        let list_y = ry;
        let list_h = foot - list_y;
        o.canvas.rect(lx, list_y, lw, list_h, c::GM_950);
        for (row, (i, item)) in b
            .files
            .iter()
            .enumerate()
            .skip(b.scroll)
            .take((list_h / m::ROW_H) as usize)
            .enumerate()
        {
            let yy = list_y + row as i32 * m::ROW_H;
            let rect = [lx, yy, lw, m::ROW_H];
            let selected = p.value == item.path;
            let fill = if selected {
                c::AMBER_DEEP
            } else if o.over(rect) {
                c::GM_700
            } else if row % 2 == 0 {
                c::GM_900
            } else {
                c::GM_950
            };
            let d = &mut o.canvas;
            d.rect(lx, yy, lw, m::ROW_H, fill);
            d.icon(
                lx + 6,
                yy + 2,
                if item.directory {
                    Icon::Folder
                } else {
                    Icon::Lib
                },
                if selected { c::AMBER } else { c::INK_MUTED },
                fill,
            );
            d.styled(
                lx + 6 + m::ICON + space::SPACE_2,
                baseline(yy, m::ROW_H, Style::Value),
                &fit(&item.name, lw - 40, Style::Value),
                if selected { c::AMBER_BRIGHT } else { c::INK },
                Style::Value,
            );
            o.hit(rect, Action::BrowserPick(i));
        }
        let d = &mut o.canvas;
        let hint = if matches!(p.kind, PromptKind::File(FileAction::Save)) {
            if hangar_core::save::protected_name(&p.value).is_some() {
                "Retail LIB names are protected. Choose a different file name."
            } else {
                "Saves a custom LIB; an existing file is kept as a numbered .BAK."
            }
        } else {
            "Click a folder to open it; select a file, then confirm."
        };
        d.styled(
            bx,
            baseline(foot + space::SPACE_1, m::ROW_H, Style::Label),
            &fit(hint, bw, Style::Label),
            c::INK_MUTED,
            Style::Label,
        );
        d.styled(
            bx,
            baseline(foot + m::ROW_H + space::SPACE_1, m::ROW_H, Style::Label),
            "File path",
            c::INK_MUTED,
            Style::Label,
        );
        self.dialog_input(
            o,
            [bx, foot + 2 * m::ROW_H + space::SPACE_1, bw, 26],
            &p.value,
            p.caret,
        );
        if let Some(error) = self.status.strip_prefix("Error: ") {
            let ey = y + h - space::SPACE_4 - m::BUTTON_H;
            o.canvas
                .icon(bx, ey + 3, Icon::Warning, c::DANGER, c::GM_800);
            o.canvas.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ey, m::BUTTON_H, Style::Label),
                &fit(error, bw - 260, Style::Label),
                c::DANGER,
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            [x, y, w, h],
            &[],
            Some("Cancel"),
            Some(
                Btn::new(if matches!(p.kind, PromptKind::File(FileAction::Open)) {
                    "Open LIB"
                } else if matches!(p.kind, PromptKind::File(FileAction::Save)) {
                    "Save LIB"
                } else {
                    "Apply"
                })
                .primary(),
            ),
            Action::Apply,
        );
    }
}
