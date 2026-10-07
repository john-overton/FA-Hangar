use alloc::{string::String, vec::Vec};
use hangar_core::{archive::Archive, save::Storage, Result};

struct Files;
impl Storage for Files {
    fn exists(&mut self, path: &str) -> Result<bool> {
        crate::platform::save_exists(path)
    }
    fn write_new(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
        crate::platform::write_new(path, bytes)
    }
    fn move_new(&mut self, from: &str, to: &str) -> Result<()> {
        crate::platform::move_new(from, to)
    }
    fn remove(&mut self, path: &str) -> Result<()> {
        crate::platform::remove_file(path)
    }
}

pub fn library(path: &str, bytes: &[u8]) -> Result<Option<String>> {
    hangar_core::save::validate_destination(path)?;
    Archive::parse(Vec::from(bytes))?;
    hangar_core::save::library(&mut Files, path, bytes)
}

/// Runs on both Windows CI architectures as well as Linux. Only synthetic data.
pub fn smoke() {
    let name = "HGSAVE.LIB";
    // Refuse to touch a pre-existing scratch path, including prior backups.
    for p in [name, "HGSAVE.BAK", "HGSAVE.B01", "HGSAVE.TMP"] {
        assert!(
            !crate::platform::save_exists(p).unwrap(),
            "Save smoke scratch path exists"
        );
    }
    let mut a = Archive::empty();
    a.entries
        .push(hangar_core::archive::Entry::new("OLD.TXT", b"old".to_vec()).unwrap());
    let before = a.bytes().unwrap();
    a.entries
        .push(hangar_core::archive::Entry::new("TEST.TXT", b"new".to_vec()).unwrap());
    a.changed();
    let after = a.bytes().unwrap();
    assert_eq!(library(name, &before).unwrap(), None);
    let backup = library(name, &after).unwrap().unwrap();
    assert_eq!(backup, "HGSAVE.BAK");
    assert_eq!(crate::platform::read(&backup).unwrap(), before);
    assert_eq!(crate::platform::read(name).unwrap(), after);
    let backup2 = library(name, &before).unwrap().unwrap();
    assert_eq!(backup2, "HGSAVE.B01");
    assert_eq!(crate::platform::read(&backup2).unwrap(), after);
    assert_eq!(crate::platform::read(&backup).unwrap(), before);
    assert_eq!(crate::platform::read(name).unwrap(), before);
    assert!(library("fa_2.lib", &after)
        .unwrap_err()
        .contains("protected retail LIB"));
    for p in [name, backup.as_str(), backup2.as_str()] {
        crate::platform::remove_file(p).unwrap();
    }
}
