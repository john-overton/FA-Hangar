//! Private aircraft resource graph. BRF strings and bounded module filename
//! literals are relocated by name without moving any compiled module bytes.
use crate::{
    archive::{validate_name, Archive, Entry, ARCHIVE_LIMIT},
    authoring::validate_id,
    brf::Brf,
    invalid, slice, u16_at, u32_at, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

#[derive(Debug)]
pub struct Package {
    pub archive: Archive,
    pub mapping: Vec<(String, String)>,
    pub donor: String,
    pub id: String,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug)]
enum Location {
    Text(usize),
    Literal {
        at: usize,
        len: usize,
        stem_only: bool,
    },
}
#[derive(Clone, Debug)]
struct Reference {
    target: String,
    location: Location,
}
struct Node {
    bytes: Vec<u8>,
    refs: Vec<Reference>,
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
fn resource_extension(s: &str) -> bool {
    matches!(
        ext(s),
        "PT" | "SH"
            | "HUD"
            | "PTS"
            | "BI"
            | "JT"
            | "OT"
            | "SEE"
            | "ECM"
            | "GAS"
            | "PIC"
            | "PAL"
            | "FNT"
            | "5K"
            | "8K"
            | "11K"
            | "22K"
            | "WAV"
    )
}
fn leaf(s: &str) -> bool {
    matches!(
        ext(s),
        "PIC" | "PAL" | "FNT" | "5K" | "8K" | "11K" | "22K" | "WAV"
    )
}
fn name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'()-@^_`{}~.".contains(&b)
}
fn root_reference(brf: &Brf, label: &str) -> Result<Option<String>> {
    let f = brf
        .fields
        .iter()
        .find(|f| f.label == label)
        .ok_or_else(|| format!("Unrecognized donor schema: {label}"))?;
    if f.kind == "dword" && f.value == "0" {
        return Ok(None);
    }
    if f.kind != "ptr" {
        return Err(format!("Unsupported donor pointer {label}"));
    }
    let fields: Vec<_> = brf
        .fields
        .iter()
        .filter(|v| v.block == f.value && v.kind == "string")
        .collect();
    if fields.len() != 1 {
        return Err(format!("Expected one resource in donor block {}", f.value));
    }
    let name = unquote(&fields[0].value).to_ascii_uppercase();
    validate_name(&name)?;
    Ok(Some(name))
}
fn sections(bytes: &[u8]) -> Result<Vec<(usize, usize, bool)>> {
    if slice(bytes, 0, 2)? != b"MZ" {
        return Err(invalid("Expected an inert PL/PE module"));
    }
    let p = u32_at(bytes, 60)?;
    if !matches!(slice(bytes, p, 4)?, b"PL\0\0" | b"PE\0\0") || u16_at(bytes, p + 4)? != 0x14c {
        return Err(invalid("Unsupported module signature/machine"));
    }
    let count = u16_at(bytes, p + 6)?;
    let optional = u16_at(bytes, p + 20)?;
    if count == 0 || count > 32 || optional < 32 {
        return Err(invalid("Invalid module section table"));
    }
    let table = p
        .checked_add(24 + optional)
        .ok_or_else(|| invalid("Section offset overflow"))?;
    let mut ranges = Vec::new();
    let mut occupied: Vec<(usize, usize)> = Vec::new();
    for i in 0..count {
        let s = slice(bytes, table + i * 40, 40)?;
        let section_name =
            core::str::from_utf8(s[..8].split(|b| *b == 0).next().unwrap()).unwrap_or("");
        let size = u32_at(s, 16)?;
        let at = u32_at(s, 20)?;
        if size == 0 {
            continue;
        }
        slice(bytes, at, size)?;
        if at < table + count * 40 {
            return Err(invalid("Module section overlaps its headers"));
        }
        if occupied.iter().any(|(a, n)| at < *a + *n && *a < at + size) {
            return Err(invalid("Overlapping module sections"));
        }
        occupied.push((at, size));
        if matches!(section_name, "CODE" | "DATA" | ".data" | ".rdata" | ".text") {
            ranges.push((at, size.min(u32_at(s, 8)?), section_name == "CODE"));
        }
    }
    Ok(ranges)
}
/// Recognize complete null-terminated filenames in module sections, never byte
/// substrings in images/audio. Extensionless picture names require a named E2
/// operand or the established ~/underscore picture prefix and a catalog match.
fn references(
    name: &str,
    bytes: &[u8],
    catalog: &BTreeSet<String>,
    unresolved: &mut BTreeSet<String>,
) -> Result<Vec<Reference>> {
    if leaf(name) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    if bytes.starts_with(b"[brent's_relocatable_format]") {
        let brf = Brf::parse(bytes, ext(name))?;
        for (i, f) in brf
            .fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.kind == "string")
        {
            let target = unquote(&f.value).to_ascii_uppercase();
            if validate_name(&target).is_ok()
                && (catalog.contains(&target) || resource_extension(&target))
            {
                out.push(Reference {
                    target,
                    location: Location::Text(i),
                });
            }
        }
    } else if bytes.starts_with(b"MZ") {
        for (at, len, code) in sections(bytes)? {
            let b = &bytes[at..at + len];
            let mut start = 0;
            for (i, value) in b.iter().enumerate() {
                if name_byte(*value) {
                    continue;
                }
                if *value == 0 && (1..=12).contains(&(i - start)) {
                    let token = core::str::from_utf8(&b[start..i])
                        .unwrap()
                        .to_ascii_uppercase();
                    let e2 = code && start >= 2 && b[start - 2..start] == [0xe2, 0];
                    const HUD_NAMES: &[usize] = &[
                        1, 0xe, 0x1b, 0x13c, 0x149, 0x156, 0x163, 0x170, 0x17d, 0x18a, 0x197,
                        0x1a4, 0x275, 0x282,
                    ];
                    let hud = ext(name) == "HUD" && code && HUD_NAMES.contains(&start);
                    let width = if e2 {
                        14
                    } else if hud {
                        13
                    } else {
                        0
                    };
                    let capacity = if width > 0
                        && start + width <= b.len()
                        && b[i..start + width].iter().all(|b| *b == 0)
                    {
                        width - 1
                    } else {
                        i - start
                    };
                    if token.contains('.') {
                        if validate_name(&token).is_ok()
                            && (catalog.contains(&token) || resource_extension(&token))
                        {
                            out.push(Reference {
                                target: token,
                                location: Location::Literal {
                                    at: at + start,
                                    len: capacity,
                                    stem_only: false,
                                },
                            });
                        }
                    } else {
                        let candidate = format!("{token}.PIC");
                        if hud
                            && token.starts_with('~')
                            && token.len() >= 3
                            && !catalog.contains(&candidate)
                            && unresolved.len() < 128
                        {
                            unresolved.insert(format!("{name}: unresolved module name {token}; no matching PIC, left unchanged."));
                        }
                        if validate_name(&candidate).is_ok()
                            && (e2
                                || ((hud || token.starts_with('~') || token.starts_with('_'))
                                    && catalog.contains(&candidate)))
                        {
                            out.push(Reference {
                                target: candidate,
                                location: Location::Literal {
                                    at: at + start,
                                    len: capacity,
                                    stem_only: true,
                                },
                            });
                        }
                    }
                }
                start = i + 1;
            }
        }
    } else {
        return Err(format!(
            "Cannot inspect dependencies of {name}: unsupported resource encoding"
        ));
    }
    Ok(out)
}
fn base36(mut n: usize) -> String {
    let mut b = Vec::new();
    loop {
        b.push(b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"[n % 36]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    b.reverse();
    String::from_utf8(b).unwrap()
}
fn prefix(s: &str) -> &str {
    if s.starts_with(['~', '_', '$', '&', '#', '^']) {
        &s[..1]
    } else {
        ""
    }
}
fn assign(
    old: &str,
    new: String,
    map: &mut BTreeMap<String, String>,
    used: &mut BTreeSet<String>,
    budget: &BTreeMap<String, usize>,
) -> Result<()> {
    validate_name(&new)?;
    if stem(&new).len() > *budget.get(old).unwrap_or(&8) {
        return Err(format!(
            "{old} has a short compiled name slot; use a shorter aircraft ID"
        ));
    }
    if used.contains(&new) {
        return Err(format!(
            "Name collision: {new}. Choose a different aircraft ID"
        ));
    }
    used.insert(new.clone());
    map.insert(old.into(), new);
    Ok(())
}
fn generated(old: &str, id: &str, limit: usize, used: &BTreeSet<String>) -> Result<String> {
    let pre = prefix(stem(old));
    for n in 0..46656 {
        let suffix = base36(n);
        if pre.len() + suffix.len() > limit {
            break;
        }
        let count = (limit - pre.len() - suffix.len()).min(id.len());
        let candidate = format!("{}{}{}.{}", pre, &id[..count], suffix, ext(old));
        if !used.contains(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "No free short name for {old}; its compiled filename slot is too small"
    ))
}

/// Sources are in priority order: current document first. All source filenames
/// are reserved so the generated library cannot override any scanned resource.
pub fn build(sources: &[&Archive], donor: &str, id: &str, title: &str) -> Result<Package> {
    let mut catalog = BTreeSet::new();
    let mut providers = BTreeMap::new();
    for (si, archive) in sources.iter().enumerate() {
        for (ei, e) in archive.entries.iter().enumerate() {
            catalog.insert(e.name.clone());
            providers.entry(e.name.clone()).or_insert((si, ei));
        }
    }
    build_with(&catalog, donor, id, title, |name| {
        let (si, ei) = providers
            .get(name)
            .ok_or_else(|| format!("Missing referenced resource {name}"))?;
        sources[*si].entries[*ei].read()
    })
}
pub fn build_with(
    catalog: &BTreeSet<String>,
    donor: &str,
    id: &str,
    title: &str,
    mut read: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Package> {
    let id = validate_id(id)?;
    if title.is_empty()
        || title.len() > 40
        || !title.is_ascii()
        || title.contains(['"', ';', '\r', '\n'])
        || title.bytes().any(|b| b < 32)
    {
        return Err(invalid(
            "Display name must be 1..40 plain ASCII characters without quotes/semicolons",
        ));
    }
    let donor = donor.to_ascii_uppercase();
    if ext(&donor) != "PT" {
        return Err(invalid("Select an aircraft PT to duplicate"));
    }
    let root_bytes = read(&donor)?;
    let root = Brf::parse(&root_bytes, "PT")?;
    let main = root_reference(&root, "object.shape")?.ok_or("Donor has no main shape")?;
    let shadow = root_reference(&root, "object.shadowShape")?.ok_or("Donor has no shadow shape")?;
    let family = shadow
        .strip_suffix("_S.SH")
        .ok_or("Donor does not use the reviewed _S.SH damage family")?;
    let mut roots = vec![donor.clone(), main.clone(), shadow.clone()];
    for suffix in ["A", "B", "C", "D"] {
        roots.push(format!("{family}_{suffix}.SH"));
    }
    let hud = root_reference(&root, "object.hudName")?.or_else(|| {
        let n = format!("{}.HUD", stem(&donor));
        catalog.contains(&n).then_some(n)
    });
    if let Some(h) = &hud {
        roots.push(h.clone());
    }
    let private_palette = format!("{}.PAL", stem(&donor));
    let palette = if catalog.contains(&private_palette) {
        Some(private_palette)
    } else if catalog.contains("PALETTE.PAL") {
        Some("PALETTE.PAL".into())
    } else {
        None
    };
    if let Some(name) = &palette {
        roots.push(name.clone());
    }
    let mut unresolved = BTreeSet::new();
    let mut graph = BTreeMap::<String, Node>::new();
    let mut pending = roots;
    let mut bytes_total = 0usize;
    while let Some(name) = pending.pop() {
        if graph.contains_key(&name) {
            continue;
        }
        if graph.len() >= 4096 {
            return Err(invalid("Aircraft graph exceeds 4096 resources"));
        }
        if ext(&name) == "PT" && name != donor {
            return Err(format!("The donor graph references another aircraft {name}; correct its identity/reference before exporting"));
        }
        let bytes = read(&name)?;
        bytes_total = bytes_total
            .checked_add(bytes.len())
            .ok_or("Resource size overflow")?;
        if bytes_total > ARCHIVE_LIMIT {
            return Err(invalid("Aircraft graph exceeds 128 MiB"));
        }
        let refs = references(&name, &bytes, catalog, &mut unresolved)
            .map_err(|e| format!("{name}: {e}"))?;
        for r in &refs {
            if !catalog.contains(&r.target) {
                return Err(format!(
                    "{name} references missing {}. Add its source LIB and retry",
                    r.target
                ));
            }
            pending.push(r.target.clone());
        }
        // FA's ordnance menu derives the icon from the store's definition stem.
        if matches!(ext(&name), "JT" | "SEE" | "ECM" | "GAS") {
            let icon = format!("${}.PIC", stem(&name));
            if catalog.contains(&icon) {
                pending.push(icon);
            }
        }
        graph.insert(name, Node { bytes, refs });
    }
    let mut budget = BTreeMap::<String, usize>::new();
    for name in graph.keys() {
        budget.insert(name.clone(), 8);
    }
    for node in graph.values() {
        for r in &node.refs {
            if let Location::Literal { len, stem_only, .. } = r.location {
                let cap = if stem_only {
                    len
                } else {
                    len.checked_sub(ext(&r.target).len() + 1)
                        .ok_or("Bad literal capacity")?
                };
                let b = budget.get_mut(&r.target).unwrap();
                *b = (*b).min(cap);
            }
        }
    }
    let mut mapping = BTreeMap::new();
    let mut used = catalog.clone();
    assign(&donor, format!("{id}.PT"), &mut mapping, &mut used, &budget)?;
    assign(&main, format!("{id}.SH"), &mut mapping, &mut used, &budget)?;
    for suffix in ["A", "B", "C", "D", "S"] {
        assign(
            &format!("{family}_{suffix}.SH"),
            format!("{id}_{suffix}.SH"),
            &mut mapping,
            &mut used,
            &budget,
        )?;
    }
    if let Some(h) = &hud {
        assign(h, format!("{id}.HUD"), &mut mapping, &mut used, &budget)?;
    }
    if let Some(name) = &palette {
        assign(name, format!("{id}.PAL"), &mut mapping, &mut used, &budget)?;
    }

    // Preserve store/icon stem relationships rather than independently aliasing icons.
    for name in graph
        .keys()
        .filter(|n| matches!(ext(n), "JT" | "SEE" | "ECM" | "GAS"))
    {
        let icon = format!("${}.PIC", stem(name));
        let mut limit = budget[name];
        if graph.contains_key(&icon) {
            limit = limit.min(budget[&icon].saturating_sub(1));
        }
        let mut n = 0;
        loop {
            let candidate = generated(name, &format!("{id}{}", base36(n)), limit, &used)?;
            let new_icon = format!("${}.PIC", stem(&candidate));
            if !graph.contains_key(&icon) || !used.contains(&new_icon) {
                assign(name, candidate, &mut mapping, &mut used, &budget)?;
                if graph.contains_key(&icon) {
                    assign(&icon, new_icon, &mut mapping, &mut used, &budget)?;
                }
                break;
            }
            // Exclude this spelling before finding the next candidate.
            used.insert(candidate);
            n += 1;
            if n > 4096 {
                return Err(invalid("Cannot allocate store/icon names"));
            }
        }
    }
    // Keep recognizable texture/cockpit families (base, H/S and _A/_B/...)
    // together so their conventional suffix relationships remain intact.
    let mut pictures: Vec<_> = graph
        .keys()
        .filter(|n| ext(n) == "PIC" && !mapping.contains_key(*n))
        .cloned()
        .collect();
    pictures.sort_unstable_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    const SUFFIXES: &[&str] = &[
        "", "H", "S", "_A", "_B", "_C", "_D", "_S", "_L", "_R", "_P", "_LH", "_LS", "_RH", "_RS",
        "_CH", "_CS",
    ];
    for name in pictures {
        if mapping.contains_key(&name) {
            continue;
        }
        let base = stem(&name);
        let members: Vec<_> = SUFFIXES
            .iter()
            .filter_map(|suffix| {
                let n = format!("{base}{suffix}.PIC");
                (graph.contains_key(&n) && !mapping.contains_key(&n)).then_some((n, *suffix))
            })
            .collect();
        let limit = members
            .iter()
            .map(|(n, s)| budget[n].saturating_sub(s.len()))
            .min()
            .unwrap_or(budget[&name]);
        let mut attempts = 0;
        loop {
            let candidate = generated(&name, &id, limit, &used)?;
            let names: Vec<_> = members
                .iter()
                .map(|(_, suffix)| format!("{}{suffix}.PIC", stem(&candidate)))
                .collect();
            if names.iter().all(|n| !used.contains(n)) {
                for ((old, _), new) in members.iter().zip(names) {
                    assign(old, new, &mut mapping, &mut used, &budget)?;
                }
                break;
            }
            used.insert(candidate);
            attempts += 1;
            if attempts > 4096 {
                return Err(invalid("Cannot allocate texture family names"));
            }
        }
    }
    for name in graph.keys() {
        if !mapping.contains_key(name) {
            let new = generated(name, &id, budget[name], &used)?;
            assign(name, new, &mut mapping, &mut used, &budget)?;
        }
    }
    let mut archive = Archive::empty();
    let name_block = root
        .fields
        .iter()
        .find(|f| f.label == "object.ot_names" && f.kind == "ptr")
        .ok_or("No aircraft name block")?
        .value
        .clone();
    let labels: Vec<_> = root
        .fields
        .iter()
        .enumerate()
        .filter(|(_, f)| f.block == name_block && f.kind == "string")
        .map(|(i, _)| i)
        .collect();
    if labels.len() != 3 {
        return Err(invalid("Expected three aircraft identity strings"));
    }
    let mut ordered: Vec<_> = graph.keys().cloned().collect();
    ordered.retain(|n| n != &donor);
    ordered.insert(0, donor.clone());
    for name in ordered {
        let node = graph.remove(&name).unwrap();
        let mut bytes = node.bytes;
        if bytes.starts_with(b"[brent's_relocatable_format]") {
            let brf = Brf::parse(&bytes, ext(&name))?;
            let mut edits = BTreeMap::new();
            for r in &node.refs {
                if let Location::Text(index) = r.location {
                    edits.insert(index, format!("\"{}\"", mapping[&r.target]));
                }
            }
            if name == donor {
                edits.insert(labels[0], format!("\"{title}\""));
                edits.insert(labels[1], format!("\"{title}\""));
                edits.insert(labels[2], format!("\"{id}.PT\""));
            }
            // Descending source ranges preserve comments/unknown operands exactly.
            for (index, value) in edits.iter().rev() {
                let f = &brf.fields[*index];
                bytes.splice(f.start..f.end, value.bytes());
            }
            Brf::parse(&bytes, ext(&name))?;
        } else {
            for r in &node.refs {
                if let Location::Literal { at, len, stem_only } = r.location {
                    let target = &mapping[&r.target];
                    let text = if stem_only { stem(target) } else { target };
                    if text.len() > len {
                        return Err(invalid("New module name exceeds original string slot"));
                    }
                    bytes[at..at + len].fill(0);
                    bytes[at..at + text.len()].copy_from_slice(text.as_bytes());
                }
            }
        }
        archive.entries.push(Entry::new(&mapping[&name], bytes)?);
    }
    archive.entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    // Verify closure after rewriting, including extensionless PIC operands.
    let mut output_catalog = BTreeSet::new();
    for e in &archive.entries {
        output_catalog.insert(e.name.clone());
    }
    let mut audit_catalog = catalog.clone();
    audit_catalog.extend(output_catalog.iter().cloned());
    for e in &archive.entries {
        for r in references(&e.name, &e.read()?, &audit_catalog, &mut BTreeSet::new())? {
            if !output_catalog.contains(&r.target) {
                return Err(format!(
                    "Output validation: {} still references {}",
                    e.name, r.target
                ));
            }
        }
    }
    archive.bytes()?;
    let mut notes = vec![
        "Referenced files are copied and privately renamed; source files are preserved.".into(),
        "Runtime-generated assets and game procedures remain game-provided.".into(),
        "Some resource names are shortened to fit the original file format.".into(),
    ];
    notes.extend(unresolved);
    let mut mapping: Vec<_> = mapping.into_iter().collect();
    let main_name = format!("{id}.SH");
    let family_prefix = format!("{id}_");
    let priority = |(old, new): &(String, String)| {
        if old == &donor {
            0
        } else if new == &main_name {
            1
        } else if new.starts_with(&family_prefix) && new.ends_with(".SH") {
            2
        } else if new.ends_with(".HUD") {
            3
        } else if new.ends_with(".PIC") {
            4
        } else {
            5
        }
    };
    mapping.sort_unstable_by(|a, b| priority(a).cmp(&priority(b)).then_with(|| a.0.cmp(&b.0)));
    Ok(Package {
        archive,
        mapping,
        donor,
        id,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn module(code: &[u8], imports: &[u8]) -> Vec<u8> {
        let mut b = crate::model::demo_shape();
        b.truncate(256);
        b[70..72].copy_from_slice(&2u16.to_le_bytes());
        for (at, n) in [
            (128, code.len()),
            (136, code.len()),
            (168, imports.len()),
            (176, imports.len()),
            (180, 256 + code.len()),
        ] {
            b[at..at + 4].copy_from_slice(&(n as u32).to_le_bytes());
        }
        b[160..166].copy_from_slice(b".idata");
        b.extend(code);
        b.extend(imports);
        b
    }
    fn source() -> Archive {
        let mut a = Archive::empty();
        let mut pt = String::from_utf8(crate::brf::demo()).unwrap();
        pt = pt
            .replace("dword 0 ; hudName", "ptr cockpit ; hudName")
            .replace(
                "end\r\n",
                ":cockpit\r\nstring \"DEMO.HUD\"\r\n:weapon\r\nstring \"SHOT.JT\"\r\nend\r\n",
            );
        a.entries
            .push(Entry::new("DEMO.PT", pt.into_bytes()).unwrap());
        let b = crate::model::demo_textured();
        let mut code = b[256..].to_vec();
        // An unreachable texture still belongs to the copied module, not only its displayed pose.
        code.extend([0xe2, 0]);
        code.extend(b"HIDDEN.PIC\0\0\0\0");
        a.entries
            .push(Entry::new("DEMO.SH", module(&code, b"SYMBOL.PIC\0")).unwrap());
        for suffix in ["A", "B", "C", "D", "S"] {
            a.entries.push(
                Entry::new(&format!("DEMO_{suffix}.SH"), crate::model::demo_shape()).unwrap(),
            );
        }
        let mut hud = vec![0; 0x2b2];
        for (at, name) in [
            (1, b"~PANEL\0".as_slice()),
            (0xe, b"~PANELH\0".as_slice()),
            (0x282, b"~PANEL_W\0".as_slice()),
        ] {
            hud[at..at + name.len()].copy_from_slice(name);
        }
        a.entries
            .push(Entry::new("DEMO.HUD", module(&hud, &[])).unwrap());
        a.entries.push(
            Entry::new(
                "SHOT.JT",
                b"[brent's_relocatable_format]\nstring \"SHOT.JT\"\nstring \"SFX.11K\"\nend\n"
                    .to_vec(),
            )
            .unwrap(),
        );
        // A filename-looking sequence in sample bytes must never be rewritten or traversed.
        a.entries
            .push(Entry::new("SFX.11K", b"DEMO.SH\0sample".to_vec()).unwrap());
        for name in [
            "DEMO.PIC",
            "HIDDEN.PIC",
            "$SHOT.PIC",
            "~PANEL.PIC",
            "~PANELH.PIC",
            "SYMBOL.PIC",
        ] {
            a.entries
                .push(Entry::new(name, crate::picture::demo()).unwrap());
        }
        a.entries
            .push(Entry::new("PALETTE.PAL", vec![0; 768]).unwrap());
        a
    }
    #[test]
    fn graph_is_private_recursive_lossless_and_bounded() {
        let a = source();
        let original = a.bytes().unwrap();
        let p = build(&[&a], "DEMO.PT", "NEW", "New aircraft").unwrap();
        let mapping: BTreeMap<_, _> = p.mapping.iter().cloned().collect();
        assert_eq!(p.archive.entries.len(), 16);
        assert!(mapping.contains_key("HIDDEN.PIC"));
        assert!(!mapping.contains_key("SYMBOL.PIC"));
        assert_eq!(
            mapping["~PANELH.PIC"],
            format!("{}H.PIC", stem(&mapping["~PANEL.PIC"]))
        );
        assert_eq!(
            mapping["$SHOT.PIC"],
            format!("${}.PIC", stem(&mapping["SHOT.JT"]))
        );
        assert!(p.notes.iter().any(|n| n.contains("~PANEL_W")));
        let mut catalog = BTreeSet::new();
        for e in &a.entries {
            catalog.insert(e.name.clone());
        }
        for (old, new) in &p.mapping {
            assert!(a.find(new).is_none(), "collision {new}");
            let before = a.entries[a.find(old).unwrap()].read().unwrap();
            let after = p.archive.entries[p.archive.find(new).unwrap()]
                .read()
                .unwrap();
            if leaf(old) {
                assert_eq!(before, after);
            } else if before.starts_with(b"MZ") {
                assert_eq!(before.len(), after.len());
                let refs = references(old, &before, &catalog, &mut BTreeSet::new()).unwrap();
                for (i, (a, b)) in before.iter().zip(&after).enumerate() {
                    if a != b {
                        assert!(refs.iter().any(|r|matches!(r.location,Location::Literal{at,len,..} if i>=at&&i<at+len)),"changed module byte outside a filename: {i}");
                    }
                }
            }
        }
        let decoded = Archive::parse(p.archive.bytes().unwrap()).unwrap();
        assert_eq!(decoded.entries.len(), p.mapping.len());
        assert_eq!(a.bytes().unwrap(), original);
        let again = build(&[&p.archive], "NEW.PT", "NEXT", "Next variant").unwrap();
        assert!(again.archive.find("NEXT.PAL").is_some());
        assert!(again
            .archive
            .entries
            .iter()
            .all(|e| p.archive.find(&e.name).is_none()));
    }
    #[test]
    fn missing_dependencies_and_collisions_block_before_output() {
        let mut a = source();
        assert!(build(&[&a], "DEMO.PT", "DEMO", "Same").is_err());
        a.entries.retain(|e| e.name != "SFX.11K");
        let e = build(&[&a], "DEMO.PT", "NEW", "New").unwrap_err();
        assert!(e.contains("SFX.11K"));
        let mut extra = Archive::empty();
        extra
            .entries
            .push(Entry::new("SFX.11K", vec![128]).unwrap());
        assert!(build(&[&a, &extra], "DEMO.PT", "NEW", "New").is_ok());
    }
    #[test]
    fn index_reads_only_directory_and_checks_eof() {
        let a = source();
        let b = a.bytes().unwrap();
        let n = 7 + (a.entries.len() + 1) * 18;
        let index = crate::archive::directory(&b[..n], b.len()).unwrap();
        assert_eq!(index.len(), a.entries.len());
        for (i, e) in index.iter().enumerate() {
            assert_eq!(
                crate::archive::decode_payload(e.flag, b[e.offset..e.offset + e.size].to_vec())
                    .unwrap(),
                a.entries[i].read().unwrap()
            );
        }
        assert!(crate::archive::directory(&b[..n], b.len() + 1).is_err());
        assert!(crate::archive::directory(&b[..n - 1], b.len()).is_err());
    }
}
