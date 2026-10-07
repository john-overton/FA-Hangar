//! Display palette resolution: one resolver sets `base_palette` for every
//! view, caches its answer until the selection's owner, the open LIBs or
//! their PAL entries change, and remembers the last game palette beside the
//! executable. Copies and duplicates take the resolved palette as their
//! `<ID>.PAL` companion.
use super::*;
use core::sync::atomic::{AtomicBool, Ordering};
use hangar_core::{
    archive,
    palette::{self as pal, Step},
    save::protected_name,
};

/// The smoke test neither reads nor writes the remembered palette file.
pub(crate) static NO_MEMORY: AtomicBool = AtomicBool::new(false);
/// Sibling LIBs read (directory and one range each) for `PALETTE.PAL`.
const SIBLINGS: usize = 32;

/// A resolved palette: its stored 768 bytes and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Found {
    pub bytes: Vec<u8>,
    pub label: String,
    /// The game's `PALETTE.PAL` (from a retail LIB, or remembered from one).
    pub retail: bool,
}
/// What the cached answer depends on.
#[derive(Clone)]
struct CacheKey {
    library: u64,
    path: String,
    libraries: Vec<(u64, String)>,
    owner: Option<String>,
    pals: Vec<(u64, Entry)>,
}
impl CacheKey {
    fn same(&self, o: &CacheKey) -> bool {
        self.library == o.library
            && self.path == o.path
            && self.libraries == o.libraries
            && self.owner == o.owner
            && self.pals.len() == o.pals.len()
            && self
                .pals
                .iter()
                .zip(&o.pals)
                .all(|(a, b)| a.0 == b.0 && a.1.same_storage(&b.1))
    }
}
/// Resolver state, boxed to keep `App` small.
#[derive(Default)]
pub(super) struct State {
    key: Option<CacheKey>,
    pub step: Option<Step>,
    /// None: grayscale.
    pub found: Option<Found>,
    /// Folder -> its first sibling `PALETTE.PAL`, for the current document set.
    folder: Option<(String, Option<Found>)>,
    /// The remembered game palette; the outer None until it is read.
    remembered: Option<Option<Found>>,
    /// Where the loaded display palette (`palette_override`) came from.
    pub loaded_label: String,
    /// Uncached resolutions, counted for the smoke test.
    pub resolves: u32,
}
fn leaf(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}
fn unhex(text: &str) -> Option<Vec<u8>> {
    let t = text.trim().as_bytes();
    if !t.len().is_multiple_of(2) {
        return None;
    }
    t.chunks(2)
        .map(|p| u8::from_str_radix(core::str::from_utf8(p).ok()?, 16).ok())
        .collect()
}
impl App {
    /// Set `base_palette` for the selection: Load palette, PALETTE.PAL in
    /// this LIB, the owning object's `<ID>.PAL`, the only other PAL here,
    /// PALETTE.PAL in another open LIB, in a LIB in this folder, the
    /// remembered game palette, else a grayscale ramp.
    pub(super) fn resolve_palette(&mut self) {
        if let Some(colors) = self.palette_override.as_deref().copied() {
            let label = if self.pal.loaded_label.is_empty() {
                "Loaded palette".to_string()
            } else {
                self.pal.loaded_label.clone()
            };
            self.pal.key = None;
            self.pal.step = Some(Step::Loaded);
            self.pal.found = Some(Found {
                bytes: pal::encode(&colors),
                label,
                retail: false,
            });
            *self.base_palette = colors;
            self.palette_loaded = true;
            return;
        }
        let owner = self
            .doc
            .archive
            .entries
            .get(self.selected)
            .and_then(|e| pal::owner_palette(&self.dependencies, &self.doc.archive, &e.name));
        let mut pals = Vec::new();
        for (id, archive) in core::iter::once((self.library_id, &self.doc.archive))
            .chain(self.libraries.iter().map(|l| (l.id, &l.doc.archive)))
        {
            for e in archive.entries.iter().filter(|e| e.name.ends_with(".PAL")) {
                pals.push((id, e.clone()));
            }
        }
        let key = CacheKey {
            library: self.library_id,
            path: self.path.clone(),
            libraries: self
                .libraries
                .iter()
                .map(|l| (l.id, l.path.clone()))
                .collect(),
            owner,
            pals,
        };
        if !self.pal.key.as_ref().is_some_and(|k| k.same(&key)) {
            if self
                .pal
                .key
                .as_ref()
                .is_none_or(|k| k.path != key.path || k.libraries != key.libraries)
            {
                self.pal.folder = None;
            }
            self.pal.resolves += 1;
            let (step, found) = self.resolve_uncached(key.owner.as_deref());
            if let Some(f) = found
                .as_ref()
                .filter(|f| f.retail && step != Step::Remembered)
            {
                self.remember_palette(f.clone());
            }
            self.pal.step = Some(step);
            self.pal.found = found;
            self.pal.key = Some(key);
        }
        match self
            .pal
            .found
            .as_ref()
            .and_then(|f| picture::palette(&f.bytes).ok())
        {
            Some(colors) => {
                *self.base_palette = colors;
                self.palette_loaded = true;
            }
            None => {
                *self.base_palette = core::array::from_fn(|i| [i as u8; 3]);
                self.palette_loaded = false;
            }
        }
    }
    fn resolve_uncached(&mut self, owner: Option<&str>) -> (Step, Option<Found>) {
        if let Some((i, step)) = pal::local(&self.doc.archive, owner) {
            let e = &self.doc.archive.entries[i];
            if let Ok(bytes) = e.read() {
                let label = if step == Step::Single {
                    format!("{}, the only PAL in this LIB", e.name)
                } else {
                    e.name.clone()
                };
                let retail = step == Step::Game && protected_name(&self.path).is_some();
                return (
                    step,
                    Some(Found {
                        bytes,
                        label,
                        retail,
                    }),
                );
            }
        }
        if let Some(f) = self.open_game_palette(false) {
            return (Step::OpenLib, Some(f));
        }
        if let Some(f) = self.folder_palette() {
            return (Step::Folder, Some(f));
        }
        if let Some(f) = self.remembered_palette() {
            return (Step::Remembered, Some(f));
        }
        (Step::Grayscale, None)
    }
    /// PALETTE.PAL in another open LIB, retail names first.
    fn open_game_palette(&self, retail_only: bool) -> Option<Found> {
        let mut libs: Vec<&libraries_ui::Library> = self.libraries.iter().collect();
        libs.sort_unstable_by_key(|l| pal::sibling_rank(&l.path));
        libs.into_iter().find_map(|l| {
            let retail = protected_name(&l.path).is_some();
            if retail_only && !retail {
                return None;
            }
            let i = l.doc.archive.find(pal::GAME)?;
            let bytes = l.doc.archive.entries[i].read().ok()?;
            pal::valid(&bytes).then(|| Found {
                bytes,
                label: format!("PALETTE.PAL from {}", leaf(&l.path)),
                retail,
            })
        })
    }
    /// The first PALETTE.PAL in a LIB beside the current one: directory
    /// reads and one range read each, retail names first, cached per folder.
    fn folder_palette(&mut self) -> Option<Found> {
        if !self.file_backed || self.path.is_empty() {
            return None;
        }
        let folder = Self::parent_path(&self.path);
        if let Some((f, found)) = &self.pal.folder {
            if *f == folder {
                return found.clone();
            }
        }
        let found = self.scan_folder(&folder);
        self.pal.folder = Some((folder, found.clone()));
        found
    }
    fn scan_folder(&self, folder: &str) -> Option<Found> {
        let current = leaf(&self.path);
        let mut libs: Vec<crate::ui::FileItem> = crate::platform::list_dir(folder)
            .ok()?
            .into_iter()
            .filter(|i| {
                !i.directory
                    && i.name.to_ascii_uppercase().ends_with(".LIB")
                    && !i.name.eq_ignore_ascii_case(current)
                    && !self.other_library_at(&i.path)
            })
            .collect();
        libs.sort_unstable_by_key(|i| pal::sibling_rank(&i.name));
        libs.iter().take(SIBLINGS).find_map(|item| {
            let entries = cloning_ui::index(&item.path).ok()??;
            let e = entries.iter().find(|e| e.name == pal::GAME)?;
            let stored = crate::platform::read_range(&item.path, e.offset, e.size).ok()?;
            let bytes = archive::decode_payload(e.flag, stored).ok()?;
            pal::valid(&bytes).then(|| Found {
                bytes,
                label: format!("PALETTE.PAL from {}", item.name),
                retail: protected_name(&item.name).is_some(),
            })
        })
    }
    fn remembered_palette(&mut self) -> Option<Found> {
        if self.pal.remembered.is_none() {
            let lines = if NO_MEMORY.load(Ordering::Relaxed) {
                Vec::new()
            } else {
                crate::platform::load_palette_memory()
            };
            let found = match lines.as_slice() {
                [label, digits, ..] => unhex(digits).filter(|b| pal::valid(b)).map(|bytes| Found {
                    bytes,
                    label: label.trim().to_string(),
                    retail: true,
                }),
                _ => None,
            };
            self.pal.remembered = Some(found);
        }
        self.pal.remembered.clone().flatten()
    }
    /// Keep `found` as the last game palette: beside the executable, or for
    /// this session when that location is read-only.
    pub(super) fn remember_palette(&mut self, found: Found) {
        if self
            .remembered_palette()
            .is_some_and(|r| r.bytes == found.bytes && r.label == found.label)
        {
            return;
        }
        if !NO_MEMORY.load(Ordering::Relaxed) {
            let _ = crate::platform::save_palette_memory(&[found.label.clone(), hex(&found.bytes)]);
        }
        self.pal.remembered = Some(Some(found));
    }
    /// The game's own PALETTE.PAL, for comparing a custom LIB's copy.
    pub(super) fn retail_palette(&mut self) -> Option<Found> {
        if let Some(f) = self.open_game_palette(true) {
            return Some(f);
        }
        if let Some(f) = self.folder_palette().filter(|f| f.retail) {
            return Some(f);
        }
        self.remembered_palette()
    }
    /// The resolved palette's bytes and source, for copies; None in grayscale.
    pub(super) fn game_palette(&self) -> Option<(Vec<u8>, String)> {
        self.pal
            .found
            .as_ref()
            .map(|f| (f.bytes.clone(), self.palette_label()))
    }
    /// Where the colors on screen come from.
    pub(super) fn palette_label(&self) -> String {
        match (self.pal.step, &self.pal.found) {
            (Some(Step::Remembered), Some(f)) => format!("{}, remembered", f.label),
            (_, Some(f)) => f.label.clone(),
            _ => "Grayscale; no game palette found".into(),
        }
    }
    /// No palette resolved: colors are a grayscale ramp.
    pub(super) fn palette_gray(&self) -> bool {
        self.pal.found.is_none()
    }
    /// Load palette: the display palette for this LIB, remembered when it
    /// is the game's PALETTE.PAL.
    pub(super) fn load_palette(&mut self, path: &str) -> Result<()> {
        let bytes = crate::platform::read(path)?;
        let (raw, label, game) = if bytes.starts_with(b"EALIB") {
            let archive = Archive::parse(bytes)?;
            let at = archive
                .find(pal::GAME)
                .ok_or("This LIB has no PALETTE.PAL")?;
            (
                archive.entries[at].read()?,
                format!("PALETTE.PAL from {}", leaf(path)),
                protected_name(path).is_some(),
            )
        } else {
            let name = leaf(path).to_ascii_uppercase();
            let game = name == pal::GAME;
            (bytes, name, game)
        };
        let palette = picture::palette(&raw)?;
        if game {
            self.remember_palette(Found {
                bytes: raw,
                label: label.clone(),
                retail: true,
            });
        }
        self.palette_override = Some(Box::new(palette));
        self.pal.loaded_label = format!("{label} (loaded)");
        self.refresh();
        self.status = "Display palette loaded; original resource palettes remain unchanged".into();
        Ok(())
    }
    /// Package checks: aircraft without a local palette, and a custom
    /// PALETTE.PAL that would recolor the game.
    pub(super) fn palette_checks(&mut self, report: &mut hangar_core::validation::Report) {
        let retail = self.retail_palette().map(|f| f.bytes);
        let custom = protected_name(&self.path).is_none();
        pal::check(report, &self.doc.archive, retail.as_deref(), custom);
    }
}

/// A valid 6-bit palette that differs per `level`.
fn smoke_pal(level: u8) -> Vec<u8> {
    (0..768)
        .map(|i| ((i / 3 + level as usize) % 64) as u8)
        .collect()
}
fn smoke_def(refs: &[&str]) -> Vec<u8> {
    let mut text = String::from("[brent's_relocatable_format]\n");
    for r in refs {
        text.push_str(&format!("string \"{r}\"\n"));
    }
    text.push_str("end\n");
    text.into_bytes()
}
/// ONE.PT draws ONE.SH; TWO.PT draws TWO.SH, which draws DEMO.PIC (a
/// retail texture, no palette of its own). `palettes` adds ONE.PAL and TWO.PAL.
fn smoke_pair(palettes: bool) -> Archive {
    let mut a = Archive::empty();
    let mut add = |n: &str, b: Vec<u8>| a.entries.push(Entry::new(n, b).unwrap());
    add("ONE.PT", smoke_def(&["ONE.SH"]));
    add("TWO.PT", smoke_def(&["TWO.SH"]));
    add("ONE.SH", model::demo_shape());
    add("TWO.SH", model::demo_textured());
    let texture = picture::to_retail_texture(&picture::demo(), None).unwrap();
    add("DEMO.PIC", texture.bytes);
    if palettes {
        add("ONE.PAL", smoke_pal(1));
        add("TWO.PAL", smoke_pal(2));
    }
    a
}
fn smoke_game(level: u8) -> Archive {
    let mut a = Archive::empty();
    a.entries
        .push(Entry::new(pal::GAME, smoke_pal(level)).unwrap());
    a
}
impl App {
    /// Only `archive` open, nothing remembered, no loaded palette.
    fn smoke_palette_reset(&mut self, archive: Archive, path: &str) {
        self.libraries.clear();
        self.prompt = None;
        self.browser = None;
        self.transfer_plan = None;
        self.transfer_source = None;
        self.transfer_move = false;
        self.palette_override = None;
        self.pal = Box::default();
        self.pal.remembered = Some(None);
        self.doc = Document::new(archive);
        self.path = path.into();
        self.file_backed = false;
        self.selected = 0;
        self.mode = Mode::Browse;
        self.refresh();
    }
    fn smoke_pick(&mut self, name: &str) {
        let i = self
            .doc
            .archive
            .find(name)
            .unwrap_or_else(|| panic!("{name}"));
        self.select_entry(i);
    }
    /// Text drawn in the region right of `x` and below `y`.
    fn smoke_drawn(&self, needle: &str, x: i32, y: i32) -> bool {
        self.draw().commands.iter().any(|d| {
            matches!(d, Draw::Text(tx, ty, t, _, _) if *tx >= x && *ty >= y && t.contains(needle))
        })
    }
    fn smoke_press_hit(&mut self, predicate: &dyn Fn(view::Action) -> bool) {
        let rect = self.chrome_hit(predicate).expect("control missing");
        self.chrome_click(rect);
    }
    fn smoke_answer(&mut self, values: &[&str]) {
        for value in values {
            self.key(Key::Char('a'), true, false);
            for c in value.chars() {
                self.key(Key::Char(c), false, false);
            }
            self.key(Key::Enter, false, false);
        }
    }
    /// Copy `name` from the active LIB to a new open LIB holding `target`
    /// through Copy to; the review is open afterwards.
    fn smoke_copy_to(&mut self, name: &str, target: Archive) {
        let source = self.library_id;
        self.install_library(Document::new(target), "TARGET.LIB".into())
            .unwrap();
        let id = self.library_id;
        self.refresh();
        self.switch_library(source).unwrap();
        self.smoke_pick(name);
        self.act(view::Action::TransferTo(id, false));
        assert!(
            matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::TransferReview)
            ),
            "{}",
            self.status
        );
    }
    fn smoke_both_sizes(&mut self, state: &str) {
        for (w, h) in [(1280, 800), (800, 600)] {
            (self.width, self.height) = (w, h);
            self.smoke_geometry(state);
        }
        (self.width, self.height) = (1280, 800);
    }
    /// Resolution order, caching, the indicator and grayscale notice, Load
    /// palette, package checks, and the `<ID>.PAL` companion of Copy to,
    /// duplicates and exports, with their hit geometry at both window sizes.
    pub fn smoke_palette(&mut self) {
        let colors = |level: u8| picture::palette(&smoke_pal(level)).unwrap();
        // Two PTs with their own palettes: every entry shows its owner's.
        self.smoke_palette_reset(smoke_pair(true), "PAIR.LIB");
        for (name, owner) in [
            ("ONE.PT", 1),
            ("ONE.SH", 1),
            ("TWO.PT", 2),
            ("TWO.SH", 2),
            ("DEMO.PIC", 2),
        ] {
            self.smoke_pick(name);
            assert_eq!(*self.base_palette, colors(owner), "{name}");
            assert_eq!(self.pal.step, Some(Step::Owner), "{name}");
            let label = if owner == 1 { "ONE.PAL" } else { "TWO.PAL" };
            assert_eq!(self.palette_label(), label);
        }
        // Cached: the same owner and a plain refresh do not re-resolve.
        let n = self.pal.resolves;
        self.smoke_pick("TWO.SH");
        self.refresh();
        assert_eq!(self.pal.resolves, n);
        self.smoke_pick("ONE.SH");
        assert_eq!(self.pal.resolves, n + 1);
        // An edited PAL entry re-resolves; undo restores it.
        let at = self.doc.archive.find("ONE.PAL").unwrap();
        self.doc.replace(at, smoke_pal(3)).unwrap();
        self.refresh();
        assert_eq!(*self.base_palette, colors(3));
        self.doc.undo();
        self.refresh();
        assert_eq!(*self.base_palette, colors(1));
        // The source shows in the Paint palette header and in Details.
        self.smoke_pick("DEMO.PIC");
        assert!(self.mode == Mode::Media);
        assert!(self.smoke_drawn("TWO.PAL", self.right(), 0));
        self.dock = 2;
        assert!(self.smoke_drawn("TWO.PAL", 0, self.dock_y()));
        self.dock = 0;
        // One PAL left: every entry uses it.
        self.doc
            .transaction(Vec::new(), &["TWO.PAL".into()])
            .unwrap();
        self.smoke_pick("TWO.SH");
        assert_eq!(self.pal.step, Some(Step::Single));
        assert_eq!(*self.base_palette, colors(1));

        // No palette anywhere: grayscale, the notice, and Load palette.
        self.smoke_palette_reset(smoke_pair(false), "BARE.LIB");
        self.smoke_pick("DEMO.PIC");
        assert!(self.palette_gray() && !self.palette_loaded);
        assert_eq!(*self.base_palette, core::array::from_fn(|i| [i as u8; 3]));
        assert!(self.smoke_drawn("No game palette found", self.right(), 0));
        self.smoke_both_sizes("palette notice");
        crate::platform::write_new("HGP_LOAD.PAL", &smoke_pal(4)).unwrap();
        let load = self.smoke_find(&|a| matches!(a, view::Action::File(FileAction::Palette)));
        self.chrome_click(load);
        assert!(matches!(
            self.prompt.as_ref().map(|p| &p.kind),
            Some(PromptKind::File(FileAction::Palette))
        ));
        self.smoke_answer(&["HGP_LOAD.PAL"]);
        crate::platform::remove_file("HGP_LOAD.PAL").unwrap();
        assert!(self.prompt.is_none(), "{}", self.status);
        assert_eq!(self.pal.step, Some(Step::Loaded));
        assert_eq!(self.palette_label(), "HGP_LOAD.PAL (loaded)");
        assert_eq!(*self.base_palette, colors(4));
        assert!(!self.smoke_drawn("No game palette found", self.right(), 0));
        // Not PALETTE.PAL: nothing is remembered.
        assert_eq!(self.pal.remembered, Some(None));

        // Later steps: another open LIB, then the remembered palette.
        self.smoke_palette_reset(smoke_pair(false), "BARE.LIB");
        let bare = self.library_id;
        self.install_library(Document::new(smoke_game(5)), "FA_2.LIB".into())
            .unwrap();
        self.switch_library(bare).unwrap();
        self.smoke_pick("ONE.SH");
        assert_eq!(self.pal.step, Some(Step::OpenLib));
        assert_eq!(self.palette_label(), "PALETTE.PAL from FA_2.LIB");
        assert_eq!(*self.base_palette, colors(5));
        // A retail PALETTE.PAL is remembered (for this session in the smoke test).
        let remembered = self.pal.remembered.clone().flatten().unwrap();
        assert_eq!(remembered.bytes, smoke_pal(5));
        self.libraries.clear();
        self.refresh();
        assert_eq!(self.pal.step, Some(Step::Remembered));
        assert_eq!(
            self.palette_label(),
            "PALETTE.PAL from FA_2.LIB, remembered"
        );

        // A LIB in the same folder: directory and one range read.
        self.smoke_palette_reset(Archive::empty(), "");
        crate::platform::write_new("HGP_SIB.LIB", &smoke_game(10).bytes().unwrap()).unwrap();
        crate::platform::write_new("HGP_CUR.LIB", &smoke_pair(false).bytes().unwrap()).unwrap();
        self.open("HGP_CUR.LIB").unwrap();
        self.smoke_pick("ONE.SH");
        crate::platform::remove_file("HGP_SIB.LIB").unwrap();
        crate::platform::remove_file("HGP_CUR.LIB").unwrap();
        assert_eq!(self.pal.step, Some(Step::Folder));
        if self.palette_label() == "PALETTE.PAL from HGP_SIB.LIB" {
            assert_eq!(*self.base_palette, colors(10));
        }
        // Neither shape has an owner palette: the same answer, not re-resolved.
        let n = self.pal.resolves;
        self.smoke_pick("TWO.SH");
        assert_eq!(self.pal.resolves, n);
        // A PAL edit re-resolves; the folder scan is cached for this set
        // of open LIBs, so the answer stays although the files are gone.
        self.doc
            .transaction(vec![Entry::new("BAD.PAL", vec![1, 2, 3]).unwrap()], &[])
            .unwrap();
        self.refresh();
        assert_eq!(self.pal.resolves, n + 1);
        assert_eq!(self.pal.step, Some(Step::Folder));
        self.smoke_palette_reset(smoke_pair(false), "BARE.LIB");
        self.pal.remembered = Some(Some(remembered));
        self.refresh();

        // Package checks: aircraft without a palette in their LIB.
        let report = self.package_report();
        assert!(report.checks.iter().any(|c| c
            .message
            .starts_with("ONE.PT, TWO.PT: colors in Hangar need a palette")));
        // A custom PALETTE.PAL that differs from the retail one recolors the game.
        self.doc
            .transaction(vec![Entry::new(pal::GAME, smoke_pal(6)).unwrap()], &[])
            .unwrap();
        self.refresh();
        let report = self.package_report();
        assert!(report
            .checks
            .iter()
            .any(|c| c.message.contains("recolor the whole game")));

        // Copy to: ONE.PAL comes along as its own row; Skip and Copy toggle it.
        self.smoke_palette_reset(smoke_pair(true), "PAIR.LIB");
        self.smoke_copy_to("ONE.PT", Archive::empty());
        let plan = self.transfer_plan.as_ref().unwrap();
        let (row, from) = plan.palette.clone().expect("palette row");
        assert_eq!(plan.items[row].entry.name, "ONE.PAL");
        assert_eq!(from, "ONE.PAL in the source");
        self.smoke_both_sizes("transfer review palette");
        self.smoke_press_hit(&|a| matches!(a, view::Action::TransferChoice(i, false) if i == row));
        assert_eq!(
            self.transfer_plan.as_ref().unwrap().items[row].choice,
            hangar_core::resource_ops::Choice::KeepTarget
        );
        self.smoke_press_hit(&|a| matches!(a, view::Action::TransferChoice(i, true) if i == row));
        self.smoke_press_hit(&|a| matches!(a, view::Action::Apply));
        assert!(self.transfer_plan.is_none(), "{}", self.status);
        let at = self.doc.archive.find("ONE.PAL").expect("ONE.PAL copied");
        assert_eq!(self.doc.archive.entries[at].read().unwrap(), smoke_pal(1));
        assert!(self.doc.archive.find("ONE.PT").is_some());
        assert!(self.doc.archive.find(pal::GAME).is_none());
        // One undo step takes both away.
        self.doc.undo();
        assert!(self.doc.archive.entries.is_empty());

        // A target with PALETTE.PAL gets no companion; the notes say why.
        self.smoke_palette_reset(smoke_pair(true), "PAIR.LIB");
        self.smoke_copy_to("ONE.PT", smoke_game(7));
        let plan = self.transfer_plan.as_ref().unwrap();
        assert!(plan.palette.is_none());
        assert!(plan.notes.iter().any(|n| n.contains("has PALETTE.PAL")));
        assert!(plan.items.iter().all(|i| !i.entry.name.ends_with(".PAL")));

        // No <ID>.PAL in the source: the palette Hangar shows becomes ONE.PAL.
        self.smoke_palette_reset(smoke_pair(false), "BARE.LIB");
        let bare = self.library_id;
        self.install_library(Document::new(smoke_game(8)), "FA_2.LIB".into())
            .unwrap();
        self.switch_library(bare).unwrap();
        self.smoke_copy_to("ONE.PT", Archive::empty());
        let plan = self.transfer_plan.as_ref().unwrap();
        let (row, from) = plan.palette.clone().expect("palette row");
        assert_eq!(from, "PALETTE.PAL from FA_2.LIB");
        assert_eq!(plan.items[row].entry.read().unwrap(), smoke_pal(8));
        assert!(plan
            .notes
            .iter()
            .any(|n| n == hangar_core::resource_ops::PALETTE_NOTE));
        self.smoke_press_hit(&|a| matches!(a, view::Action::Apply));
        assert!(self.doc.archive.find("ONE.PAL").is_some());
        assert!(self.doc.archive.find(pal::GAME).is_none());

        // Duplicate aircraft in a LIB without a palette brings TWIN.PAL.
        self.smoke_palette_reset(Archive::empty(), "DUP.LIB");
        self.demo();
        let demo = self.library_id;
        self.install_library(Document::new(smoke_game(9)), "FA_2.LIB".into())
            .unwrap();
        self.switch_library(demo).unwrap();
        self.smoke_pick("DEMO.PT");
        self.mode = Mode::Model;
        self.act(view::Action::DuplicateAircraft);
        self.smoke_answer(&["TWIN", "Twin", "Twin test"]);
        assert!(
            matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::DuplicateReview)
            ),
            "{}",
            self.status
        );
        let dup = self.identity.duplicate.as_ref().unwrap();
        assert_eq!(
            dup.palette(),
            Some(("TWIN.PAL", "PALETTE.PAL from FA_2.LIB", true))
        );
        self.smoke_both_sizes("duplicate review palette");
        self.smoke_press_hit(&|a| matches!(a, view::Action::DuplicatePalette(false)));
        assert!(self.identity.palette_skip);
        let dup = self.identity.duplicate.as_ref().unwrap();
        assert!(!dup.palette().unwrap().2);
        self.smoke_press_hit(&|a| matches!(a, view::Action::DuplicatePalette(true)));
        self.smoke_press_hit(&|a| matches!(a, view::Action::Apply));
        let at = self.doc.archive.find("TWIN.PAL").expect("TWIN.PAL copied");
        assert_eq!(self.doc.archive.entries[at].read().unwrap(), smoke_pal(9));
        assert!(self.doc.archive.find("TWIN.PT").is_some());
        assert!(self.doc.archive.find(pal::GAME).is_none());
        self.doc.undo();
        assert!(self.doc.archive.find("TWIN.PAL").is_none());
        self.refresh();

        // Export: the palette from the other open LIB becomes NEWX.PAL.
        self.smoke_pick("DEMO.PT");
        self.file_prompt(FileAction::Variant);
        self.smoke_answer(&["NEWX", "New", "New export"]);
        assert!(
            matches!(
                self.prompt.as_ref().map(|p| &p.kind),
                Some(PromptKind::CloneReview)
            ),
            "{}",
            self.status
        );
        let package = self.clone_draft.as_ref().unwrap();
        let at = package.archive.find("NEWX.PAL").expect("NEWX.PAL exported");
        assert_eq!(package.archive.entries[at].read().unwrap(), smoke_pal(9));
        assert!(package.archive.find(pal::GAME).is_none());
        let (_, from) = self.clone_palette_row(package).unwrap();
        assert_eq!(from, "PALETTE.PAL from FA_2.LIB");
        for (w, h) in [(1280, 800), (800, 600)] {
            (self.width, self.height) = (w, h);
            assert!(self.smoke_drawn("Palette: PALETTE.PAL", 0, 0));
            self.smoke_geometry("export review palette");
        }
        (self.width, self.height) = (1280, 800);
        self.key(Key::Escape, false, false);
        self.clone_draft = None;
        self.smoke_palette_reset(Archive::empty(), "");
        self.demo();
    }
}
impl App {
    /// `--palette-check`: the palette each entry resolves (step and
    /// source), then Copy to of `copy` into a new LIB through the review,
    /// saved create-new to `output`, with the PALs it received.
    #[cfg(not(windows))]
    pub fn palette_check(&mut self, output: &str, copy: &str, names: &[String]) -> Result<String> {
        if protected_name(output).is_some() {
            return Err("Choose a new custom LIB name".into());
        }
        let mut report = String::new();
        for name in names.iter().map(String::as_str).chain([copy]) {
            let at = self
                .doc
                .archive
                .find(name)
                .ok_or_else(|| format!("{name} not found"))?;
            self.select_entry(at);
            report += &format!(
                "{name}: {} [{:?}]\n",
                self.palette_label(),
                self.pal.step.unwrap_or(Step::Grayscale)
            );
        }
        let source = self.library_id;
        self.install_library(Document::new(Archive::empty()), output.into())?;
        let target = self.library_id;
        self.refresh();
        self.switch_library(source)?;
        let at = self.doc.archive.find(copy).ok_or("Copy root not found")?;
        self.select_entry(at);
        self.prepare_drop(target)?;
        // With its linked files, as Ctrl+C / Ctrl+V copies.
        self.act(view::Action::TransferDependencies);
        let plan = self.transfer_plan.as_ref().ok_or("No review")?;
        report += &format!(
            "Copy {copy}: {} in the review; palette row {}\n",
            plan.items.len(),
            plan.palette
                .as_ref()
                .map_or("none".to_string(), |(i, from)| format!(
                    "{} from {from}",
                    plan.items[*i].entry.name
                ))
        );
        for note in &plan.notes {
            report += &format!("  note: {note}\n");
        }
        self.apply_transfer()?;
        crate::platform::write_new(output, &self.doc.archive.bytes()?)?;
        let written = Archive::parse(crate::platform::read(output)?)?;
        for e in written.entries.iter().filter(|e| e.name.ends_with(".PAL")) {
            report += &format!("{output}: {} ({} bytes)\n", e.name, e.read()?.len());
        }
        report += &format!(
            "{output}: {} entries, PALETTE.PAL {}\n",
            written.entries.len(),
            if written.find(pal::GAME).is_some() {
                "present"
            } else {
                "absent"
            }
        );
        Ok(report)
    }
}
