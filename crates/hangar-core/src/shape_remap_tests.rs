//! Synthetic fixtures for Remap from view. Never retail data.
use super::*;
use crate::shape_geometry::Geometry;
use crate::shape_testkit::Asm;
use crate::shape_texture::{assignments, restore_texture_assignment};

/// 6-bit palette values, expanded as a PAL loads them.
fn base() -> [[u8; 3]; 256] {
    core::array::from_fn(|i| {
        [i % 64, (i / 4) % 64, (i * 7) % 64].map(|v| ((v as u16 * 255 + 31) / 63) as u8)
    })
}
/// OLD.PIC, 256 x 64 in the retail layout: rows 56..63 (stored V 7..0)
/// carry `(col / 32) * 16 + V`, so U and V can both be read back from an
/// index; other rows 200.
fn old_pic() -> Pic {
    let pixels: Vec<u8> = (0..256 * 64)
        .map(|i| {
            let (col, row) = (i % 256, i / 256);
            if row >= 56 {
                ((col / 32) * 16 + (63 - row)) as u8
            } else {
                200
            }
        })
        .collect();
    Pic::parse(&retail_texture(64, &pixels).unwrap()).unwrap()
}
/// Every PIC a remap writes has the retail SH texture layout.
fn retail(out: &Remapped) -> Pic {
    assert!(is_retail_texture(&out.picture), "retail SH texture layout");
    let pic = Pic::parse(&out.picture).unwrap();
    assert_eq!(pic.width, 256);
    assert!(pic.height <= 1280 && pic.palette.is_empty());
    assert_eq!(out.picture.len(), 64 + 256 * pic.height + 4 * pic.height);
    pic
}
type Spec<'a> = (&'a [[i16; 3]], [i32; 3], Option<Vec<[u16; 2]>>, u8);
/// A shape drawing each quad from OLD.PIC (with UVs) or flat (without),
/// lit so its normal points away from `inside`.
fn shape(faces: &[Spec]) -> (Vec<u8>, Vec<usize>) {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
    a.b(&[0xe2, 0]).b(b"OLD.PIC\0\0\0\0\0\0\0");
    // One vertex buffer, so the faces follow each other.
    let mut points = Vec::new();
    let mut corners = Vec::new();
    for (p, _, _, _) in faces {
        let n = points.len() as u16;
        corners.push((n..n + p.len() as u16).collect::<Vec<_>>());
        points.extend_from_slice(p);
    }
    let list: Vec<(&[u16], [i32; 3])> = faces
        .iter()
        .zip(&corners)
        .map(|((_, inside, _, _), c)| (c.as_slice(), *inside))
        .collect();
    let uvs: Vec<Vec<[u16; 2]>> = faces
        .iter()
        .map(|(_, _, uv, _)| uv.clone().unwrap_or_default())
        .collect();
    a.solid_uv(0, &points, &list, faces[0].3, &uvs);
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    let b = a.finish();
    let at: Vec<usize> = Model::parse(&b)
        .unwrap()
        .faces
        .iter()
        .map(|f| f.offset)
        .collect();
    assert_eq!(at.len(), faces.len());
    (b, at)
}
/// The right-hand fin: 40 units forward by 40 up in x = 0, facing +x, its
/// UVs squeezing 64 x 8 texels onto it (each texel 5 units tall).
const FIN: [[i16; 3]; 4] = [[0, -20, 0], [0, 20, 0], [0, 20, 40], [0, -20, 40]];
fn fin_uv() -> Vec<[u16; 2]> {
    alloc::vec![[0, 0], [255, 0], [255, 7], [0, 7]]
}
fn view(yaw: i32, pitch: i32, pose: &Pose) -> View<'_> {
    View { yaw, pitch, pose }
}
fn remap(src: &[u8], faces: &[usize], v: &View, fit: Fit, fill: Fill) -> Result<Remapped> {
    let mut textures = BTreeMap::new();
    textures.insert(String::from("OLD.PIC"), old_pic());
    let palette = base();
    remap_from_view(
        src,
        faces,
        "NEW.PIC",
        v,
        fit,
        fill,
        &Sources {
            textures: &textures,
            palette: &palette,
        },
    )
}
fn face(b: &[u8], offset: usize) -> crate::model::Face {
    Model::parse(b)
        .unwrap()
        .faces
        .into_iter()
        .find(|f| f.offset == offset)
        .unwrap()
}
/// Texel a UV point lands on, as the renderer samples it.
fn texel(p: &Pic, u: i64, v: i64) -> u8 {
    let col = u.clamp(0, p.width as i64 - 1) as usize;
    let row = (p.height as i64 - 1 - v).clamp(0, p.height as i64 - 1) as usize;
    p.pixels[row * p.width + col]
}
/// Share of interior sample points of the face where the old and the new
/// texture show the same index: points by barycentrics over each fan
/// triangle, UVs interpolated as the renderer does.
fn agreement(
    before: &[u8],
    after: &[u8],
    old: usize,
    new: usize,
    pic: &Pic,
) -> (usize, usize, String) {
    let (a, b) = (face(before, old), face(after, new));
    let reference = old_pic();
    let n = 31i64;
    let (mut same, mut all, mut log) = (0, 0, String::new());
    for j in 1..a.uv.len() - 1 {
        let ids = [0, j, j + 1];
        for p in 1..n {
            for q in 1..n - p {
                let w = [p, q, n - p - q];
                let at = |uv: &[[i32; 2]], c: usize| -> i64 {
                    (0..3).map(|k| w[k] * uv[ids[k]][c] as i64).sum::<i64>() / n
                };
                let x = texel(&reference, at(&a.uv, 0), at(&a.uv, 1));
                let y = texel(pic, at(&b.uv, 0), at(&b.uv, 1));
                all += 1;
                same += usize::from(x == y);
                if x != y && log.len() < 600 {
                    log += &format!(
                        " old {:?}={x} new {:?}={y};",
                        [at(&a.uv, 0), at(&a.uv, 1)],
                        [at(&b.uv, 0), at(&b.uv, 1)]
                    );
                }
            }
        }
    }
    (same, all, log)
}
fn agrees(before: &[u8], out: &Remapped, old: usize, k: usize) {
    let pic = retail(out);
    let (same, all, log) = agreement(before, &out.shape, old, out.faces[k], &pic);
    assert!(
        same * 100 >= all * 85,
        "only {same} of {all} sample points match:{log}"
    );
}

#[test]
fn stretched_fin_bakes_square_texels_from_the_side() {
    let (src, at) = shape(&[(&FIN, [-1, 0, 20], Some(fin_uv()), 9)]);
    let pose = Pose::new();
    // Side view from the right: forward is screen right.
    let out = remap(
        &src,
        &at,
        &view(90, 0, &pose),
        Fit::Density(1 << 16),
        Fill::Bake,
    )
    .unwrap();
    assert_eq!(
        out.size,
        [256, 44],
        "40 x 40 units at one texel per unit, padded to 256 wide"
    );
    assert_eq!(out.density, 1 << 16);
    let f = face(&out.shape, out.faces[0]);
    assert_eq!(f.texture, "NEW.PIC");
    let us: Vec<i32> = f.uv.iter().map(|p| p[0]).collect();
    let vs: Vec<i32> = f.uv.iter().map(|p| p[1]).collect();
    assert_eq!(us.iter().max().unwrap() - us.iter().min().unwrap(), 40);
    assert_eq!(vs.iter().max().unwrap() - vs.iter().min().unwrap(), 40);
    let pic = retail(&out);
    assert!(pic.paintable);
    // Rows: V 6 at the top down to 0, 40/7 rows each (the renderer reaches
    // V 7 only on the top edge). Columns: forward (old U) to the right.
    let mut rows = [0; 8];
    for row in 2..42 {
        let line: Vec<u8> = (2..42).map(|c| pic.pixels[row * 256 + c]).collect();
        let v = line[0] % 16;
        assert!(line.iter().all(|x| x % 16 == v), "row {row} is one old V");
        rows[v as usize] += 1;
        if row > 2 {
            assert!(
                v <= pic.pixels[(row - 1) * 256 + 2] % 16,
                "V falls downwards"
            );
        }
        assert_eq!(line[0] / 16, 0, "rear on the left");
        assert_eq!(line[39] / 16, 7, "front on the right");
        assert!(line.windows(2).all(|w| w[0] / 16 <= w[1] / 16));
    }
    assert!(rows[..7].iter().all(|n| (5..=6).contains(n)), "{rows:?}");
    assert_eq!(rows[7], 0);
    // The margin repeats the edge, so nothing outside reads index 200.
    assert!(pic.pixels.iter().all(|x| *x != 200));
    assert_eq!(out.covered, 40 * 40);
    assert_eq!(out.shared, 0);
}

#[test]
fn bake_matches_the_old_look_in_rotated_views() {
    let (src, at) = shape(&[(&FIN, [-1, 0, 20], Some(fin_uv()), 9)]);
    let pose = Pose::new();
    for (yaw, pitch) in [(90, 0), (60, 25), (120, -30), (100, 40)] {
        let v = view(yaw, pitch, &pose);
        let out = remap(&src, &at, &v, Fit::Density(4 << 16), Fill::Bake).unwrap();
        agrees(&src, &out, at[0], 0);
        // The sheet takes the projected layout's aspect (square texels).
        let q: Vec<[i32; 3]> = FIN
            .iter()
            .map(|p| view_point(yaw, pitch, p.map(|x| x as i32 * 256)))
            .collect();
        let ext = |k: usize| {
            q.iter().map(|p| p[k]).max().unwrap() - q.iter().map(|p| p[k]).min().unwrap()
        };
        let f = face(&out.shape, out.faces[0]);
        let span = |k: usize| {
            (f.uv.iter().map(|p| p[k]).max().unwrap() - f.uv.iter().map(|p| p[k]).min().unwrap())
                as i64
        };
        let (w, h) = (span(0), span(1));
        assert_eq!(out.size[1] as i64, h + 4);
        assert!(
            (w * ext(1) as i64 - h * ext(0) as i64).abs() <= 2 * ext(0).max(ext(1)) as i64,
            "{yaw}/{pitch}: {w} x {h} vs {} x {}",
            ext(0),
            ext(1)
        );
    }
}

#[test]
fn mirrored_side_reads_correctly_from_its_own_front() {
    // A plate drawn from both sides; from the left only the -x face shows.
    let (src, at) = shape(&[
        (&FIN, [-1, 0, 20], Some(fin_uv()), 9),
        (&FIN, [1, 0, 20], Some(fin_uv()), 9),
    ]);
    let pose = Pose::new();
    let left = view(270, 0, &pose);
    let e = remap(&src, &at[..1], &left, Fit::Density(1 << 16), Fill::Bake).unwrap_err();
    assert!(e.contains("faces away from the view"), "{e}");
    let out = remap(&src, &at[1..], &left, Fit::Density(4 << 16), Fill::Bake).unwrap();
    agrees(&src, &out, at[1], 0);
    // From the left, forward is screen left: old U falls left to right,
    // exactly as the viewport shows it.
    // 40 units at 4 texels per unit: columns 2..162.
    let pic = retail(&out);
    let w = pic.width;
    let row = pic.height / 2;
    assert_eq!(pic.pixels[row * w + 3] / 16, 7);
    assert_eq!(pic.pixels[row * w + 160] / 16, 0);
    // The right side, seen from the right, reads the other way round.
    let right = remap(
        &src,
        &at[..1],
        &view(90, 0, &pose),
        Fit::Density(4 << 16),
        Fill::Bake,
    )
    .unwrap();
    let p = retail(&right);
    assert_eq!(p.pixels[row * w + 3] / 16, 0);
    assert_eq!(p.pixels[row * w + 160] / 16, 7);
}

#[test]
fn edge_on_faces_are_refused() {
    let (src, at) = shape(&[(&FIN, [-1, 0, 20], Some(fin_uv()), 9)]);
    let pose = Pose::new();
    for (yaw, pitch) in [(0, 0), (180, 0), (0, 90), (12, 0), (90 + 78, 0)] {
        let e = remap(
            &src,
            &at,
            &view(yaw, pitch, &pose),
            Fit::Density(1 << 16),
            Fill::Bake,
        )
        .unwrap_err();
        assert!(
            e.contains("turn the view to face the panel"),
            "{yaw}/{pitch}: {e}"
        );
    }
    assert!(remap(
        &src,
        &at,
        &view(90 + 70, 0, &pose),
        Fit::Density(1 << 16),
        Fill::Bake
    )
    .is_ok());
}

#[test]
fn neighbouring_faces_share_one_layout_and_one_continuation() {
    let rear: [[i16; 3]; 4] = [[0, -20, 0], [0, 0, 0], [0, 0, 40], [0, -20, 40]];
    let front: [[i16; 3]; 4] = [[0, 0, 0], [0, 20, 0], [0, 20, 40], [0, 0, 40]];
    let (src, at) = shape(&[
        (
            &rear,
            [-1, -10, 20],
            Some(alloc::vec![[0, 0], [127, 0], [127, 7], [0, 7]]),
            9,
        ),
        (
            &front,
            [-1, 10, 20],
            Some(alloc::vec![[128, 0], [255, 0], [255, 7], [128, 7]]),
            9,
        ),
    ]);
    let pose = Pose::new();
    let out = remap(
        &src,
        &at,
        &view(90, 0, &pose),
        Fit::Density(4 << 16),
        Fill::Bake,
    )
    .unwrap();
    let m = Model::parse(&out.shape).unwrap();
    let g = Geometry::parse(&out.shape).unwrap();
    assert_eq!(assignments(&g).len(), 1, "one run, one continuation");
    // Shared corners take the same new UV: the layout is continuous.
    let corners = |o: usize| -> Vec<([i32; 3], [i32; 2])> {
        let f = m.faces.iter().find(|f| f.offset == o).unwrap();
        f.indices
            .iter()
            .zip(&f.uv)
            .map(|(i, uv)| (m.vertices[*i].point, *uv))
            .collect()
    };
    let (a, b) = (corners(out.faces[0]), corners(out.faces[1]));
    let shared: Vec<_> = a
        .iter()
        .filter(|(p, _)| b.iter().any(|(q, _)| q == p))
        .collect();
    assert_eq!(shared.len(), 2);
    for (p, uv) in shared {
        assert_eq!(b.iter().find(|(q, _)| q == p).unwrap().1, *uv);
    }
    agrees(&src, &out, at[0], 0);
    agrees(&src, &out, at[1], 1);
    assert_eq!(out.size, [256, 164]);
}

#[test]
fn flat_faces_bake_their_colour_and_blank_fills_the_dominant_index() {
    let rear: [[i16; 3]; 4] = [[0, -20, 0], [0, 0, 0], [0, 0, 40], [0, -20, 40]];
    let front: [[i16; 3]; 4] = [[0, 0, 0], [0, 30, 0], [0, 30, 40], [0, 0, 40]];
    let (src, at) = shape(&[
        (&rear, [-1, -10, 20], None, 35),
        (
            &front,
            [-1, 10, 20],
            Some(alloc::vec![[0, 0], [255, 0], [255, 7], [0, 7]]),
            9,
        ),
    ]);
    let pose = Pose::new();
    let v = view(90, 0, &pose);
    let out = remap(&src, &at, &v, Fit::Density(2 << 16), Fill::Bake).unwrap();
    let pic = retail(&out);
    let flat = face(&out.shape, out.faces[0]);
    assert!(flat.sub & 4 != 0 && flat.uv.len() == 4, "now textured");
    assert_eq!(flat.texture, "NEW.PIC");
    // Rear 20 units: columns 2..42 at 2 texels per unit are colour 35 (a game-palette index).
    assert_eq!(out.size, [256, 84]);
    for row in 2..82 {
        for col in 2..41 {
            assert_eq!(pic.pixels[row * 256 + col], 35);
        }
    }
    agrees(&src, &out, at[1], 1);
    // Blank: every texel takes the dominant index, here the flat colour
    // (20 x 40 texels against 30 x 40 spread over 64 indices).
    let blank = remap(&src, &at, &v, Fit::Density(2 << 16), Fill::Blank).unwrap();
    assert_eq!(blank.dominant, 35);
    let p = retail(&blank);
    assert!(p.pixels.iter().all(|x| *x == 35));
    assert_eq!(blank.shape, out.shape, "fill changes only the texture");
}

#[test]
fn remap_reverses_with_use_shape_texture() {
    let (src, at) = shape(&[
        (&FIN, [-1, 0, 20], Some(fin_uv()), 9),
        (
            &[[0, -20, 50], [0, 20, 50], [0, 20, 60], [0, -20, 60]],
            [-1, 0, 55],
            None,
            40,
        ),
    ]);
    let pose = Pose::new();
    type Drawn = Vec<(String, Vec<[i32; 2]>, u8, Vec<[i32; 3]>)>;
    let drawn = |b: &[u8]| -> Drawn {
        let m = Model::parse(b).unwrap();
        let mut v: Vec<_> = m
            .faces
            .iter()
            .map(|f| {
                (
                    f.texture.clone(),
                    f.uv.clone(),
                    f.sub,
                    f.indices.iter().map(|i| m.vertices[*i].point).collect(),
                )
            })
            .collect();
        v.sort_unstable();
        v
    };
    let out = remap(
        &src,
        &at,
        &view(90, 0, &pose),
        Fit::Density(2 << 16),
        Fill::Bake,
    )
    .unwrap();
    let g0 = Geometry::parse(&src).unwrap();
    let g1 = Geometry::parse(&out.shape).unwrap();
    assert!(g1.inventory.contiguous());
    assert_eq!(g1.inventory.opaque_bytes(), g0.inventory.opaque_bytes());
    assert_eq!(g1.inventory.bindings, g0.inventory.bindings);
    let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
    assert_eq!(drawn(&back.shape), drawn(&src));
    // Remapping already remapped faces moves them again without nesting.
    let mut textures = BTreeMap::new();
    textures.insert(String::from("NEW.PIC"), Pic::parse(&out.picture).unwrap());
    let palette = base();
    let again = remap_from_view(
        &out.shape,
        &out.faces,
        "NEXT.PIC",
        &view(90, 0, &pose),
        Fit::Density(1 << 16),
        Fill::Bake,
        &Sources {
            textures: &textures,
            palette: &palette,
        },
    )
    .unwrap();
    assert_eq!(
        assignments(&Geometry::parse(&again.shape).unwrap()).len(),
        1
    );
    let back = restore_texture_assignment(&again.shape, &again.faces).unwrap();
    assert_eq!(drawn(&back.shape), drawn(&src));
}

#[test]
fn sizes_clamp_and_refuse_outside_the_sheet_limits() {
    let (src, at) = shape(&[(&FIN, [-1, 0, 20], Some(fin_uv()), 9)]);
    let pose = Pose::new();
    let v = view(90, 0, &pose);
    let plan = remap_plan(&src, &at, &v, Fit::Density(64 << 16)).unwrap();
    assert_eq!(
        plan.size,
        [256, 256],
        "252 texels wide at most, rows with it"
    );
    let plan = remap_plan(&src, &at, &v, Fit::Density(1 << 12)).unwrap();
    assert_eq!(plan.size, [256, 8], "always 256 wide, at least 8 rows");
    let plan = remap_plan(&src, &at, &v, Fit::Size([256, 32])).unwrap();
    assert_eq!(plan.size, [256, 32]);
    assert!(plan.uv[0].iter().all(|p| p[1] >= 2 && p[1] <= 30));
    for size in [[64, 32], [300, 32], [256, 1281], [256, 4]] {
        let e = remap_plan(&src, &at, &v, Fit::Size(size)).unwrap_err();
        assert!(e.contains("256 pixels wide and 8 to 1,280 rows"), "{e}");
    }
    // A tall panel scales both ways into 1,280 rows.
    let tall: [[i16; 3]; 4] = [[0, -5, 0], [0, 5, 0], [0, 5, 400], [0, -5, 400]];
    let (high, faces) = shape(&[(&tall, [-1, 0, 200], Some(fin_uv()), 9)]);
    let plan = remap_plan(&high, &faces, &v, Fit::Density(4 << 16)).unwrap();
    assert_eq!(plan.size, [256, 1280]);
    let u: Vec<i32> = plan.uv[0].iter().map(|p| p[0]).collect();
    assert_eq!(u.iter().max().unwrap() - u.iter().min().unwrap(), 32);
    let out = remap(&high, &faces, &v, Fit::Density(4 << 16), Fill::Bake).unwrap();
    assert_eq!(retail(&out).height, 1280);
    let e = remap(&src, &[at[0] + 1], &v, Fit::Density(1 << 16), Fill::Bake).unwrap_err();
    assert!(e.contains("not drawn"), "{e}");
    // A face whose PIC is not loaded cannot be baked.
    let palette = base();
    let none = BTreeMap::new();
    let e = remap_from_view(
        &src,
        &at,
        "NEW.PIC",
        &v,
        Fit::Density(1 << 16),
        Fill::Bake,
        &Sources {
            textures: &none,
            palette: &palette,
        },
    )
    .unwrap_err();
    assert!(e.contains("OLD.PIC is not loaded"), "{e}");
}

#[test]
fn remap_uses_no_floating_point() {
    for source in [
        include_str!("shape_remap.rs"),
        include_str!("shape_remap_tests.rs"),
    ] {
        for word in [concat!("f", "32"), concat!("f", "64")] {
            assert!(!source.contains(word));
        }
    }
}
