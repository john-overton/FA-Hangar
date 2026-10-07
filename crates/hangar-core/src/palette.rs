//! Display palettes: which PAL an entry is drawn with, the `<ID>.PAL`
//! companion that travels with a copied object, and the palette package
//! checks. FA itself draws with its global `PALETTE.PAL`; the private
//! `<ID>.PAL` an object carries is what Hangar shows it with.
use crate::{
    archive::{Archive, Entry},
    dependencies::Index,
    picture,
    save::PROTECTED_LIBS,
    validation::{Level, Report},
};
use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
    vec::Vec,
};

/// FA's global palette. Never written into a custom LIB automatically.
pub const GAME: &str = "PALETTE.PAL";
/// Bound on names visited when looking for an entry's owners.
const OWNER_SCAN: usize = 4096;

fn ext(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or("")
}
fn stem(s: &str) -> &str {
    s.split('.').next().unwrap_or(s)
}
/// Object definitions a palette companion follows: PT, JT, OT and NT.
pub fn object(name: &str) -> bool {
    matches!(ext(name), "PT" | "JT" | "OT" | "NT")
}
/// `F14.PT` -> `F14.PAL`.
pub fn private_name(object: &str) -> String {
    format!("{}.PAL", stem(object).to_ascii_uppercase())
}
/// A 768-byte 6-bit palette.
pub fn valid(bytes: &[u8]) -> bool {
    picture::palette(bytes).is_ok()
}
/// The stored 768-byte 6-bit form of an expanded palette. Exact for any
/// palette `picture::palette` produced.
pub fn encode(colors: &[[u8; 3]; 256]) -> Vec<u8> {
    colors
        .iter()
        .flatten()
        .map(|v| ((*v as u16 * 63 + 127) / 255) as u8)
        .collect()
}
fn valid_entry(entry: &Entry) -> bool {
    entry.read().is_ok_and(|b| valid(&b))
}

/// Objects that own `name`, nearest first: the name itself when it is an
/// object, then every object that reaches it through stored references and
/// reviewed conventions (the links the export and identity graphs follow).
/// Within one distance PTs come first, then by name.
pub fn owners(index: &Index, name: &str) -> Vec<String> {
    let name = name.to_ascii_uppercase();
    let mut out = Vec::new();
    let mut visited = BTreeSet::new();
    visited.insert(name.clone());
    let mut level = vec![name];
    while !level.is_empty() && visited.len() < OWNER_SCAN {
        let mut found: Vec<String> = level.iter().filter(|n| object(n)).cloned().collect();
        found.sort_unstable_by(|a, b| (ext(a) != "PT", a).cmp(&(ext(b) != "PT", b)));
        out.extend(found);
        let mut next = Vec::new();
        for n in &level {
            for user in index.incoming(n) {
                if visited.len() >= OWNER_SCAN {
                    break;
                }
                if visited.insert(user.clone()) {
                    next.push(user.clone());
                }
            }
        }
        level = next;
    }
    out
}
/// The `<ID>.PAL` of the nearest owner of `name` that `archive` holds.
pub fn owner_palette(index: &Index, archive: &Archive, name: &str) -> Option<String> {
    owners(index, name)
        .iter()
        .map(|o| private_name(o))
        .find(|p| archive.find(p).is_some())
}

/// Where a display palette came from, in resolution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Load palette.
    Loaded,
    /// `PALETTE.PAL` in the current LIB.
    Game,
    /// The owning object's `<ID>.PAL` in the current LIB.
    Owner,
    /// The only other valid PAL in the current LIB.
    Single,
    /// `PALETTE.PAL` in another open LIB.
    OpenLib,
    /// `PALETTE.PAL` in a LIB in the current LIB's folder.
    Folder,
    /// The last game palette Hangar resolved, remembered beside the executable.
    Remembered,
    /// No palette: a grayscale ramp.
    Grayscale,
}
/// The current LIB's own answer: `PALETTE.PAL`, then the owner's `<ID>.PAL`,
/// then the only other valid PAL. Invalid PALs are passed over.
pub fn local(archive: &Archive, owner: Option<&str>) -> Option<(usize, Step)> {
    let usable = |name: &str| {
        archive
            .find(name)
            .filter(|i| valid_entry(&archive.entries[*i]))
    };
    if let Some(i) = usable(GAME) {
        return Some((i, Step::Game));
    }
    if let Some(i) = owner.and_then(usable) {
        return Some((i, Step::Owner));
    }
    let mut single = None;
    for (i, e) in archive.entries.iter().enumerate() {
        if ext(&e.name) != "PAL" || e.name == GAME || !valid_entry(e) {
            continue;
        }
        if single.is_some() {
            return None;
        }
        single = Some(i);
    }
    single.map(|i| (i, Step::Single))
}
/// Order in which sibling LIBs are searched for `PALETTE.PAL`: FA_2.LIB,
/// FA_1.LIB, the other retail names, then the rest by name.
pub fn sibling_rank(file: &str) -> (usize, String) {
    let leaf = file
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(file)
        .to_ascii_uppercase();
    let rank = match leaf.as_str() {
        "FA_2.LIB" => 0,
        "FA_1.LIB" => 1,
        _ => PROTECTED_LIBS
            .iter()
            .position(|n| *n == leaf)
            .map_or(PROTECTED_LIBS.len() + 2, |i| i + 2),
    };
    (rank, leaf)
}

/// The palette companion offered with a copied or duplicated object.
#[derive(Clone, Debug)]
pub struct Companion {
    /// `<new ID>.PAL`.
    pub entry: Entry,
    /// Where its bytes come from, such as `F14.PAL in the source`.
    pub from: String,
    /// The target's different entry with this name; the review resolves it.
    pub previous: Option<Entry>,
}
#[derive(Clone, Debug)]
pub enum Offer {
    Copy(Companion),
    /// Nothing to copy, and why (empty when the item is not an object).
    Skip(String),
}
/// The palette that goes with `object` copied from `source` into `target`
/// as `new_object` (the same name for a cross-LIB copy). `<ID>.PAL` from the
/// source wins; otherwise `game`, the palette Hangar resolved, is copied
/// under the new private name. Skipped when the target has `PALETTE.PAL` or
/// already holds identical bytes under that name. Never named `PALETTE.PAL`:
/// FA keeps the newest of duplicate names, so a game palette in a mod LIB
/// would recolor every aircraft.
pub fn companion(
    source: &Archive,
    target: &Archive,
    object_name: &str,
    new_object: &str,
    game: Option<(&[u8], &str)>,
) -> Offer {
    if !object(object_name) || !object(new_object) {
        return Offer::Skip(String::new());
    }
    if target.find(GAME).is_some() {
        return Offer::Skip(
            "The target has PALETTE.PAL; no palette companion is needed".to_string(),
        );
    }
    let name = private_name(new_object);
    let own = private_name(object_name);
    let (entry, from) = match source
        .find(&own)
        .map(|i| &source.entries[i])
        .filter(|e| valid_entry(e))
    {
        Some(e) => match e.renamed(&name) {
            Ok(entry) => (entry, format!("{own} in the source")),
            Err(error) => return Offer::Skip(error),
        },
        None => match game.filter(|(bytes, _)| valid(bytes)) {
            Some((bytes, label)) => match Entry::new(&name, bytes.to_vec()) {
                Ok(entry) => (entry, label.to_string()),
                Err(error) => return Offer::Skip(error),
            },
            None => {
                return Offer::Skip(format!(
                    "No game palette found for {name}; colors in Hangar need one"
                ))
            }
        },
    };
    let previous = target.find(&name).map(|i| target.entries[i].clone());
    if let Some(p) = &previous {
        if p.same_payload(&entry) || p.read().ok() == entry.read().ok() {
            return Offer::Skip(format!("The target already has an identical {name}"));
        }
    }
    Offer::Copy(Companion {
        entry,
        from,
        previous,
    })
}

/// Palette package checks. Info for aircraft Hangar cannot color from this
/// LIB alone; a warning when a custom LIB's `PALETTE.PAL` differs from the
/// game's (`retail`), since FA would draw every aircraft with it.
pub fn check(report: &mut Report, archive: &Archive, retail: Option<&[u8]>, custom: bool) {
    // `local` for every PT, reading each PAL once.
    // Inserted one by one: collecting a set runs a stable sort, whose stack
    // buffer the CRT-free x86_64 build cannot probe.
    let mut pals = BTreeSet::new();
    for e in &archive.entries {
        if ext(&e.name) == "PAL" && valid_entry(e) {
            pals.insert(e.name.as_str());
        }
    }
    let bare: Vec<&str> = if pals.contains(GAME) || pals.len() == 1 {
        Vec::new()
    } else {
        archive
            .entries
            .iter()
            .filter(|e| ext(&e.name) == "PT" && !pals.contains(private_name(&e.name).as_str()))
            .map(|e| e.name.as_str())
            .collect()
    };
    if !bare.is_empty() {
        let mut names = bare.iter().take(4).copied().collect::<Vec<_>>().join(", ");
        if bare.len() > 4 {
            names.push_str(&format!(" and {} more aircraft", bare.len() - 4));
        }
        report.add(
            Level::Info,
            None,
            format!(
                "{names}: colors in Hangar need a palette; add one with Load palette or copy from FA_2.LIB"
            ),
        );
    }
    if !custom {
        return;
    }
    let (Some(retail), Some(i)) = (retail, archive.find(GAME)) else {
        return;
    };
    if archive.entries[i].read().is_ok_and(|b| b != retail) {
        report.add(
            Level::Warning,
            Some(GAME),
            "Differs from the game's PALETTE.PAL; FA loads the newest duplicate name, so this would recolor the whole game".into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;
    fn brf(name: &str, references: &[&str]) -> Entry {
        let mut text = String::from("[brent's_relocatable_format]\n");
        for target in references {
            text.push_str(&format!("string \"{target}\"\n"));
        }
        text.push_str("end\n");
        Entry::new(name, text.into_bytes()).unwrap()
    }
    fn pal(level: u8) -> Vec<u8> {
        vec![level; 768]
    }
    /// Two aircraft with their own palettes; ONE carries a weapon whose
    /// shape draws DEMO.PIC, which TWO's shape also draws.
    fn fixture() -> Archive {
        let mut a = Archive::empty();
        a.entries = vec![
            brf("ONE.PT", &["ONE.SH", "GUN.JT"]),
            brf("TWO.PT", &["TWO.SH"]),
            brf("GUN.JT", &["GUN.SH"]),
            Entry::new("ONE.SH", model::demo_shape()).unwrap(),
            Entry::new("TWO.SH", model::demo_textured()).unwrap(),
            Entry::new("GUN.SH", model::demo_textured()).unwrap(),
            Entry::new("DEMO.PIC", picture::demo()).unwrap(),
            Entry::new("ONE.PAL", pal(10)).unwrap(),
            Entry::new("TWO.PAL", pal(20)).unwrap(),
        ];
        a
    }
    #[test]
    fn owners_follow_the_reference_graph_nearest_first() {
        let a = fixture();
        let mut index = Index::default();
        index.update(&a);
        assert_eq!(owners(&index, "ONE.PT"), vec!["ONE.PT"]);
        assert_eq!(owners(&index, "one.sh"), vec!["ONE.PT"]);
        assert_eq!(owners(&index, "GUN.SH"), vec!["GUN.JT", "ONE.PT"]);
        // Equal distance: PT first. DEMO.PIC <- TWO.SH <- TWO.PT and
        // DEMO.PIC <- GUN.SH <- GUN.JT <- ONE.PT.
        assert_eq!(
            owners(&index, "DEMO.PIC"),
            vec!["TWO.PT", "GUN.JT", "ONE.PT"]
        );
        assert_eq!(
            owner_palette(&index, &a, "ONE.SH").as_deref(),
            Some("ONE.PAL")
        );
        assert_eq!(
            owner_palette(&index, &a, "TWO.SH").as_deref(),
            Some("TWO.PAL")
        );
        // GUN.PAL is absent: the weapon shape falls through to its aircraft.
        assert_eq!(
            owner_palette(&index, &a, "GUN.SH").as_deref(),
            Some("ONE.PAL")
        );
        assert_eq!(
            owner_palette(&index, &a, "DEMO.PIC").as_deref(),
            Some("TWO.PAL")
        );
        assert_eq!(private_name("f14.pt"), "F14.PAL");
    }
    #[test]
    fn local_order_is_game_owner_then_single() {
        let mut a = fixture();
        let one = a.find("ONE.PAL").unwrap();
        let two = a.find("TWO.PAL").unwrap();
        assert_eq!(local(&a, Some("ONE.PAL")), Some((one, Step::Owner)));
        assert_eq!(local(&a, Some("TWO.PAL")), Some((two, Step::Owner)));
        // Two private PALs and no owner: neither is the only one.
        assert_eq!(local(&a, None), None);
        a.entries.remove(two);
        assert_eq!(local(&a, Some("TWO.PAL")), Some((one, Step::Single)));
        // An invalid PALETTE.PAL is passed over; a valid one wins.
        a.entries.push(Entry::new(GAME, vec![99; 768]).unwrap());
        assert_eq!(local(&a, Some("ONE.PAL")), Some((one, Step::Owner)));
        let last = a.entries.len() - 1;
        a.entries[last] = Entry::new(GAME, pal(30)).unwrap();
        assert_eq!(local(&a, Some("ONE.PAL")), Some((last, Step::Game)));
    }
    #[test]
    fn encode_inverts_the_expansion() {
        let bytes: Vec<u8> = (0..768).map(|i| (i % 64) as u8).collect();
        assert_eq!(encode(&picture::palette(&bytes).unwrap()), bytes);
    }
    #[test]
    fn siblings_prefer_retail_names() {
        let mut names = vec!["/g/ZMOD.LIB", "/g/fa_1.lib", "/g/FA_4B.LIB", "/g/FA_2.LIB"];
        names.sort_unstable_by_key(|n| sibling_rank(n));
        assert_eq!(
            names,
            vec!["/g/FA_2.LIB", "/g/fa_1.lib", "/g/FA_4B.LIB", "/g/ZMOD.LIB"]
        );
    }
    #[test]
    fn companions_carry_the_private_palette_or_the_game_palette() {
        let source = fixture();
        let mut target = Archive::empty();
        let game = pal(40);
        // The source's own <ID>.PAL wins, stored bytes kept.
        let Offer::Copy(c) = companion(&source, &target, "ONE.PT", "ONE.PT", Some((&game, "G")))
        else {
            panic!("expected a companion")
        };
        assert_eq!(c.entry.name, "ONE.PAL");
        assert_eq!(c.from, "ONE.PAL in the source");
        assert!(c
            .entry
            .same_payload(&source.entries[source.find("ONE.PAL").unwrap()]));
        // Renamed for a duplicate under a new ID.
        let Offer::Copy(c) = companion(&source, &source, "ONE.PT", "NEW.PT", None) else {
            panic!("expected a companion")
        };
        assert_eq!(c.entry.name, "NEW.PAL");
        // No private PAL: the resolved game palette under the object's name.
        let Offer::Copy(c) = companion(
            &source,
            &target,
            "GUN.JT",
            "GUN.JT",
            Some((&game, "PALETTE.PAL from FA_2.LIB")),
        ) else {
            panic!("expected a companion")
        };
        assert_eq!(
            (c.entry.name.as_str(), c.from.as_str()),
            ("GUN.PAL", "PALETTE.PAL from FA_2.LIB")
        );
        assert_eq!(c.entry.read().unwrap(), game);
        assert!(c.previous.is_none());
        // Nothing resolved, or not an object: no companion.
        assert!(matches!(
            companion(&source, &target, "GUN.JT", "GUN.JT", None),
            Offer::Skip(r) if r.contains("No game palette")
        ));
        assert!(matches!(
            companion(&source, &target, "GUN.SH", "GUN.SH", Some((&game, "G"))),
            Offer::Skip(r) if r.is_empty()
        ));
        // An invalid game palette is never copied.
        assert!(matches!(
            companion(
                &source,
                &target,
                "GUN.JT",
                "GUN.JT",
                Some((&[1, 2, 3], "G"))
            ),
            Offer::Skip(_)
        ));
        // Same name, different bytes: offered against the target's entry.
        target.entries.push(Entry::new("ONE.PAL", pal(50)).unwrap());
        let Offer::Copy(c) = companion(&source, &target, "ONE.PT", "ONE.PT", None) else {
            panic!("expected a companion")
        };
        assert!(c.previous.is_some());
        // Identical bytes: skipped.
        target.entries[0] = Entry::new("ONE.PAL", pal(10)).unwrap();
        assert!(matches!(
            companion(&source, &target, "ONE.PT", "ONE.PT", None),
            Offer::Skip(r) if r.contains("identical ONE.PAL")
        ));
        // The target's PALETTE.PAL already colors it: skipped.
        target.entries = vec![Entry::new(GAME, pal(1)).unwrap()];
        assert!(matches!(
            companion(&source, &target, "ONE.PT", "ONE.PT", Some((&game, "G"))),
            Offer::Skip(r) if r.contains("has PALETTE.PAL")
        ));
    }
    #[test]
    fn checks_report_bare_aircraft_and_a_recoloring_game_palette() {
        let mut a = fixture();
        let mut report = Report::default();
        check(&mut report, &a, None, true);
        assert!(report.checks.is_empty());
        let two = a.find("TWO.PAL").unwrap();
        a.entries.remove(two);
        let one = a.find("ONE.PAL").unwrap();
        a.entries.remove(one);
        check(&mut report, &a, None, true);
        assert_eq!(report.checks.len(), 1);
        assert_eq!(report.checks[0].level, Level::Info);
        assert!(report.checks[0]
            .message
            .starts_with("ONE.PT, TWO.PT: colors in Hangar"));
        // Long lists are cut after four names.
        let mut many = a.clone();
        for n in ["A", "B", "C", "D"] {
            many.entries.push(brf(&format!("{n}.PT"), &[]));
        }
        let mut long = Report::default();
        check(&mut long, &many, None, true);
        assert!(long.checks[0]
            .message
            .starts_with("ONE.PT, TWO.PT, A.PT, B.PT and 2 more aircraft: colors"));
        // A custom PALETTE.PAL: fine when it matches the game's, a warning otherwise.
        a.entries.push(Entry::new(GAME, pal(7)).unwrap());
        let mut report = Report::default();
        check(&mut report, &a, Some(&pal(7)), true);
        assert!(report.checks.is_empty());
        check(&mut report, &a, Some(&pal(8)), true);
        assert_eq!(report.warnings, 1);
        assert!(report.checks[0].message.contains("recolor the whole game"));
        // A retail LIB is the game palette's owner: never warned.
        let mut report = Report::default();
        check(&mut report, &a, Some(&pal(8)), false);
        assert!(report.checks.is_empty());
    }
}
