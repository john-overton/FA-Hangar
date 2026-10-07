//! Private object resource graph, including the reviewed aircraft family. BRF strings and bounded module filename
//! literals are relocated by name without moving any compiled module bytes.
use crate::{
    archive::{validate_name, Archive, Entry, ARCHIVE_LIMIT},
    authoring::validate_id,
    brf::Brf,
    dependencies::{references, Location, Reference},
    invalid, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
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
fn dependencies_known(name: &str, bytes: &[u8]) -> bool {
    crate::dependencies::leaf(name)
        || bytes.starts_with(b"MZ")
        || bytes.starts_with(b"[brent's_relocatable_format]")
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
            "{old} has a short compiled name slot; use a shorter object ID"
        ));
    }
    if used.contains(&new) {
        return Err(format!(
            "Name collision: {new}. Choose a different object ID"
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
    validate_name(&donor)?;
    let root_bytes = read(&donor)?;
    let root = if root_bytes.starts_with(b"[brent's_relocatable_format]") {
        Some(Brf::parse(&root_bytes, ext(&donor))?)
    } else {
        None
    };
    let optional_reference = |label: &str| -> Result<Option<String>> {
        if let Some(b) = &root {
            if b.fields.iter().any(|f| f.label == label) {
                return root_reference(b, label);
            }
        }
        Ok(None)
    };
    let main = optional_reference("object.shape")?;
    let shadow = optional_reference("object.shadowShape")?;
    let family = if ext(&donor) == "PT" {
        Some(
            shadow
                .as_deref()
                .and_then(|s| s.strip_suffix("_S.SH"))
                .ok_or("Aircraft does not use the reviewed _S.SH family")?
                .to_string(),
        )
    } else {
        None
    };
    if family.is_some() && main == shadow {
        return Err(invalid(
            "Main and shadow share a file; this aircraft family needs separate authoring",
        ));
    }
    let mut roots = vec![donor.clone()];
    if let Some(name) = &main {
        roots.push(name.clone());
    }
    if let Some(name) = &shadow {
        roots.push(name.clone());
    }
    if let Some(family) = &family {
        for suffix in ["A", "B", "C", "D"] {
            roots.push(format!("{family}_{suffix}.SH"));
        }
    }
    let hud = optional_reference("object.hudName")?.or_else(|| {
        let n = format!("{}.HUD", stem(&donor));
        (ext(&donor) == "PT" && catalog.contains(&n)).then_some(n)
    });
    if let Some(h) = &hud {
        roots.push(h.clone());
    }
    let private_palette = format!("{}.PAL", stem(&donor));
    let palette = if matches!(ext(&donor), "5K" | "8K" | "11K" | "22K" | "WAV" | "PAL") {
        None
    } else if catalog.contains(&private_palette) {
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
            return Err(invalid("Object graph exceeds 4096 resources"));
        }
        if ext(&name) == "PT" && name != donor {
            return Err(format!("The donor graph references another aircraft {name}; correct its identity/reference before exporting"));
        }
        let bytes = read(&name)?;
        bytes_total = bytes_total
            .checked_add(bytes.len())
            .ok_or("Resource size overflow")?;
        if bytes_total > ARCHIVE_LIMIT {
            return Err(invalid("Object graph exceeds 128 MiB"));
        }
        let refs = if !dependencies_known(&name, &bytes) {
            unresolved.insert(format!(
                "{name}: opaque bytes copied unchanged; dependency discovery unavailable."
            ));
            Vec::new()
        } else {
            references(&name, &bytes, catalog, &mut unresolved)
                .map_err(|e| format!("{name}: {e}"))?
        };
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
    assign(
        &donor,
        format!("{id}.{}", ext(&donor)),
        &mut mapping,
        &mut used,
        &budget,
    )?;
    if let Some(main) = &main {
        if main != &donor {
            assign(main, format!("{id}.SH"), &mut mapping, &mut used, &budget)?;
        }
    }
    if let Some(family) = &family {
        for suffix in ["A", "B", "C", "D", "S"] {
            let name = format!("{family}_{suffix}.SH");
            if !mapping.contains_key(&name) {
                assign(
                    &name,
                    format!("{id}_{suffix}.SH"),
                    &mut mapping,
                    &mut used,
                    &budget,
                )?;
            }
        }
    }
    if let Some(h) = &hud {
        if !mapping.contains_key(h) {
            assign(h, format!("{id}.HUD"), &mut mapping, &mut used, &budget)?;
        }
    }
    if let Some(name) = &palette {
        if !mapping.contains_key(name) {
            assign(name, format!("{id}.PAL"), &mut mapping, &mut used, &budget)?;
        }
    }

    // Preserve store/icon stem relationships rather than independently aliasing icons.
    for name in graph
        .keys()
        .filter(|n| matches!(ext(n), "JT" | "SEE" | "ECM" | "GAS"))
    {
        if let Some(new_name) = mapping.get(name).cloned() {
            let icon = format!("${}.PIC", stem(name));
            if graph.contains_key(&icon) {
                assign(
                    &icon,
                    format!("${}.PIC", stem(&new_name)),
                    &mut mapping,
                    &mut used,
                    &budget,
                )?;
            }
            continue;
        }
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
    // Stored originals follow their privately renamed textures; a taken
    // companion name is reported rather than overwritten.
    let mut originals = Vec::new();
    for (old, new) in &mapping {
        let (Some(org), Some(target)) = (
            crate::originals::companion(old),
            crate::originals::companion(new),
        ) else {
            continue;
        };
        if !catalog.contains(&org) {
            continue;
        }
        match read(&org) {
            Ok(bytes) if crate::picture::Pic::parse(&bytes).is_ok() => {
                if used.insert(target.clone()) {
                    originals.push((target, bytes));
                } else {
                    unresolved.insert(format!(
                        "{org}: stored original not copied; {target} is already in use."
                    ));
                }
            }
            _ => {}
        }
    }
    let mut archive = Archive::empty();
    let mut identities = Vec::new();
    if let Some(root) = &root {
        for pointer in root.fields.iter().filter(|f| {
            f.kind == "ptr"
                && (f.label.ends_with(".ot_names")
                    || f.label.ends_with(".si_names")
                    || matches!(f.value.as_str(), "ot_names" | "si_names"))
        }) {
            let labels: Vec<_> = root
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| f.block == pointer.value && f.kind == "string")
                .map(|(i, _)| i)
                .collect();
            if labels.len() == 3
                && root
                    .fields
                    .iter()
                    .filter(|f| f.block == pointer.value)
                    .count()
                    == 3
            {
                identities.push(labels);
            }
        }
        if identities.is_empty() {
            unresolved.insert("Display-name block unrecognized; existing text preserved.".into());
        }
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
                for labels in &identities {
                    edits.insert(labels[0], format!("\"{title}\""));
                    edits.insert(labels[1], format!("\"{title}\""));
                    edits.insert(labels[2], format!("\"{id}.{}\"", ext(&donor)));
                }
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
    let copied_originals = originals.len();
    for (name, bytes) in originals {
        archive.entries.push(Entry::new(&name, bytes)?);
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
        let bytes = e.read()?;
        if !dependencies_known(&e.name, &bytes) {
            continue;
        }
        for r in references(&e.name, &bytes, &audit_catalog, &mut BTreeSet::new())? {
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
    if copied_originals > 0 {
        notes.push(format!(
            "{copied_originals} stored original texture{} (.ORG) copied with {}; remove them for distribution builds.",
            if copied_originals == 1 { "" } else { "s" },
            if copied_originals == 1 { "its PIC" } else { "their PICs" }
        ));
    }
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
        // A painted texture's stored original, and an unrelated non-PIC .ORG.
        a.entries
            .push(Entry::new("~PANEL.ORG", crate::picture::demo()).unwrap());
        a.entries
            .push(Entry::new("DEMO.ORG", b"notes".to_vec()).unwrap());
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
        // 16 mapped resources plus the stored original of ~PANEL.PIC. DEMO.ORG
        // is not a PIC payload, so it is not treated as DEMO.PIC's original.
        assert_eq!(p.archive.entries.len(), 17);
        let panel_original = format!("{}.ORG", stem(&mapping["~PANEL.PIC"]));
        assert_eq!(
            p.archive.entries[p.archive.find(&panel_original).unwrap()]
                .read()
                .unwrap(),
            crate::picture::demo()
        );
        assert!(!mapping.contains_key("~PANEL.ORG"));
        assert!(p
            .archive
            .find(&format!("{}.ORG", stem(&mapping["DEMO.PIC"])))
            .is_none());
        assert!(p.notes.iter().any(|n| n.contains("1 stored original")));
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
            if crate::dependencies::leaf(old) {
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
        assert_eq!(decoded.entries.len(), p.mapping.len() + 1);
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

#[cfg(test)]
mod object_tests {
    use super::*;
    fn weapon() -> Vec<u8> {
        let mut s = String::from("[brent's_relocatable_format]\n");
        for schema in [crate::schema::OBJECT, crate::schema::PROJECTILE] {
            for (kind, name) in schema {
                let (k, v) = if *kind == "ptr" && ["ot_names", "si_names", "shape"].contains(name) {
                    ("ptr", *name)
                } else if *kind == "ptr" {
                    ("dword", "0")
                } else if *kind == "symbol" {
                    ("symbol", "_DEMO")
                } else {
                    (*kind, "0")
                };
                s.push_str(&format!("{k} {v} ; {name}\n"));
            }
        }
        s.push_str(":ot_names\nstring \"Old\"\nstring \"Old weapon\"\nstring \"OLD.JT\"\n:si_names\nstring \"Old\"\nstring \"Old weapon\"\nstring \"OLD.JT\"\n:shape\nstring \"BODY.SH\"\nend\n");
        s.into_bytes()
    }
    #[test]
    fn weapon_export_keeps_numeric_values_and_private_icon_pair() {
        let mut a = Archive::empty();
        a.entries = vec![
            Entry::new("OLD.JT", weapon()).unwrap(),
            Entry::new("BODY.SH", crate::model::demo_shape()).unwrap(),
            Entry::new("$OLD.PIC", crate::picture::demo()).unwrap(),
        ];
        let before = a.bytes().unwrap();
        let out = build(&[&a], "OLD.JT", "NEW", "New missile").unwrap();
        assert!(out.archive.find("NEW.JT").is_some());
        assert!(out.archive.find("NEW.SH").is_some());
        assert!(out.archive.find("$NEW.PIC").is_some());
        let bytes = out.archive.entries[out.archive.find("NEW.JT").unwrap()]
            .read()
            .unwrap();
        let b = Brf::parse(&bytes, "JT").unwrap();
        let original = Brf::parse(&weapon(), "JT").unwrap();
        for (a, b) in original.fields.iter().zip(&b.fields) {
            if a.kind != "string" {
                assert_eq!(a.value, b.value);
            }
        }
        assert_eq!(
            b.fields
                .iter()
                .filter(|f| f.kind == "string" && f.value == "\"New missile\"")
                .count(),
            4
        );
        assert_eq!(a.bytes().unwrap(), before);
    }
    #[test]
    fn leaf_and_opaque_resources_export_without_inventing_dependencies() {
        let mut a = Archive::empty();
        a.entries = vec![
            Entry::new("SOUND.11K", b"BAD.SH\0".to_vec()).unwrap(),
            Entry::new("OPAQUE.BIN", vec![1, 2, 3]).unwrap(),
        ];
        let out = build(&[&a], "SOUND.11K", "NEW", "Sound").unwrap();
        assert_eq!(out.archive.entries.len(), 1);
        assert_eq!(out.archive.entries[0].read().unwrap(), b"BAD.SH\0");
        let out = build(&[&a], "OPAQUE.BIN", "NEW", "Unknown").unwrap();
        assert_eq!(out.archive.entries[0].read().unwrap(), vec![1, 2, 3]);
        assert!(out.notes.iter().any(|s| s.contains("opaque")));
    }
}
