//! Object identity: the short name, long name and self reference stored in a
//! definition's names block, and in-place aircraft rename and duplicate plans
//! that share the export graph's ownership rules.
use crate::{
    archive::{validate_name, Archive, Entry, ARCHIVE_LIMIT},
    authoring::validate_id,
    brf::Brf,
    clone_aircraft::{self, Graph, Names, Package, Policy},
    dependencies, invalid, originals,
    resource_ops::{rewrite, Choice, Item, Plan},
    Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};

/// Display-name limit. No evidence of a game-side limit was found; retail
/// names reach 11 (short) and 28 (long) characters. Kept from the export.
pub const NAME_LIMIT: usize = 40;
/// ASCII without quotes, semicolons or control characters, 1..40 long.
pub fn validate_display(label: &str, text: &str) -> Result<()> {
    if text.is_empty()
        || text.len() > NAME_LIMIT
        || !text.is_ascii()
        || text.contains(['"', ';'])
        || text.bytes().any(|b| b < 32 || b == 127)
    {
        return Err(format!(
            "{label} must be 1..{NAME_LIMIT} plain ASCII characters without quotes/semicolons"
        ));
    }
    Ok(())
}
fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}
fn ext(s: &str) -> &str {
    s.rsplit('.').next().unwrap_or("")
}
fn stem(s: &str) -> &str {
    s.split('.').next().unwrap_or(s)
}
/// Recognized identity blocks as [short, long, self reference] field indices:
/// every ot_names/si_names pointer whose block holds exactly three strings.
/// The object block (ot_names) comes first in BRF order.
pub fn blocks(brf: &Brf) -> Vec<[usize; 3]> {
    let mut out = Vec::new();
    for pointer in brf.fields.iter().filter(|f| {
        f.kind == "ptr"
            && (f.label.ends_with(".ot_names")
                || f.label.ends_with(".si_names")
                || matches!(f.value.as_str(), "ot_names" | "si_names"))
    }) {
        let block: Vec<_> = brf
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.block == pointer.value)
            .collect();
        if block.len() == 3 && block.iter().all(|(_, f)| f.kind == "string") {
            let at = [block[0].0, block[1].0, block[2].0];
            if !out.contains(&at) {
                out.push(at);
            }
        }
    }
    out
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub short: String,
    pub long: String,
    /// Stored self reference, such as `F14.PT`.
    pub reference: String,
}
/// The first identity block's text, unquoted.
pub fn read(brf: &Brf) -> Option<Identity> {
    let [s, l, r] = *blocks(brf).first()?;
    let text = |i: usize| unquote(&brf.fields[i].value).to_string();
    Some(Identity {
        short: text(s),
        long: text(l),
        reference: text(r),
    })
}
/// Replace the short and/or long name in place. Other identity blocks that
/// hold the same text (a weapon's si_names) follow, so they stay in step.
/// Nothing else changes; unchanged text returns the input bytes.
pub fn set_names(
    bytes: &[u8],
    extension: &str,
    short: Option<&str>,
    long: Option<&str>,
) -> Result<Vec<u8>> {
    let brf = Brf::parse(bytes, extension)?;
    let all = blocks(&brf);
    let first = *all
        .first()
        .ok_or("No recognized names block; the stored text is unchanged")?;
    let mut edits = Vec::new();
    for (slot, value, label) in [(0, short, "Short name"), (1, long, "Long name")] {
        let Some(value) = value else {
            continue;
        };
        validate_display(label, value)?;
        let old = &brf.fields[first[slot]].value;
        if unquote(old) == value {
            continue;
        }
        for block in &all {
            if brf.fields[block[slot]].value == *old {
                edits.push((block[slot], format!("\"{value}\"")));
            }
        }
    }
    if edits.is_empty() {
        return Ok(bytes.to_vec());
    }
    brf.edit_many(bytes, &edits, extension)
}

/// Why a graph resource keeps its name and references.
fn conventional_share(name: &str, graph: &Graph) -> Option<&'static str> {
    if name == graph.donor {
        return None;
    }
    match ext(name) {
        "JT" => Some("Weapon; shared by convention"),
        "5K" | "8K" | "11K" | "22K" | "WAV" => Some("Sound; shared by convention"),
        "FNT" => Some("Font; shared by convention"),
        "PT" | "NT" | "OT" => Some("Another object; shared by convention"),
        "PAL" if graph.palette.as_deref() == Some(name) && stem(name) != stem(&graph.donor) => {
            Some("Game palette; shared by convention")
        }
        _ => None,
    }
}
fn store(name: &str) -> bool {
    matches!(ext(name), "SEE" | "ECM" | "GAS")
}
const SUFFIXES: &[&str] = &[
    "H", "S", "_A", "_B", "_C", "_D", "_S", "_L", "_R", "_P", "_LH", "_LS", "_RH", "_RS", "_CH",
    "_CS",
];
/// Names that only move together: the damage family, a store and its icon,
/// and a texture with its suffix family. Each group is sorted and disjoint.
fn groups(graph: &Graph) -> Vec<BTreeSet<String>> {
    let r = &graph.resources;
    let mut raw: Vec<BTreeSet<String>> = Vec::new();
    if let Some(family) = &graph.family {
        raw.push(set(["A", "B", "C", "D", "S"]
            .iter()
            .map(|s| format!("{family}_{s}.SH"))
            .filter(|n| r.contains(n))));
    }
    for name in r {
        if matches!(ext(name), "JT" | "SEE" | "ECM" | "GAS") {
            let icon = format!("${}.PIC", stem(name));
            if r.contains(&icon) {
                raw.push(set([name.clone(), icon]));
            }
        }
        if ext(name) == "PIC" {
            let s = stem(name);
            let base = SUFFIXES
                .iter()
                .filter_map(|x| s.strip_suffix(x))
                .find(|b| !b.is_empty() && r.contains(&format!("{b}.PIC")))
                .unwrap_or(s);
            let mut g = set(SUFFIXES
                .iter()
                .map(|x| format!("{base}{x}.PIC"))
                .filter(|n| r.contains(n)));
            g.insert(format!("{base}.PIC"));
            g.retain(|n| r.contains(n));
            if g.len() > 1 {
                raw.push(g);
            }
        }
    }
    // Merge overlapping groups.
    let mut out: Vec<BTreeSet<String>> = Vec::new();
    for g in raw {
        let mut merged = g;
        let mut keep = Vec::new();
        for o in out {
            if o.iter().any(|n| merged.contains(n)) {
                merged.extend(o);
            } else {
                keep.push(o);
            }
        }
        keep.push(merged);
        out = keep;
    }
    out
}
/// The export graph of an aircraft split into resources private to it and
/// resources shared in this LIB. Users come from a full scan of the LIB's
/// stored references and reviewed conventions.
#[derive(Debug)]
pub struct Ownership {
    pub graph: Graph,
    pub private: BTreeSet<String>,
    /// Shared graph resource -> reason.
    pub shared: BTreeMap<String, String>,
    groups: Vec<BTreeSet<String>>,
    /// Unparsed resources (missions and other text) that name the aircraft.
    pub mentions: Vec<String>,
    /// Resources whose references could not be read; never rewritten.
    pub unreadable: Vec<String>,
}
fn mentions(bytes: &[u8], name: &str) -> bool {
    let n = name.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~.".contains(&b);
    bytes.windows(n.len()).enumerate().any(|(i, w)| {
        w.eq_ignore_ascii_case(n)
            && (i == 0 || !word(bytes[i - 1]))
            && bytes.get(i + n.len()).is_none_or(|b| !word(*b))
    })
}
/// Insert one by one: collecting a set sorts a buffer first, and the stable
/// sort's stack buffer cannot be probed by the CRT-free x86_64 build.
fn set(names: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for n in names {
        out.insert(n);
    }
    out
}
fn catalog(archive: &Archive) -> BTreeSet<String> {
    set(archive.entries.iter().map(|e| e.name.clone()))
}
fn read_entry(archive: &Archive, name: &str) -> Result<Vec<u8>> {
    let i = archive
        .find(name)
        .ok_or_else(|| format!("Missing referenced resource {name}"))?;
    archive.entries[i].read()
}
pub fn ownership(archive: &Archive, aircraft: &str) -> Result<Ownership> {
    let aircraft = aircraft.to_ascii_uppercase();
    if ext(&aircraft) != "PT" || archive.find(&aircraft).is_none() {
        return Err(invalid("Select an aircraft PT in this LIB"));
    }
    let names = catalog(archive);
    let graph = clone_aircraft::graph(&names, &aircraft, |n| read_entry(archive, n))?;
    // Every user of every name in this LIB, by stored reference or convention.
    let mut users: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut found = Vec::new();
    let mut unreadable = Vec::new();
    let mut decoded = 0usize;
    for e in &archive.entries {
        if dependencies::leaf(&e.name) {
            continue;
        }
        let Ok(bytes) = e.read() else {
            unreadable.push(e.name.clone());
            continue;
        };
        decoded = decoded.saturating_add(bytes.len());
        if decoded > ARCHIVE_LIMIT {
            return Err(invalid("LIB reference scan exceeds 128 MiB"));
        }
        if !bytes.starts_with(b"MZ") && !bytes.starts_with(b"[brent's_relocatable_format]") {
            if mentions(&bytes, &aircraft) {
                found.push(e.name.clone());
            }
            continue;
        }
        let mut notes = BTreeSet::new();
        match dependencies::references(&e.name, &bytes, &names, &mut notes) {
            Ok(refs) => {
                for r in refs.into_iter().filter(|r| r.target != e.name) {
                    users.entry(r.target).or_default().insert(e.name.clone());
                }
            }
            Err(_) => unreadable.push(e.name.clone()),
        }
        for link in dependencies::conventional(&e.name, &bytes, &names) {
            if link.target != e.name {
                users.entry(link.target).or_default().insert(e.name.clone());
            }
        }
    }
    let groups = groups(&graph);
    let group = |name: &str| -> BTreeSet<String> {
        groups
            .iter()
            .find(|g| g.contains(name))
            .cloned()
            .unwrap_or_else(|| set([name.to_string()]))
    };
    let mut private = graph.resources.clone();
    let mut shared = BTreeMap::new();
    let share_group = |name: &str,
                       reason: String,
                       private: &mut BTreeSet<String>,
                       shared: &mut BTreeMap<String, String>| {
        for member in group(name) {
            if private.remove(&member) {
                let why = if member == name {
                    reason.clone()
                } else {
                    format!("Moves with {name}")
                };
                shared.insert(member, why);
            }
        }
    };
    for name in &graph.resources {
        if let Some(reason) = conventional_share(name, &graph) {
            share_group(name, reason.into(), &mut private, &mut shared);
        }
    }
    // A resource with any user outside the private set is shared, until stable.
    loop {
        let outside = private.iter().find_map(|name| {
            (name != &graph.donor)
                .then(|| users.get(name))
                .flatten()
                .and_then(|u| u.iter().find(|u| !private.contains(*u)))
                .map(|u| (name.clone(), u.clone()))
        });
        let Some((name, user)) = outside else {
            break;
        };
        share_group(&name, format!("Used by {user}"), &mut private, &mut shared);
    }
    Ok(Ownership {
        graph,
        private,
        shared,
        groups,
        mentions: found,
        unreadable,
    })
}
impl Ownership {
    /// The names that move with `name` (itself included).
    pub fn group(&self, name: &str) -> BTreeSet<String> {
        self.groups
            .iter()
            .find(|g| g.contains(name))
            .cloned()
            .unwrap_or_else(|| set([name.to_string()]))
    }
    /// Copied whatever the review says: the aircraft and a HUD found by its name.
    pub fn always_copied(&self, name: &str) -> bool {
        name == self.graph.donor
            || (self.graph.default_hud && self.graph.hud.as_deref() == Some(name))
    }
    /// Duplicate defaults: copy the private resources; share weapons, sounds,
    /// sensors and everything already shared in this LIB.
    pub fn default_share(&self) -> BTreeSet<String> {
        let mut share = BTreeSet::new();
        for name in &self.graph.resources {
            if self.always_copied(name) {
                continue;
            }
            if !self.private.contains(name) || store(name) {
                share.extend(self.group(name));
            }
        }
        share.retain(|n| !self.always_copied(n));
        share
    }
    /// Flip one resource (and its group) between copy and share. Returns
    /// whether it is now shared.
    pub fn toggle(&self, share: &mut BTreeSet<String>, name: &str) -> Result<bool> {
        let name = name.to_ascii_uppercase();
        let group = self.group(&name);
        if let Some(fixed) = group.iter().find(|n| self.always_copied(n)) {
            return Err(if *fixed == self.graph.donor {
                format!("{fixed} is the aircraft; it is always copied")
            } else {
                format!("{fixed} is found by the aircraft's name; it is always copied")
            });
        }
        let now = !share.contains(&name);
        for member in group {
            if now {
                share.insert(member);
            } else {
                share.remove(&member);
            }
        }
        Ok(now)
    }
    pub fn reason(&self, name: &str) -> &str {
        self.shared.get(name).map_or(
            if store(name) {
                "Sensor or store; shared by default"
            } else {
                "Chosen to share"
            },
            String::as_str,
        )
    }
}
fn prefix(s: &str) -> &str {
    if s.starts_with(['~', '_', '$', '&', '#', '^']) {
        &s[..1]
    } else {
        ""
    }
}
/// `F14_C.SH` -> `F14Z_C.SH`, `~F14CP.PIC` -> `~F14ZCP.PIC`: the old ID after
/// any prefix character is replaced. None when the stem does not start with it.
fn substitute(name: &str, old: &str, new: &str) -> Option<String> {
    let (s, e) = name.rsplit_once('.')?;
    let pre = prefix(s);
    s[pre.len()..]
        .strip_prefix(old)
        .map(|tail| format!("{pre}{new}{tail}.{e}"))
}

/// In-place rename of an aircraft's reference ID and its private resources.
#[derive(Debug)]
pub struct Rename {
    pub old: String,
    pub new: String,
    /// Old -> new, stored originals (.ORG) included.
    pub renames: Vec<(String, String)>,
    /// Private resources whose names do not carry the old ID; unchanged.
    pub kept: Vec<String>,
    /// Shared graph resources left alone, with the reason.
    pub shared: Vec<(String, String)>,
    /// Other resources whose stored references are rewritten in place.
    pub rewritten: Vec<String>,
    pub refusals: Vec<String>,
    pub notes: Vec<String>,
    /// Unparsed resources that name the aircraft; not rewritten.
    pub mentions: Vec<String>,
    plan: Plan,
}
impl Rename {
    pub fn ready(&self) -> bool {
        self.refusals.is_empty()
    }
    /// True when the new ID is the old one: nothing changes.
    pub fn unchanged(&self) -> bool {
        self.renames.is_empty() && self.rewritten.is_empty()
    }
    /// One transaction; stale plans fail rather than overwrite later edits.
    pub fn apply(&self, doc: &mut crate::document::Document) -> Result<()> {
        if let Some(first) = self.refusals.first() {
            return Err(first.clone());
        }
        if self.unchanged() {
            return Ok(());
        }
        self.plan.apply(doc)
    }
    pub fn warning(&self) -> String {
        format!(
            "Missions and other LIBs that refer to {} by name will not find the renamed aircraft.",
            self.old
        )
    }
}
pub fn rename(archive: &Archive, aircraft: &str, new_id: &str) -> Result<Rename> {
    let aircraft = aircraft.to_ascii_uppercase();
    let id = validate_id(new_id.trim())?;
    let old_id = stem(&aircraft).to_string();
    let mut out = Rename {
        old: aircraft.clone(),
        new: format!("{id}.PT"),
        renames: Vec::new(),
        kept: Vec::new(),
        shared: Vec::new(),
        rewritten: Vec::new(),
        refusals: Vec::new(),
        notes: Vec::new(),
        mentions: Vec::new(),
        plan: Plan::default(),
    };
    let own = ownership(archive, &aircraft)?;
    out.shared = own
        .shared
        .iter()
        .map(|(n, r)| (n.clone(), r.clone()))
        .collect();
    if id == old_id {
        out.notes.push("Same reference ID; nothing changes.".into());
        return Ok(out);
    }
    out.mentions = own.mentions.clone();
    let mut map = BTreeMap::new();
    for name in &own.private {
        match substitute(name, &old_id, &id) {
            Some(new) => {
                if validate_name(&new).is_err() {
                    out.refusals
                        .push(format!("{name} -> {new} is longer than an 8.3 name"));
                    continue;
                }
                map.insert(name.clone(), new);
            }
            None => out.kept.push(name.clone()),
        }
    }
    if own.graph.default_hud {
        if let Some(hud) = own.graph.hud.as_ref().filter(|h| !map.contains_key(*h)) {
            out.refusals.push(match own.shared.get(hud) {
                Some(why) => format!(
                    "{hud} is found by {aircraft}'s name but is shared ({why}); the renamed aircraft would lose it"
                ),
                None => format!("{hud} is found by {aircraft}'s name and cannot follow the new ID"),
            });
        }
    }
    // Stored originals follow their PICs.
    let mut companions = Vec::new();
    for (old, new) in &map {
        if let Some(org) = originals::backup(archive, old) {
            match originals::companion(new) {
                Some(target) => companions.push((org.name.clone(), target)),
                None => out
                    .refusals
                    .push(format!("{}: stored original cannot follow {new}", org.name)),
            }
        }
    }
    let mut all: Vec<(String, String)> = map.iter().map(|(a, b)| (a.clone(), b.clone())).collect();
    all.extend(companions.iter().cloned());
    let leaving = set(all.iter().map(|(o, _)| o.clone()));
    let mut arriving = BTreeSet::new();
    for (old, new) in &all {
        if !arriving.insert(new.clone()) {
            out.refusals.push(format!(
                "{old} -> {new}: two resources would share this name"
            ));
        } else if archive.find(new).is_some() && !leaving.contains(new) {
            out.refusals
                .push(format!("{old} -> {new}: {new} already exists in this LIB"));
        }
    }
    // Rewrite every recognized stored reference in the LIB within its slot.
    let names = catalog(archive);
    let mut contents = BTreeMap::new();
    let mut decoded = 0usize;
    for e in &archive.entries {
        if dependencies::leaf(&e.name) || own.unreadable.contains(&e.name) {
            continue;
        }
        let bytes = e.read()?;
        if !bytes.starts_with(b"MZ") && !bytes.starts_with(b"[brent's_relocatable_format]") {
            continue;
        }
        decoded = decoded.saturating_add(bytes.len());
        if decoded > ARCHIVE_LIMIT {
            return Err(invalid("Rename scan exceeds 128 MiB"));
        }
        match rewrite(&e.name, &bytes, &map, &names) {
            Ok(changed) if changed != bytes => {
                contents.insert(e.name.clone(), changed);
            }
            Ok(_) => {}
            Err(error) => out.refusals.push(error),
        }
    }
    for name in &own.unreadable {
        out.notes.push(format!(
            "{name}: references could not be read; it is not rewritten"
        ));
    }
    // Same-stem files no stored reference ties to the aircraft keep their names.
    for e in &archive.entries {
        if stem(&e.name) == old_id
            && !leaving.contains(&e.name)
            && !own.shared.contains_key(&e.name)
        {
            out.notes.push(format!(
                "{} keeps its name; {aircraft} holds no stored reference to it",
                e.name
            ));
        }
    }
    let mut plan = Plan::default();
    for (old, new) in &all {
        let i = archive.find(old).ok_or("Resource missing")?;
        let source = &archive.entries[i];
        let entry = match contents.remove(old) {
            Some(bytes) => Entry::new(new, bytes)?,
            None => source.renamed(new)?,
        };
        plan.items.push(Item {
            entry,
            previous: None,
            choice: Choice::TakeSource,
            conflict: false,
        });
        plan.removals.push(old.clone());
        plan.removed_before.push(source.clone());
    }
    for (name, bytes) in contents {
        let i = archive.find(&name).ok_or("Resource missing")?;
        out.rewritten.push(name.clone());
        plan.items.push(Item {
            entry: Entry::new(&name, bytes)?,
            previous: Some(archive.entries[i].clone()),
            choice: Choice::TakeSource,
            conflict: false,
        });
    }
    out.renames = all;
    // The aircraft first, then by old name.
    out.renames
        .sort_unstable_by(|a, b| (a.0 != aircraft, &a.0).cmp(&(b.0 != aircraft, &b.0)));
    out.plan = plan;
    Ok(out)
}

/// A same-LIB duplicate: the export package built against this LIB, with
/// shared names referenced rather than copied, and its insertion plan.
#[derive(Debug)]
pub struct Duplicate {
    pub package: Package,
    plan: Plan,
}
impl Duplicate {
    /// The new aircraft's name, such as `F18Z.PT`.
    pub fn root(&self) -> String {
        format!("{}.PT", self.package.id)
    }
    pub fn apply(&self, doc: &mut crate::document::Document) -> Result<()> {
        if self.plan.items.iter().any(|i| i.conflict) {
            return Err(invalid("A copied name already exists; rebuild the review"));
        }
        self.plan.apply(doc)
    }
}
pub fn duplicate(
    archive: &Archive,
    own: &Ownership,
    id: &str,
    names: Names<'_>,
    share: &BTreeSet<String>,
) -> Result<Duplicate> {
    let id = validate_id(id.trim())?;
    let catalog = catalog(archive);
    if catalog.contains(&format!("{id}.PT")) {
        return Err(format!("{id}.PT already exists; choose a new ID"));
    }
    let package = clone_aircraft::build_sharing(
        &catalog,
        &own.graph.donor,
        &id,
        names,
        // The same LIB provides every name, so kept names behave as before.
        &Policy {
            keep_unresolved: true,
            ..Policy::default()
        },
        share,
        |n| read_entry(archive, n),
    )?;
    let mut plan = Plan::default();
    for e in &package.archive.entries {
        let previous = archive.find(&e.name).map(|i| archive.entries[i].clone());
        let conflict = previous.is_some();
        plan.items.push(Item {
            entry: e.clone(),
            previous,
            choice: if conflict {
                Choice::Unresolved
            } else {
                Choice::TakeSource
            },
            conflict,
        });
    }
    Ok(Duplicate { package, plan })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::Document, model, picture};
    fn module(code: &[u8]) -> Vec<u8> {
        let mut b = model::demo_shape();
        b.truncate(256);
        b[70..72].copy_from_slice(&1u16.to_le_bytes());
        for (at, n) in [(128, code.len()), (136, code.len())] {
            b[at..at + 4].copy_from_slice(&(n as u32).to_le_bytes());
        }
        b.extend(code);
        b
    }
    fn textured(name: &str) -> Vec<u8> {
        let mut b = model::demo_textured();
        b[258..272].fill(0);
        b[258..258 + name.len()].copy_from_slice(name.as_bytes());
        b
    }
    fn brf(strings: &[&str]) -> Vec<u8> {
        let mut s = String::from("[brent's_relocatable_format]\n");
        for t in strings {
            s.push_str(&format!("string \"{t}\"\n"));
        }
        s.push_str("end\n");
        s.into_bytes()
    }
    fn pt(id: &str, extra: &[&str]) -> Vec<u8> {
        let mut text = String::from_utf8(crate::brf::demo())
            .unwrap()
            .replace("DEMO", id)
            .replace("Demo", id);
        let mut tail = String::from(":stores\r\n");
        for s in extra {
            tail.push_str(&format!("string \"{s}\"\r\n"));
        }
        tail.push_str("end\r\n");
        text = text.replace("end\r\n", &tail);
        text.into_bytes()
    }
    /// DEMO.PT and TWO.PT. DEMO owns its shapes, skin family, cockpit art,
    /// HUD (found by name) and sensor; SHOT.JT, SFX.11K and PALETTE.PAL are
    /// shared by convention; SHARED.PIC is also drawn by TWO.SH.
    fn fixture() -> Archive {
        let mut a = Archive::empty();
        let mut add = |n: &str, b: Vec<u8>| a.entries.push(Entry::new(n, b).unwrap());
        add("DEMO.PT", pt("DEMO", &["SHOT.JT", "DEMOR.SEE", "SFX.11K"]));
        add("DEMO.SH", textured("DEMO.PIC"));
        add("DEMO_A.SH", textured("DEMO_A.PIC"));
        add("DEMO_B.SH", textured("SHARED.PIC"));
        for s in ["C", "D", "S"] {
            add(&format!("DEMO_{s}.SH"), model::demo_shape());
        }
        let mut hud = vec![0; 0x2b2];
        hud[1..8].copy_from_slice(b"~DEMOCP");
        add("DEMO.HUD", module(&hud));
        add("SHOT.JT", brf(&["SHOT.JT", "SFX.11K"]));
        add("DEMOR.SEE", brf(&["DEMOR.SEE"]));
        add("SFX.11K", b"DEMO.SH\0sample".to_vec());
        for n in [
            "DEMO.PIC",
            "DEMO_A.PIC",
            "SHARED.PIC",
            "~DEMOCP.PIC",
            "$SHOT.PIC",
            "$DEMOR.PIC",
        ] {
            add(n, picture::demo());
        }
        add("DEMO.ORG", picture::demo());
        add("PALETTE.PAL", vec![0; 768]);
        add("TWO.PT", pt("TWO", &["SHOT.JT"]));
        add("TWO.SH", textured("SHARED.PIC"));
        for s in ["A", "B", "C", "D", "S"] {
            add(&format!("TWO_{s}.SH"), model::demo_shape());
        }
        add("CARRIER.NT", brf(&["CARRIER.NT", "DEMO.PT"]));
        add("MISSION.M", b"textFormat\nobj\n\ttype demo.pt\n".to_vec());
        a
    }
    fn payload(a: &Archive, n: &str) -> Vec<u8> {
        a.entries[a.find(n).unwrap_or_else(|| panic!("{n}"))]
            .read()
            .unwrap()
    }
    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }
    #[test]
    fn names_edit_in_place_and_follow_matching_blocks() {
        let bytes = pt("DEMO", &[]);
        let b = Brf::parse(&bytes, "PT").unwrap();
        let id = read(&b).unwrap();
        assert_eq!(
            (id.short.as_str(), id.long.as_str(), id.reference.as_str()),
            ("DEMO", "Synthetic aircraft", "DEMO.PT")
        );
        let out = set_names(&bytes, "PT", Some("F-14"), None).unwrap();
        let after = read(&Brf::parse(&out, "PT").unwrap()).unwrap();
        assert_eq!(after.short, "F-14");
        assert_eq!(after.long, "Synthetic aircraft");
        // Only the operand changed: every other byte is where it was.
        let at = bytes.windows(6).position(|w| w == b"\"DEMO\"").unwrap();
        assert_eq!(out[..at], bytes[..at]);
        assert_eq!(out[at + 6..], bytes[at + 6..]);
        assert_eq!(set_names(&bytes, "PT", Some("DEMO"), None).unwrap(), bytes);
        for bad in ["", "a\"b", "semi;colon", "tab\t", "\u{e9}t\u{e9}"] {
            assert!(set_names(&bytes, "PT", Some(bad), None).is_err(), "{bad}");
        }
        assert!(set_names(&bytes, "PT", None, Some(&"x".repeat(41))).is_err());
        assert!(set_names(&bytes, "PT", None, Some(&"x".repeat(40))).is_ok());
        // A weapon's si_names copy follows the ot_names text it matches.
        let jt = "[brent's_relocatable_format]\nptr ot_names\nptr si_names\n:ot_names\nstring \"Old\"\nstring \"Old weapon\"\nstring \"OLD.JT\"\n:si_names\nstring \"Old\"\nstring \"Other\"\nstring \"OLD.JT\"\nend\n";
        let out = set_names(jt.as_bytes(), "JT", Some("New"), Some("New weapon")).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("\"New\"").count(), 2);
        assert_eq!(text.matches("\"New weapon\"").count(), 1);
        assert!(text.contains("\"Other\""));
    }
    #[test]
    fn ownership_splits_private_and_shared() {
        let a = fixture();
        let own = ownership(&a, "DEMO.PT").unwrap();
        assert_eq!(
            own.private,
            set(&[
                "$DEMOR.PIC",
                "DEMO.HUD",
                "DEMO.PIC",
                "DEMO.PT",
                "DEMO.SH",
                "DEMOR.SEE",
                "DEMO_A.PIC",
                "DEMO_A.SH",
                "DEMO_B.SH",
                "DEMO_C.SH",
                "DEMO_D.SH",
                "DEMO_S.SH",
                "~DEMOCP.PIC",
            ])
        );
        let shared: BTreeSet<_> = own.shared.keys().cloned().collect();
        assert_eq!(
            shared,
            set(&[
                "$SHOT.PIC",
                "PALETTE.PAL",
                "SFX.11K",
                "SHARED.PIC",
                "SHOT.JT"
            ])
        );
        assert_eq!(own.shared["SHARED.PIC"], "Used by TWO.SH");
        assert_eq!(own.shared["$SHOT.PIC"], "Moves with SHOT.JT");
        assert_eq!(own.mentions, ["MISSION.M"]);
        assert!(own.graph.default_hud);
        // A family member used elsewhere shares the whole damage family.
        let mut b = fixture();
        b.entries
            .push(Entry::new("OTHER.OT", brf(&["DEMO_C.SH"])).unwrap());
        let own = ownership(&b, "DEMO.PT").unwrap();
        for s in ["A", "B", "C", "D", "S"] {
            assert!(own.shared.contains_key(&format!("DEMO_{s}.SH")));
        }
        assert_eq!(own.shared["DEMO_C.SH"], "Used by OTHER.OT");
        assert!(own.private.contains("DEMO.SH"));
        // DEMO_A.PIC is only drawn by the now-shared DEMO_A.SH, and moves
        // with DEMO.PIC as a skin family.
        assert!(own.shared.contains_key("DEMO_A.PIC"));
        assert!(own.shared.contains_key("DEMO.PIC"));
    }
    #[test]
    fn rename_rewrites_every_reference_and_undoes_exactly() {
        let a = fixture();
        let before = a.bytes().unwrap();
        let mut doc = Document::new(a.clone());
        let plan = rename(&doc.archive, "DEMO.PT", "neo").unwrap();
        assert!(plan.ready(), "{:?}", plan.refusals);
        let map: BTreeMap<_, _> = plan.renames.iter().cloned().collect();
        assert_eq!(plan.renames[0], ("DEMO.PT".into(), "NEO.PT".into()));
        for (old, new) in [
            ("DEMO_C.SH", "NEO_C.SH"),
            ("~DEMOCP.PIC", "~NEOCP.PIC"),
            ("$DEMOR.PIC", "$NEOR.PIC"),
            ("DEMOR.SEE", "NEOR.SEE"),
            ("DEMO.HUD", "NEO.HUD"),
            ("DEMO.ORG", "NEO.ORG"),
        ] {
            assert_eq!(map[old], new);
        }
        assert_eq!(map.len(), 14);
        assert_eq!(plan.rewritten, ["CARRIER.NT"]);
        assert_eq!(plan.mentions, ["MISSION.M"]);
        assert!(plan.warning().contains("refer to DEMO.PT by name"));
        plan.apply(&mut doc).unwrap();
        let b = &doc.archive;
        for old in map.keys() {
            assert!(b.find(old).is_none(), "{old}");
        }
        // Self reference and display names.
        let id = read(&Brf::parse(&payload(b, "NEO.PT"), "PT").unwrap()).unwrap();
        assert_eq!(id.reference, "NEO.PT");
        assert_eq!(id.short, "DEMO");
        let text = String::from_utf8(payload(b, "NEO.PT")).unwrap();
        for name in ["NEO.SH", "NEO_S.SH", "NEOR.SEE", "SHOT.JT", "SFX.11K"] {
            assert!(text.contains(&format!("\"{name}\"")), "{name}");
        }
        assert!(String::from_utf8(payload(b, "CARRIER.NT"))
            .unwrap()
            .contains("\"NEO.PT\""));
        // Fixed-slot module names: E2 texture and the HUD's stem-only name.
        assert!(model::Model::parse(&payload(b, "NEO.SH"))
            .unwrap()
            .textures
            .contains("NEO.PIC"));
        let hud = payload(b, "NEO.HUD");
        assert_eq!(&hud[257..264], b"~NEOCP\0");
        assert_eq!(hud.len(), payload(&a, "DEMO.HUD").len());
        // Nothing outside the plan changed.
        let touched: BTreeSet<_> = map
            .values()
            .cloned()
            .chain(plan.rewritten.clone())
            .collect();
        for e in &a.entries {
            if map.contains_key(&e.name) || touched.contains(&e.name) {
                continue;
            }
            assert!(
                e.same_storage(&b.entries[b.find(&e.name).unwrap()]),
                "{}",
                e.name
            );
        }
        assert_eq!(payload(b, "MISSION.M"), payload(&a, "MISSION.M"));
        assert_eq!(payload(b, "TWO_B.SH"), payload(&a, "TWO_B.SH"));
        // Unchanged leaf payloads keep their storage under the new name.
        assert!(b.entries[b.find("NEO.PIC").unwrap()]
            .same_payload(&a.entries[a.find("DEMO.PIC").unwrap()]));
        let mut index = dependencies::Index::default();
        index.update(b);
        assert_eq!(
            index.incoming("NEO.PIC").cloned().collect::<Vec<_>>(),
            ["NEO.SH"]
        );
        assert!(doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), before);
        assert!(!doc.undo());
    }
    #[test]
    fn same_id_rename_is_identical() {
        let a = fixture();
        let before = a.bytes().unwrap();
        let mut doc = Document::new(a);
        let plan = rename(&doc.archive, "demo.pt", "DEMO").unwrap();
        assert!(plan.unchanged() && plan.ready());
        plan.apply(&mut doc).unwrap();
        assert!(!doc.dirty());
        assert!(!doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), before);
    }
    #[test]
    fn rename_refuses_collisions_overflow_and_long_names() {
        let mut a = fixture();
        a.entries
            .push(Entry::new("NEO_C.SH", model::demo_shape()).unwrap());
        let before = a.bytes().unwrap();
        let mut doc = Document::new(a);
        let plan = rename(&doc.archive, "DEMO.PT", "NEO").unwrap();
        assert!(!plan.ready());
        assert!(
            plan.refusals[0].contains("NEO_C.SH already exists"),
            "{:?}",
            plan.refusals
        );
        assert!(plan.apply(&mut doc).is_err());
        assert_eq!(doc.archive.bytes().unwrap(), before);
        // ~DEMOCP -> ~LONGIDCP has a 9-character stem.
        let plan = rename(&fixture(), "DEMO.PT", "LONGID").unwrap();
        assert!(plan
            .refusals
            .iter()
            .any(|r| r.contains("~DEMOCP.PIC -> ~LONGIDCP.PIC is longer than an 8.3 name")));
        // A plain module literal has only its own length to grow into.
        let mut a = fixture();
        let mut shape = textured("DEMO.PIC");
        shape.extend(b"\0DEMO.PIC\0");
        let size = (shape.len() - 256) as u32;
        shape[128..132].copy_from_slice(&size.to_le_bytes());
        shape[136..140].copy_from_slice(&size.to_le_bytes());
        let at = a.find("DEMO.SH").unwrap();
        a.entries[at] = Entry::new("DEMO.SH", shape).unwrap();
        let plan = rename(&a, "DEMO.PT", "DEMOX").unwrap();
        assert!(
            plan.refusals
                .iter()
                .any(|r| r.contains("DEMOX.PIC exceeds a compiled filename slot (8 bytes)")),
            "{:?}",
            plan.refusals
        );
        // A HUD found by name that another aircraft also uses cannot follow.
        let mut a = fixture();
        a.entries
            .push(Entry::new("USER.OT", brf(&["DEMO.HUD"])).unwrap());
        let plan = rename(&a, "DEMO.PT", "NEO").unwrap();
        assert!(plan
            .refusals
            .iter()
            .any(|r| r.contains("DEMO.HUD is found by DEMO.PT's name")));
    }
    #[test]
    fn duplicate_copies_private_and_references_shared_names() {
        let a = fixture();
        let before = a.bytes().unwrap();
        let mut doc = Document::new(a.clone());
        let own = ownership(&doc.archive, "DEMO.PT").unwrap();
        let mut share = own.default_share();
        assert_eq!(
            share,
            set(&[
                "$DEMOR.PIC",
                "$SHOT.PIC",
                "DEMOR.SEE",
                "PALETTE.PAL",
                "SFX.11K",
                "SHARED.PIC",
                "SHOT.JT"
            ])
        );
        let names = Names {
            short: "Twin",
            long: "Twin test aircraft",
        };
        let dup = duplicate(&doc.archive, &own, "TWIN", names, &share).unwrap();
        let p = &dup.package;
        let map: BTreeMap<_, _> = p.mapping.iter().cloned().collect();
        assert_eq!(map["DEMO.PT"], "TWIN.PT");
        assert_eq!(map["DEMO_C.SH"], "TWIN_C.SH");
        assert!(map.contains_key("DEMO.PIC") && map.contains_key("~DEMOCP.PIC"));
        for shared in [
            "SHOT.JT",
            "SFX.11K",
            "SHARED.PIC",
            "DEMOR.SEE",
            "PALETTE.PAL",
        ] {
            assert!(!map.contains_key(shared), "{shared}");
            assert!(p.shared.contains(&shared.to_string()), "{shared}");
        }
        // The copied skin's stored original follows it.
        let org = originals::companion(&map["DEMO.PIC"]).unwrap();
        assert!(p.archive.find(&org).is_some());
        dup.apply(&mut doc).unwrap();
        let b = &doc.archive;
        let text = String::from_utf8(payload(b, "TWIN.PT")).unwrap();
        for name in [
            "\"SHOT.JT\"",
            "\"DEMOR.SEE\"",
            "\"SFX.11K\"",
            "\"Twin\"",
            "\"Twin test aircraft\"",
            "\"TWIN.PT\"",
        ] {
            assert!(text.contains(name), "{name}");
        }
        assert!(model::Model::parse(&payload(b, "TWIN_B.SH"))
            .unwrap()
            .textures
            .contains("SHARED.PIC"));
        // The original aircraft is untouched and every name resolves.
        for e in &a.entries {
            assert!(e.same_storage(&b.entries[b.find(&e.name).unwrap()]));
        }
        let report = crate::validation::inspect(&doc, &mut Default::default());
        assert_eq!(report.errors, 0, "{}", report.summary());
        assert!(doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), before);

        // Copy the sensor too: its icon follows; the HUD found by name and the
        // aircraft itself cannot be shared.
        assert!(!own.toggle(&mut share, "DEMOR.SEE").unwrap());
        assert!(!share.contains("$DEMOR.PIC"));
        assert!(own.toggle(&mut share, "DEMO.HUD").is_err());
        assert!(own.toggle(&mut share, "DEMO.PT").is_err());
        // Share the damage family: the copy keeps DEMO_S.SH, which the game
        // derives DEMO_A..D from.
        assert!(own.toggle(&mut share, "DEMO_B.SH").unwrap());
        assert!(share.contains("DEMO_S.SH") && share.contains("DEMO_A.SH"));
        let dup = duplicate(&doc.archive, &own, "TWIN", names, &share).unwrap();
        let map: BTreeMap<_, _> = dup.package.mapping.iter().cloned().collect();
        assert!(map.contains_key("DEMOR.SEE") && map.contains_key("$DEMOR.PIC"));
        assert_eq!(
            map["$DEMOR.PIC"],
            format!("${}.PIC", stem(&map["DEMOR.SEE"]))
        );
        assert!(!map.contains_key("DEMO_A.SH") && !map.contains_key("DEMO_A.PIC"));
        dup.apply(&mut doc).unwrap();
        let text = String::from_utf8(payload(&doc.archive, "TWIN.PT")).unwrap();
        assert!(text.contains("\"DEMO_S.SH\"") && !text.contains("\"DEMOR.SEE\""));
        let report = crate::validation::inspect(&doc, &mut Default::default());
        assert_eq!(report.errors, 0, "{}", report.summary());
        assert!(doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), before);
        // Collisions are checked against the whole LIB.
        assert!(duplicate(&doc.archive, &own, "TWO", names, &share).is_err());
    }
}
