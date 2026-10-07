//! The colour a runtime marking sits on, for the sheet Make paintable
//! creates. The new sheet is a solid fill (index 255 transparency is not
//! verified in the game), so it must match the surface under the marking or
//! a painted roundel shows a square of the wrong colour around it.
//!
//! The surface is the drawn face behind the marking face: every corner
//! within `TOLERANCE` units of the marking's plane, the marking's centre
//! inside it when both are projected along the marking's normal, and not a
//! marking itself. Its colour at the centre is the face's flat colour, or
//! the texel its UVs map the centre to (barycentric over a fan, integers).
//! Among several, the nearest plane wins, faces turned the same way before
//! faces turned away. With none, the shape's skin index, then the colour
//! the marking faces store.
use crate::{
    model::{Face, Model},
    picture::Pic,
    shape_geometry::{pose_drawing, Geometry},
    shape_markings::{markings_of, Marking},
};
use alloc::{collections::BTreeMap, collections::BTreeSet, string::String, vec::Vec};

/// How far a face may stand from the marking's plane and still be the
/// surface under it, in the shape's own units.
pub const TOLERANCE: i128 = 12;

/// How the sheet's panel area is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    /// The colour of the surface under the marking.
    Surface,
    /// The colour the marking faces store.
    Panel,
    /// A game palette index.
    Index(u8),
}
/// Where a fill colour came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillSource {
    /// A face under the marking, by file offset.
    Surface(usize),
    /// The shape's skin index: nothing was found under the marking.
    Skin,
    /// The colour the marking faces store.
    Stored,
    /// Chosen.
    Picked,
}
/// A fill colour and where it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fill {
    pub color: u8,
    pub source: FillSource,
}
/// Texture bytes by name, as the caller's LIB holds them.
pub type Textures<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

type P = [i128; 3];
fn point(p: [i32; 3]) -> P {
    p.map(|v| v as i128)
}
fn sub(a: P, b: P) -> P {
    core::array::from_fn(|k| a[k] - b[k])
}
fn dot(a: P, b: P) -> i128 {
    (0..3).map(|k| a[k] * b[k]).sum()
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
/// Newell normal of a polygon, unnormalised.
fn newell(points: &[P]) -> P {
    let mut n = [0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}
fn corners(m: &Model, f: &Face) -> Option<Vec<P>> {
    f.indices
        .iter()
        .map(|i| m.vertices.get(*i).map(|v| point(v.point)))
        .collect()
}
/// Where `at` falls on the polygon when both are projected along `d`:
/// the UV there (or `[0, 0]` for a face without UVs), or `None` outside.
fn through(face: &[P], uv: &[[i32; 2]], at: P, d: P) -> Option<[i128; 2]> {
    for t in 1..face.len().saturating_sub(1) {
        let (a, b, c) = (face[0], face[t], face[t + 1]);
        let total = dot(d, cross(sub(b, a), sub(c, a)));
        if total == 0 {
            continue;
        }
        let w = [
            dot(d, cross(sub(b, at), sub(c, at))),
            dot(d, cross(sub(c, at), sub(a, at))),
            dot(d, cross(sub(a, at), sub(b, at))),
        ];
        let s = total.signum();
        if w.iter().any(|x| x * s < 0) {
            continue;
        }
        if uv.len() != face.len() {
            return Some([0, 0]);
        }
        let at_uv = |k: usize| -> i128 {
            let q = [uv[0], uv[t], uv[t + 1]];
            let sum: i128 = (0..3).map(|j| w[j] * q[j][k] as i128).sum::<i128>() * s;
            // Rounded to the nearest texel.
            (2 * sum + total.abs()).div_euclid(2 * total.abs())
        };
        return Some([at_uv(0), at_uv(1)]);
    }
    None
}
/// Whether a face draws a named texture through its UVs (a flat face keeps
/// the current texture name but has no UVs).
fn textured(f: &Face) -> bool {
    !f.texture.is_empty() && !f.uv.is_empty() && f.uv.len() == f.indices.len()
}
/// The commonest of `colors`, lowest index first on a tie.
fn commonest(colors: impl Iterator<Item = u8>) -> Option<u8> {
    let mut counts = [0usize; 256];
    let mut any = false;
    for c in colors {
        counts[c as usize] += 1;
        any = true;
    }
    any.then(|| {
        (0..256)
            .max_by_key(|i| (counts[*i], core::cmp::Reverse(*i)))
            .unwrap_or(0) as u8
    })
}

/// Colour of the surface under one marking face of `m`, with the surface's
/// file offset.
fn under(
    m: &Model,
    marking: &Face,
    skip: &BTreeSet<usize>,
    pics: &mut BTreeMap<String, Option<Pic>>,
    textures: Textures,
) -> Option<(u8, usize)> {
    let mine = corners(m, marking)?;
    if mine.len() < 3 {
        return None;
    }
    let d = newell(&mine);
    let len = mine.len() as i128;
    let centre: P = core::array::from_fn(|k| mine.iter().map(|p| p[k]).sum::<i128>() / len);
    let norm = dot(d, d);
    if norm == 0 {
        return None;
    }
    // (turned away, squared offset from the plane, face, corners)
    let mut found: Vec<(bool, i128, usize)> = Vec::new();
    for (i, f) in m.faces.iter().enumerate() {
        if skip.contains(&f.offset) || f.offset == marking.offset {
            continue;
        }
        let Some(theirs) = corners(m, f).filter(|c| c.len() >= 3) else {
            continue;
        };
        let off = theirs
            .iter()
            .map(|p| {
                let s = dot(d, sub(*p, centre));
                s * s
            })
            .max()
            .unwrap_or(0);
        if off > TOLERANCE * TOLERANCE * norm {
            continue;
        }
        if through(&theirs, &[], centre, d).is_none() {
            continue;
        }
        found.push((dot(d, newell(&theirs)) < 0, off, i));
    }
    found.sort_unstable();
    for (_, _, i) in found {
        let f = &m.faces[i];
        let Some(theirs) = corners(m, f) else {
            continue;
        };
        if !textured(f) {
            return Some((f.color, f.offset));
        }
        let pic = pics
            .entry(f.texture.to_ascii_uppercase())
            .or_insert_with(|| textures(&f.texture).and_then(|b| Pic::parse(&b).ok()));
        let Some(pic) = pic else {
            continue;
        };
        let Some([u, v]) = through(&theirs, &f.uv, centre, d) else {
            continue;
        };
        let col = u.clamp(0, pic.width as i128 - 1) as usize;
        let row = (pic.height as i128 - 1 - v).clamp(0, pic.height as i128 - 1) as usize;
        if pic.mask.get(row * pic.width + col) == Some(&true) {
            return Some((pic.pixels[row * pic.width + col], f.offset));
        }
    }
    None
}
/// The shape's skin index: the commonest colour among its untextured
/// faces, else the commonest index of the texture most faces draw.
fn skin(
    m: &Model,
    skip: &BTreeSet<usize>,
    pics: &mut BTreeMap<String, Option<Pic>>,
    textures: Textures,
) -> Option<u8> {
    let drawn = || m.faces.iter().filter(|f| !skip.contains(&f.offset));
    if let Some(c) = commonest(drawn().filter(|f| !textured(f)).map(|f| f.color)) {
        return Some(c);
    }
    let mut uses: BTreeMap<String, usize> = BTreeMap::new();
    for f in drawn() {
        *uses.entry(f.texture.to_ascii_uppercase()).or_insert(0) += 1;
    }
    let (name, _) = uses
        .into_iter()
        .max_by_key(|(n, c)| (*c, core::cmp::Reverse(n.clone())))?;
    let pic = pics
        .entry(name.clone())
        .or_insert_with(|| textures(&name).and_then(|b| Pic::parse(&b).ok()))
        .as_ref()?;
    commonest(
        pic.pixels
            .iter()
            .zip(&pic.mask)
            .filter(|(_, m)| **m)
            .map(|(p, _)| *p),
    )
}
/// The fill `mode` gives the faces of `row` (a slot of the shape `source`
/// that `g` parsed). The surface under every face is looked up; the
/// commonest colour wins.
pub(crate) fn fill_for(
    source: &[u8],
    g: &Geometry,
    row: &Marking,
    mode: FillMode,
    textures: Textures,
) -> Fill {
    let stored = || {
        let colors = row
            .faces
            .iter()
            .filter_map(|o| g.face_at(*o))
            .map(|i| (g.faces[i].color & 0xff) as u8);
        commonest(colors).unwrap_or(0)
    };
    match mode {
        FillMode::Index(color) => {
            return Fill {
                color,
                source: FillSource::Picked,
            }
        }
        FillMode::Panel => {
            return Fill {
                color: stored(),
                source: FillSource::Stored,
            }
        }
        FillMode::Surface => {}
    }
    let mut skip = BTreeSet::new();
    for r in markings_of(g) {
        skip.extend(r.faces.iter().copied());
        skip.extend(r.painted.iter().map(|p| p.0));
        skip.extend(r.contested.iter().map(|c| c.0));
    }
    let mut pics = BTreeMap::new();
    let mut seen: Vec<(u8, usize)> = Vec::new();
    for o in &row.faces {
        let Some((_, m)) = pose_drawing(source, &g.inventory, *o) else {
            continue;
        };
        if let Some(f) = m.faces.iter().find(|f| f.offset == *o) {
            seen.extend(under(&m, f, &skip, &mut pics, textures));
        }
    }
    if let Some(color) = commonest(seen.iter().map(|s| s.0)) {
        let at = seen.iter().find(|s| s.0 == color).map_or(0, |s| s.1);
        return Fill {
            color,
            source: FillSource::Surface(at),
        };
    }
    let skin = Model::parse(source)
        .ok()
        .and_then(|m| skin(&m, &skip, &mut pics, textures));
    match skin {
        Some(color) => Fill {
            color,
            source: FillSource::Skin,
        },
        None => Fill {
            color: stored(),
            source: FillSource::Stored,
        },
    }
}
/// The fill `mode` gives `slot` of a shape, without changing it.
pub fn fill_preview(
    source: &[u8],
    slot: u16,
    mode: FillMode,
    textures: Textures,
) -> crate::Result<Fill> {
    let g = Geometry::parse(source)?;
    let row = markings_of(&g)
        .into_iter()
        .find(|r| r.slot == slot)
        .ok_or_else(|| alloc::format!("No E0 record selects slot {slot}"))?;
    if row.faces.is_empty() {
        return Err(alloc::format!("Slot {slot} draws no textured face"));
    }
    Ok(fill_for(source, &g, &row, mode, textures))
}
