//! Create an isolated aircraft variant from a donor PT and an imported main SH.
//! This is an overlay for the donor game's installation, not a new flight model.
use crate::{
    archive::{validate_name, Archive, Entry},
    brf::Brf,
    invalid,
    model::Model,
    Result,
};
use alloc::{string::String, vec::Vec};
#[derive(Debug)]
pub struct Variant {
    pub archive: Archive,
    pub donor: String,
    pub shared: Vec<String>,
    pub missing_textures: Vec<String>,
}
fn reference<'a>(brf: &'a Brf, field: &str) -> Result<&'a str> {
    let f = brf
        .fields
        .iter()
        .find(|f| f.label == field && f.kind == "ptr")
        .ok_or_else(|| {
            invalid("Donor must have a recognized PT schema and explicit shape/name references")
        })?;
    Ok(&f.value)
}
fn string_block(brf: &Brf, block: &str) -> Vec<usize> {
    brf.fields
        .iter()
        .enumerate()
        .filter_map(|(i, f)| (f.block == block && f.kind == "string").then_some(i))
        .collect()
}
fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}
pub fn validate_id(id: &str) -> Result<String> {
    let id = id.to_ascii_uppercase();
    if id.is_empty() || id.len() > 6 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        return Err(invalid(
            "Use 1..6 letters, digits or underscore; suffixes reserve two characters",
        ));
    }
    validate_name(&format!("{id}.PT"))?;
    Ok(id)
}
pub fn create(
    source: &Archive,
    donor: &str,
    shape: Vec<u8>,
    id: &str,
    title: &str,
) -> Result<Variant> {
    let id = validate_id(id)?;
    if title.is_empty()
        || title.len() > 40
        || !title.is_ascii()
        || title.contains(['"', ';', '\r', '\n'])
        || title.bytes().any(|c| c < 32)
    {
        return Err(invalid(
            "Display name must be 1..40 plain ASCII characters, without quotes or semicolons",
        ));
    }
    let donor = donor.to_ascii_uppercase();
    if !donor.ends_with(".PT") {
        return Err(invalid(
            "Select an aircraft PT donor; generic object creation is not implemented yet",
        ));
    }
    let at = source
        .find(&donor)
        .ok_or_else(|| invalid("Donor PT not found"))?;
    let mut bytes = source.entries[at].read()?;
    let brf = Brf::parse(&bytes, "PT")?;
    let names = string_block(&brf, reference(&brf, "object.ot_names")?);
    let main = string_block(&brf, reference(&brf, "object.shape")?);
    let shadow = string_block(&brf, reference(&brf, "object.shadowShape")?);
    if names.len() != 3 || main.len() != 1 || shadow.len() != 1 {
        return Err(invalid("Unsupported donor name/shape block layout"));
    }
    let shadow_name = unquote(&brf.fields[shadow[0]].value).to_ascii_uppercase();
    let stem = shadow_name
        .strip_suffix("_S.SH")
        .ok_or_else(|| invalid("Donor shadow must use the reviewed _S.SH family convention"))?;
    let model = Model::parse(&shape)?;
    let names_to_create: Vec<String> = core::iter::once(format!("{id}.PT"))
        .chain(core::iter::once(format!("{id}.SH")))
        .chain(
            ["A", "B", "C", "D", "S"]
                .into_iter()
                .map(|suffix| format!("{id}_{suffix}.SH")),
        )
        .collect();
    if names_to_create.iter().any(|n| source.find(n).is_some()) {
        return Err(invalid("Variant name collides with an existing resource"));
    }
    let mut aliases = Vec::new();
    for suffix in ["A", "B", "C", "D", "S"] {
        let from = format!("{stem}_{suffix}.SH");
        let at = source.find(&from).ok_or_else(|| {
            format!("Missing donor companion {from}; cannot build a complete aircraft family")
        })?;
        aliases.push(Entry::new(
            &format!("{id}_{suffix}.SH"),
            source.entries[at].read()?,
        )?);
    }
    // Descending ranges prevent earlier edits from invalidating later positions.
    let mut edits = vec![
        (names[0], format!("\"{title}\"")),
        (names[1], format!("\"{title}\"")),
        (names[2], format!("\"{id}.PT\"")),
        (main[0], format!("\"{id}.SH\"")),
        (shadow[0], format!("\"{id}_S.SH\"")),
    ];
    edits.sort_by_key(|(i, _)| core::cmp::Reverse(*i));
    for (i, value) in edits {
        let current = Brf::parse(&bytes, "PT")?;
        bytes = current.edit(&bytes, i, &value, "PT")?;
    }
    let mut archive = Archive::empty();
    archive
        .entries
        .push(Entry::new(&format!("{id}.PT"), bytes)?);
    archive
        .entries
        .push(Entry::new(&format!("{id}.SH"), shape)?);
    archive.entries.extend(aliases);
    let mut missing_textures = Vec::new();
    // These are references observed during bounded static-pose traversal, not
    // proof of complete animation/LOD dependency closure.
    for name in &model.textures {
        let name = if name.contains('.') {
            name.clone()
        } else {
            format!("{name}.PIC")
        };
        if archive.find(&name).is_some() {
            continue;
        }
        if let Some(at) = source.find(&name) {
            archive
                .entries
                .push(Entry::new(&name, source.entries[at].read()?)?);
        } else {
            missing_textures.push(name);
        }
    }
    let current = Brf::parse(&archive.entries[0].read()?, "PT")?;
    let mut shared = Vec::new();
    for f in current.fields.iter().filter(|f| f.kind == "string") {
        let n = unquote(&f.value).to_ascii_uppercase();
        if validate_name(&n).is_ok() && archive.find(&n).is_none() && !shared.contains(&n) {
            shared.push(n);
        }
    }
    archive.bytes()?;
    Ok(Variant {
        archive,
        donor,
        shared,
        missing_textures,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn donor() -> Archive {
        let s = String::from_utf8(crate::brf::demo())
            .unwrap()
            .replace("DEMO", "DONOR");
        let mut a = Archive::empty();
        a.entries
            .push(Entry::new("DONOR.PT", s.into_bytes()).unwrap());
        for suffix in ["", "_A", "_B", "_C", "_D", "_S"] {
            a.entries.push(
                Entry::new(&format!("DONOR{suffix}.SH"), crate::model::demo_shape()).unwrap(),
            );
        }
        a
    }
    #[test]
    fn new_family_roundtrip_and_donor_unchanged() {
        let a = donor();
        let before = a.bytes().unwrap();
        let v = create(
            &a,
            "DONOR.PT",
            crate::model::demo_shape(),
            "newjet",
            "New aircraft",
        )
        .unwrap();
        assert_eq!(v.archive.entries.len(), 7);
        assert!(v.shared.is_empty());
        assert!(v.missing_textures.is_empty());
        let packed = Archive::parse(v.archive.bytes().unwrap()).unwrap();
        let pt = Brf::parse(&packed.entries[0].read().unwrap(), "PT").unwrap();
        assert!(pt.fields.iter().any(|f| f.value == "\"NEWJET_S.SH\""));
        assert!(pt.fields.iter().any(|f| f.value == "\"NEWJET.PT\""));
        for suffix in ["A", "B", "C", "D", "S"] {
            assert_eq!(
                packed.entries[packed.find(&format!("NEWJET_{suffix}.SH")).unwrap()]
                    .read()
                    .unwrap(),
                a.entries[a.find(&format!("DONOR_{suffix}.SH")).unwrap()]
                    .read()
                    .unwrap()
            );
        }
        assert_eq!(a.bytes().unwrap(), before);
    }
    #[test]
    fn missing_family_collision_invalid_shape_and_id_fail() {
        let mut a = donor();
        assert!(create(&a, "DONOR.PT", crate::model::demo_shape(), "DONOR", "X").is_err());
        assert!(validate_id("TOOLONG").is_err());
        assert!(create(&a, "DONOR.PT", vec![0], "NEW", "X").is_err());
        a.entries.pop();
        assert!(create(&a, "DONOR.PT", crate::model::demo_shape(), "NEW", "X").is_err());
    }
}
