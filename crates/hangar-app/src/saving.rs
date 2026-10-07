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

/// FA's loader view of the folder `path` saves into, after saving a LIB of
/// `entries` entries there; `None` when it is not a game folder (no FA.EXE
/// and no retail LIB name) or cannot be listed. Only names and the first
/// seven bytes of `.LIB`-named files are read.
pub fn game_folder(path: &str, entries: usize) -> Option<hangar_core::save::GameFolder> {
    use hangar_core::save::{self, FolderFile};
    let (folder, leaf) = match path.rfind(['/', '\\']) {
        Some(at) => (
            String::from(if at == 0 { &path[..1] } else { &path[..at] }),
            &path[at + 1..],
        ),
        None => (crate::platform::current_dir(), path),
    };
    let items = crate::platform::list_dir(&folder).ok()?;
    let names: Vec<&str> = items
        .iter()
        .filter(|i| !i.directory)
        .map(|i| i.name.as_str())
        .collect();
    if !save::is_game_folder(&names) {
        return None;
    }
    let files: Vec<FolderFile> = items
        .iter()
        .filter(|i| !i.directory)
        .map(|i| FolderFile {
            name: i.name.clone(),
            lib_entries: save::loads_as_lib(&i.name)
                .then(|| crate::platform::read_range(&i.path, 0, 7).ok())
                .flatten()
                .filter(|h| h.starts_with(b"EALIB"))
                .map(|h| u16::from_le_bytes([h[5], h[6]]) as usize),
        })
        .collect();
    Some(save::game_folder(&files, leaf, entries))
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
