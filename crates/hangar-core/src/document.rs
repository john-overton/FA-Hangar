//! Entry-granular history shares original archive storage; no whole-LIB undo copies.
use crate::{
    archive::{Archive, Entry},
    invalid, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};
#[derive(Clone)]
struct Change {
    at: usize,
    before: Option<Entry>,
    after: Option<Entry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Removed,
}
impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Modified => "Modified",
            Self::Removed => "Removed",
        }
    }
}
pub struct Document {
    pub archive: Archive,
    undo: Vec<Vec<Change>>,
    redo: Vec<Vec<Change>>,
    revision: u64,
    saved_revision: Option<u64>,
    saved: BTreeMap<String, Entry>,
}
impl Document {
    pub fn new(archive: Archive) -> Self {
        let mut saved = BTreeMap::new();
        for e in &archive.entries {
            saved.insert(e.name.clone(), e.clone());
        }
        Self {
            saved,
            archive,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            saved_revision: Some(0),
        }
    }
    pub fn dirty(&self) -> bool {
        self.saved_revision != Some(self.revision)
    }
    pub fn mark_unsaved(&mut self) {
        self.saved_revision = None;
        self.saved.clear();
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = Some(self.revision);
        self.saved.clear();
        for e in &self.archive.entries {
            self.saved.insert(e.name.clone(), e.clone());
        }
    }
    pub fn saved_entry(&self, name: &str) -> Option<&Entry> {
        self.saved.get(name)
    }
    pub fn entry_changed(&self, entry: &Entry) -> bool {
        self.saved
            .get(&entry.name)
            .is_none_or(|old| !old.same_storage(entry))
    }
    pub fn changed_count(&self) -> usize {
        let mut names = BTreeSet::new();
        for e in &self.archive.entries {
            names.insert(e.name.as_str());
        }
        self.archive
            .entries
            .iter()
            .filter(|e| self.entry_changed(e))
            .count()
            + self
                .saved
                .keys()
                .filter(|name| !names.contains(name.as_str()))
                .count()
    }
    /// Includes deleted entries, which no longer appear in the archive directory.
    pub fn changes(&self) -> Vec<(String, ChangeKind)> {
        let mut changes = BTreeMap::new();
        let mut current = BTreeSet::new();
        for e in &self.archive.entries {
            current.insert(&e.name);
        }
        for e in &self.archive.entries {
            if self.entry_changed(e) {
                changes.insert(
                    e.name.clone(),
                    if self.saved.contains_key(&e.name) {
                        ChangeKind::Modified
                    } else {
                        ChangeKind::Added
                    },
                );
            }
        }
        for name in self.saved.keys().filter(|n| !current.contains(n)) {
            changes.insert(name.clone(), ChangeKind::Removed);
        }
        changes.into_iter().collect()
    }
    fn apply(&mut self, change: Change) {
        self.archive.changed();
        match (&change.before, &change.after) {
            (Some(_), Some(e)) => self.archive.entries[change.at] = e.clone(),
            (None, Some(e)) => self.archive.entries.insert(change.at, e.clone()),
            (Some(_), None) => {
                self.archive.entries.remove(change.at);
            }
            _ => {}
        }
    }
    fn change(&mut self, c: Change) {
        self.change_batch(vec![c]);
    }
    fn change_batch(&mut self, changes: Vec<Change>) {
        if !self.redo.is_empty() && self.saved_revision.is_some_and(|r| r > self.revision) {
            self.saved_revision = None;
        }
        for c in &changes {
            self.apply(c.clone());
        }
        self.undo.push(changes);
        self.redo.clear();
        self.revision += 1;
        // Cap retained changed payloads, while retaining at least the last operation.
        while self.undo.len() > 1
            && (self.undo.len() > 64
                || self
                    .undo
                    .iter()
                    .flatten()
                    .map(|c| c.after.as_ref().map_or(0, Entry::stored_len))
                    .sum::<usize>()
                    > 32 * 1024 * 1024)
        {
            self.undo.remove(0);
        }
    }
    /// Preflight the complete transaction against a shared-buffer draft. Failed
    /// validation leaves both bytes and history untouched.
    pub fn transaction(&mut self, entries: Vec<Entry>, removals: &[String]) -> Result<()> {
        let mut draft = self.archive.clone();
        let mut changes = Vec::new();
        let mut seen = BTreeSet::new();
        for name in removals {
            if !seen.insert(name.to_ascii_uppercase()) {
                return Err(invalid("Duplicate removal"));
            }
            let at = draft
                .find(name)
                .ok_or("Resource to remove no longer exists")?;
            let old = draft.entries.remove(at);
            changes.push(Change {
                at,
                before: Some(old),
                after: None,
            });
        }
        seen.clear();
        for entry in entries {
            crate::archive::validate_name(&entry.name)?;
            if !seen.insert(entry.name.clone()) {
                return Err(invalid("Duplicate transaction entry"));
            }
            if let Some(at) = draft.find(&entry.name) {
                if draft.entries[at].same_storage(&entry) {
                    continue;
                }
                let old = core::mem::replace(&mut draft.entries[at], entry.clone());
                changes.push(Change {
                    at,
                    before: Some(old),
                    after: Some(entry),
                });
            } else {
                let at = draft.entries.len();
                draft.entries.push(entry.clone());
                changes.push(Change {
                    at,
                    before: None,
                    after: Some(entry),
                });
            }
        }
        if changes.is_empty() {
            return Ok(());
        }
        draft.changed();
        draft.bytes()?;
        self.change_batch(changes);
        Ok(())
    }
    pub fn replace(&mut self, at: usize, bytes: Vec<u8>) -> Result<()> {
        let old = self
            .archive
            .entries
            .get(at)
            .ok_or_else(|| invalid("No selected entry"))?;
        if old.read().ok().as_deref() == Some(bytes.as_slice()) {
            return Ok(());
        }
        let new = Entry::new(&old.name, bytes)?;
        self.change(Change {
            at,
            before: Some(old.clone()),
            after: Some(new),
        });
        Ok(())
    }
    pub fn import(&mut self, name: &str, bytes: Vec<u8>) -> Result<()> {
        if self.archive.find(name).is_some() {
            return Err(invalid("Name already exists; select Replace entry"));
        }
        let new = Entry::new(name, bytes)?;
        self.change(Change {
            at: self.archive.entries.len(),
            before: None,
            after: Some(new),
        });
        Ok(())
    }
    pub fn remove(&mut self, at: usize) -> Result<()> {
        let old = self
            .archive
            .entries
            .get(at)
            .ok_or_else(|| invalid("No selected entry"))?
            .clone();
        self.change(Change {
            at,
            before: Some(old),
            after: None,
        });
        Ok(())
    }
    pub fn clone_texture(
        &mut self,
        name: &str,
        pixels: Vec<u8>,
        shape_index: usize,
        shape: Vec<u8>,
    ) -> Result<()> {
        if self.archive.find(name).is_some() {
            return Err(invalid("Texture name already exists"));
        }
        let old = self
            .archive
            .entries
            .get(shape_index)
            .ok_or_else(|| invalid("Shape missing"))?
            .clone();
        let new_shape = Entry::new(&old.name, shape)?;
        let new_texture = Entry::new(name, pixels)?;
        self.change_batch(vec![
            Change {
                at: shape_index,
                before: Some(old),
                after: Some(new_shape),
            },
            Change {
                at: self.archive.entries.len(),
                before: None,
                after: Some(new_texture),
            },
        ]);
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(changes) = self.undo.pop() {
            for c in changes.iter().rev() {
                self.apply(Change {
                    at: c.at,
                    before: c.after.clone(),
                    after: c.before.clone(),
                });
            }
            self.redo.push(changes);
            self.revision -= 1;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(changes) = self.redo.pop() {
            for c in &changes {
                self.apply(c.clone());
            }
            self.undo.push(changes);
            self.revision += 1;
            true
        } else {
            false
        }
    }
    pub fn summary(&self) -> String {
        format!(
            "{} entries | {}",
            self.archive.entries.len(),
            if self.dirty() { "Modified" } else { "Saved" }
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_save_branch_and_entry_operations() {
        let mut d = Document::new(Archive::empty());
        d.import("A.PT", vec![1]).unwrap();
        d.mark_saved();
        assert_eq!(d.changed_count(), 0);
        d.replace(0, vec![2]).unwrap();
        assert!(d.dirty());
        assert_eq!(d.changed_count(), 1);
        assert_eq!(d.saved_entry("A.PT").unwrap().read().unwrap(), vec![1]);
        d.undo();
        assert!(!d.dirty());
        d.redo();
        assert!(d.dirty());
        d.mark_saved();
        d.undo();
        d.replace(0, vec![3]).unwrap();
        assert!(d.dirty());
        assert!(!d.redo());
        d.remove(0).unwrap();
        assert!(d.archive.entries.is_empty());
        d.undo();
        assert_eq!(d.archive.entries[0].read().unwrap(), vec![3]);
    }
}
#[cfg(test)]
mod texture_tests {
    use super::*;
    #[test]
    fn cloned_texture_and_reference_are_one_undo_step() {
        let mut archive = Archive::empty();
        archive
            .entries
            .push(Entry::new("DEMO.SH", crate::model::demo_textured()).unwrap());
        archive
            .entries
            .push(Entry::new("DEMO.PIC", crate::picture::demo()).unwrap());
        let before = archive.bytes().unwrap();
        let original = archive.entries[1].read().unwrap();
        let (shape, count) = crate::model::Model::retarget_texture(
            &archive.entries[0].read().unwrap(),
            "DEMO.PIC",
            "NEW.PIC",
        )
        .unwrap();
        assert_eq!(count, 1);
        let mut d = Document::new(archive);
        d.clone_texture("NEW.PIC", original.clone(), 0, shape)
            .unwrap();
        assert_eq!(d.archive.entries.len(), 3);
        assert!(
            crate::model::Model::parse(&d.archive.entries[0].read().unwrap())
                .unwrap()
                .textures
                .contains("NEW.PIC")
        );
        assert_eq!(d.archive.entries[1].read().unwrap(), original);
        assert!(d.undo());
        assert!(!d.dirty());
        assert_eq!(d.archive.bytes().unwrap(), before);
        assert!(d.redo());
        assert_eq!(d.archive.entries.len(), 3);
    }
}
