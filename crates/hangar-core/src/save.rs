//! Filename policy and recoverable LIB replacement, shared by GUI and CLI.
use crate::Result;
use alloc::{format, string::String};

/// Retail SETUP.ESA plus both discs, including bundled demo/installer LIBs.
/// SWPATCH.LIB is a toolkit/mod library, not a retail archive.
pub const PROTECTED_LIBS: &[&str] = &[
    "FA_1.LIB",
    "FA_2.LIB",
    "FA_3.LIB",
    "FA_4B.LIB",
    "FA_4C.LIB",
    "FA_4D.LIB",
    "FA_7.LIB",
    "FA_10.LIB",
    "FA_10B.LIB",
    "FA_11.LIB",
    "FA_11B.LIB",
    "LHX0.LIB",
    "LHX1.LIB",
    "LHX2.LIB",
    "LHX3.LIB",
    "LHX4.LIB",
    "_SETUP.LIB",
];

pub fn protected_name(path: &str) -> Option<&'static str> {
    // Recognize drive-relative paths, Win32 trailing dots/spaces and streams
    // even on Linux, so their reserved names receive the same diagnostic.
    let path = if path.as_bytes().get(1) == Some(&b':') {
        &path[2..]
    } else {
        path
    };
    let leaf = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let leaf = leaf
        .split(':')
        .next()
        .unwrap_or(leaf)
        .trim_end_matches(['.', ' ']);
    PROTECTED_LIBS
        .iter()
        .copied()
        .find(|name| leaf.eq_ignore_ascii_case(name))
}

pub fn guard_output(path: &str) -> Result<()> {
    if let Some(name) = protected_name(path) {
        return Err(format!("{name} is a protected retail LIB. Open/extract is allowed; save with a different LIB name."));
    }
    Ok(())
}

pub fn validate_destination(path: &str) -> Result<()> {
    guard_output(path)?;
    let rest = if path.as_bytes().get(1) == Some(&b':') && path.as_bytes()[0].is_ascii_alphabetic()
    {
        &path[2..]
    } else {
        path
    };
    let leaf = rest.rsplit(['/', '\\']).next().unwrap_or(rest);
    let stem = leaf
        .split('.')
        .next()
        .unwrap_or(leaf)
        .trim_end()
        .to_ascii_uppercase();
    let device = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ((stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.len() == 4
        && matches!(stem.as_bytes()[3], b'1'..=b'9'));
    if !path.is_ascii()
        || device
        || rest.bytes().any(|b| b < 32 || b"<>:\"|?*".contains(&b))
        || leaf.ends_with(['.', ' '])
        || !leaf.to_ascii_uppercase().ends_with(".LIB")
        || leaf.len() <= 4
    {
        return Err("Choose an ASCII .LIB filename without trailing dots/spaces or special Windows path syntax".into());
    }
    if leaf[..leaf.len() - 4].to_ascii_uppercase().contains(".LIB") {
        return Err("Choose a LIB name with .LIB only at its end: FA loads every file whose name contains .LIB".into());
    }
    Ok(())
}

/// All moves MUST fail if the destination already exists. A failed write must
/// clean up only its own partial file. Never truncate an existing file.
pub trait Storage {
    fn exists(&mut self, path: &str) -> Result<bool>;
    fn write_new(&mut self, path: &str, bytes: &[u8]) -> Result<()>;
    fn move_new(&mut self, from: &str, to: &str) -> Result<()>;
    fn remove(&mut self, path: &str) -> Result<()>;
}

/// Most numbered backups (`.B01` to `.B99`) or stages beside one LIB.
pub const NUMBERED: usize = 99;
/// Companion names for `path` (which ends in `.LIB`): `<STEM>.BAK` then
/// `<STEM>.B01` .. `<STEM>.B99` for backups, `<STEM>.TMP` then `.T01` ..
/// for the stage. They never contain ".LIB": FA.EXE treats every file whose
/// upper-cased name contains ".LIB" and that starts with EALIB as another
/// LIB, so the old `X.LIB.bak` backups were loaded as duplicate LIBs.
pub fn companion(path: &str, letter: char, n: usize) -> String {
    let stem = &path[..path.len().saturating_sub(4)];
    match (letter, n) {
        ('B', 0) => format!("{stem}.BAK"),
        ('T', 0) => format!("{stem}.TMP"),
        _ => format!("{stem}.{letter}{n:02}"),
    }
}
fn unused(fs: &mut impl Storage, path: &str, letter: char) -> Result<String> {
    for i in 0..=NUMBERED {
        let candidate = companion(path, letter, i);
        if !fs.exists(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(format!(
        "No unused {} name beside {path}; move old {} files away",
        if letter == 'B' { "backup" } else { "staging" },
        if letter == 'B' {
            ".BAK/.B01-.B99"
        } else {
            ".TMP/.T01-.T99"
        }
    ))
}

/// Stage and flush first, preserve the old file under a fresh backup name, then
/// install without clobbering. On install failure restore the old name if free.
/// Abrupt shutdown between moves can leave the old LIB at the backup path.
pub fn library(fs: &mut impl Storage, path: &str, bytes: &[u8]) -> Result<Option<String>> {
    validate_destination(path)?;
    let backup = if fs.exists(path)? {
        Some(unused(fs, path, 'B')?)
    } else {
        None
    };
    let stage = unused(fs, path, 'T')?;
    fs.write_new(&stage, bytes)?;
    if let Some(old) = &backup {
        if let Err(e) = fs.move_new(path, old) {
            let _ = fs.remove(&stage);
            return Err(format!("Cannot preserve previous LIB; save stopped: {e}"));
        }
    }
    if let Err(e) = fs.move_new(&stage, path) {
        if let Some(old) = &backup {
            if let Err(restore) = fs.move_new(old, path) {
                return Err(format!("Save failed: {e}. Previous LIB: {old}; new LIB: {stage}. Restore failed: {restore}"));
            }
        }
        let _ = fs.remove(&stage);
        return Err(format!("Save failed; previous LIB preserved: {e}"));
    }
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::BTreeMap, vec::Vec};
    #[derive(Default)]
    struct Disk {
        files: BTreeMap<String, Vec<u8>>,
        moves: usize,
        fail_moves: Vec<usize>,
        fail_write: bool,
        arrivals: BTreeMap<usize, (String, Vec<u8>)>,
    }
    impl Storage for Disk {
        fn exists(&mut self, p: &str) -> Result<bool> {
            Ok(self.files.contains_key(p))
        }
        fn write_new(&mut self, p: &str, b: &[u8]) -> Result<()> {
            if self.fail_write || self.files.contains_key(p) {
                return Err("write failed".into());
            }
            self.files.insert(p.into(), b.into());
            Ok(())
        }
        fn move_new(&mut self, a: &str, b: &str) -> Result<()> {
            self.moves += 1;
            if let Some((name, bytes)) = self.arrivals.remove(&self.moves) {
                self.files.insert(name, bytes);
            }
            if self.fail_moves.contains(&self.moves) || self.files.contains_key(b) {
                return Err("move failed".into());
            }
            let bytes = self.files.remove(a).ok_or("missing source")?;
            self.files.insert(b.into(), bytes);
            Ok(())
        }
        fn remove(&mut self, p: &str) -> Result<()> {
            self.files.remove(p);
            Ok(())
        }
    }
    fn disk() -> Disk {
        let mut d = Disk::default();
        d.files.insert("MOD.LIB".into(), b"old".to_vec());
        d
    }
    #[test]
    fn reserved_names_and_windows_aliases() {
        for name in PROTECTED_LIBS {
            for p in [
                String::from(*name),
                format!("C:\\Games\\{}", name.to_ascii_lowercase()),
                format!("/tmp/{name}"),
                format!("C:{name}. "),
                format!("{name}:stream"),
            ] {
                let mut d = Disk::default();
                assert!(library(&mut d, &p, b"new")
                    .unwrap_err()
                    .contains("protected retail LIB"));
                assert!(d.files.is_empty());
            }
        }
        for p in [
            "FA_2_MOD.LIB",
            "swpatch.lib",
            "MOD.LIB",
            "C:\\FA_1.LIB\\CUSTOM.LIB",
        ] {
            assert!(validate_destination(p).is_ok(), "{p}");
        }
        for p in [
            "MOD.LIB.",
            "MOD.LIB ",
            "MOD.LIB:stream",
            "\\\\?\\C:\\MOD.LIB",
            "NUL",
            "MOD.TXT",
            "M\0.LIB",
            "NUL.LIB",
            "COM1.LIB",
            "LPT9.LIB",
        ] {
            assert!(validate_destination(p).is_err(), "{p}");
        }
    }
    #[test]
    fn repeated_save_preserves_every_backup_and_existing_stage() {
        let mut d = disk();
        d.files.insert("MOD.TMP".into(), b"unrelated".to_vec());
        assert_eq!(
            library(&mut d, "MOD.LIB", b"new").unwrap().as_deref(),
            Some("MOD.BAK")
        );
        assert_eq!(
            library(&mut d, "MOD.LIB", b"third").unwrap().as_deref(),
            Some("MOD.B01")
        );
        assert_eq!(d.files["MOD.BAK"], b"old");
        assert_eq!(d.files["MOD.B01"], b"new");
        assert_eq!(d.files["MOD.LIB"], b"third");
        assert_eq!(d.files["MOD.TMP"], b"unrelated");
        assert!(!d.files.contains_key("MOD.T01"));
        // No companion name contains .LIB in any case.
        assert!(d
            .files
            .keys()
            .all(|n| n == "MOD.LIB" || !n.to_ascii_uppercase().contains(".LIB")));
    }
    #[test]
    fn failures_keep_old_lib_and_report_recovery_paths() {
        for step in 0..3 {
            let mut d = disk();
            if step == 0 {
                d.fail_write = true;
            } else {
                d.fail_moves.push(step);
            }
            assert!(library(&mut d, "MOD.LIB", b"new").is_err());
            assert_eq!(d.files.len(), 1);
            assert_eq!(d.files["MOD.LIB"], b"old");
        }
        let mut d = disk();
        d.fail_moves = vec![2, 3];
        let e = library(&mut d, "MOD.LIB", b"new").unwrap_err();
        assert!(e.contains("MOD.BAK") && e.contains("MOD.TMP"));
        assert_eq!(d.files["MOD.BAK"], b"old");
        assert_eq!(d.files["MOD.TMP"], b"new");
    }
    #[test]
    fn new_lib_needs_no_backup() {
        let mut d = Disk::default();
        assert_eq!(library(&mut d, "MOD.LIB", b"new").unwrap(), None);
        assert_eq!(d.files.len(), 1);
        assert_eq!(d.files["MOD.LIB"], b"new");
    }
    #[test]
    fn files_appearing_during_save_are_never_clobbered() {
        let mut d = disk();
        d.arrivals
            .insert(1, ("MOD.BAK".into(), b"other backup".to_vec()));
        assert!(library(&mut d, "MOD.LIB", b"new").is_err());
        assert_eq!(d.files["MOD.LIB"], b"old");
        assert_eq!(d.files["MOD.BAK"], b"other backup");
        let mut d = disk();
        d.arrivals
            .insert(2, ("MOD.LIB".into(), b"other save".to_vec()));
        assert!(library(&mut d, "MOD.LIB", b"new").is_err());
        assert_eq!(d.files["MOD.LIB"], b"other save");
        assert_eq!(d.files["MOD.BAK"], b"old");
        assert_eq!(d.files["MOD.TMP"], b"new");
    }
    #[test]
    fn backups_are_numbered_8_3_names_without_lib() {
        assert_eq!(
            companion("C:\\FA\\TopGun.LIB", 'B', 0),
            "C:\\FA\\TopGun.BAK"
        );
        assert_eq!(companion("mod.lib", 'B', 7), "mod.B07");
        assert_eq!(companion("MOD.LIB", 'T', 99), "MOD.T99");
        let mut d = disk();
        d.files.insert("MOD.BAK".into(), Vec::new());
        for n in 1..=NUMBERED {
            d.files.insert(companion("MOD.LIB", 'B', n), Vec::new());
        }
        let e = library(&mut d, "MOD.LIB", b"new").unwrap_err();
        assert!(e.contains("No unused backup name"), "{e}");
        assert_eq!(d.files["MOD.LIB"], b"old");
        for p in ["X.LIB.LIB", "C:\\FA\\a.lib.LIB", "B.Lib.lib"] {
            assert!(validate_destination(p)
                .unwrap_err()
                .contains(".LIB only at its end"));
        }
    }
}
