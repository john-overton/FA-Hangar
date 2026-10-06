use super::view::{border, icon, label_fit, text_fit, Action, Icon, Layout};
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
        let Some(p) = &self.prompt else {
            return;
        };
        let Some(b) = &self.browser else {
            return;
        };
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(620);
        let x = (self.width - w) / 2;
        let y = (self.height - h) / 2;
        let side = 180;
        o.hits.clear();
        let d = &mut o.canvas;
        d.rect(x + 4, y + 5, w, h, c::GM_1000);
        d.rect(x, y, w, h, c::GM_800);
        border(d, x, y, w, h, c::LINE_STRONG);
        d.rect(x + 1, y + 1, w - 2, 31, c::GM_700);
        label_fit(d, x + 12, y + 22, w - 28, &p.title, c::INK);
        o.button([x + 10, y + 41, 52, 24], "Up", Action::BrowserUp, false);
        o.button(
            [x + 69, y + 41, 69, 24],
            "Drives",
            Action::BrowserRoots,
            false,
        );
        o.canvas
            .rect(x + side, y + 40, w - side - 12, 25, c::GM_950);
        text_fit(
            &mut o.canvas,
            x + side + 8,
            y + 58,
            w - side - 28,
            &b.folder,
            c::INK_MUTED,
        );
        o.canvas.label(x + 14, y + 94, "RECENT LIBS", c::INK_FAINT);
        for (i, path) in self.recent.iter().enumerate() {
            let yy = y + 109 + i as i32 * 46;
            if yy + 38 > y + h - 134 {
                break;
            }
            let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
            o.canvas.rect(x + 8, yy, side - 18, 40, c::GM_900);
            text_fit(&mut o.canvas, x + 16, yy + 16, side - 34, name, c::INK);
            text_fit(
                &mut o.canvas,
                x + 16,
                yy + 33,
                side - 34,
                path,
                c::INK_FAINT,
            );
            o.hit([x + 8, yy, side - 18, 40], Action::Recent(i));
        }
        let list_y = y + 78;
        let list_h = h - 218;
        o.canvas
            .rect(x + side, list_y, w - side - 12, list_h, c::GM_950);
        for (row, (i, item)) in b
            .files
            .iter()
            .enumerate()
            .skip(b.scroll)
            .take((list_h / 24) as usize)
            .enumerate()
        {
            let yy = list_y + row as i32 * 24;
            let selected = p.value == item.path;
            o.canvas.rect(
                x + side,
                yy,
                w - side - 12,
                24,
                if selected {
                    c::AMBER_DEEP
                } else if row % 2 == 0 {
                    c::GM_900
                } else {
                    c::GM_950
                },
            );
            icon(
                &mut o.canvas,
                x + side + 8,
                yy + 4,
                if item.directory {
                    Icon::Lib
                } else {
                    Icon::Mission
                },
                if item.directory {
                    c::STEEL
                } else {
                    c::INK_MUTED
                },
            );
            text_fit(
                &mut o.canvas,
                x + side + 33,
                yy + 17,
                w - side - 51,
                &item.name,
                if selected { c::AMBER } else { c::INK },
            );
            o.hit([x + side, yy, w - side - 12, 24], Action::BrowserPick(i));
        }
        o.canvas.label(
            x + side,
            y + h - 122,
            if matches!(p.kind, PromptKind::File(FileAction::Save)) {
                if hangar_core::save::protected_name(&p.value).is_some() {
                    "Protected retail name: choose a different LIB filename."
                } else {
                    "Save custom LIB; an existing file gets a numbered .bak backup."
                }
            } else {
                "Click a folder to enter; select a file, then Open / Apply."
            },
            c::INK_FAINT,
        );
        o.canvas.label(
            x + 12,
            y + h - 103,
            "FILE / PATH (type to edit; Ctrl+A clears)",
            c::INK_MUTED,
        );
        o.canvas.rect(x + 12, y + h - 93, w - 24, 28, c::GM_950);
        border(&mut o.canvas, x + 12, y + h - 93, w - 24, 28, c::FOCUS);
        let tail: String = p
            .value
            .chars()
            .rev()
            .take(((w - 46) / 7) as usize)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        o.canvas
            .text(x + 20, y + h - 74, &format!("{tail}_"), c::INK);
        if self.status.starts_with("Error:") {
            label_fit(
                &mut o.canvas,
                x + 12,
                y + h - 47,
                w - 244,
                &self.status,
                c::DANGER,
            );
        }
        o.button(
            [x + w - 224, y + h - 46, 96, 29],
            "Cancel",
            Action::Cancel,
            false,
        );
        o.button(
            [x + w - 116, y + h - 46, 104, 29],
            if matches!(p.kind, PromptKind::File(FileAction::Open)) {
                "Open LIB"
            } else if matches!(p.kind, PromptKind::File(FileAction::Save)) {
                "Save LIB"
            } else {
                "Apply"
            },
            Action::Apply,
            true,
        );
    }
}
