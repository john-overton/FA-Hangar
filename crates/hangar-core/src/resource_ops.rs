//! Reviewed resource transfers and bounded filename rewrites.
use crate::{
    archive::{Archive, Entry},
    brf::Brf,
    dependencies::{self, Location},
    document::Document,
    invalid, originals, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Unresolved,
    KeepTarget,
    TakeSource,
}
#[derive(Clone, Debug)]
pub struct Item {
    pub entry: Entry,
    pub previous: Option<Entry>,
    pub choice: Choice,
    pub conflict: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub items: Vec<Item>,
    pub removals: Vec<String>,
    removed_before: Vec<Entry>,
    pub notes: Vec<String>,
}
impl Plan {
    pub fn ready(&self) -> bool {
        self.items
            .iter()
            .all(|i| i.choice != Choice::Unresolved || self.follows_kept(i))
    }
    /// A stored original (`X.ORG`) follows its texture: when the target keeps
    /// its own `X.PIC`, the source's original for a different PIC is not copied.
    pub fn follows_kept(&self, item: &Item) -> bool {
        originals::texture_of(&item.entry.name).is_some_and(|pic| {
            self.items
                .iter()
                .any(|i| i.entry.name == pic && i.conflict && i.choice == Choice::KeepTarget)
        })
    }
    pub fn apply(&self, doc: &mut Document) -> Result<()> {
        if !self.ready() {
            return Err(invalid(
                "Choose Keep target or Take source for every collision",
            ));
        }
        for item in &self.items {
            let current = doc
                .archive
                .find(&item.entry.name)
                .map(|i| &doc.archive.entries[i]);
            match (current, &item.previous) {
                (None, None) => {}
                (Some(a), Some(b)) if a.same_storage(b) => {}
                _ => return Err(invalid("Target LIB changed; rebuild the review")),
            }
        }
        for original in &self.removed_before {
            if !doc
                .archive
                .find(&original.name)
                .is_some_and(|i| doc.archive.entries[i].same_storage(original))
            {
                return Err(invalid("Removed resource changed; rebuild the review"));
            }
        }
        let entries = self
            .items
            .iter()
            .filter(|i| i.choice == Choice::TakeSource && !self.follows_kept(i))
            .map(|i| i.entry.clone())
            .collect();
        doc.transaction(entries, &self.removals)
    }
}
/// Apply a reviewed cross-document move atomically in memory. Shared dependencies
/// remain in the source; each document retains one independent undo step.
pub fn move_between(
    source: &mut Document,
    target: &mut Document,
    plan: &Plan,
    root: &str,
) -> Result<usize> {
    let root = root.to_ascii_uppercase();
    let mut removals = BTreeSet::new();
    for item in &plan.items {
        let Some(i) = source.archive.find(&item.entry.name) else {
            continue;
        };
        if !source.archive.entries[i].same_storage(&item.entry) {
            return Err(invalid("Source LIB changed; rebuild the move review"));
        }
        if (item.choice == Choice::TakeSource && !plan.follows_kept(item))
            || target
                .archive
                .find(&item.entry.name)
                .is_some_and(|i| same(&target.archive.entries[i], &item.entry))
        {
            removals.insert(item.entry.name.clone());
        }
    }
    if !removals.contains(&root) {
        return Err(invalid(
            "Take the selected source item (or an identical target) before moving it",
        ));
    }
    let mut index = crate::dependencies::Index::default();
    index.update(&source.archive);
    loop {
        let shared: Vec<_> = removals
            .iter()
            .filter(|name| {
                **name != root && index.incoming(name).any(|user| !removals.contains(user))
            })
            .cloned()
            .collect();
        if shared.is_empty() {
            break;
        }
        for name in shared {
            removals.remove(&name);
        }
    }
    // A texture that stays in the source keeps its stored original there.
    let staying: Vec<_> = removals
        .iter()
        .filter(|name| {
            **name != root
                && originals::texture_of(name).is_some_and(|pic| {
                    !removals.contains(&pic) && source.archive.find(&pic).is_some()
                })
        })
        .cloned()
        .collect();
    for name in staying {
        removals.remove(&name);
    }
    let mut new_source = source.clone();
    let mut new_target = target.clone();
    plan.apply(&mut new_target)?;
    let removals: Vec<_> = removals.into_iter().collect();
    new_source.transaction(Vec::new(), &removals)?;
    *source = new_source;
    *target = new_target;
    Ok(removals.len())
}
fn same(a: &Entry, b: &Entry) -> bool {
    // Compare stored representations without decompressing arbitrary collisions.
    a.same_storage(b) || (a.flag() == b.flag() && a.stored() == b.stored())
}
/// Missing dependencies stay explicit in the review; thin mod libraries may
/// intentionally rely on game resources. No source file is written.
pub fn transfer(
    source: &Archive,
    target: &Archive,
    root: &str,
    include_dependencies: bool,
) -> Result<Plan> {
    transfer_from(&[source], target, root, include_dependencies)
}
pub fn transfer_from(
    sources: &[&Archive],
    target: &Archive,
    root: &str,
    include_dependencies: bool,
) -> Result<Plan> {
    let mut plan = Plan::default();
    let mut catalog = BTreeSet::new();
    for source in sources {
        for e in &source.entries {
            catalog.insert(e.name.clone());
        }
    }
    if catalog.len() > 131072 {
        return Err(invalid("Transfer source catalog exceeds 131072 names"));
    }
    let mut decoded = 0usize;
    let mut visited = BTreeSet::new();
    let mut companions = BTreeSet::new();
    let mut pending = vec![root.to_ascii_uppercase()];
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        if visited.len() > 4096 {
            return Err(invalid("Transfer exceeds 4096 resources"));
        }
        let primary = sources
            .first()
            .and_then(|a| a.find(&name).map(|i| &a.entries[i]));
        let mut found = primary;
        if found.is_none() {
            for archive in sources.iter().skip(1) {
                if let Some(i) = archive.find(&name) {
                    let candidate = &archive.entries[i];
                    if let Some(old) = found {
                        if !same(old, candidate) {
                            return Err(format!("Ambiguous dependency {name} in open source LIBs; close a conflicting source or copy from the intended owner"));
                        }
                    } else {
                        found = Some(candidate);
                    }
                }
            }
        }
        if companions.contains(&name) && !found.is_some_and(originals::valid) {
            continue;
        }
        let Some(entry) = found.cloned() else {
            if target.find(&name).is_none() {
                plan.notes.push(format!(
                    "External dependency {name} is absent from the open source LIBs and target"
                ));
            }
            continue;
        };
        let previous = target.find(&name).map(|i| target.entries[i].clone());
        let identical = previous.as_ref().is_some_and(|p| same(&entry, p));
        let conflict = previous.is_some() && !identical;
        plan.items.push(Item {
            entry,
            previous,
            choice: if conflict {
                Choice::Unresolved
            } else if identical {
                Choice::KeepTarget
            } else {
                Choice::TakeSource
            },
            conflict,
        });
        // A texture's stored original travels with it, even without dependencies.
        if let Some(org) = originals::companion(&name).filter(|n| catalog.contains(n)) {
            companions.insert(org.clone());
            pending.push(org);
        }
        if include_dependencies && !dependencies::leaf(&name) {
            match found.unwrap().read() {
                Ok(bytes) => {
                    decoded = decoded.saturating_add(bytes.len());
                    if decoded > crate::archive::ARCHIVE_LIMIT {
                        return Err(invalid("Transfer scan exceeds 128 MiB decoded data"));
                    }
                    let mut notes = BTreeSet::new();
                    match dependencies::references(&name, &bytes, &catalog, &mut notes) {
                        Ok(refs) => pending.extend(refs.into_iter().map(|r| r.target)),
                        Err(issue) => plan
                            .notes
                            .push(format!("{name}: unverified dependencies: {issue}")),
                    }
                    pending.extend(
                        dependencies::conventional(&name, &bytes, &catalog)
                            .into_iter()
                            .map(|l| l.target),
                    );
                    plan.notes.extend(notes);
                }
                Err(issue) => plan
                    .notes
                    .push(format!("{name}: dependencies unverified: {issue}")),
            }
        }
    }
    if plan.items.is_empty() {
        return Err(invalid("No source resource"));
    }
    plan.notes.push(if include_dependencies{"Includes observed stored references and reviewed resource conventions; other runtime lookups remain external".into()}else{"Copies only the selected resource; its references remain shared or external".into()});
    Ok(plan)
}
fn rewrite(
    name: &str,
    bytes: &[u8],
    map: &BTreeMap<String, String>,
    catalog: &BTreeSet<String>,
) -> Result<Vec<u8>> {
    let refs = dependencies::references(name, bytes, catalog, &mut BTreeSet::new())?;
    let mut out = bytes.to_vec();
    let mut edits = Vec::new();
    for r in refs {
        let Some(new) = map.get(&r.target) else {
            continue;
        };
        match r.location {
            Location::Text(i) => edits.push((i, format!("\"{new}\""))),
            Location::Literal { at, len, stem_only } => {
                let text = if stem_only {
                    new.split('.').next().unwrap_or(new)
                } else {
                    new.as_str()
                };
                if text.len() > len {
                    return Err(format!(
                        "{name}: {new} exceeds a compiled filename slot ({len} bytes)"
                    ));
                }
                let slot = out
                    .get_mut(at..at + len + 1)
                    .ok_or("Filename slot outside resource")?;
                slot.fill(0);
                slot[..text.len()].copy_from_slice(text.as_bytes());
            }
        }
    }
    if !edits.is_empty() {
        out = Brf::parse(bytes, name.rsplit('.').next().unwrap_or(""))
            .and_then(|b| b.edit_many(bytes, &edits, name.rsplit('.').next().unwrap_or("")))?;
    }
    Ok(out)
}
pub fn rename(archive: &Archive, old: &str, new: &str, duplicate: bool) -> Result<Plan> {
    let old = old.to_ascii_uppercase();
    let new = new.to_ascii_uppercase();
    crate::archive::validate_name(&new)?;
    if archive.find(&new).is_some() {
        return Err(invalid("Destination name already exists"));
    }
    if old.rsplit('.').next() != new.rsplit('.').next() {
        return Err(invalid("Keep the resource type extension"));
    }
    let selected = archive.find(&old).ok_or("Resource missing")?;
    let mut catalog = BTreeSet::new();
    for e in &archive.entries {
        catalog.insert(e.name.clone());
    }
    // Implicit names cannot be fixed by patching a literal. Keep this case in
    // the aircraft-family clone workflow until whole-family renaming is proven.
    if !duplicate {
        if old.ends_with("_S.SH") {
            return Err(invalid(
                "Shadow names define the damage family; use New aircraft for a private family",
            ));
        }
        if old.ends_with(".PT")
            && catalog.contains(&format!("{}.PAL", old.split('.').next().unwrap()))
        {
            return Err(invalid(
                "Aircraft has a private palette binding; use New aircraft to preserve it",
            ));
        }
        let mut inspected = 0usize;
        for e in &archive.entries {
            if !matches!(
                e.name.rsplit('.').next(),
                Some("PT" | "JT" | "SEE" | "ECM" | "GAS")
            ) {
                continue;
            }
            if let Ok(bytes) = e.read() {
                inspected = inspected.saturating_add(bytes.len());
                if inspected > crate::archive::ARCHIVE_LIMIT {
                    return Err(invalid("Rename scan exceeds 128 MiB"));
                }
                for link in dependencies::conventional(&e.name, &bytes, &catalog) {
                    if link.target == old
                        || (e.name == old
                            && matches!(
                                link.evidence,
                                "Default HUD convention" | "Store-icon convention"
                            ))
                    {
                        return Err(format!(
                            "{} participates in {}; use New aircraft for a private family",
                            old, link.evidence
                        ));
                    }
                }
            }
        }
    }
    let mut map = BTreeMap::new();
    map.insert(old.clone(), new.clone());
    let mut plan = Plan::default();
    let mut renamed = archive.entries[selected].renamed(&new)?;
    if !dependencies::leaf(&old) {
        if let Ok(bytes) = archive.entries[selected].read() {
            if bytes.starts_with(b"MZ") || bytes.starts_with(b"[brent's_relocatable_format]") {
                let rewritten = rewrite(&old, &bytes, &map, &catalog)?;
                if rewritten != bytes {
                    renamed = Entry::new(&new, rewritten)?;
                }
            } else {
                plan.notes
                    .push(format!("{old}: opaque contents are unchanged"));
            }
        }
    }
    plan.items.push(Item {
        entry: renamed,
        previous: None,
        choice: Choice::TakeSource,
        conflict: false,
    });
    // The stored original follows its texture's name (and is copied for a duplicate).
    if let Some(org) = originals::backup(archive, &old) {
        let name = originals::companion(&new).ok_or("Stored original needs a PIC name")?;
        if archive.find(&name).is_some() {
            return Err(format!(
                "{name} already exists; the stored original of {old} cannot follow"
            ));
        }
        plan.items.push(Item {
            entry: org.renamed(&name)?,
            previous: None,
            choice: Choice::TakeSource,
            conflict: false,
        });
        if !duplicate {
            plan.removals.push(org.name.clone());
            plan.removed_before.push(org.clone());
        }
        plan.notes
            .push(format!("Stored original {} follows as {name}", org.name));
    }
    if !duplicate {
        plan.removals.push(old.clone());
        plan.removed_before.push(archive.entries[selected].clone());
        let mut inspected = 0usize;
        for e in &archive.entries {
            if e.name == old || dependencies::leaf(&e.name) {
                continue;
            }
            let bytes = match e.read() {
                Ok(b) => b,
                Err(issue) => {
                    plan.notes
                        .push(format!("{}: dependency scan unavailable: {issue}", e.name));
                    continue;
                }
            };
            inspected = inspected.saturating_add(bytes.len());
            if inspected > crate::archive::ARCHIVE_LIMIT {
                return Err(invalid("Rename scan exceeds 128 MiB"));
            }
            if bytes.starts_with(b"MZ") || bytes.starts_with(b"[brent's_relocatable_format]") {
                let out = rewrite(&e.name, &bytes, &map, &catalog)?;
                if out != bytes {
                    plan.items.push(Item {
                        entry: Entry::new(&e.name, out)?,
                        previous: Some(e.clone()),
                        choice: Choice::TakeSource,
                        conflict: false,
                    });
                }
            } else {
                plan.notes
                    .push(format!("{}: opaque dependency scan unavailable", e.name));
            }
        }
    }
    plan.notes.push(if duplicate{"Duplicate keeps shared resource references; New aircraft creates a private aircraft package".into()}else{"Only reviewed stored filename references are rewritten; game-generated names remain unverified".into()});
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dependencies::Index;
    fn source() -> Archive {
        let mut a = Archive::empty();
        a.entries = vec![
            Entry::new("DEMO.SH", crate::model::demo_textured()).unwrap(),
            Entry::new("DEMO.PIC", crate::picture::demo()).unwrap(),
        ];
        a
    }
    #[test]
    fn transfers_require_collision_choices_and_undo_as_one_transaction() {
        let source = source();
        let mut target = Archive::empty();
        target
            .entries
            .push(Entry::new("DEMO.PIC", vec![1, 2, 3]).unwrap());
        let original = target.bytes().unwrap();
        let mut doc = Document::new(target);
        let mut plan = transfer(&source, &doc.archive, "DEMO.SH", true).unwrap();
        assert_eq!(plan.items.len(), 2);
        assert!(!plan.ready());
        assert!(plan.apply(&mut doc).is_err());
        assert_eq!(doc.archive.bytes().unwrap(), original);
        let collision = plan.items.iter_mut().find(|i| i.conflict).unwrap();
        collision.choice = Choice::KeepTarget;
        plan.apply(&mut doc).unwrap();
        assert!(doc.archive.find("DEMO.SH").is_some());
        assert_eq!(
            doc.archive.entries[doc.archive.find("DEMO.PIC").unwrap()]
                .read()
                .unwrap(),
            vec![1, 2, 3]
        );
        assert!(doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), original);
        let mut plan = transfer(&source, &doc.archive, "DEMO.SH", true).unwrap();
        plan.items.iter_mut().find(|i| i.conflict).unwrap().choice = Choice::TakeSource;
        plan.apply(&mut doc).unwrap();
        assert_eq!(
            doc.archive.entries[doc.archive.find("DEMO.PIC").unwrap()].stored(),
            source.entries[1].stored()
        );
        doc.undo();
        assert_eq!(doc.archive.bytes().unwrap(), original);
    }
    #[test]
    fn renaming_rewrites_references_and_checks_stale_removals() {
        let mut doc = Document::new(source());
        let original = doc.archive.bytes().unwrap();
        let plan = rename(&doc.archive, "DEMO.PIC", "PAINT.PIC", false).unwrap();
        plan.apply(&mut doc).unwrap();
        assert!(doc.archive.find("DEMO.PIC").is_none());
        assert!(doc.archive.find("PAINT.PIC").is_some());
        let mut index = Index::default();
        index.update(&doc.archive);
        assert_eq!(index.incoming("PAINT.PIC").count(), 1);
        doc.undo();
        assert_eq!(doc.archive.bytes().unwrap(), original);
        let plan = rename(&doc.archive, "DEMO.PIC", "PAINT.PIC", false).unwrap();
        doc.replace(1, vec![1]).unwrap();
        assert!(plan.apply(&mut doc).is_err());
        let plan = rename(&doc.archive, "DEMO.PIC", "COPY.PIC", true).unwrap();
        plan.apply(&mut doc).unwrap();
        assert!(doc.archive.find("DEMO.PIC").is_some());
    }
    #[test]
    fn too_long_compiled_name_and_implicit_family_rename_fail_before_editing() {
        let mut a = source();
        let mut shape = a.entries[0].read().unwrap();
        shape.extend(b"\0DEMO.PIC\0");
        let size = (shape.len() - 256) as u32;
        shape[128..132].copy_from_slice(&size.to_le_bytes());
        shape[136..140].copy_from_slice(&size.to_le_bytes());
        a.entries[0] = Entry::new("DEMO.SH", shape).unwrap();
        let before = a.bytes().unwrap();
        assert!(rename(&a, "DEMO.PIC", "LONGNAME.PIC", false).is_err());
        assert_eq!(a.bytes().unwrap(), before);
        a.entries
            .push(Entry::new("DEMO.PT", crate::brf::demo()).unwrap());
        a.entries
            .push(Entry::new("DEMO_A.SH", crate::model::demo_shape()).unwrap());
        assert!(rename(&a, "DEMO_A.SH", "OTHER.SH", false).is_err());
    }
    #[test]
    fn dependencies_resolve_across_source_libraries_without_guessing_conflicting_providers() {
        let mut owner = source();
        let picture = owner.entries.pop().unwrap();
        let mut images = Archive::empty();
        images.entries.push(picture);
        let target = Archive::empty();
        let plan = transfer_from(&[&owner, &images], &target, "DEMO.SH", true).unwrap();
        assert_eq!(plan.items.len(), 2);
        let mut conflicting = images.clone();
        conflicting.entries[0] = Entry::new("DEMO.PIC", vec![1]).unwrap();
        assert!(transfer_from(&[&owner, &images, &conflicting], &target, "DEMO.SH", true).is_err());
        let plan =
            transfer_from(&[&owner, &images, &conflicting], &target, "DEMO.SH", false).unwrap();
        assert_eq!(plan.items.len(), 1);
    }
    fn with_original() -> Archive {
        let mut a = source();
        let org = a.entries[1].renamed("DEMO.ORG").unwrap();
        a.entries.push(org);
        a.entries[1] = Entry::new("DEMO.PIC", vec![9; 4]).unwrap();
        a
    }
    #[test]
    fn stored_originals_follow_rename_duplicate_delete_and_copy() {
        let mut doc = Document::new(with_original());
        let original = doc.archive.bytes().unwrap();
        let backup = doc.archive.entries[2].clone();
        let plan = rename(&doc.archive, "DEMO.PIC", "PAINT.PIC", false).unwrap();
        plan.apply(&mut doc).unwrap();
        assert!(doc.archive.find("DEMO.ORG").is_none());
        let moved = &doc.archive.entries[doc.archive.find("PAINT.ORG").unwrap()];
        assert!(moved.same_payload(&backup));
        assert!(doc.undo());
        assert_eq!(doc.archive.bytes().unwrap(), original);
        let plan = rename(&doc.archive, "DEMO.PIC", "COPY.PIC", true).unwrap();
        plan.apply(&mut doc).unwrap();
        assert!(doc.archive.find("DEMO.ORG").is_some());
        assert!(doc.archive.find("COPY.ORG").is_some());
        doc.undo();
        doc.transaction(vec![Entry::new("TAKEN.ORG", vec![1]).unwrap()], &[])
            .unwrap();
        assert!(rename(&doc.archive, "DEMO.PIC", "TAKEN.PIC", false).is_err());
        doc.undo();
        let removals = originals::removals(&doc.archive, "DEMO.PIC");
        doc.transaction(Vec::new(), &removals).unwrap();
        assert!(doc.archive.find("DEMO.ORG").is_none());
        doc.undo();
        assert_eq!(doc.archive.bytes().unwrap(), original);
        // Copies carry the original even without dependencies; a kept target PIC keeps its own.
        let mut target = Document::new(Archive::empty());
        let plan = transfer(&doc.archive, &target.archive, "DEMO.PIC", false).unwrap();
        assert_eq!(plan.items.len(), 2);
        plan.apply(&mut target).unwrap();
        assert!(target.archive.find("DEMO.ORG").is_some());
        let mut target = Archive::empty();
        target
            .entries
            .push(Entry::new("DEMO.PIC", vec![1]).unwrap());
        let mut target = Document::new(target);
        let mut plan = transfer(&doc.archive, &target.archive, "DEMO.SH", true).unwrap();
        plan.items.iter_mut().find(|i| i.conflict).unwrap().choice = Choice::KeepTarget;
        assert!(plan.ready());
        plan.apply(&mut target).unwrap();
        assert!(target.archive.find("DEMO.ORG").is_none());
        // An identical target PIC still receives the original; a move keeps the pair together.
        let mut source = doc.clone();
        let mut target = Archive::empty();
        target
            .entries
            .push(Entry::new("DEMO.PIC", vec![9; 4]).unwrap());
        let mut target = Document::new(target);
        let plan = transfer(&source.archive, &target.archive, "DEMO.PIC", false).unwrap();
        assert!(plan.items.iter().all(|i| !plan.follows_kept(i)));
        move_between(&mut source, &mut target, &plan, "DEMO.PIC").unwrap();
        assert!(target.archive.find("DEMO.ORG").is_some());
        assert!(source.archive.find("DEMO.ORG").is_none());
        assert!(source.archive.find("DEMO.PIC").is_none());
    }
    #[test]
    fn moving_a_shared_texture_keeps_its_original_in_the_source() {
        let mut a = with_original();
        a.entries.push(
            Entry::new(
                "ONE.OT",
                b"[brent's_relocatable_format]\nstring \"DEMO.SH\"\nend\n".to_vec(),
            )
            .unwrap(),
        );
        a.entries.push(
            Entry::new(
                "TWO.OT",
                b"[brent's_relocatable_format]\nstring \"DEMO.SH\"\nend\n".to_vec(),
            )
            .unwrap(),
        );
        let mut source = Document::new(a);
        let mut target = Document::new(Archive::empty());
        let plan = transfer(&source.archive, &target.archive, "ONE.OT", true).unwrap();
        assert!(plan.items.iter().any(|i| i.entry.name == "DEMO.ORG"));
        move_between(&mut source, &mut target, &plan, "ONE.OT").unwrap();
        assert!(source.archive.find("DEMO.PIC").is_some());
        assert!(source.archive.find("DEMO.ORG").is_some());
        assert!(target.archive.find("DEMO.ORG").is_some());
        let mut source = Document::new(with_original());
        let mut target = Document::new(Archive::empty());
        let plan = transfer(&source.archive, &target.archive, "DEMO.PIC", false).unwrap();
        move_between(&mut source, &mut target, &plan, "DEMO.PIC").unwrap();
        assert!(source.archive.find("DEMO.ORG").is_none());
        assert!(target.archive.find("DEMO.ORG").is_some());
    }
    #[test]
    fn transaction_failure_is_atomic() {
        let mut doc = Document::new(source());
        let before = doc.archive.bytes().unwrap();
        let e = Entry::new("NEW.PIC", vec![1]).unwrap();
        assert!(doc
            .transaction(vec![e.clone(), e], &["DEMO.SH".into()])
            .is_err());
        assert!(!doc.dirty());
        assert_eq!(doc.archive.bytes().unwrap(), before);
    }
}

#[cfg(test)]
mod move_tests {
    use super::*;
    fn fixture(shared: bool) -> Archive {
        let mut a = Archive::empty();
        a.entries.push(
            Entry::new(
                "ONE.OT",
                b"[brent's_relocatable_format]\nstring \"DEMO.SH\"\nend\n".to_vec(),
            )
            .unwrap(),
        );
        a.entries
            .push(Entry::new("DEMO.SH", crate::model::demo_textured()).unwrap());
        a.entries
            .push(Entry::new("DEMO.PIC", crate::picture::demo()).unwrap());
        if shared {
            a.entries.push(
                Entry::new(
                    "OTHER.OT",
                    b"[brent's_relocatable_format]\nstring \"DEMO.SH\"\nend\n".to_vec(),
                )
                .unwrap(),
            );
        }
        a
    }
    #[test]
    fn move_keeps_shared_dependency_closure_and_both_histories() {
        let mut source = Document::new(fixture(true));
        let original = source.archive.bytes().unwrap();
        let mut target = Document::new(Archive::empty());
        let plan = transfer(&source.archive, &target.archive, "ONE.OT", true).unwrap();
        assert_eq!(
            move_between(&mut source, &mut target, &plan, "ONE.OT").unwrap(),
            1
        );
        assert!(source.archive.find("ONE.OT").is_none());
        assert!(source.archive.find("DEMO.SH").is_some());
        assert!(source.archive.find("DEMO.PIC").is_some());
        assert_eq!(target.archive.entries.len(), 3);
        assert!(source.undo());
        assert!(target.undo());
        assert_eq!(source.archive.bytes().unwrap(), original);
        assert!(target.archive.entries.is_empty());
        let mut source = Document::new(fixture(false));
        let plan = transfer(&source.archive, &target.archive, "ONE.OT", true).unwrap();
        assert_eq!(
            move_between(&mut source, &mut target, &plan, "ONE.OT").unwrap(),
            3
        );
        assert!(source.archive.entries.is_empty());
    }
    #[test]
    fn move_is_atomic_on_stale_source_target_and_skipped_root() {
        let mut source = Document::new(fixture(false));
        let mut target = Document::new(Archive::empty());
        let plan = transfer(&source.archive, &target.archive, "ONE.OT", false).unwrap();
        target
            .transaction(
                vec![Entry::new("ONE.OT", b"changed".to_vec()).unwrap()],
                &[],
            )
            .unwrap();
        let before = source.archive.bytes().unwrap();
        let target_before = target.archive.bytes().unwrap();
        assert!(move_between(&mut source, &mut target, &plan, "ONE.OT").is_err());
        assert_eq!(source.archive.bytes().unwrap(), before);
        assert_eq!(target.archive.bytes().unwrap(), target_before);
        let mut plan = transfer(&source.archive, &target.archive, "ONE.OT", false).unwrap();
        plan.items[0].choice = Choice::KeepTarget;
        assert!(move_between(&mut source, &mut target, &plan, "ONE.OT").is_err());
        plan.items[0].choice = Choice::TakeSource;
        source.replace(0, b"new source".to_vec()).unwrap();
        assert!(move_between(&mut source, &mut target, &plan, "ONE.OT").is_err());
        assert_eq!(target.archive.bytes().unwrap(), target_before);
    }
}
