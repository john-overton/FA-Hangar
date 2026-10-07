//! Private object resource graph, including the reviewed aircraft family. BRF strings and bounded module filename
//! literals are relocated by name without moving any compiled module bytes.
use crate::{
    archive::{validate_name, Archive, Entry, ARCHIVE_LIMIT},
    authoring::validate_id,
    brf::Brf,
    dependencies::{references, Evidence, Location, Reference},
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
    /// Copied resources only; kept unresolved names are never mapped.
    pub mapping: Vec<(String, String)>,
    pub donor: String,
    pub id: String,
    pub notes: Vec<String>,
    /// References absent from the document and every searched catalog.
    pub unresolved: Vec<Unresolved>,
    /// Names the copied resources keep referencing instead of copying.
    pub shared: Vec<String>,
}
/// Short and long display names written into recognized identity blocks.
#[derive(Clone, Copy, Debug)]
pub struct Names<'a> {
    pub short: &'a str,
    pub long: &'a str,
}
impl<'a> From<&'a str> for Names<'a> {
    /// One title for both names, as the single-title CLI form writes it.
    fn from(title: &'a str) -> Self {
        Self {
            short: title,
            long: title,
        }
    }
}
impl<'a> From<&'a String> for Names<'a> {
    fn from(title: &'a String) -> Self {
        title.as_str().into()
    }
}
/// How the export treats one name that no searched LIB provides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// The stored name stays byte-for-byte, as in the source LIB.
    Keep,
    /// The copied resource is retargeted to this PIC, which joins the package.
    Substitute(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// Source name of the referencing resource.
    pub resource: String,
    pub target: String,
    pub evidence: Evidence,
    pub resolution: Resolution,
}
impl Unresolved {
    /// False when the name sits in an SH texture record no traversal reaches.
    pub fn drawn(&self) -> Option<bool> {
        match self.evidence {
            Evidence::Texture(reached) => reached,
            _ => None,
        }
    }
    /// Only texture names can be retargeted to another PIC.
    pub fn texture(&self) -> bool {
        ext(&self.target) == "PIC"
    }
}
/// Explicit treatment of unresolved names. The default refuses the export.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Policy {
    /// Keep every unresolved name without a substitute as in the source.
    pub keep_unresolved: bool,
    /// (referencing resource, or "" for any, unresolved name) -> PIC to use.
    pub substitutes: BTreeMap<(String, String), String>,
}
impl Policy {
    fn choice(&self, resource: &str, target: &str) -> Option<Resolution> {
        self.substitutes
            .get(&(resource.into(), target.into()))
            .or_else(|| self.substitutes.get(&(String::new(), target.into())))
            .map(|n| Resolution::Substitute(n.to_ascii_uppercase()))
            .or_else(|| self.keep_unresolved.then_some(Resolution::Keep))
    }
}
struct Node {
    bytes: Vec<u8>,
    refs: Vec<Reference>,
}
/// Names absent from the document and every searched catalog, and their policy.
#[derive(Default)]
struct Ledger {
    found: Vec<Unresolved>,
    refused: Vec<Unresolved>,
    resolutions: BTreeMap<(String, String), Resolution>,
}
impl Ledger {
    /// Records one unresolved reference; returns a substitute PIC to copy.
    fn note(
        &mut self,
        policy: &Policy,
        catalog: &BTreeSet<String>,
        resource: &str,
        target: &str,
        evidence: Evidence,
    ) -> Result<Option<String>> {
        let key = (resource.to_string(), target.to_string());
        if self.resolutions.contains_key(&key)
            || self
                .refused
                .iter()
                .any(|u| u.resource == resource && u.target == target)
        {
            return Ok(None);
        }
        if self.found.len() + self.refused.len() >= 4096 {
            return Err(invalid("Export has more than 4096 unresolved references"));
        }
        let mut entry = Unresolved {
            resource: key.0.clone(),
            target: key.1.clone(),
            evidence,
            resolution: Resolution::Keep,
        };
        let Some(resolution) = policy.choice(resource, target) else {
            self.refused.push(entry);
            return Ok(None);
        };
        let mut copy = None;
        if let Resolution::Substitute(new) = &resolution {
            validate_name(new)?;
            if ext(target) != "PIC" || ext(new) != "PIC" {
                return Err(format!(
                    "{target} in {resource} is not a texture; only PIC references can be substituted"
                ));
            }
            if !catalog.contains(new) {
                return Err(format!(
                    "Substitute {new} is not in this LIB or any searched source LIB"
                ));
            }
            copy = Some(new.clone());
        }
        entry.resolution = resolution.clone();
        self.resolutions.insert(key, resolution);
        self.found.push(entry);
        Ok(copy)
    }
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
pub fn build<'a>(
    sources: &[&Archive],
    donor: &str,
    id: &str,
    title: impl Into<Names<'a>>,
    policy: &Policy,
) -> Result<Package> {
    let mut catalog = BTreeSet::new();
    let mut providers = BTreeMap::new();
    for (si, archive) in sources.iter().enumerate() {
        for (ei, e) in archive.entries.iter().enumerate() {
            catalog.insert(e.name.clone());
            providers.entry(e.name.clone()).or_insert((si, ei));
        }
    }
    build_with(&catalog, donor, id, title, policy, |name| {
        let (si, ei) = providers
            .get(name)
            .ok_or_else(|| format!("Missing referenced resource {name}"))?;
        sources[*si].entries[*ei].read()
    })
}
fn refusal(found: &[Unresolved]) -> String {
    let mut names = Vec::new();
    for u in found.iter().take(8) {
        names.push(format!("{} in {}", u.target, u.resource));
    }
    if found.len() > 8 {
        names.push(format!("{} more", found.len() - 8));
    }
    let one = found.len() == 1;
    format!(
        "Unresolved in source: {}. No searched LIB provides {}. Add the LIB that does, {}keep {} as in the source",
        names.join(", "),
        if one { "this name" } else { "these names" },
        if found.iter().any(|u| u.texture()) {
            "substitute a packaged texture, or "
        } else {
            "or "
        },
        if one { "it" } else { "them" },
    )
}
pub fn build_with<'a>(
    catalog: &BTreeSet<String>,
    donor: &str,
    id: &str,
    names: impl Into<Names<'a>>,
    policy: &Policy,
    read: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Package> {
    build_sharing(
        catalog,
        donor,
        id,
        names.into(),
        policy,
        &BTreeSet::new(),
        read,
    )
}
/// The object graph and the roles of its reviewed roots, read without naming
/// or rewriting anything. Unresolved names are kept.
#[derive(Debug, Default)]
pub struct Graph {
    pub donor: String,
    /// Every graph resource present in the catalog, the donor included.
    pub resources: BTreeSet<String>,
    pub main: Option<String>,
    pub shadow: Option<String>,
    /// Damage-family base: `F14` for `F14_S.SH`.
    pub family: Option<String>,
    pub hud: Option<String>,
    /// The HUD is found by the donor's name (null hudName pointer).
    pub default_hud: bool,
    pub palette: Option<String>,
    pub unresolved: Vec<Unresolved>,
}
pub fn graph(
    catalog: &BTreeSet<String>,
    donor: &str,
    mut read: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Graph> {
    let donor = donor.to_ascii_uppercase();
    validate_name(&donor)?;
    let policy = Policy {
        keep_unresolved: true,
        ..Policy::default()
    };
    let c = collect(catalog, &donor, &policy, &BTreeSet::new(), &mut read)?;
    // Inserted one by one: collecting a set would use the stable sort.
    let mut resources = BTreeSet::new();
    for name in c.graph.keys() {
        resources.insert(name.clone());
    }
    Ok(Graph {
        resources,
        donor,
        main: c.main,
        shadow: c.shadow,
        family: c.family,
        default_hud: c.default_hud,
        hud: c.hud,
        palette: c.palette,
        unresolved: c.ledger.found,
    })
}
struct Collected {
    root: Option<Brf>,
    main: Option<String>,
    shadow: Option<String>,
    family: Option<String>,
    hud: Option<String>,
    default_hud: bool,
    palette: Option<String>,
    graph: BTreeMap<String, Node>,
    ledger: Ledger,
    diagnostics: BTreeSet<String>,
    /// Shared names the collected resources reference; never traversed.
    shared: BTreeSet<String>,
}
fn collect(
    catalog: &BTreeSet<String>,
    donor: &str,
    policy: &Policy,
    share: &BTreeSet<String>,
    read: &mut impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Collected> {
    if share.contains(donor) {
        return Err(invalid("The selected object itself is always copied"));
    }
    let root_bytes = read(donor)?;
    let root = if root_bytes.starts_with(b"[brent's_relocatable_format]") {
        Some(Brf::parse(&root_bytes, ext(donor))?)
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
    let family = if ext(donor) == "PT" {
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
    let mut ledger = Ledger::default();
    let mut shared = BTreeSet::new();
    // Stored root names absent everywhere are classified with the donor's references.
    let mut roots = vec![donor.to_string()];
    let mut root_name = |name: &String, roots: &mut Vec<String>| {
        if share.contains(name) {
            shared.insert(name.clone());
        } else {
            roots.push(name.clone());
        }
    };
    for name in main.iter().chain(&shadow) {
        if catalog.contains(name) {
            root_name(name, &mut roots);
        }
    }
    if let Some(family) = &family {
        for suffix in ["A", "B", "C", "D"] {
            let name = format!("{family}_{suffix}.SH");
            if catalog.contains(&name) {
                root_name(&name, &mut roots);
            } else {
                ledger.note(policy, catalog, donor, &name, Evidence::Convention)?;
            }
        }
    }
    let explicit_hud = optional_reference("object.hudName")?;
    let named_hud = explicit_hud.is_none();
    let hud = explicit_hud.or_else(|| {
        let n = format!("{}.HUD", stem(donor));
        (ext(donor) == "PT" && catalog.contains(&n)).then_some(n)
    });
    let default_hud = named_hud && hud.is_some();
    if let Some(h) = hud.as_ref().filter(|h| catalog.contains(*h)) {
        if default_hud && share.contains(h) {
            return Err(format!(
                "{h} is found by the aircraft's name; it is always copied"
            ));
        }
        root_name(h, &mut roots);
    }
    let private_palette = format!("{}.PAL", stem(donor));
    let palette = if matches!(ext(donor), "5K" | "8K" | "11K" | "22K" | "WAV" | "PAL") {
        None
    } else if catalog.contains(&private_palette) {
        Some(private_palette)
    } else if catalog.contains("PALETTE.PAL") {
        Some("PALETTE.PAL".into())
    } else {
        None
    };
    if let Some(name) = &palette {
        root_name(name, &mut roots);
    }
    let palette = palette.filter(|p| !share.contains(p));
    let mut diagnostics = BTreeSet::new();
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
            diagnostics.insert(format!(
                "{name}: opaque bytes copied unchanged; dependency discovery unavailable."
            ));
            Vec::new()
        } else {
            references(&name, &bytes, catalog, &mut diagnostics)
                .map_err(|e| format!("{name}: {e}"))?
        };
        for r in &refs {
            if share.contains(&r.target) {
                shared.insert(r.target.clone());
            } else if catalog.contains(&r.target) {
                pending.push(r.target.clone());
            } else if let Some(new) = ledger.note(policy, catalog, &name, &r.target, r.evidence)? {
                pending.push(new);
            }
        }
        // FA's ordnance menu derives the icon from the store's definition stem.
        if matches!(ext(&name), "JT" | "SEE" | "ECM" | "GAS") {
            let icon = format!("${}.PIC", stem(&name));
            if catalog.contains(&icon) {
                if share.contains(&icon) {
                    return Err(format!(
                        "{icon} follows {name}; copy or share them together"
                    ));
                }
                pending.push(icon);
            }
        }
        graph.insert(name, Node { bytes, refs });
    }
    Ok(Collected {
        root,
        main,
        shadow,
        family,
        hud,
        default_hud,
        palette,
        graph,
        ledger,
        diagnostics,
        shared,
    })
}
/// `build_with`, except that names in `share` are not copied: copied
/// resources keep their stored references to them, byte for byte. Used to
/// duplicate an aircraft inside the LIB that already holds the shared names.
pub fn build_sharing(
    catalog: &BTreeSet<String>,
    donor: &str,
    id: &str,
    names: Names<'_>,
    policy: &Policy,
    share: &BTreeSet<String>,
    mut read: impl FnMut(&str) -> Result<Vec<u8>>,
) -> Result<Package> {
    let id = validate_id(id)?;
    crate::identity::validate_display("Short name", names.short)?;
    crate::identity::validate_display("Long name", names.long)?;
    let donor = donor.to_ascii_uppercase();
    validate_name(&donor)?;
    let Collected {
        root,
        main,
        family,
        hud,
        palette,
        mut graph,
        ledger,
        mut diagnostics,
        shared,
        ..
    } = collect(catalog, &donor, policy, share, &mut read)?;
    if !ledger.refused.is_empty() {
        return Err(refusal(&ledger.refused));
    }
    let Ledger {
        found, resolutions, ..
    } = ledger;
    for ((resource, old), new) in &policy.substitutes {
        if !found.iter().any(|u| {
            &u.target == old
                && (resource.is_empty() || &u.resource == resource)
                && u.resolution == Resolution::Substitute(new.to_ascii_uppercase())
        }) {
            return Err(format!(
                "{old} is not an unresolved reference of this export; nothing to substitute"
            ));
        }
    }
    // The copied name a reference is rewritten to; None keeps the stored bytes.
    let effective = |resource: &str, r: &Reference| -> Option<String> {
        if share.contains(&r.target) {
            return None;
        }
        if catalog.contains(&r.target) {
            return Some(r.target.clone());
        }
        match resolutions.get(&(resource.to_string(), r.target.clone())) {
            Some(Resolution::Substitute(new)) => Some(new.clone()),
            _ => None,
        }
    };
    let mut budget = BTreeMap::<String, usize>::new();
    for name in graph.keys() {
        budget.insert(name.clone(), 8);
    }
    for (name, node) in &graph {
        for r in &node.refs {
            if let Location::Literal { len, stem_only, .. } = r.location {
                let Some(target) = effective(name, r) else {
                    continue;
                };
                let cap = if stem_only {
                    len
                } else {
                    len.checked_sub(ext(&target).len() + 1)
                        .ok_or("Bad literal capacity")?
                };
                let b = budget.get_mut(&target).ok_or("Unmapped reference")?;
                *b = (*b).min(cap);
            }
        }
    }
    let mut mapping = BTreeMap::new();
    let mut used = catalog.clone();
    // A kept name must never become the private name of a copied resource.
    let mut kept = BTreeSet::new();
    for u in &found {
        if u.resolution == Resolution::Keep {
            kept.insert(u.target.clone());
            used.insert(u.target.clone());
        }
    }
    assign(
        &donor,
        format!("{id}.{}", ext(&donor)),
        &mut mapping,
        &mut used,
        &budget,
    )?;
    if let Some(main) = &main {
        if main != &donor && graph.contains_key(main) {
            assign(main, format!("{id}.SH"), &mut mapping, &mut used, &budget)?;
        }
    }
    if let Some(family) = &family {
        for suffix in ["A", "B", "C", "D", "S"] {
            let name = format!("{family}_{suffix}.SH");
            if !mapping.contains_key(&name) && graph.contains_key(&name) {
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
        if !mapping.contains_key(h) && graph.contains_key(h) {
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
                    diagnostics.insert(format!(
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
        identities = crate::identity::blocks(root);
        if identities.is_empty() {
            diagnostics.insert("Display-name block unrecognized; existing text preserved.".into());
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
                if let (Location::Text(index), Some(target)) = (&r.location, effective(&name, r)) {
                    edits.insert(*index, format!("\"{}\"", mapping[&target]));
                }
            }
            if name == donor {
                for labels in &identities {
                    edits.insert(labels[0], format!("\"{}\"", names.short));
                    edits.insert(labels[1], format!("\"{}\"", names.long));
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
                    // Kept names stay byte-for-byte; substitutes use the same bounded slot.
                    let Some(source) = effective(&name, r) else {
                        continue;
                    };
                    let target = &mapping[&source];
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
            if !output_catalog.contains(&r.target)
                && !kept.contains(&r.target)
                && !shared.contains(&r.target)
            {
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
    let kept_count = found
        .iter()
        .filter(|u| u.resolution == Resolution::Keep)
        .count();
    if kept_count > 0 {
        notes.push(format!(
            "{kept_count} unresolved reference{} kept as in the source LIB; no searched LIB provides {}.",
            if kept_count == 1 { "" } else { "s" },
            if kept_count == 1 { "it" } else { "them" }
        ));
    }
    for u in &found {
        if let Resolution::Substitute(new) = &u.resolution {
            notes.push(format!(
                "{} in {} retargeted to {} ({}).",
                u.target, u.resource, new, mapping[new]
            ));
        }
    }
    notes.extend(diagnostics);
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
        unresolved: found,
        shared: shared.into_iter().collect(),
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
        let p = build(&[&a], "DEMO.PT", "NEW", "New aircraft", &Policy::default()).unwrap();
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
        let again = build(
            &[&p.archive],
            "NEW.PT",
            "NEXT",
            "Next variant",
            &Policy::default(),
        )
        .unwrap();
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
        assert!(build(&[&a], "DEMO.PT", "DEMO", "Same", &Policy::default()).is_err());
        a.entries.retain(|e| e.name != "SFX.11K");
        let e = build(&[&a], "DEMO.PT", "NEW", "New", &Policy::default()).unwrap_err();
        assert!(e.contains("SFX.11K"));
        let mut extra = Archive::empty();
        extra
            .entries
            .push(Entry::new("SFX.11K", vec![128]).unwrap());
        assert!(build(&[&a, &extra], "DEMO.PT", "NEW", "New", &Policy::default()).is_ok());
    }
    /// `source()` plus a second aircraft; both C damage shapes draw `name`,
    /// which no catalog provides, in a reached E2 record.
    fn dangling(name: &str) -> (Archive, Vec<u8>) {
        let mut a = source();
        let mut ghost = crate::model::demo_textured();
        assert_eq!(&ghost[256..258], [0xe2, 0]);
        ghost[258..272].fill(0);
        ghost[258..258 + name.len()].copy_from_slice(name.as_bytes());
        let pt = a.entries[a.find("DEMO.PT").unwrap()].read().unwrap();
        let two = String::from_utf8(pt).unwrap().replace("DEMO", "TWO");
        a.entries
            .push(Entry::new("TWO.PT", two.into_bytes()).unwrap());
        for (from, to) in [("DEMO.SH", "TWO.SH"), ("DEMO.HUD", "TWO.HUD")] {
            let bytes = a.entries[a.find(from).unwrap()].read().unwrap();
            a.entries.push(Entry::new(to, bytes).unwrap());
        }
        for suffix in ["A", "B", "C", "D", "S"] {
            let bytes = if suffix == "C" {
                ghost.clone()
            } else {
                crate::model::demo_shape()
            };
            let at = a.find(&format!("DEMO_{suffix}.SH")).unwrap();
            a.entries[at] = Entry::new(&format!("DEMO_{suffix}.SH"), bytes.clone()).unwrap();
            a.entries
                .push(Entry::new(&format!("TWO_{suffix}.SH"), bytes).unwrap());
        }
        (a, ghost)
    }
    fn read(p: &Package, name: &str) -> Vec<u8> {
        p.archive.entries[p.archive.find(name).unwrap()]
            .read()
            .unwrap()
    }
    fn keep() -> Policy {
        Policy {
            keep_unresolved: true,
            ..Policy::default()
        }
    }
    #[test]
    fn unresolved_names_refuse_by_default_and_keep_preserves_bytes() {
        let (a, ghost) = dangling("GHOST.PIC");
        let before = a.bytes().unwrap();
        let e = build(&[&a], "DEMO.PT", "NEW", "New", &Policy::default()).unwrap_err();
        assert!(
            e.starts_with("Unresolved in source: GHOST.PIC in DEMO_C.SH"),
            "{e}"
        );
        assert!(e.contains("Add the LIB") && e.contains("substitute"), "{e}");
        // A source LIB that provides the name resolves it; nothing is unresolved.
        let mut extra = Archive::empty();
        extra
            .entries
            .push(Entry::new("GHOST.PIC", crate::picture::demo()).unwrap());
        let p = build(&[&a, &extra], "DEMO.PT", "NEW", "New", &Policy::default()).unwrap();
        assert!(p.unresolved.is_empty());
        assert!(p.mapping.iter().any(|(old, _)| old == "GHOST.PIC"));

        let p = build(&[&a], "DEMO.PT", "NEW", "New", &keep()).unwrap();
        assert_eq!(
            p.unresolved,
            [Unresolved {
                resource: "DEMO_C.SH".into(),
                target: "GHOST.PIC".into(),
                evidence: Evidence::Texture(Some(true)),
                resolution: Resolution::Keep,
            }]
        );
        assert_eq!(p.unresolved[0].drawn(), Some(true));
        // Not renamed, not copied, not a private resource.
        assert!(p
            .mapping
            .iter()
            .all(|(old, new)| old != "GHOST.PIC" && new != "GHOST.PIC"));
        assert!(p.archive.find("GHOST.PIC").is_none());
        assert_eq!(read(&p, "NEW_C.SH"), ghost);
        assert!(p
            .notes
            .iter()
            .any(|n| n.contains("1 unresolved reference kept")));
        assert_eq!(a.bytes().unwrap(), before);
        // Bytes outside the decoded records keep the bounded heuristic; their
        // reachability is unknown rather than guessed.
        let mut a = a;
        a.entries.retain(|e| e.name != "HIDDEN.PIC");
        let p = build(&[&a], "DEMO.PT", "NEW", "New", &keep()).unwrap();
        let hidden = p
            .unresolved
            .iter()
            .find(|u| u.target == "HIDDEN.PIC")
            .unwrap();
        assert_eq!(
            (hidden.resource.as_str(), hidden.drawn()),
            ("DEMO.SH", None)
        );
        assert_eq!(p.unresolved.len(), 2);
    }
    #[test]
    fn substitute_patches_only_the_copied_name_slot() {
        let (a, ghost) = dangling("GHOST.PIC");
        let before = a.bytes().unwrap();
        let mut policy = Policy::default();
        policy.substitutes.insert(
            ("DEMO_C.SH".into(), "GHOST.PIC".into()),
            "symbol.pic".into(),
        );
        let p = build(&[&a], "DEMO.PT", "NEW", "New", &policy).unwrap();
        let mapping: BTreeMap<_, _> = p.mapping.iter().cloned().collect();
        // The substitute was not otherwise referenced; it joins the package.
        let private = &mapping["SYMBOL.PIC"];
        assert_eq!(read(&p, private), crate::picture::demo());
        assert_eq!(
            p.unresolved[0].resolution,
            Resolution::Substitute("SYMBOL.PIC".into())
        );
        let after = read(&p, "NEW_C.SH");
        assert_eq!(after.len(), ghost.len());
        let slot = 256 + 2..256 + 16;
        for (i, (x, y)) in ghost.iter().zip(&after).enumerate() {
            assert!(x == y || slot.contains(&i), "byte {i} changed");
        }
        let mut name = private.as_bytes().to_vec();
        name.resize(14, 0);
        assert_eq!(after[slot], name[..]);
        assert!(crate::model::Model::parse(&after)
            .unwrap()
            .textures
            .contains(private));
        assert!(p
            .notes
            .iter()
            .any(|n| n.contains("retargeted to SYMBOL.PIC")));
        // The second aircraft and the source keep the stored name.
        assert_eq!(a.bytes().unwrap(), before);
        assert_eq!(
            a.entries[a.find("TWO_C.SH").unwrap()].read().unwrap(),
            ghost
        );
        let two = build(&[&a], "TWO.PT", "NEXT", "Next", &keep()).unwrap();
        assert_eq!(read(&two, "NEXT_C.SH"), ghost);
        assert_eq!(two.unresolved[0].resource, "TWO_C.SH");
        // Any-resource substitutes apply to every reference of the name.
        let mut any = Policy::default();
        any.substitutes
            .insert((String::new(), "GHOST.PIC".into()), "DEMO.PIC".into());
        let p = build(&[&a], "DEMO.PT", "NEW", "New", &any).unwrap();
        let mapping: BTreeMap<_, _> = p.mapping.iter().cloned().collect();
        assert!(crate::model::Model::parse(&read(&p, "NEW_C.SH"))
            .unwrap()
            .textures
            .contains(&mapping["DEMO.PIC"]));
    }
    #[test]
    fn substitutes_are_bounded_and_explicit() {
        let (a, _) = dangling("GHOST.PIC");
        let fail = |policy: Policy, old: &str, new: &str| {
            let mut policy = policy;
            policy
                .substitutes
                .insert((String::new(), old.into()), new.into());
            build(&[&a], "DEMO.PT", "NEW", "New", &policy).unwrap_err()
        };
        let none = Policy::default;
        assert!(fail(none(), "GHOST.PIC", "SFX.11K").contains("only PIC references"));
        assert!(fail(none(), "GHOST.PIC", "NOWHERE.PIC").contains("not in this LIB"));
        // A typo does not resolve GHOST.PIC, and is reported rather than ignored.
        assert!(fail(none(), "GOST.PIC", "DEMO.PIC").contains("Unresolved in source"));
        let e = fail(keep(), "GOST.PIC", "DEMO.PIC");
        assert!(e.contains("GOST.PIC is not an unresolved reference"), "{e}");
    }
    #[test]
    fn kept_names_are_reserved_and_collisions_still_block() {
        // NEW0.PIC is the first private name the generator would choose.
        let (a, _) = dangling("NEW0.PIC");
        let p = build(&[&a], "DEMO.PT", "NEW", "New", &keep()).unwrap();
        assert_eq!(p.unresolved[0].target, "NEW0.PIC");
        assert!(p.mapping.iter().all(|(_, new)| new != "NEW0.PIC"));
        assert!(p.archive.find("NEW0.PIC").is_none());
        // A kept name equal to a fixed private name is a collision, not an overwrite.
        let (a, _) = dangling("NEW.PAL");
        let e = build(&[&a], "DEMO.PT", "NEW", "New", &keep()).unwrap_err();
        assert!(e.contains("Name collision: NEW.PAL"), "{e}");
        let (a, _) = dangling("GHOST.PIC");
        assert!(build(&[&a], "DEMO.PT", "DEMO", "Same", &keep()).is_err());
        assert!(build(&[&a], "DEMO.PT", "TWO", "Same", &keep()).is_err());
    }
    #[test]
    fn missing_family_members_are_unresolved_conventions() {
        let mut a = source();
        a.entries.retain(|e| e.name != "DEMO_D.SH");
        let e = build(&[&a], "DEMO.PT", "NEW", "New", &Policy::default()).unwrap_err();
        assert!(
            e.contains("DEMO_D.SH in DEMO.PT") && !e.contains("substitute"),
            "{e}"
        );
        let p = build(&[&a], "DEMO.PT", "NEW", "New", &keep()).unwrap();
        assert_eq!(p.unresolved[0].evidence, Evidence::Convention);
        assert!(p.archive.find("NEW_D.SH").is_none());
        assert!(p.archive.find("NEW_C.SH").is_some());
        assert!(p.mapping.iter().all(|(old, _)| old != "DEMO_D.SH"));
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
        let out = build(&[&a], "OLD.JT", "NEW", "New missile", &Policy::default()).unwrap();
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
        // Separate short and long names land in their own operands of every block.
        let names = Names {
            short: "NEW",
            long: "New missile",
        };
        let out = build(&[&a], "OLD.JT", "NEW", names, &Policy::default()).unwrap();
        let bytes = out.archive.entries[out.archive.find("NEW.JT").unwrap()]
            .read()
            .unwrap();
        let b = Brf::parse(&bytes, "JT").unwrap();
        let blocks = crate::identity::blocks(&b);
        assert_eq!(blocks.len(), 2);
        for [s, l, r] in blocks {
            assert_eq!(b.fields[s].value, "\"NEW\"");
            assert_eq!(b.fields[l].value, "\"New missile\"");
            assert_eq!(b.fields[r].value, "\"NEW.JT\"");
        }
        let long = Names {
            short: "NEW",
            long: "a;b",
        };
        let e = build(&[&a], "OLD.JT", "NEW", long, &Policy::default()).unwrap_err();
        assert!(e.starts_with("Long name must be"), "{e}");
    }
    #[test]
    fn leaf_and_opaque_resources_export_without_inventing_dependencies() {
        let mut a = Archive::empty();
        a.entries = vec![
            Entry::new("SOUND.11K", b"BAD.SH\0".to_vec()).unwrap(),
            Entry::new("OPAQUE.BIN", vec![1, 2, 3]).unwrap(),
        ];
        let out = build(&[&a], "SOUND.11K", "NEW", "Sound", &Policy::default()).unwrap();
        assert_eq!(out.archive.entries.len(), 1);
        assert_eq!(out.archive.entries[0].read().unwrap(), b"BAD.SH\0");
        let out = build(&[&a], "OPAQUE.BIN", "NEW", "Unknown", &Policy::default()).unwrap();
        assert_eq!(out.archive.entries[0].read().unwrap(), vec![1, 2, 3]);
        assert!(out.notes.iter().any(|s| s.contains("opaque")));
    }
}
