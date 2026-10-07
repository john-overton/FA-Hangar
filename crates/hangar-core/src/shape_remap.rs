//! Remap from view: give the selected faces a new raw PIC laid out as the
//! viewport shows them. The faces' corners go through the viewport camera
//! (`model::view_point`, orthographic), so the new texture is the screen
//! image of the panels: screen right is U, screen up is V (stored V runs up,
//! so PIC row 0 is the top of the view). Texels are square at the shape's
//! texel density, which undoes UVs that stretch a panel in space.
//!
//! **Bake** fills each texel the faces cover from what they draw now: the
//! texel centre is located in the face's new UV triangle (the same fan the
//! renderer uses) by integer barycentrics, which give the point's old UVs,
//! and the old PIC is sampled there (nearest, palette index). Flat faces
//! bake their colour. Where faces overlap in the view, the one nearest the
//! viewer wins, as on screen. **Blank** fills the panels with their
//! dominant baked index. Texels outside the panels repeat the nearest panel
//! texel for `MARGIN + 1` pixels, then take the dominant index.
//!
//! The faces are then drawn from the new PIC through
//! `shape_texture::assign_texture_uvs`, with its proofs and verification.
use crate::{
    invalid,
    model::{view_point, Model, Pose},
    picture::Pic,
    shape_edit::raw_sheet,
    shape_texture::{assign_texture_uvs, Fit, PANEL_MAX, PANEL_MIN},
    Result,
};
use alloc::{collections::BTreeMap, string::String, vec::Vec};

/// The camera and pose the faces are seen in, as the viewport draws them.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub yaw: i32,
    pub pitch: i32,
    pub pose: &'a Pose,
}
/// How the new PIC is filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// Each texel from what the face draws there now.
    Bake,
    /// The panels' dominant baked index (a flat face's colour).
    Blank,
}
/// The pictures and palette the faces draw with now.
pub struct Sources<'a> {
    /// PICs by upper-case name with extension (`X.PIC`).
    pub textures: &'a BTreeMap<String, Pic>,
    /// Base palette (8-bit RGB): flat colours and PICs without a full one.
    pub palette: &'a [[u8; 3]; 256],
}
/// Pixels kept clear around the panels on each side.
pub const MARGIN: u32 = 2;
/// A face's projected area must be at least 1/EDGE_ON of its true area
/// (it faces the view within about 75 degrees).
pub const EDGE_ON: i128 = 4;
/// Projection scale: 1/256 source unit, as the renderer's fixed point.
const SCALE: i64 = 256;

/// What a face draws now.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Old {
    Texture {
        name: String,
        uv: Vec<[i32; 2]>,
        keyed: bool,
    },
    Flat(u8),
}
/// A planned remap: the new PIC size and every face's new UVs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The faces (file offsets), each once, in request order.
    pub faces: Vec<usize>,
    pub size: [u32; 2],
    /// Texel density used, Q16 texels per source unit.
    pub density: u32,
    /// New UVs per face, one per corner in record order (V as stored).
    pub uv: Vec<Vec<[i32; 2]>>,
    /// What each face draws now.
    old: Vec<Old>,
    /// View depth per corner (larger is nearer the viewer).
    depth: Vec<Vec<i64>>,
}
/// Result of a remap.
#[derive(Clone, Debug)]
pub struct Remapped {
    pub shape: Vec<u8>,
    /// The new PIC (raw kind 0, full palette).
    pub picture: Vec<u8>,
    /// New file offsets of `Plan::faces`, in order.
    pub faces: Vec<usize>,
    pub size: [u32; 2],
    pub density: u32,
    /// Texels inside the panels, and those two or more panels share.
    pub covered: usize,
    pub shared: usize,
    /// The panels' dominant index (Blank's fill, the margin's fallback).
    pub dominant: u8,
}
/// `X.PIC` for a face's texture name as stored.
pub fn texture_key(name: &str) -> String {
    let n = name.trim().to_ascii_uppercase();
    if n.contains('.') {
        n
    } else {
        n + ".PIC"
    }
}
fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << (128 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}
/// Twice the area of a polygon in 3D (Newell's method), integer.
fn area3(points: &[[i64; 3]]) -> i128 {
    let mut n = [0i128; 3];
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        for (k, axis) in n.iter_mut().enumerate() {
            let (y, z) = ((k + 1) % 3, (k + 2) % 3);
            *axis += (a[y] as i128 - b[y] as i128) * (a[z] as i128 + b[z] as i128);
        }
    }
    isqrt(n.iter().map(|v| (v * v) as u128).sum()) as i128
}
/// Twice the signed area of a 2D polygon.
fn area2(points: &[[i64; 2]]) -> i128 {
    let mut s = 0i128;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        s += a[0] as i128 * b[1] as i128 - b[0] as i128 * a[1] as i128;
    }
    s
}
/// Plan a remap: project the faces as the view shows them, refuse faces
/// seen edge-on or from behind, and fit the layout into a PIC with square
/// texels (`Fit::Density`, clamped to 8..256 per side with a margin, or a
/// given `Fit::Size`).
pub fn remap_plan(source: &[u8], faces: &[usize], view: &View, fit: Fit) -> Result<Plan> {
    if faces.is_empty() {
        return Err(invalid("Select panels to remap"));
    }
    if faces.len() > 4096 {
        return Err(invalid("Remap at most 4096 faces at a time"));
    }
    let model = Model::with_pose(source, view.pose)?;
    let mut unique = Vec::new();
    for o in faces {
        if !unique.contains(o) {
            unique.push(*o);
        }
    }
    let mut projected: Vec<Vec<[i64; 3]>> = Vec::new();
    let mut old = Vec::new();
    for o in &unique {
        let f = model
            .faces
            .iter()
            .find(|f| f.offset == *o)
            .ok_or_else(|| format!("Face at {o:X} is not drawn in this pose"))?;
        if f.sub & 4 != 0 && f.texture.is_empty() {
            return Err(format!(
                "Face at {o:X} draws runtime markings (no named texture); it cannot be remapped"
            ));
        }
        let points: Vec<[i32; 3]> = f
            .indices
            .iter()
            .map(|i| model.vertices.get(*i).map(|v| v.point))
            .collect::<Option<_>>()
            .ok_or("Face corner missing")?;
        if points.iter().flatten().any(|v| v.unsigned_abs() > 1 << 17) {
            return Err(format!("Face at {o:X} lies too far from the origin"));
        }
        let q: Vec<[i64; 3]> = points
            .iter()
            .map(|p| {
                view_point(view.yaw, view.pitch, p.map(|v| v * SCALE as i32)).map(|v| v as i64)
            })
            .collect();
        let true_area = area3(
            &points
                .iter()
                .map(|p| p.map(|v| v as i64 * SCALE))
                .collect::<Vec<_>>(),
        );
        let seen = area2(&q.iter().map(|p| [p[0], p[1]]).collect::<Vec<_>>());
        if true_area == 0 {
            return Err(format!("Face at {o:X} is degenerate"));
        }
        if EDGE_ON * seen.abs() < true_area {
            return Err(format!(
                "Face at {o:X} is nearly edge-on to the view; turn the view to face the panel"
            ));
        }
        // A face the renderer culls is seen from behind: its texture would
        // read mirrored from the front. Faces without a stored normal are
        // drawn from both sides and map as seen.
        if f.normal
            .is_some_and(|n| view_point(view.yaw, view.pitch, n)[2] < 0)
        {
            return Err(format!(
                "Face at {o:X} faces away from the view; turn the view to face the panel"
            ));
        }
        old.push(if f.sub & 4 != 0 && f.uv.len() == f.indices.len() {
            Old::Texture {
                name: texture_key(&f.texture),
                uv: f.uv.clone(),
                keyed: f.sub & 8 != 0,
            }
        } else {
            Old::Flat(f.color)
        });
        projected.push(q);
    }
    let mut lo = [i64::MAX; 2];
    let mut hi = [i64::MIN; 2];
    for p in projected.iter().flatten() {
        for k in 0..2 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let e = [(hi[0] - lo[0]) as i128, (hi[1] - lo[1]) as i128];
    if e[0] <= 0 || e[1] <= 0 {
        return Err(invalid("The panels have no extent in this view"));
    }
    let m = MARGIN as i128;
    let (min, max) = (PANEL_MIN as i128, PANEL_MAX as i128);
    let (size, content) = match fit {
        Fit::Density(d) => {
            let d = d.max(1) as i128;
            // Proj units are 1/256 unit; density is Q16: 2^24 in all.
            let mut c = e.map(|x| ((x * d + (1 << 24) - 1) >> 24).max(1));
            let big = c[0].max(c[1]);
            let inner = max - 2 * m;
            if big > inner {
                c = c.map(|x| ((x * inner + big / 2) / big).max(1));
            }
            let size = c.map(|x| (x + 2 * m).clamp(min, max) as u32);
            (size, c)
        }
        Fit::Size(s) => {
            if s.iter().any(|v| !(PANEL_MIN..=PANEL_MAX).contains(v)) {
                return Err(format!(
                    "Remapped textures are {PANEL_MIN} to {PANEL_MAX} pixels a side"
                ));
            }
            (s, s.map(|v| v as i128 - 2 * m))
        }
    };
    // One scale for both axes, the tighter one: square texels.
    let (num, den) = if content[0] * e[1] <= content[1] * e[0] {
        (content[0], e[0])
    } else {
        (content[1], e[1])
    };
    let map = |x: i128| -> i128 { (2 * x * num + den) / (2 * den) };
    let off = [0, 1].map(|k| m + (content[k] - map(e[k])).max(0) / 2);
    let density = ((num << 24) / den).clamp(1, u32::MAX as i128) as u32;
    let uv: Vec<Vec<[i32; 2]>> = projected
        .iter()
        .map(|q| {
            q.iter()
                .map(|p| {
                    [
                        (off[0] + map((p[0] - lo[0]) as i128)) as i32,
                        (off[1] + map((p[1] - lo[1]) as i128)) as i32,
                    ]
                })
                .collect()
        })
        .collect();
    if uv
        .iter()
        .flatten()
        .any(|p| p[0] >= size[0] as i32 || p[1] >= size[1] as i32)
    {
        return Err(invalid("The layout exceeds the texture size"));
    }
    Ok(Plan {
        faces: unique,
        size,
        density,
        uv,
        old,
        depth: projected
            .iter()
            .map(|q| q.iter().map(|p| p[2]).collect())
            .collect(),
    })
}
/// Nearest palette entry by RGB distance (ties: the lowest index).
fn nearest(palette: &[[u8; 3]; 256], rgb: [u8; 3]) -> u8 {
    (0..256)
        .min_by_key(|i| {
            (0..3)
                .map(|k| (palette[*i][k] as i32 - rgb[k] as i32).pow(2))
                .sum::<i32>()
        })
        .unwrap_or(0) as u8
}
/// Index map from `from` colours into `to`: identity where they agree.
fn translate(from: &[[u8; 3]; 256], to: &[[u8; 3]; 256]) -> [u8; 256] {
    core::array::from_fn(|i| {
        if from[i] == to[i] {
            i as u8
        } else {
            nearest(to, from[i])
        }
    })
}
/// The baked raster (row 0 at the top), its palette and coverage counts.
struct Baked {
    pixels: Vec<u8>,
    palette: [[u8; 3]; 256],
    covered: usize,
    shared: usize,
    dominant: u8,
}
fn bake(plan: &Plan, fill: Fill, sources: &Sources) -> Result<Baked> {
    let base = sources.palette;
    let pic = |name: &str| -> Result<&Pic> {
        sources.textures.get(name).ok_or_else(|| {
            format!("{name} is not loaded; open the LIB that holds it to remap its faces")
        })
    };
    // The palette of the first textured face's PIC, else the base palette.
    let palette = match plan.old.iter().find_map(|o| match o {
        Old::Texture { name, .. } => Some(name),
        Old::Flat(_) => None,
    }) {
        Some(name) => pic(name)?.colors(base),
        None => *base,
    };
    let flat = translate(base, &palette);
    let [w, h] = plan.size.map(|v| v as usize);
    let mut out = alloc::vec![0u8; w * h];
    let mut depth = alloc::vec![i64::MIN; w * h];
    let mut owner = alloc::vec![usize::MAX; w * h];
    let mut seen = alloc::vec![(usize::MAX, 0u8); w * h];
    let edge = |a: [i64; 2], b: [i64; 2], x: i64, y: i64| -> i64 {
        (x - a[0]) * (b[1] - a[1]) - (y - a[1]) * (b[0] - a[0])
    };
    for (k, old) in plan.old.iter().enumerate() {
        let (src, table) = match old {
            Old::Texture { name, .. } => {
                let p = pic(name)?;
                (Some(p), translate(&p.colors(base), &palette))
            }
            Old::Flat(_) => (None, flat),
        };
        let uv = &plan.uv[k];
        let z = &plan.depth[k];
        for j in 1..uv.len().saturating_sub(1) {
            let ids = [0, j, j + 1];
            let t = ids.map(|i| [2 * uv[i][0] as i64, 2 * uv[i][1] as i64]);
            let area = edge(t[0], t[1], t[2][0], t[2][1]);
            if area == 0 {
                continue;
            }
            let sign = area.signum();
            let area = area.abs();
            let range = |c: usize, n: usize| -> (usize, usize) {
                let lo = ids.iter().map(|i| uv[*i][c]).min().unwrap_or(0).max(0) as usize;
                let hi = ids.iter().map(|i| uv[*i][c]).max().unwrap_or(0).max(0) as usize;
                (lo.min(n - 1), hi.min(n - 1))
            };
            let (u0, u1) = range(0, w);
            let (v0, v1) = range(1, h);
            for v in v0..=v1 {
                for u in u0..=u1 {
                    let (x, y) = (2 * u as i64 + 1, 2 * v as i64 + 1);
                    let wt = [
                        edge(t[1], t[2], x, y) * sign,
                        edge(t[2], t[0], x, y) * sign,
                        edge(t[0], t[1], x, y) * sign,
                    ];
                    if wt.iter().any(|q| *q < 0) {
                        continue;
                    }
                    let i = (h - 1 - v) * w + u;
                    let d = (0..3)
                        .map(|q| wt[q] as i128 * z[ids[q]] as i128)
                        .sum::<i128>()
                        / area as i128;
                    let d = d as i64;
                    let index = match (old, src) {
                        (Old::Flat(c), _) => table[*c as usize],
                        (Old::Texture { uv: o, keyed, .. }, Some(p)) => {
                            let at = |c: usize| -> i64 {
                                (0..3)
                                    .map(|q| wt[q] as i128 * o[ids[q]][c] as i128)
                                    .sum::<i128>() as i64
                                    / area
                            };
                            let col = at(0).clamp(0, p.width as i64 - 1) as usize;
                            let row = (p.height as i64 - 1 - at(1)).clamp(0, p.height as i64 - 1)
                                as usize;
                            let pi = row * p.width + col;
                            if !p.mask[pi] {
                                continue;
                            }
                            let index = p.pixels[pi];
                            if *keyed && index == 255 {
                                255
                            } else {
                                table[index as usize]
                            }
                        }
                        _ => continue,
                    };
                    if seen[i].0 != k {
                        seen[i] = (k, seen[i].1.saturating_add(1));
                    }
                    if owner[i] != usize::MAX && d < depth[i] {
                        continue;
                    }
                    out[i] = index;
                    depth[i] = d;
                    owner[i] = k;
                }
            }
        }
    }
    let covered = owner.iter().filter(|o| **o != usize::MAX).count();
    if covered == 0 {
        return Err(invalid("The panels cover no texel of the new texture"));
    }
    let shared = seen.iter().filter(|s| s.1 >= 2).count();
    let mut counts = [0usize; 256];
    for i in 0..w * h {
        if owner[i] != usize::MAX {
            counts[out[i] as usize] += 1;
        }
    }
    let dominant = (0..256)
        .max_by_key(|i| (counts[*i], core::cmp::Reverse(*i)))
        .unwrap_or(0) as u8;
    if fill == Fill::Blank {
        for i in 0..w * h {
            if owner[i] != usize::MAX {
                out[i] = dominant;
            }
        }
    }
    // Margin: repeat the nearest panel texel, then the dominant index.
    let mut filled: Vec<bool> = owner.iter().map(|o| *o != usize::MAX).collect();
    for _ in 0..=MARGIN {
        let before = filled.clone();
        let mut next = out.clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if before[i] {
                    continue;
                }
                let around = [
                    (x > 0).then(|| i - 1),
                    (x + 1 < w).then(|| i + 1),
                    (y > 0).then(|| i - w),
                    (y + 1 < h).then(|| i + w),
                ];
                if let Some(j) = around.into_iter().flatten().find(|j| before[*j]) {
                    next[i] = out[j];
                    filled[i] = true;
                }
            }
        }
        out = next;
    }
    for i in 0..w * h {
        if !filled[i] {
            out[i] = dominant;
        }
    }
    Ok(Baked {
        pixels: out,
        palette,
        covered,
        shared,
        dominant,
    })
}
/// Remap `faces` (file offsets) from the view onto a new raw PIC `name`:
/// plan the layout, bake or fill the texture, then draw the faces from it
/// through `assign_texture_uvs` (texture state proof, region rules, one
/// continuation per run, re-parse and verification). The new UVs are
/// checked once more on the re-parsed shape in the view's pose.
pub fn remap_from_view(
    source: &[u8],
    faces: &[usize],
    name: &str,
    view: &View,
    fit: Fit,
    fill: Fill,
    sources: &Sources,
) -> Result<Remapped> {
    let plan = remap_plan(source, faces, view, fit)?;
    let baked = bake(&plan, fill, sources)?;
    let picture = raw_sheet(plan.size, &baked.pixels, &baked.palette)?;
    let assigned = assign_texture_uvs(source, &plan.faces, name, &plan.uv)?;
    let key = texture_key(name);
    let model = Model::with_pose(&assigned.shape, view.pose)?;
    for (k, o) in assigned.faces.iter().enumerate() {
        let f = model
            .faces
            .iter()
            .find(|f| f.offset == *o)
            .ok_or("A remapped face is no longer drawn")?;
        if texture_key(&f.texture) != key || f.uv != plan.uv[k] {
            return Err(invalid(
                "A remapped face did not re-parse with its new texture and UVs",
            ));
        }
    }
    let pic = Pic::parse(&picture)?;
    if [pic.width as u32, pic.height as u32] != plan.size || !pic.paintable {
        return Err(invalid(
            "The new texture did not re-parse as a paintable PIC",
        ));
    }
    Ok(Remapped {
        shape: assigned.shape,
        picture,
        faces: assigned.faces,
        size: plan.size,
        density: plan.density,
        covered: baked.covered,
        shared: baked.shared,
        dominant: baked.dominant,
    })
}

#[cfg(test)]
#[path = "shape_remap_tests.rs"]
mod tests;
