use super::view::{Action, Icon, Layout};
use super::*;
use alloc::collections::BTreeSet;
use hangar_core::{
    archive::{self, IndexedEntry},
    clone_aircraft::{self, Resolution},
};
/// Review choices for names no searched LIB provides. The draft is built with
/// every such name kept; Export stays disabled until `ack` while any is kept.
#[derive(Default)]
pub(super) struct Unresolved {
    /// (referencing resource, unresolved name) -> substitute PIC.
    pub substitutes: BTreeMap<(String, String), String>,
    pub ack: bool,
    /// Review row whose texture picker is open.
    pub pick: Option<usize>,
    pub scroll: usize,
}
/// Rows of the review's Unresolved in source list shown at once.
const UNRESOLVED_ROWS: usize = 3;
const PICKS: usize = 12;
struct Source {
    path: String,
    entries: Vec<IndexedEntry>,
    explicit: bool,
}
fn normalized(path: &str) -> String {
    let p = path.replace('\\', "/");
    if cfg!(windows) {
        p.to_ascii_uppercase()
    } else {
        p
    }
}
pub(crate) fn index(path: &str) -> Result<Option<Vec<IndexedEntry>>> {
    let size = crate::platform::file_size(path)?;
    if size < 7 {
        return Ok(None);
    }
    let head = crate::platform::read_range(path, 0, 7)?;
    if &head[..5] != b"EALIB" {
        return Ok(None);
    }
    let count = u16::from_le_bytes([head[5], head[6]]) as usize;
    let len = 7 + (count + 1) * 18;
    if len > size {
        return Err(format!("{path}: truncated LIB directory"));
    }
    let bytes = crate::platform::read_range(path, 0, len)?;
    archive::directory(&bytes, size)
        .map(Some)
        .map_err(|e| format!("{path}: {e}"))
}
impl App {
    pub(super) fn begin_clone(&mut self) {
        self.clone_draft = None;
        self.clone_scroll = 0;
        self.clone_unresolved = Box::default();
        self.clone_sources.clear();
        self.browser = None;
        let stem = self.name().split('.').next().unwrap_or("NEW");
        let prefix: String = stem.chars().take(4).collect();
        let names = self
            .brf
            .as_ref()
            .and_then(hangar_core::identity::read)
            .unwrap_or_else(|| hangar_core::identity::Identity {
                short: stem.into(),
                long: stem.into(),
                reference: String::new(),
            });
        let limit = hangar_core::identity::NAME_LIMIT;
        self.identity.in_place = false;
        self.identity.short = names.short.chars().take(limit).collect();
        self.clone_title = format!(
            "{} variant",
            names.long.chars().take(limit - 8).collect::<String>()
        );
        self.variant_id = format!("{prefix}V1");
        self.clone_step(1);
        self.status =
            "The selected object and its resource graph will be copied into a separate LIB".into();
    }
    /// Wizard step prompt: 1 new ID, 2 short name, 3 long name. The same
    /// steps export to a new LIB or, in place, duplicate an aircraft.
    pub(super) fn clone_step(&mut self, step: u8) {
        let task = if self.identity.in_place {
            format!("Duplicate aircraft {}", self.name())
        } else {
            format!("Export object from {}", self.name())
        };
        let (kind, title, value) = match step {
            1 => (
                PromptKind::CloneId,
                format!("{task}, step 1: new ID of 1 to 6 letters or digits"),
                self.variant_id.clone(),
            ),
            2 => (
                PromptKind::CloneShort,
                format!("{task}, step 2: short name, as in lists"),
                self.identity.short.clone(),
            ),
            _ => (
                PromptKind::CloneTitle,
                format!("{task}, step 3: long name"),
                self.clone_title.clone(),
            ),
        };
        self.prompt = Some(Prompt {
            kind,
            title,
            value,
            axis: 0,
        });
    }
    pub(super) fn add_clone_source(&mut self, path: &str) -> Result<()> {
        if index(path)?.is_none() {
            return Err("Choose an EALIB source library".into());
        }
        self.clone_sources
            .retain(|p| normalized(p) != normalized(path));
        self.clone_sources.push(path.into());
        self.clone_step(3);
        self.status = "Source LIB added; press Enter to rebuild the review".into();
        Ok(())
    }
    pub(super) fn build_clone(&self) -> Result<clone_aircraft::Package> {
        let mut catalog = BTreeSet::new();
        for e in &self.doc.archive.entries {
            catalog.insert(e.name.clone());
        }
        for library in &self.libraries {
            for entry in &library.doc.archive.entries {
                catalog.insert(entry.name.clone());
            }
        }
        let mut sources = Vec::new();
        let mut seen = BTreeSet::new();
        seen.insert(normalized(&self.path));
        let mut candidates: Vec<_> = self
            .clone_sources
            .iter()
            .rev()
            .map(|p| (p.clone(), true))
            .collect();
        if crate::platform::file_size(&self.path).is_ok() {
            let parent = Self::parent_path(&self.path);
            for item in crate::platform::list_dir(&parent)? {
                if !item.directory && item.name.to_ascii_uppercase().ends_with(".LIB") {
                    candidates.push((item.path, false));
                }
            }
        }
        for (path, explicit) in candidates {
            if !seen.insert(normalized(&path)) {
                continue;
            }
            if sources.len() >= 64 {
                return Err("More than 64 source LIBs; use a smaller source folder".into());
            }
            if let Some(entries) = index(&path)? {
                for e in &entries {
                    catalog.insert(e.name.clone());
                }
                if catalog.len() > 131072 {
                    return Err(
                        "Source catalog exceeds 131072 names; use a smaller LIB folder".into(),
                    );
                }
                sources.push(Source {
                    path,
                    entries,
                    explicit,
                });
            }
        }
        // The palette Hangar shows the donor with is the one the export
        // carries as <ID>.PAL when this LIB has neither its own nor
        // PALETTE.PAL; other LIBs' PALETTE.PAL copies never conflict.
        let game = self.game_palette();
        let own_palette = hangar_core::palette::private_name(self.name());
        let local = |n: &str| self.doc.archive.find(n).is_some();
        let game = game.filter(|_| !local(&own_palette) && !local(hangar_core::palette::GAME));
        if game.is_some() && !catalog.contains(&own_palette) {
            catalog.insert(hangar_core::palette::GAME.into());
        }
        let mut package = clone_aircraft::build_with(
            &catalog,
            self.name(),
            &self.variant_id,
            clone_aircraft::Names {
                short: &self.identity.short,
                long: &self.clone_title,
            },
            // Built with every unresolved name kept so the review can list
            // them; the Export button stays gated on the acknowledgement.
            &clone_aircraft::Policy {
                keep_unresolved: true,
                substitutes: self.clone_unresolved.substitutes.clone(),
            },
            |name| {
                if let Some(i) = self.doc.archive.find(name) {
                    return self.doc.archive.entries[i].read();
                }
                if let Some((bytes, _)) =
                    game.as_ref().filter(|_| name == hangar_core::palette::GAME)
                {
                    return Ok(bytes.clone());
                }
                // Explicit source choices win; an open document supplies its in-memory bytes.
                for source in sources.iter().filter(|s| s.explicit) {
                    if let Some(library) = self
                        .libraries
                        .iter()
                        .find(|l| normalized(&l.path) == normalized(&source.path))
                    {
                        if let Some(i) = library.doc.archive.find(name) {
                            return library.doc.archive.entries[i].read();
                        }
                    } else if let Some(e) = source.entries.iter().find(|e| e.name == name) {
                        return archive::decode_payload(
                            e.flag,
                            crate::platform::read_range(&source.path, e.offset, e.size)?,
                        );
                    }
                }
                let mut selected: Option<(Vec<u8>, &str)> = None;
                for library in &self.libraries {
                    if let Some(i) = library.doc.archive.find(name) {
                        let bytes = library.doc.archive.entries[i].read()?;
                        if let Some((old, path)) = &selected {
                            if old != &bytes {
                                return Err(format!("Conflicting open {name} in {path} and {}; choose an explicit source",library.path));
                            }
                        } else {
                            selected = Some((bytes, &library.path));
                        }
                    }
                }
                if let Some((bytes, _)) = selected.take() {
                    return Ok(bytes);
                }
                for source in &sources {
                    if self
                        .libraries
                        .iter()
                        .any(|l| normalized(&l.path) == normalized(&source.path))
                    {
                        continue;
                    }
                    if let Some(e) = source.entries.iter().find(|e| e.name == name) {
                        let stored = crate::platform::read_range(&source.path, e.offset, e.size)?;
                        let bytes = archive::decode_payload(e.flag, stored)?;
                        if source.explicit {
                            return Ok(bytes);
                        }
                        if let Some((old, path)) = &selected {
                            if old != &bytes {
                                return Err(format!("Conflicting {name} in {path} and {}. Use Add source LIB to select the intended source",source.path));
                            }
                        } else {
                            selected = Some((bytes, &source.path));
                        }
                    }
                }
                selected
                    .map(|(bytes, _)| bytes)
                    .ok_or_else(|| format!("Missing {name}; use Add source LIB to locate it"))
            },
        )?;
        if self.clone_palette_row(&package).is_none() && hangar_core::palette::object(self.name()) {
            package.notes.insert(
                0,
                "No palette found: the new LIB has no <ID>.PAL, so Hangar shows it in grayscale until one is loaded".into(),
            );
        }
        Ok(package)
    }
    /// The export's `<ID>.PAL` row: its index in the mapping and where its
    /// bytes come from.
    pub(super) fn clone_palette_row(
        &self,
        package: &clone_aircraft::Package,
    ) -> Option<(usize, String)> {
        let name = format!("{}.PAL", package.id);
        let at = package.mapping.iter().position(|(_, new)| *new == name)?;
        let old = &package.mapping[at].0;
        let from = if self.doc.archive.find(old).is_some() {
            format!("{old} in this LIB")
        } else if old == hangar_core::palette::GAME {
            self.game_palette()
                .map_or_else(|| old.clone(), |(_, label)| label)
        } else {
            format!("{old} from a source LIB")
        };
        Some((at, from))
    }
    pub(super) fn clone_review(&self, o: &mut Layout) {
        use theme::{metric as m, space};
        use widgets::{baseline, Btn};
        let Some(package) = &self.clone_draft else {
            return;
        };
        let w = (self.width - 32).min(900);
        let h = (self.height - 48).min(660);
        let (x, y) = ((self.width - w) / 2, (self.height - h) / 2);
        o.hits.clear();
        let title = format!(
            "Review export: {} \u{2192} {}.{}",
            package.donor,
            package.id,
            extension(&package.donor)
        );
        let [bx, by, bw, _] = self.dialog_frame(o, [x, y, w, h], &title);
        let d = &mut o.canvas;
        d.styled(
            bx,
            baseline(by, m::ROW_H, Style::Label),
            &fit(
                &format!(
                    "{} private resources. Source files are preserved; suggested output {}.LIB.",
                    package.archive.entries.len(),
                    package.id
                ),
                bw,
                Style::Label,
            ),
            c::INK,
            Style::Label,
        );
        let mut head = by + m::ROW_H + space::SPACE_2;
        let mut picker = None;
        if !package.unresolved.is_empty() {
            let (used, anchor) = self.clone_unresolved_section(o, [bx, head, bw]);
            head += used + space::SPACE_2;
            picker = anchor;
        }
        let d = &mut o.canvas;
        d.rect(bx, head, bw, m::ROW_H, c::GM_900);
        let nx = bx + bw / 2;
        for (hx, label) in [(bx + 8, "FROM DONOR"), (nx, "IN NEW LIB")] {
            d.styled(
                hx,
                baseline(head, m::ROW_H, Style::Section),
                label,
                c::INK_MUTED,
                Style::Section,
            );
        }
        let notes = package.notes.len().min(4) as i32;
        let foot = y + h - space::SPACE_4 - m::BUTTON_H - space::SPACE_3 - (notes + 1) * m::ROW_H;
        let rows = ((foot - head - m::ROW_H) / m::ROW_H).max(1) as usize;
        let palette = self.clone_palette_row(package);
        for (row, (old, new)) in package
            .mapping
            .iter()
            .skip(self.clone_scroll)
            .take(rows)
            .enumerate()
        {
            let old = match &palette {
                Some((at, from)) if *at == row + self.clone_scroll => {
                    format!("Palette: {from}")
                }
                _ => old.clone(),
            };
            let yy = head + m::ROW_H + row as i32 * m::ROW_H;
            d.rect(
                bx,
                yy,
                bw,
                m::ROW_H,
                if row % 2 == 0 { c::GM_950 } else { c::GM_900 },
            );
            let base = baseline(yy, m::ROW_H, Style::Value);
            d.styled(
                bx + 8,
                base,
                &fit(&old, nx - bx - 16, Style::Value),
                c::INK_MUTED,
                Style::Value,
            );
            d.styled(
                nx,
                base,
                &fit(new, bx + bw - nx - 8, Style::Value),
                c::AMBER,
                Style::Value,
            );
        }
        d.styled(
            bx,
            baseline(foot, m::ROW_H, Style::Label),
            &fit(
                "Wheel scrolls the renames. No exported name replaces a scanned resource.",
                bw,
                Style::Label,
            ),
            c::INK_MUTED,
            Style::Label,
        );
        for (i, note) in package.notes.iter().take(4).enumerate() {
            let ny = foot + (i as i32 + 1) * m::ROW_H;
            let warn = i >= 3;
            d.icon(
                bx,
                ny + 2,
                if warn { Icon::Warning } else { Icon::Info },
                if warn { c::AMBER } else { c::INK_MUTED },
                c::GM_800,
            );
            d.styled(
                bx + m::ICON + space::SPACE_1,
                baseline(ny, m::ROW_H, Style::Label),
                &fit(note, bw - m::ICON - space::SPACE_1, Style::Label),
                if warn { c::INK } else { c::INK_MUTED },
                Style::Label,
            );
        }
        self.dialog_actions(
            o,
            [x, y, w, h],
            &[
                ("Add source LIB", Action::File(FileAction::CloneSource)),
                ("Back", Action::CloneBack),
            ],
            Some("Cancel"),
            Some(
                Btn::new("Export new LIB")
                    .with_icon(Icon::Package)
                    .primary()
                    .enabled(self.clone_exportable()),
            ),
            Action::Apply,
        );
        // The texture picker floats over the dialog; its hits are checked first.
        if let (Some(row), Some((px, py))) = (self.clone_unresolved.pick, picker) {
            let picks = self.clone_picks(row);
            let mut items: Vec<widgets::Item> = picks
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    widgets::Item::new(name, Action::CloneTexture(i)).icon(Icon::Image)
                })
                .collect();
            if !items.is_empty() {
                items.push(widgets::Item::sep());
            }
            items.push(widgets::Item::new(
                "Other PIC in a source LIB\u{2026}",
                Action::ClonePickOther,
            ));
            o.menu(px, py, &items, Action::MenuPad);
        }
    }
    /// Unresolved in source: one row per name with its resource, evidence and
    /// choice, then the acknowledgement. Returns the height used and, when the
    /// texture picker is open, where it hangs.
    fn clone_unresolved_section(
        &self,
        o: &mut Layout,
        [x, y, w]: [i32; 3],
    ) -> (i32, Option<(i32, i32)>) {
        use theme::{metric as m, space};
        use widgets::{baseline, subhead, Btn, Check};
        let Some(package) = &self.clone_draft else {
            return (0, None);
        };
        let list = &package.unresolved;
        let rh = m::BUTTON_H + space::SPACE_1;
        let first = self
            .clone_unresolved
            .scroll
            .min(list.len().saturating_sub(UNRESOLVED_ROWS));
        let shown = list.len().min(UNRESOLVED_ROWS);
        subhead(&mut o.canvas, x, y, w, "Unresolved in source");
        if list.len() > shown {
            let range = format!("{}\u{2013}{} of {}", first + 1, first + shown, list.len());
            let rw = text_width(&range, Style::ValueSm);
            o.canvas.styled(
                x + w - rw,
                baseline(y, m::ROW_H, Style::ValueSm),
                &range,
                c::INK_MUTED,
                Style::ValueSm,
            );
        }
        let mut anchor = None;
        let mut ry = y + m::ROW_H;
        for (i, u) in list.iter().enumerate().skip(first).take(shown) {
            o.canvas.rect(
                x,
                ry,
                w,
                rh,
                if (i - first) % 2 == 0 {
                    c::GM_950
                } else {
                    c::GM_900
                },
            );
            o.canvas.icon(
                x + space::SPACE_1,
                ry + (rh - m::ICON) / 2,
                Icon::Warning,
                c::AMBER,
                c::GM_950,
            );
            let keep = Btn::new("Keep as in source");
            let substitute = match &u.resolution {
                Resolution::Substitute(name) => name.as_str(),
                Resolution::Keep => "Use texture\u{2026}",
            };
            let pick = Btn::new(substitute).with_icon(Icon::Image);
            let items = [
                (
                    keep.on(u.resolution == Resolution::Keep),
                    Action::CloneKeep(i),
                ),
                (
                    pick.on(matches!(u.resolution, Resolution::Substitute(_))),
                    Action::ClonePick(i),
                ),
            ];
            let cw = if u.texture() {
                Layout::segmented_width(&items)
            } else {
                0
            };
            let tx = x + space::SPACE_1 + m::ICON + space::SPACE_2;
            let name_w = text_width(&u.target, Style::Value).min(w / 3);
            o.canvas.styled(
                tx,
                baseline(ry, rh, Style::Value),
                &fit(&u.target, name_w, Style::Value),
                c::INK,
                Style::Value,
            );
            let mut detail = format!("in {} \u{b7} {}", u.resource, u.evidence.label());
            if u.drawn() == Some(false) {
                detail.push_str(" \u{b7} not drawn by any pose");
            }
            let dx = tx + name_w + space::SPACE_2;
            let right = x + w - space::SPACE_1 - cw;
            if u.texture() {
                o.canvas.styled(
                    dx,
                    baseline(ry, rh, Style::Label),
                    &fit(&detail, right - space::SPACE_2 - dx, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                );
                let rect = [right, ry + (rh - m::BUTTON_H) / 2, cw, m::BUTTON_H];
                o.segmented(rect, &items);
                if self.clone_unresolved.pick == Some(i) {
                    // At natural width the second member starts after the first.
                    anchor = Some((right + 1 + items[0].0.width(), rect[1] + rect[3] + 1));
                }
            } else {
                // Only texture names can be retargeted; others are kept or absent.
                let state = if u.evidence == hangar_core::dependencies::Evidence::Convention {
                    "Absent, as in source"
                } else {
                    "Kept as in source"
                };
                let sw = text_width(state, Style::Label);
                o.canvas.styled(
                    x + w - space::SPACE_2 - sw,
                    baseline(ry, rh, Style::Label),
                    state,
                    c::INK_MUTED,
                    Style::Label,
                );
                o.canvas.styled(
                    dx,
                    baseline(ry, rh, Style::Label),
                    &fit(&detail, x + w - 2 * space::SPACE_2 - sw - dx, Style::Label),
                    c::INK_MUTED,
                    Style::Label,
                );
            }
            ry += rh;
        }
        let kept = self.clone_kept();
        if kept > 0 {
            let label = format!(
                "Export with {}, as in the source LIB",
                view::count(kept, "unresolved reference", "unresolved references")
            );
            o.checkbox(
                x,
                ry + space::SPACE_1,
                &label,
                if self.clone_unresolved.ack {
                    Check::On
                } else {
                    Check::Off
                },
                Action::CloneAck,
                true,
            );
            ry += m::ROW_H + space::SPACE_1;
        }
        (ry - y, anchor)
    }
    pub(super) fn clone_review_prompt(&mut self) {
        self.prompt = Some(Prompt {
            kind: PromptKind::CloneReview,
            title: "Export object: review private resources".into(),
            value: String::new(),
            axis: 0,
        });
    }
    /// Wheel over the Unresolved in source rows scrolls them, not the renames.
    pub(super) fn clone_unresolved_wheel(&mut self, delta: i32) -> bool {
        use theme::{metric as m, space};
        let len = self.clone_draft.as_ref().map_or(0, |p| p.unresolved.len());
        let h = (self.height - 48).min(660);
        let top = (self.height - h) / 2 + view::DIALOG_HEAD + space::SPACE_3;
        let bottom = top
            + 2 * m::ROW_H
            + space::SPACE_2
            + UNRESOLVED_ROWS as i32 * (m::BUTTON_H + space::SPACE_1);
        if len <= UNRESOLVED_ROWS || self.mouse[1] < top || self.mouse[1] >= bottom {
            return false;
        }
        let first = self.clone_unresolved.scroll.min(len - UNRESOLVED_ROWS) as i32;
        self.clone_unresolved.scroll =
            (first - delta).clamp(0, (len - UNRESOLVED_ROWS) as i32) as usize;
        true
    }
    /// Unresolved references the draft keeps (not substituted).
    pub(super) fn clone_kept(&self) -> usize {
        self.clone_draft.as_ref().map_or(0, |p| {
            p.unresolved
                .iter()
                .filter(|u| u.resolution == Resolution::Keep)
                .count()
        })
    }
    pub(super) fn clone_exportable(&self) -> bool {
        self.clone_kept() == 0 || self.clone_unresolved.ack
    }
    /// Texture picker: PICs already in the package, the referencing shape's
    /// own family first. Other source PICs are typed by name.
    pub(super) fn clone_picks(&self, row: usize) -> Vec<String> {
        let Some(u) = self
            .clone_draft
            .as_ref()
            .and_then(|p| p.unresolved.get(row))
        else {
            return Vec::new();
        };
        let stem = u.resource.split('.').next().unwrap_or("");
        let mut picks: Vec<String> = self
            .clone_draft
            .iter()
            .flat_map(|p| &p.mapping)
            .map(|(old, _)| old)
            .filter(|n| n.ends_with(".PIC"))
            .cloned()
            .collect();
        // Unstable sort: the stable one needs a stack buffer the CRT-free build cannot probe.
        picks.sort_unstable_by(|a, b| (!a.contains(stem), a).cmp(&(!b.contains(stem), b)));
        picks.truncate(PICKS);
        picks
    }
    /// Keep (`None`) or retarget one review row, then rebuild the draft. A
    /// failed rebuild restores the previous choice.
    pub(super) fn clone_choose(&mut self, row: usize, texture: Option<String>) -> Result<()> {
        let u = self
            .clone_draft
            .as_ref()
            .and_then(|p| p.unresolved.get(row))
            .ok_or("No unresolved reference in this row")?
            .clone();
        let key = (u.resource.clone(), u.target.clone());
        let kept = self.clone_kept();
        self.clone_unresolved.pick = None;
        let texture = texture.map(|t| t.trim().to_ascii_uppercase());
        let previous = match &texture {
            Some(t) => self
                .clone_unresolved
                .substitutes
                .insert(key.clone(), t.clone()),
            None => self.clone_unresolved.substitutes.remove(&key),
        };
        match self.build_clone() {
            Ok(package) => {
                self.clone_draft = Some(Box::new(package));
                // Keeping more names than were acknowledged asks again.
                if self.clone_kept() > kept {
                    self.clone_unresolved.ack = false;
                }
                self.status = match texture {
                    Some(t) => format!(
                        "{} in {} now uses {t}; it joins the package. The source LIB is unchanged",
                        u.target, u.resource
                    ),
                    None => format!("{} in {} kept as in source", u.target, u.resource),
                };
                Ok(())
            }
            Err(e) => {
                match previous {
                    Some(old) => self.clone_unresolved.substitutes.insert(key, old),
                    None => self.clone_unresolved.substitutes.remove(&key),
                };
                Err(e)
            }
        }
    }
}
impl App {
    pub fn smoke_clone(&mut self) {
        self.doc = Document::new(Archive::empty());
        self.demo();
        self.select_entry(1);
        let before = self.doc.archive.bytes().unwrap();
        crate::platform::write_new("HGC_SRC.LIB", &before).unwrap();
        let directory = index("HGC_SRC.LIB").unwrap().unwrap();
        let entry = directory.iter().find(|e| e.name == "DEMO.PIC").unwrap();
        let bytes = crate::platform::read_range("HGC_SRC.LIB", entry.offset, entry.size).unwrap();
        assert_eq!(
            archive::decode_payload(entry.flag, bytes).unwrap(),
            self.doc.archive.entries[self.doc.archive.find("DEMO.PIC").unwrap()]
                .read()
                .unwrap()
        );
        crate::platform::remove_file("HGC_SRC.LIB").unwrap();
        self.file_prompt(FileAction::Variant);
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::CloneId
        ));
        assert!(self.browser.is_none());
        for value in ["NEWJET", "New", "New aircraft"] {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
        }
        assert!(
            matches!(self.prompt.as_ref().unwrap().kind, PromptKind::CloneReview),
            "{}",
            self.status
        );
        assert_eq!(self.doc.archive.bytes().unwrap(), before);
        let p = self.clone_draft.as_ref().unwrap();
        assert_eq!(p.archive.entries.len(), 8);
        assert!(p
            .mapping
            .iter()
            .all(|(_, new)| self.doc.archive.find(new).is_none()));
        self.key(Key::Enter, false, false);
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::File(FileAction::Save)
        ));
        assert!(self.prompt.as_ref().unwrap().value.ends_with("NEWJET.LIB"));
        assert!(self.doc.dirty());
        assert_eq!(self.doc.archive.entries[0].name, "NEWJET.PT");
        // Steps 2 and 3 fill the short and long names separately.
        let bytes = self.doc.archive.entries[0].read().unwrap();
        let id = hangar_core::identity::read(&Brf::parse(&bytes, "PT").unwrap()).unwrap();
        assert_eq!(
            (id.short.as_str(), id.long.as_str(), id.reference.as_str()),
            ("New", "New aircraft", "NEWJET.PT")
        );
        let reopened = Archive::parse(self.doc.archive.bytes().unwrap()).unwrap();
        assert_eq!(reopened.entries.len(), 8);
        self.key(Key::Escape, false, false);
        // Directory-only platform reader is exercised against an existing LIB by Linux CLI QA.
        self.smoke_clone_unresolved();
    }
    /// Synthetic demo whose DEMO_C.SH draws GHOST.PIC, which no LIB provides,
    /// taken through the export wizard to its review.
    pub(super) fn smoke_ghost_review(&mut self, id: &str) -> Vec<u8> {
        self.doc = Document::new(Archive::empty());
        self.demo();
        let mut ghost = hangar_core::model::demo_textured();
        ghost[258..272].fill(0);
        ghost[258..267].copy_from_slice(b"GHOST.PIC");
        let mut a = self.doc.archive.clone();
        let at = a.find("DEMO_C.SH").unwrap();
        a.entries[at] = archive::Entry::new("DEMO_C.SH", ghost.clone()).unwrap();
        self.doc = Document::new(a);
        self.refresh();
        self.select_entry(self.doc.archive.find("DEMO.PT").unwrap());
        self.file_prompt(FileAction::Variant);
        for value in [id, "Ghost", "Ghost test"] {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
        }
        assert!(
            matches!(self.prompt.as_ref().unwrap().kind, PromptKind::CloneReview),
            "{}",
            self.status
        );
        ghost
    }
    fn smoke_clone_hit(&mut self, predicate: &dyn Fn(Action) -> bool) -> bool {
        match self.chrome_hit(predicate) {
            Some(rect) => {
                self.chrome_click(rect);
                true
            }
            None => false,
        }
    }
    /// Save the reviewed export as `output`, reopen it from disk and return it.
    fn smoke_clone_export(&mut self, output: &str) -> Archive {
        assert!(!crate::platform::save_exists(output).unwrap());
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::Apply)));
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::File(FileAction::Save)
        ));
        self.key(Key::Char('a'), true, false);
        for c in output.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        assert!(!self.doc.dirty(), "{}", self.status);
        let reopened = Archive::parse(crate::platform::read(output).unwrap()).unwrap();
        crate::platform::remove_file(output).unwrap();
        reopened
    }
    /// Unresolved in source: blocked by default, substitute through the
    /// picker, typed source PIC refused, then acknowledged, exported and reopened.
    fn smoke_clone_unresolved(&mut self) {
        let ghost = self.smoke_ghost_review("GHJET");
        let source = self.doc.archive.bytes().unwrap();
        let p = self.clone_draft.as_ref().unwrap();
        assert_eq!(p.unresolved.len(), 1);
        assert_eq!(p.unresolved[0].resource, "DEMO_C.SH");
        assert!(p.archive.find("GHOST.PIC").is_none());
        assert!(p.mapping.iter().all(|(old, _)| old != "GHOST.PIC"));
        // Blocked: no Export hit region, and Enter explains the checkbox.
        assert!(self.chrome_hit(&|a| matches!(a, Action::Apply)).is_none());
        self.key(Key::Enter, false, false);
        assert!(self.status.starts_with("Error: Tick Export with 1"));
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::CloneReview
        ));
        assert!(self.clone_draft.is_some());
        // Use texture: the picker lists package PICs; choosing one enables Export.
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::ClonePick(0))));
        assert_eq!(self.clone_unresolved.pick, Some(0));
        let picks = self.clone_picks(0);
        let i = picks.iter().position(|n| n == "DEMO.PIC").unwrap();
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::CloneTexture(j) if j == i)));
        let p = self.clone_draft.as_ref().unwrap();
        assert_eq!(
            p.unresolved[0].resolution,
            Resolution::Substitute("DEMO.PIC".into())
        );
        assert!(self.clone_exportable());
        // Keep again: the acknowledgement is required once more.
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::CloneKeep(0))));
        assert!(!self.clone_exportable());
        // Other PIC: a name no LIB provides is refused and the row stays kept.
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::ClonePick(0))));
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::ClonePickOther)));
        for c in "NOWHERE.PIC".chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        assert!(self.status.contains("NOWHERE.PIC is not in this LIB"));
        self.key(Key::Escape, false, false);
        assert!(matches!(
            self.prompt.as_ref().unwrap().kind,
            PromptKind::CloneReview
        ));
        let p = self.clone_draft.as_ref().unwrap();
        assert_eq!(p.unresolved[0].resolution, Resolution::Keep);
        // Acknowledge, export, reopen: the copied shape keeps GHOST.PIC as stored.
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::CloneAck)));
        assert!(self.clone_exportable());
        assert_eq!(self.doc.archive.bytes().unwrap(), source);
        let out = self.smoke_clone_export("HGC_GHOST.LIB");
        assert_eq!(
            out.entries[out.find("GHJET_C.SH").unwrap()].read().unwrap(),
            ghost
        );
        assert!(out.find("GHOST.PIC").is_none());

        // Substitute path, exported and reopened: only the copied name slot changes.
        self.smoke_ghost_review("GHSUB");
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::ClonePick(0))));
        let i = self
            .clone_picks(0)
            .iter()
            .position(|n| n == "DEMO.PIC")
            .unwrap();
        assert!(self.smoke_clone_hit(&|a| matches!(a, Action::CloneTexture(j) if j == i)));
        let private = self
            .clone_draft
            .as_ref()
            .unwrap()
            .mapping
            .iter()
            .find(|(old, _)| old == "DEMO.PIC")
            .unwrap()
            .1
            .clone();
        let out = self.smoke_clone_export("HGC_GHSUB.LIB");
        let shape = out.entries[out.find("GHSUB_C.SH").unwrap()].read().unwrap();
        assert_eq!(shape.len(), ghost.len());
        assert_eq!(&shape[258..258 + private.len()], private.as_bytes());
        assert_eq!(shape[..258], ghost[..258]);
        assert_eq!(shape[272..], ghost[272..]);
        assert!(out.find(&private).is_some());
        self.doc = Document::new(Archive::empty());
    }
}

impl App {
    #[cfg(not(windows))]
    pub fn clone_export_check(
        &mut self,
        donor: &str,
        id: &str,
        title: &str,
        output: &str,
    ) -> Result<()> {
        let at = self
            .doc
            .archive
            .find(donor)
            .ok_or("Source object not found")?;
        self.select_entry(at);
        let before = self.doc.archive.bytes()?;
        self.file_prompt(FileAction::Variant);
        for value in [id, title, title] {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
            if self.status.starts_with("Error:") {
                return Err(self.status.clone());
            }
        }
        if !matches!(
            self.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::CloneReview)
        ) {
            return Err("Wizard did not reach review".into());
        }
        if self.doc.archive.bytes()? != before {
            return Err("Source document changed before export".into());
        }
        if !self.clone_exportable() {
            // Names no searched LIB provides: acknowledge through the checkbox.
            let rect = self
                .chrome_hit(&|a| matches!(a, Action::CloneAck))
                .ok_or("Unresolved acknowledgement missing")?;
            self.chrome_click(rect);
            println!("{}", self.status);
        }
        self.key(Key::Enter, false, false);
        self.key(Key::Char('a'), true, false);
        for c in output.chars() {
            self.key(Key::Char(c), false, false);
        }
        self.key(Key::Enter, false, false);
        if self.doc.dirty() {
            return Err(self.status.clone());
        }
        self.open(output)?;
        if self
            .doc
            .archive
            .find(&format!("{}.{}", id.to_ascii_uppercase(), extension(donor)))
            .is_none()
        {
            return Err("Exported object identity missing".into());
        }
        Ok(())
    }
}
