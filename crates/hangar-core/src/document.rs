//! Entry-granular history shares original archive storage; no whole-LIB undo copies.
use crate::{
    archive::{Archive, Entry},
    invalid, Result,
};
use alloc::{string::String, vec::Vec};
#[derive(Clone)]
struct Change {
    at: usize,
    before: Option<Entry>,
    after: Option<Entry>,
}
pub struct Document {
    pub archive: Archive,
    undo: Vec<Change>,
    redo: Vec<Change>,
    revision: u64,
    saved_revision: Option<u64>,
}
impl Document {
    pub fn new(archive: Archive) -> Self {
        Self {
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
    }
    pub fn mark_saved(&mut self) {
        self.saved_revision = Some(self.revision);
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
        if !self.redo.is_empty() && self.saved_revision.is_some_and(|r| r > self.revision) {
            self.saved_revision = None;
        }
        self.apply(c.clone());
        self.undo.push(c);
        self.redo.clear();
        self.revision += 1;
        // Cap retained changed payloads, while retaining at least the last operation.
        while self.undo.len() > 1
            && (self.undo.len() > 64
                || self
                    .undo
                    .iter()
                    .map(|c| c.after.as_ref().map_or(0, Entry::stored_len))
                    .sum::<usize>()
                    > 32 * 1024 * 1024)
        {
            self.undo.remove(0);
        }
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
    pub fn undo(&mut self) -> bool {
        if let Some(c) = self.undo.pop() {
            self.apply(Change {
                at: c.at,
                before: c.after.clone(),
                after: c.before.clone(),
            });
            self.redo.push(c);
            self.revision -= 1;
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(c) = self.redo.pop() {
            self.apply(c.clone());
            self.undo.push(c);
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
        d.replace(0, vec![2]).unwrap();
        assert!(d.dirty());
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
