//! Windows resources for `tore-hangar.exe`: the app icon (RT_GROUP_ICON 1 and
//! its RT_ICON images, copied from the committed .ico) and a VERSIONINFO.
//! Written as a .res file into OUT_DIR and handed to the linker; lld-link and
//! link.exe both convert .res inputs themselves. Other targets get nothing.
use std::{env, fs, path::Path};

const ICO: &str = "../../tore-hangar-design/icons/app/tore-hangar.ico";
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
const LANG_EN_US: u16 = 0x0409;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={ICO}");
    let var = |name: &str| env::var(name).unwrap_or_default();
    if var("CARGO_CFG_TARGET_OS") != "windows" || var("CARGO_CFG_TARGET_ENV") != "msvc" {
        return;
    }
    let ico_path = Path::new(&var("CARGO_MANIFEST_DIR")).join(ICO);
    let ico = fs::read(&ico_path).unwrap_or_else(|e| panic!("{}: {e}", ico_path.display()));
    let mut res = Vec::new();
    resource(&mut res, 0, 0, 0, 0, &[]); // the empty entry every .res starts with
    icon_resources(&mut res, &ico).unwrap_or_else(|e| panic!("{}: {e}", ico_path.display()));
    let version = [
        "CARGO_PKG_VERSION_MAJOR",
        "CARGO_PKG_VERSION_MINOR",
        "CARGO_PKG_VERSION_PATCH",
    ]
    .map(|name| var(name).parse::<u16>().unwrap_or(0));
    resource(
        &mut res,
        RT_VERSION,
        1,
        0x0030,
        LANG_EN_US,
        &version_info(version, &var("CARGO_PKG_VERSION")),
    );
    let out = Path::new(&var("OUT_DIR")).join("tore-hangar.res");
    fs::write(&out, res).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
    println!("cargo:rustc-link-arg-bins={}", out.display());
}

fn u16_at(data: &[u8], at: usize) -> Result<u16, String> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| format!("truncated at {at}"))
}

fn u32_at(data: &[u8], at: usize) -> Result<u32, String> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| format!("truncated at {at}"))
}

/// One .res entry with ordinal type and name (a 32-byte header), DWORD padded.
fn resource(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, lang: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    for word in [0xffff, kind, 0xffff, id] {
        out.extend_from_slice(&u16::to_le_bytes(word));
    }
    out.extend_from_slice(&0u32.to_le_bytes()); // DataVersion
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&lang.to_le_bytes());
    out.extend_from_slice(&[0; 8]); // Version, Characteristics
    out.extend_from_slice(data);
    out.resize(out.len().next_multiple_of(4), 0);
}

/// Splits an .ico into RT_ICON 1..n and the RT_GROUP_ICON 1 directory naming
/// them: the group entry is the ICONDIRENTRY with the file offset replaced by
/// the icon's resource id.
fn icon_resources(out: &mut Vec<u8>, ico: &[u8]) -> Result<(), String> {
    if u16_at(ico, 0)? != 0 || u16_at(ico, 2)? != 1 {
        return Err("not an icon file".into());
    }
    let count = u16_at(ico, 4)?;
    if count == 0 || count > 64 {
        return Err(format!("unexpected image count {count}"));
    }
    let mut group = Vec::new();
    group.extend_from_slice(&[0, 0, 1, 0]);
    group.extend_from_slice(&count.to_le_bytes());
    for id in 1..=count {
        let entry = 6 + usize::from(id - 1) * 16;
        let head = ico.get(entry..entry + 12).ok_or("truncated directory")?;
        let size = u32_at(ico, entry + 8)? as usize;
        let offset = u32_at(ico, entry + 12)? as usize;
        let image = ico
            .get(offset..offset.checked_add(size).ok_or("bad image size")?)
            .ok_or_else(|| format!("image {id} outside the file"))?;
        resource(out, RT_ICON, id, 0x1010, LANG_EN_US, image);
        group.extend_from_slice(head);
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(out, RT_GROUP_ICON, 1, 0x1030, LANG_EN_US, &group);
    Ok(())
}

/// A VERSIONINFO block node: header, UTF-16 key, value, children, each part
/// DWORD aligned. `text` values count UTF-16 units; binary values count bytes.
fn node(key: &str, value: &[u8], text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; 6];
    let value_len = if text { value.len() / 2 } else { value.len() };
    out[2..4].copy_from_slice(&(value_len as u16).to_le_bytes());
    out[4..6].copy_from_slice(&u16::from(text).to_le_bytes());
    out.extend(wide(key));
    out.resize(out.len().next_multiple_of(4), 0);
    out.extend_from_slice(value);
    for child in children {
        out.resize(out.len().next_multiple_of(4), 0);
        out.extend_from_slice(child);
    }
    let len = out.len() as u16;
    out[0..2].copy_from_slice(&len.to_le_bytes());
    out
}

fn wide(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}

fn version_info([major, minor, patch]: [u16; 3], version: &str) -> Vec<u8> {
    let ms = (u32::from(major) << 16) | u32::from(minor);
    let ls = u32::from(patch) << 16;
    // VS_FIXEDFILEINFO: signature, structure version, file and product
    // versions, flags mask and flags, VOS__WINDOWS32 (95/98/ME and NT),
    // VFT_APP, no subtype or date.
    let fixed = [
        0xfeef_04bd,
        0x0001_0000,
        ms,
        ls,
        ms,
        ls,
        0x3f,
        0,
        4,
        1,
        0,
        0,
        0,
    ]
    .map(u32::to_le_bytes)
    .concat();
    let strings = [
        ("FileDescription", "TORE Hangar"),
        ("FileVersion", version),
        ("InternalName", "tore-hangar"),
        (
            "LegalCopyright",
            "Free software under GPL-3.0-only, without warranty",
        ),
        ("OriginalFilename", "tore-hangar.exe"),
        ("ProductName", "TORE Hangar"),
        ("ProductVersion", version),
    ]
    .map(|(key, value)| node(key, &wide(value), true, &[]));
    let table = node("040904B0", &[], true, &strings);
    let string_info = node("StringFileInfo", &[], true, &[table]);
    let translation = [LANG_EN_US, 1200].map(u16::to_le_bytes).concat();
    let var_info = node(
        "VarFileInfo",
        &[],
        true,
        &[node("Translation", &translation, false, &[])],
    );
    node("VS_VERSION_INFO", &fixed, false, &[string_info, var_info])
}
