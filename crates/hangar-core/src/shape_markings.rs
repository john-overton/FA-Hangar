//! Runtime markings: faces drawn from a runtime texture slot. An `E0 00
//! <u16 slot>` record selects the slot for the textured faces that follow,
//! until the next E2/E0; the game fills the slot with an image at run time.
//! The community SH guide names slots 0/1 left/right tail art, 2 nose art
//! and 3/4 left/right wing markings.
//!
//! The shape decides whether a marking is drawn, where and from which slot,
//! so those are what can be edited, each as a pure function from SH bytes
//! to SH bytes, re-parsed before it returns:
//!
//! - **Hide** replaces each face record with a same-size jump stub. Each run
//!   of contiguous hidden faces gets one block before the end marker,
//!   `48 back to the end of the run; stored originals`: the first site jumps
//!   to the block, later sites to their own end. The originals are never
//!   reached; they make the block self-describing, so **Show** puts the
//!   exact records back after save and reopen. A block is recognised only
//!   structurally (its sites hold exactly the stub bytes, the first jumping
//!   to it, and the originals' lengths add up to the run), never by a marker.
//! - **Reassign** changes the slot word of the slot's E0 records in place,
//!   to 0..=4 only.
//! - **Make paintable** draws the slot's faces from a new PIC in the retail
//!   texture layout through a texture-assignment continuation; its restore
//!   selector is the E0 itself, so the slot stays recorded and **Restore**
//!   is Use shape texture.
//!
//! Faces whose texture state is not proved to be one E0 slot are never
//! edited: a face drawn under a slot on one path and another texture on
//! another is refused.
use crate::{
    invalid,
    model::{Model, Pose},
    picture::{retail_texture, TEXTURE_WIDTH},
    shape_code::{Kind, Target},
    shape_edit::{continuation_start, jump, replace_continuation},
    shape_geometry::{others_unchanged, pose_drawing, verify_structure, Frame, Geometry},
    shape_texture::{
        assign_texture_uvs, assignments, atlas_density, planar, restore_texture_assignment,
        stub_bytes, Fit, Plane, DEFAULT_DENSITY,
    },
    u16_at, Result,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::String,
    vec::Vec,
};

/// Slots the community guide names; reassignment is limited to these.
pub const SLOT_COUNT: u16 = 5;
/// Name of a runtime texture slot.
pub fn slot_name(slot: u16) -> &'static str {
    match slot {
        0 => "Tail art left",
        1 => "Tail art right",
        2 => "Nose art",
        3 => "Wing marking left",
        4 => "Wing marking right",
        _ => "Unnamed slot",
    }
}
/// One runtime texture slot a shape selects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marking {
    pub slot: u16,
    /// CODE offsets of the E0 records that select this slot.
    pub records: Vec<usize>,
    /// File offsets of textured faces proved to draw from this slot.
    pub faces: Vec<usize>,
    /// File offsets of hidden faces (where their records stood).
    pub hidden: Vec<usize>,
    /// File offsets of faces made paintable: drawn from a Hangar texture
    /// assignment that restores this slot, with the texture they draw.
    pub painted: Vec<(usize, String)>,
    /// File offsets of faces drawn from this slot on some paths and with
    /// another texture on others, with the reason. Never edited.
    pub contested: Vec<(usize, String)>,
}
impl Marking {
    /// Faces of any state, for counts.
    pub fn total(&self) -> usize {
        self.faces.len() + self.hidden.len() + self.painted.len()
    }
}
/// A Hangar hide block found in a shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hidden {
    /// CODE span from the `48` back to the end of the stored originals.
    pub start: usize,
    pub end: usize,
    /// CODE spans of the hidden faces' sites, now jump stubs.
    pub sites: Vec<(usize, usize)>,
    /// CODE offsets of the stored original records, in site order.
    pub originals: Vec<usize>,
}
/// Slot word of an E0 selector's bytes.
fn slot_of(bytes: &[u8]) -> Option<u16> {
    (bytes.first() == Some(&0xe0) && bytes.len() == 4)
        .then(|| u16_at(bytes, 2).ok().map(|v| v as u16))?
}
/// Recognise the hide block whose `48` starts at CODE offset `t`.
fn recognise(g: &Geometry, t: usize) -> Option<Hidden> {
    let inv = &g.inventory;
    let i = inv.starting_at(t)?;
    let back = &inv.records[i];
    if back.kind != Kind::Sh(0x48) || back.len != 4 {
        return None;
    }
    let ret = back.pointers.iter().find_map(|p| match p.target {
        Target::Code(x) => Some(x),
        _ => None,
    })?;
    let mut originals = Vec::new();
    let mut size = 0;
    let mut j = i + 1;
    while let Some(o) = inv.records.get(j).filter(|o| o.kind == Kind::Sh(0xfc)) {
        originals.push((o.offset, o.len));
        size += o.len;
        j += 1;
        if originals.len() > 4096 {
            return None;
        }
    }
    if originals.is_empty() {
        return None;
    }
    let end = originals.last().map(|(o, l)| o + l)?;
    let mut a = ret.checked_sub(size)?;
    let mut sites = Vec::new();
    for (k, (_, len)) in originals.iter().enumerate() {
        let e = a + len;
        if a < end && t < e {
            return None;
        }
        let to = if k == 0 { t } else { e };
        if g.code.get(a..e)? != stub_bytes(a, e, to).ok()?.as_slice() {
            return None;
        }
        sites.push((a, e));
        a = e;
    }
    Some(Hidden {
        start: t,
        end,
        sites,
        originals: originals.into_iter().map(|(o, _)| o).collect(),
    })
}
/// Every Hangar hide block in an analysed shape.
pub fn hidden_blocks(g: &Geometry) -> Vec<Hidden> {
    g.inventory
        .records
        .iter()
        .filter(|r| r.kind == Kind::Sh(0x48))
        .filter_map(|r| recognise(g, r.offset))
        .collect()
}
fn marking(out: &mut BTreeMap<u16, Marking>, slot: u16) -> &mut Marking {
    out.entry(slot).or_insert_with(|| Marking {
        slot,
        ..Marking::default()
    })
}
/// The runtime slots a shape selects, with the faces drawn from each, the
/// faces Hangar hid or made paintable, and contested faces. Slots without
/// an E0 record are absent.
pub fn markings(source: &[u8]) -> Result<Vec<Marking>> {
    let g = Geometry::parse(source)?;
    Ok(markings_of(&g))
}
pub(crate) fn markings_of(g: &Geometry) -> Vec<Marking> {
    let cs = g.inventory.code_start;
    let mut out: BTreeMap<u16, Marking> = BTreeMap::new();
    for r in g
        .inventory
        .records
        .iter()
        .filter(|r| r.kind == Kind::Sh(0xe0))
    {
        if let Some(s) = slot_of(g.selector(r.offset)) {
            marking(&mut out, s).records.push(r.offset);
        }
    }
    if out.is_empty() {
        return Vec::new();
    }
    let found = assignments(g);
    let copies: BTreeSet<usize> = found
        .iter()
        .flat_map(|a| a.copies.iter().copied())
        .collect();
    for a in &found {
        if let Some(s) = slot_of(&g.code[a.restore.0..a.restore.1]) {
            for c in &a.copies {
                marking(&mut out, s)
                    .painted
                    .push((cs + c, a.texture.clone()));
            }
        }
    }
    for (i, f) in g.faces.iter().enumerate() {
        if f.content & 4 == 0 || f.frame == Frame::Unreached || copies.contains(&(f.offset - cs)) {
            continue;
        }
        match g.material(i) {
            Ok(at) => {
                if let Some(s) = slot_of(g.selector(at)) {
                    marking(&mut out, s).faces.push(f.offset);
                }
            }
            Err(why) => {
                for at in g.material_writers(i).unwrap_or_default() {
                    if let Some(s) = slot_of(g.selector(at)) {
                        let m = marking(&mut out, s);
                        if m.contested.last().map(|c| c.0) != Some(f.offset) {
                            m.contested.push((f.offset, why.clone()));
                        }
                    }
                }
            }
        }
    }
    let blocks = hidden_blocks(g);
    let sites: Vec<usize> = blocks
        .iter()
        .flat_map(|b| b.sites.iter().map(|s| s.0))
        .collect();
    for (at, state) in sites.iter().zip(g.state_at(&sites)) {
        if let Some(s) = state.ok().and_then(|x| slot_of(g.selector(x))) {
            marking(&mut out, s).hidden.push(cs + at);
        }
    }
    out.into_values().collect()
}

/// Why a face (file offset) cannot be hidden, as a refusal.
fn hide_refusal(g: &Geometry, offset: usize) -> core::result::Result<(usize, u16), String> {
    let cs = g.inventory.code_start;
    let i = g
        .face_at(offset)
        .ok_or_else(|| format!("No FC face record starts at {offset:X}"))?;
    if let Some(e) = g.face_refusal(i) {
        return Err(format!("Face at {offset:X}: {e}"));
    }
    let code = offset - cs;
    if g.inventory.bindings.iter().any(|b| b.target == code) {
        return Err(format!(
            "Face at {offset:X}: a part stub resumes drawing at this face, so it cannot be hidden"
        ));
    }
    if g.faces[i].content & 4 == 0 {
        return Err(format!(
            "Face at {offset:X} is untextured; it draws no runtime marking"
        ));
    }
    match g.material(i) {
        Ok(at) => slot_of(g.selector(at)).map(|s| (i, s)).ok_or_else(|| {
            format!("Face at {offset:X} draws a named texture, not a runtime marking")
        }),
        Err(why) => {
            let slot = g
                .material_writers(i)
                .unwrap_or_default()
                .into_iter()
                .find_map(|at| slot_of(g.selector(at)));
            Err(match slot {
                Some(s) => format!(
                    "Face at {offset:X} draws slot {s} on some paths and another texture on others ({why}); hiding it would hide both"
                ),
                None => format!(
                    "Face at {offset:X}: Hangar cannot prove it draws a runtime marking: {why}"
                ),
            })
        }
    }
}
fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < b.1 && b.0 < a.1
}
/// A face to hide: its site and the record stored in its block.
struct Site {
    site: (usize, usize),
    original: Vec<u8>,
}
/// A rebuilt shape, the hidden runs' sites, and the CODE offsets of face
/// records whose bytes moved or went.
type Rebuilt = (Vec<u8>, Vec<Vec<(usize, usize)>>, BTreeSet<usize>);
/// One UV list per face.
type Uvs = Vec<Vec<[i32; 2]>>;
/// Hide blocks that end the appended records: from the end marker back,
/// each block followed only by 1E bytes up to the next one. Their bytes
/// are rewritten together, so hiding and then showing everything removes
/// exactly what the hides added.
fn tail_blocks(g: &Geometry, blocks: &[Hidden], marker: usize) -> (usize, BTreeSet<usize>) {
    let mut from = marker;
    let mut out = BTreeSet::new();
    for _ in 0..blocks.len() {
        let next = blocks.iter().enumerate().find(|(k, b)| {
            !out.contains(k)
                && b.end <= from
                && g.code
                    .get(b.end..from)
                    .is_some_and(|r| r.iter().all(|x| *x == 0x1e))
        });
        match next {
            Some((k, b)) => {
                from = b.start;
                out.insert(k);
            }
            None => break,
        }
    }
    (from, out)
}
/// Lay out the edit: dissolve the `touched` blocks and the blocks that end
/// the appended records, put records back at `puts`, and hide every `hide`
/// site (and every untouched site of a dissolved block), one block per
/// contiguous run that nothing enters in the middle. Runs that do not fit
/// in a dissolved block's freed bytes go after the remaining records, in
/// place of the tail blocks, so the end marker moves back when they shrink.
fn rebuild(
    source: &[u8],
    g: &Geometry,
    blocks: &[Hidden],
    touched: &BTreeSet<usize>,
    mut hide: Vec<Site>,
    puts: &[((usize, usize), Vec<u8>)],
) -> Result<Rebuilt> {
    let marker = match g.inventory.end_marker {
        Some(_) => Some(continuation_start(source)?),
        None => None,
    };
    let (from, tail) = match marker {
        Some(m) => tail_blocks(g, blocks, m),
        None => (g.code.len(), BTreeSet::new()),
    };
    let shown: BTreeSet<usize> = puts.iter().map(|p| p.0 .0).collect();
    for k in tail.difference(touched) {
        for (j, site) in blocks[*k].sites.iter().enumerate() {
            let o = blocks[*k].originals[j];
            hide.push(Site {
                site: *site,
                original: g.code[o..o + site.1 - site.0].to_vec(),
            });
        }
    }
    let dissolved: Vec<&Hidden> = touched.union(&tail).map(|k| &blocks[*k]).collect();
    let mut taken: Vec<(usize, usize)> = dissolved.iter().map(|b| (b.start, b.end)).collect();
    if let Some(m) = marker {
        taken.push((from, m));
    }
    let spans: Vec<(usize, usize)> = hide
        .iter()
        .map(|h| h.site)
        .chain(puts.iter().map(|p| p.0))
        .collect();
    for (k, s) in spans.iter().enumerate() {
        if spans[..k].iter().chain(&taken).any(|x| overlaps(*x, *s)) {
            return Err(invalid("Two faces of this edit share bytes"));
        }
    }
    let stubs: BTreeSet<usize> = dissolved.iter().map(|b| b.sites[0].0 + 2).collect();
    let backs: BTreeSet<usize> = dissolved.iter().map(|b| b.start + 2).collect();
    for (a, e) in &taken {
        for (t, field) in &g.targets {
            if (a..e).contains(&t) && !stubs.contains(field) && !(a..e).contains(&field) {
                return Err(format!(
                    "CODE+{field:X} jumps into the hidden faces at CODE+{a:X}; they cannot be rebuilt"
                ));
            }
        }
        for (f, _) in &g.fields {
            if (a..e).contains(&f) && !backs.contains(f) {
                return Err(format!(
                    "The hidden faces at CODE+{a:X} carry a pointer at CODE+{f:X}; they cannot be rebuilt"
                ));
            }
        }
        if g.data.iter().any(|(t, w)| t < e && t + w > *a) {
            return Err(format!(
                "Native code addresses the hidden faces at CODE+{a:X}"
            ));
        }
    }
    taken.extend(&spans);
    let inside = |at: usize| taken.iter().any(|(a, e)| (*a..*e).contains(&at));
    let mut payload = g.code.clone();
    let mut free: Vec<(usize, usize)> = Vec::new();
    for b in &dissolved {
        payload[b.start..b.end].fill(0x1e);
        if b.start < from {
            free.push((b.start, b.end));
        }
    }
    for (site, bytes) in puts {
        if bytes.len() != site.1 - site.0 {
            return Err(invalid("A shown face no longer fits its site"));
        }
        payload[site.0..site.1].copy_from_slice(bytes);
    }
    hide.retain(|h| !shown.contains(&h.site.0));
    hide.sort_unstable_by_key(|h| h.site.0);
    let entered = |at: usize| g.targets.iter().any(|(t, f)| *t == at && !inside(*f));
    let mut runs: Vec<Vec<Site>> = Vec::new();
    for h in hide {
        match runs.last_mut() {
            Some(run) if run.last().is_some_and(|p| p.site.1 == h.site.0) && !entered(h.site.0) => {
                run.push(h)
            }
            _ => runs.push(alloc::vec![h]),
        }
    }
    let mut ext = Vec::new();
    for run in &runs {
        let size = 4 + run.iter().map(|h| h.original.len()).sum::<usize>();
        let at = match free.iter_mut().find(|(a, e)| e - a >= size) {
            Some((a, _)) => {
                let at = *a;
                *a += size;
                at
            }
            None if marker.is_some() => from + ext.len(),
            None => {
                return Err(invalid(
                    "Hiding faces needs the native end marker and import tail",
                ))
            }
        };
        let end = run.last().map_or(0, |h| h.site.1);
        let mut b = jump(at, end)?.to_vec();
        for h in run {
            b.extend(&h.original);
        }
        if at >= from {
            ext.extend(b);
        } else {
            payload[at..at + size].copy_from_slice(&b);
        }
        for (k, h) in run.iter().enumerate() {
            let to = if k == 0 { at } else { h.site.1 };
            payload[h.site.0..h.site.1].copy_from_slice(&stub_bytes(h.site.0, h.site.1, to)?);
        }
    }
    let out = match marker {
        Some(m) if from < m || !ext.is_empty() => replace_continuation(source, payload, from, ext)?,
        _ => g.install(source, &payload)?,
    };
    let sites = runs
        .iter()
        .map(|r| r.iter().map(|h| h.site).collect())
        .collect();
    // Records whose bytes moved or went: the dissolved blocks' originals and
    // whatever stood after `from`.
    let cs = g.inventory.code_start;
    let moved = g
        .faces
        .iter()
        .map(|f| f.offset - cs)
        .filter(|at| *at >= from || dissolved.iter().any(|b| (b.start..b.end).contains(at)))
        .collect();
    Ok((out, sites, moved))
}
type Key = (
    usize,
    String,
    Vec<[i32; 3]>,
    Vec<[i32; 2]>,
    u8,
    u8,
    Option<usize>,
);
fn keys(m: &Model, cs: usize, skip: &BTreeSet<usize>) -> Vec<Key> {
    let mut out: Vec<Key> = m
        .faces
        .iter()
        .filter(|f| !skip.contains(&(f.offset - cs)))
        .map(|f| {
            let points = f
                .indices
                .iter()
                .map(|i| m.vertices.get(*i).map_or([0; 3], |v| v.point))
                .collect();
            let part = f.part.and_then(|p| m.parts.get(p)).map(|p| p.offset - cs);
            (
                f.offset - cs,
                f.texture.to_ascii_uppercase(),
                points,
                f.uv.clone(),
                f.sub,
                f.color,
                part,
            )
        })
        .collect();
    out.sort_unstable();
    out
}
/// Re-parse a hide/show edit: structure, the expected hide blocks, every
/// other face record unchanged, and in the neutral pose and each pose that
/// draws a changed face, the drawn faces equal apart from `gone` (CODE
/// offsets drawn before, not after) and `back` (drawn after, not before).
fn verify(
    source: &[u8],
    g: &Geometry,
    out: &[u8],
    runs: &[Vec<(usize, usize)>],
    changed: &BTreeSet<usize>,
    gone: &BTreeSet<usize>,
    back: &BTreeSet<usize>,
) -> Result<Geometry> {
    let after = verify_structure(g, source, out)?;
    let blocks = hidden_blocks(&after);
    for run in runs {
        if !blocks.iter().any(|b| &b.sites == run) {
            return Err(invalid(
                "Hidden faces did not re-parse as a Hangar hide block",
            ));
        }
    }
    if blocks
        .iter()
        .flat_map(|b| &b.sites)
        .any(|s| back.contains(&s.0))
    {
        return Err(invalid("A shown face is still hidden"));
    }
    others_unchanged(g, &after, changed)?;
    let (cs, cs2) = (g.inventory.code_start, after.inventory.code_start);
    let mut poses: Vec<Pose> = alloc::vec![Pose::new()];
    for at in gone {
        if let Some((pose, _)) = pose_drawing(source, &g.inventory, cs + at) {
            if !poses.contains(&pose) && poses.len() < 16 {
                poses.push(pose);
            }
        }
    }
    for at in back {
        if let Some((pose, _)) = pose_drawing(out, &after.inventory, cs2 + at) {
            if !poses.contains(&pose) && poses.len() < 16 {
                poses.push(pose);
            }
        }
    }
    for pose in &poses {
        let Ok(before) = Model::with_pose(source, pose) else {
            continue;
        };
        let now = Model::with_pose(out, pose)
            .map_err(|e| format!("The edited shape no longer parses in a checked pose: {e}"))?;
        if keys(&before, cs, gone) != keys(&now, cs2, back) {
            return Err(invalid(
                "Hiding or showing markings changed another drawn face",
            ));
        }
    }
    Ok(after)
}
/// Hide the given decal faces (file offsets): each record becomes a jump
/// stub, its original kept in a hide block. Faces not proved to draw a
/// runtime slot are refused with the reason.
pub fn hide_faces(source: &[u8], faces: &[usize]) -> Result<Vec<u8>> {
    if faces.is_empty() {
        return Err(invalid("Select runtime marking faces to hide"));
    }
    if faces.len() > 4096 {
        return Err(invalid("Hide at most 4096 faces at a time"));
    }
    let g = Geometry::parse(source)?;
    let mut hide = Vec::new();
    let mut seen = BTreeSet::new();
    for offset in faces {
        if !seen.insert(*offset) {
            continue;
        }
        let (i, _) = hide_refusal(&g, *offset)?;
        let site = g.span(i);
        hide.push(Site {
            site,
            original: g.code[site.0..site.1].to_vec(),
        });
    }
    let gone: BTreeSet<usize> = hide.iter().map(|h| h.site.0).collect();
    let blocks = hidden_blocks(&g);
    let (out, runs, mut changed) = rebuild(source, &g, &blocks, &BTreeSet::new(), hide, &[])?;
    changed.extend(&gone);
    let after = verify(source, &g, &out, &runs, &changed, &gone, &BTreeSet::new())?;
    // Each hidden record moved into its block: the record count is kept.
    if after.faces.len() != g.faces.len() {
        return Err(invalid("Hiding did not keep every face record"));
    }
    Ok(out)
}
/// Show hidden faces again (file offsets of their sites): each stored
/// original goes back to its site byte for byte; the rest of its block is
/// rebuilt. Faces Hangar did not hide are refused.
pub fn show_faces(source: &[u8], sites: &[usize]) -> Result<Vec<u8>> {
    if sites.is_empty() {
        return Err(invalid("Select hidden markings to show"));
    }
    let g = Geometry::parse(source)?;
    let cs = g.inventory.code_start;
    let blocks = hidden_blocks(&g);
    let mut index = BTreeMap::new();
    for (b, x) in blocks.iter().enumerate() {
        for (k, s) in x.sites.iter().enumerate() {
            index.insert(s.0, (b, k));
        }
    }
    let mut touched: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for o in sites {
        let (b, k) = o
            .checked_sub(cs)
            .and_then(|c| index.get(&c))
            .ok_or_else(|| format!("Face at {o:X}: no faces Hangar hid start there"))?;
        touched.entry(*b).or_default().insert(*k);
    }
    let mut puts = Vec::new();
    let mut hide = Vec::new();
    let mut back = BTreeSet::new();
    for (b, shown) in &touched {
        let x = &blocks[*b];
        for (k, site) in x.sites.iter().enumerate() {
            let len = site.1 - site.0;
            let original = g.code[x.originals[k]..x.originals[k] + len].to_vec();
            if shown.contains(&k) {
                back.insert(site.0);
                puts.push((*site, original));
            } else {
                hide.push(Site {
                    site: *site,
                    original,
                });
            }
        }
    }
    let keys: BTreeSet<usize> = touched.keys().copied().collect();
    let (out, runs, changed) = rebuild(source, &g, &blocks, &keys, hide, &puts)?;
    let after = verify(source, &g, &out, &runs, &changed, &BTreeSet::new(), &back)?;
    for (site, bytes) in &puts {
        let i = after
            .face_at(after.inventory.code_start + site.0)
            .ok_or("A shown face did not re-parse")?;
        if after.code.get(site.0..site.1) != Some(bytes.as_slice())
            || after.faces[i].len != bytes.len()
        {
            return Err(invalid("A shown face did not get its exact record back"));
        }
    }
    Ok(out)
}
fn slot_row(g: &Geometry, slot: u16) -> Result<Marking> {
    markings_of(g)
        .into_iter()
        .find(|m| m.slot == slot)
        .ok_or_else(|| format!("No E0 record selects slot {slot}"))
}
/// Hide every face drawn from `slot`. Contested faces are refused.
pub fn hide_slot(source: &[u8], slot: u16) -> Result<Vec<u8>> {
    let g = Geometry::parse(source)?;
    let m = slot_row(&g, slot)?;
    if let Some((o, why)) = m.contested.first() {
        return Err(format!(
            "Face at {o:X} draws slot {slot} on some paths and another texture on others ({why}); hiding it would hide both"
        ));
    }
    if m.faces.is_empty() {
        return Err(if m.hidden.is_empty() {
            format!("Slot {slot} draws no textured face")
        } else {
            format!("Slot {slot} is already hidden")
        });
    }
    hide_faces(source, &m.faces)
}
/// Show every hidden face of `slot`.
pub fn show_slot(source: &[u8], slot: u16) -> Result<Vec<u8>> {
    let g = Geometry::parse(source)?;
    let m = slot_row(&g, slot)?;
    if m.hidden.is_empty() {
        return Err(format!("Slot {slot} has no hidden faces"));
    }
    show_faces(source, &m.hidden)
}
/// Make the E0 records of slot `from` select slot `to` instead (0..=4),
/// in place. Refused when `to` already has its own E0 record, since the
/// two rows could not be told apart afterwards.
pub fn reassign_slot(source: &[u8], from: u16, to: u16) -> Result<Vec<u8>> {
    if to >= SLOT_COUNT {
        return Err(format!(
            "Slot {to} is outside 0 to 4, the slots the game is known to fill"
        ));
    }
    let g = Geometry::parse(source)?;
    let rows = markings_of(&g);
    let row = rows
        .iter()
        .find(|m| m.slot == from)
        .ok_or_else(|| format!("No E0 record selects slot {from}"))?;
    if from == to {
        return Ok(source.to_vec());
    }
    if let Some(other) = rows.iter().find(|m| m.slot == to) {
        return Err(format!(
            "Slot {to} already has its own E0 record at CODE+{:X}; reassigning would merge the two",
            other.records[0]
        ));
    }
    let mut payload = g.code.clone();
    for at in &row.records {
        if let Some(e) = g.bytes_refusal(*at, at + 4, "this E0 record") {
            return Err(e);
        }
        payload[at + 2..at + 4].copy_from_slice(&to.to_le_bytes());
    }
    let out = g.install(source, &payload)?;
    let after = verify_structure(&g, source, &out)?;
    others_unchanged(&g, &after, &BTreeSet::new())?;
    let now = markings_of(&after);
    let moved = now.iter().find(|m| m.slot == to);
    let same = |a: &Marking, b: &Marking| {
        a.records == b.records
            && a.faces == b.faces
            && a.hidden == b.hidden
            && a.painted == b.painted
            && a.contested.len() == b.contested.len()
    };
    if now.iter().any(|m| m.slot == from) || !moved.is_some_and(|m| same(m, row)) {
        return Err(invalid(
            "Reassigning the slot did not move exactly its faces",
        ));
    }
    for (a, b) in rows
        .iter()
        .filter(|m| m.slot != from)
        .zip(now.iter().filter(|m| m.slot != to))
    {
        if !same(a, b) {
            return Err(invalid("Reassigning the slot changed another slot"));
        }
    }
    if Model::parse(source)
        .ok()
        .zip(Model::parse(&out).ok())
        .is_some_and(|(a, b)| {
            keys(&a, g.inventory.code_start, &BTreeSet::new())
                != keys(&b, after.inventory.code_start, &BTreeSet::new())
        })
    {
        return Err(invalid("Reassigning the slot changed a drawn face"));
    }
    Ok(out)
}
/// Result of Make paintable.
#[derive(Clone, Debug)]
pub struct Painted {
    pub shape: Vec<u8>,
    /// The new PIC, in the retail SH texture layout.
    pub picture: Vec<u8>,
    /// New file offsets of the slot's faces.
    pub faces: Vec<usize>,
    /// Panel area (width, height) in the 256-wide sheet.
    pub size: [u32; 2],
    /// Palette index the panel area is filled with.
    pub color: u8,
}
/// The sheet size `make_paintable` would use for a slot's faces, from the
/// shape's texel density.
pub fn paint_size(source: &[u8], slot: u16) -> Result<[u32; 2]> {
    let g = Geometry::parse(source)?;
    let m = slot_row(&g, slot)?;
    let density = Model::parse(source)
        .ok()
        .and_then(|m| atlas_density(&m))
        .unwrap_or(DEFAULT_DENSITY);
    Ok(plan_uvs(&g, &m.faces, Fit::Density(density))?.1)
}
fn plan_uvs(g: &Geometry, faces: &[usize], fit: Fit) -> Result<(Uvs, [u32; 2])> {
    if faces.is_empty() {
        return Err(invalid("The slot draws no face to make paintable"));
    }
    let mut group = Vec::new();
    for o in faces {
        let i = g.face_at(*o).ok_or("Face lost")?;
        group.push((g.face_points(i)?, g.faces[i].normal));
    }
    let p = planar(&group, Plane::Auto, fit)?;
    Ok((p.uv, p.size))
}
/// Draw every face of `slot` from a new PIC `name` instead of the runtime
/// image: planar UVs from the faces' plane at the shape's texel density
/// (or inside `size` when given, as the damage family shares one sheet),
/// the panel area filled with `color` (default: the faces' commonest
/// colour index). The faces go through `assign_texture_uvs`, so its proofs,
/// continuation rules and verification apply; the continuation restores the
/// E0, so the slot stays recorded and `restore_slot` reverses it.
pub fn make_paintable(
    source: &[u8],
    slot: u16,
    name: &str,
    size: Option<[u32; 2]>,
    color: Option<u8>,
) -> Result<Painted> {
    let g = Geometry::parse(source)?;
    let m = slot_row(&g, slot)?;
    if let Some((o, why)) = m.contested.first() {
        return Err(format!(
            "Face at {o:X} draws slot {slot} on some paths and another texture on others ({why}); it cannot be made paintable"
        ));
    }
    if m.faces.is_empty() {
        return Err(if m.hidden.is_empty() && m.painted.is_empty() {
            format!("Slot {slot} draws no textured face")
        } else if m.painted.is_empty() {
            format!("Slot {slot} is hidden; show it first")
        } else {
            format!("Slot {slot} is already paintable")
        });
    }
    let fit = match size {
        Some(s) => Fit::Size(s),
        None => Fit::Density(
            Model::parse(source)
                .ok()
                .and_then(|m| atlas_density(&m))
                .unwrap_or(DEFAULT_DENSITY),
        ),
    };
    let (uvs, size) = plan_uvs(&g, &m.faces, fit)?;
    if size[0] as usize > TEXTURE_WIDTH {
        return Err(invalid("The panel is wider than a 256-pixel texture"));
    }
    let color = color.unwrap_or_else(|| {
        let mut counts = [0usize; 256];
        for o in &m.faces {
            if let Some(i) = g.face_at(*o) {
                counts[(g.faces[i].color & 0xff) as usize] += 1;
            }
        }
        (0..256)
            .max_by_key(|i| (counts[*i], core::cmp::Reverse(*i)))
            .unwrap_or(0) as u8
    });
    let rows = (size[1] as usize).max(crate::shape_remap::MIN_ROWS as usize);
    let picture = retail_texture(rows, &alloc::vec![color; TEXTURE_WIDTH * rows])?;
    let assigned = assign_texture_uvs(source, &m.faces, name, &uvs)?;
    let after = Geometry::parse(&assigned.shape)?;
    let row = markings_of(&after).into_iter().find(|r| r.slot == slot);
    let key = name.trim().to_ascii_uppercase();
    let ok = row.is_some_and(|r| {
        r.faces.is_empty()
            && assigned
                .faces
                .iter()
                .all(|o| r.painted.iter().any(|(p, t)| p == o && *t == key))
    });
    if !ok {
        return Err(invalid(
            "The paintable faces did not re-parse on the new texture with the slot kept",
        ));
    }
    Ok(Painted {
        shape: assigned.shape,
        picture,
        faces: assigned.faces,
        size,
        color,
    })
}
/// Undo Make paintable for `slot`: the faces go back to the runtime slot
/// (Use shape texture on their continuation). Returns the shape and the
/// textures the faces drew.
pub fn restore_slot(source: &[u8], slot: u16) -> Result<(Vec<u8>, Vec<String>)> {
    let g = Geometry::parse(source)?;
    let m = slot_row(&g, slot)?;
    if m.painted.is_empty() {
        return Err(format!("Slot {slot} has no paintable faces to restore"));
    }
    let faces: Vec<usize> = m.painted.iter().map(|p| p.0).collect();
    let mut names: Vec<String> = m.painted.iter().map(|p| p.1.clone()).collect();
    names.sort_unstable();
    names.dedup();
    let cs = g.inventory.code_start;
    let first = assignments(&g)
        .into_iter()
        .filter(|a| a.copies.iter().any(|c| faces.contains(&(cs + c))))
        .map(|a| a.start)
        .min();
    let restored = restore_texture_assignment(source, &faces)?;
    // The dissolved continuation is 1E fill; when it ends the appended
    // records, drop it so the shape gets its earlier layout back.
    let shape = match first {
        Some(from) => compact(&restored.shape, from).unwrap_or(restored.shape),
        None => restored.shape,
    };
    let after = Geometry::parse(&shape)?;
    let row = markings_of(&after).into_iter().find(|r| r.slot == slot);
    if !row
        .is_some_and(|r| r.painted.is_empty() && restored.faces.iter().all(|o| r.faces.contains(o)))
    {
        return Err(invalid("The faces did not return to the runtime slot"));
    }
    Ok((shape, names))
}
/// Remove CODE bytes from `from` to the end marker when they are all 1E
/// and nothing points into them, moving the marker and import tail back.
/// The drawn model must be unchanged.
fn compact(source: &[u8], from: usize) -> Result<Vec<u8>> {
    let g = Geometry::parse(source)?;
    g.inventory.end_marker.ok_or("No end marker")?;
    let marker = continuation_start(source)?;
    if from >= marker || !g.code[from..marker].iter().all(|b| *b == 0x1e) {
        return Err(invalid("Nothing to compact"));
    }
    if g.targets.iter().any(|(t, _)| (from..marker).contains(t))
        || g.fields.iter().any(|(f, _)| (from..marker).contains(f))
        || g.data.iter().any(|(t, w)| *t < marker && t + w > from)
    {
        return Err(invalid("Something points into the padding"));
    }
    let out = replace_continuation(source, g.code.clone(), from, Vec::new())?;
    let after = verify_structure(&g, source, &out)?;
    let cs = g.inventory.code_start;
    let kept: BTreeSet<usize> = g
        .faces
        .iter()
        .map(|f| f.offset - cs)
        .filter(|a| *a >= from)
        .collect();
    others_unchanged(&g, &after, &kept)?;
    if let (Ok(a), Ok(b)) = (Model::parse(source), Model::parse(&out)) {
        if keys(&a, cs, &BTreeSet::new()) != keys(&b, after.inventory.code_start, &BTreeSet::new())
        {
            return Err(invalid("Compacting changed a drawn face"));
        }
    }
    Ok(out)
}
#[cfg(test)]
#[path = "shape_markings_tests.rs"]
mod tests;
