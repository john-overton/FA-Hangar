//! Synthetic fixtures for per-face texture assignment. Never retail data.
use super::*;
use crate::shape_geometry::flip_faces;
use crate::shape_testkit::{Asm, Shift};

const BODY: [[i16; 3]; 8] = [
    [0, 0, 0],
    [20, 0, 0],
    [0, 20, 0],
    [20, 20, 0],
    [0, 0, 20],
    [20, 0, 20],
    [40, 0, 0],
    [40, 20, 0],
];
fn lit(points: &[[i16; 3]]) -> Option<([i16; 3], [i16; 3])> {
    let p: Vec<[i32; 3]> = points.iter().map(|p| p.map(|v| v as i32)).collect();
    let n = face_normal(&p)?;
    let c: [i32; 3] =
        core::array::from_fn(|k| p.iter().map(|q| q[k]).sum::<i32>() / p.len() as i32);
    Some((n.map(|v| v as i16), c.map(|v| v as i16)))
}
fn pts(slots: &[u16]) -> Vec<[i16; 3]> {
    slots.iter().map(|s| BODY[*s as usize]).collect()
}
#[derive(Default, Clone, Copy)]
struct Opt {
    /// An unreached 48 jump into the middle of face `fc`.
    inner_c: bool,
    /// The gear stub resumes drawing exactly at face `late`.
    resume_on_late: bool,
}
/// Body textured with BASE.PIC: a run fa fb fc (byte UVs), a flat lit face
/// fd, a word-UV face fw, then a gear part whose block draws a textured face,
/// then `late`.
fn build(o: Opt) -> (Vec<u8>, Asm) {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
    a.b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
    a.label("body").verts(0, &BODY);
    a.label("fa").face(
        0x28,
        32,
        lit(&pts(&[0, 1, 2])),
        &[0, 1, 2],
        &[[0, 0], [63, 0], [0, 63]],
    );
    a.label("fb").face(
        0x28,
        33,
        lit(&pts(&[1, 3, 2])),
        &[1, 3, 2],
        &[[63, 0], [63, 63], [0, 63]],
    );
    a.label("fc").face(
        0x28,
        34,
        lit(&pts(&[1, 6, 7, 3])),
        &[1, 6, 7, 3],
        &[[0, 0], [10, 0], [10, 10], [0, 10]],
    );
    a.label("fd")
        .face(0x23, 35, lit(&pts(&[0, 4, 5, 1])), &[0, 4, 5, 1], &[]);
    a.label("fw").face(
        0x28,
        36,
        lit(&pts(&[2, 3, 5])),
        &[2, 3, 5],
        &[[0, 0], [300, 0], [0, 200]],
    );
    a.xform(
        "gear",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        Shift::One,
        true,
        0x0a,
        [5, 2, -3],
        "gearblock",
        "afterg",
    );
    a.label("afterg");
    if !o.resume_on_late {
        a.b(&[0xca, 0, 0, 0]);
    }
    a.label("late").face(
        0x28,
        37,
        lit(&pts(&[0, 1, 4])),
        &[0, 1, 4],
        &[[1, 1], [2, 2], [3, 1]],
    );
    a.jump("end");
    if o.inner_c {
        let at = a.at("fc") + 3;
        a.mark("fc+3", at).jump("fc+3");
    }
    let gear = [[0, 0, 0], [0, 0, -10], [1, 0, -10]];
    a.label("gearblock")
        .verts(8, &gear)
        .face(0x28, 40, lit(&gear), &[8, 9, 10], &[[0, 0], [5, 0], [5, 5]])
        .b(&[0x1e]);
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    let mut copy = Asm::default();
    for name in ["fa", "fb", "fc", "fd", "fw", "late", "gearblock"] {
        copy.mark(name, a.at(name));
    }
    (a.finish(), copy)
}
const CS: usize = 1024;
fn fixture() -> (Vec<u8>, Asm) {
    build(Opt::default())
}
fn face(l: &Asm, name: &str) -> usize {
    CS + l.at(name)
}
fn gear_down() -> Pose {
    let mut p = Pose::new();
    p.insert("_PLgearDown".into(), 1);
    p.insert("_PLgearPos".into(), -8192);
    p
}
fn model_face(b: &[u8], pose: &Pose, offset: usize) -> crate::model::Face {
    Model::with_pose(b, pose)
        .unwrap()
        .faces
        .into_iter()
        .find(|f| f.offset == offset)
        .unwrap_or_else(|| panic!("face {offset:X} not drawn"))
}
/// Every face drawn, as (offset, texture, uv, sub, points), sorted.
#[allow(clippy::type_complexity)]
fn drawn(b: &[u8], pose: &Pose) -> Vec<(usize, String, Vec<[i32; 2]>, u8, Vec<[i32; 3]>)> {
    let m = Model::with_pose(b, pose).unwrap();
    let mut out: Vec<_> = m
        .faces
        .iter()
        .map(|f| {
            (
                f.offset,
                f.texture.clone(),
                f.uv.clone(),
                f.sub,
                f.indices.iter().map(|i| m.vertices[*i].point).collect(),
            )
        })
        .collect();
    out.sort_unstable();
    out
}
fn keep(src: &[u8], faces: &[usize], name: &str) -> Assigned {
    assign_texture(src, faces, name, UvMode::Keep).unwrap()
}
#[test]
fn single_face_keep_moves_it_to_the_new_texture_and_back() {
    let (src, l) = fixture();
    let before = Model::parse(&src).unwrap();
    let out = keep(&src, &[face(&l, "fa")], "NEW.PIC");
    let m = Model::parse(&out.shape).unwrap();
    assert_eq!(m.faces.len(), before.faces.len());
    let f = model_face(&out.shape, &Pose::new(), out.faces[0]);
    assert_eq!(f.texture, "NEW.PIC");
    assert_eq!(f.uv, [[0, 0], [63, 0], [0, 63]]);
    // Every other face keeps its texture and record.
    for (a, b) in before.faces.iter().zip(&m.faces) {
        if a.offset != face(&l, "fa") {
            assert_eq!((a.offset, &a.texture, &a.uv), (b.offset, &b.texture, &b.uv));
            assert_eq!(src[a.offset..a.end], out.shape[b.offset..b.end]);
        }
    }
    let g = Geometry::parse(&out.shape).unwrap();
    assert!(g.inventory.contiguous());
    assert_eq!(g.inventory.opaque_bytes(), 0);
    assert_eq!(
        g.inventory.bindings,
        Geometry::parse(&src).unwrap().inventory.bindings
    );
    let found = assignments(&g);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].texture, "NEW.PIC");
    assert_eq!(found[0].sites, [(l.at("fa"), l.at("fb"))]);
    assert_eq!(
        assigned_faces(&out.shape)
            .unwrap()
            .into_iter()
            .collect::<Vec<_>>(),
        [(out.faces[0], String::from("NEW.PIC"))]
    );
    // Reverse: the original record goes back; every drawn face is as before.
    let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
    assert_eq!(back.faces, [face(&l, "fa")]);
    assert_eq!(drawn(&back.shape, &Pose::new()), drawn(&src, &Pose::new()));
    assert_eq!(drawn(&back.shape, &gear_down()), drawn(&src, &gear_down()));
    let fa = face(&l, "fa");
    let len = l.at("fb") - l.at("fa");
    assert_eq!(
        src[fa..fa + len],
        back.shape[fa..fa + len],
        "the record returns exactly"
    );
    assert!(assignments(&Geometry::parse(&back.shape).unwrap()).is_empty());
    // A dissolved continuation's space is reused by the next assignment.
    let again = keep(&back.shape, &[face(&l, "fa")], "NEW.PIC");
    assert_eq!(again.shape.len(), back.shape.len());
}
#[test]
fn contiguous_run_shares_one_detour_and_scattered_faces_get_their_own() {
    let (src, l) = fixture();
    let run = keep(
        &src,
        &[face(&l, "fa"), face(&l, "fb"), face(&l, "fc")],
        "RUN.PIC",
    );
    let found = assignments(&Geometry::parse(&run.shape).unwrap());
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].copies.len(), 3);
    for o in &run.faces {
        assert_eq!(model_face(&run.shape, &Pose::new(), *o).texture, "RUN.PIC");
    }
    let late = model_face(&run.shape, &Pose::new(), face(&l, "late"));
    assert_eq!(late.texture, "BASE.PIC");
    let scattered = keep(
        &src,
        &[face(&l, "late"), face(&l, "fa"), face(&l, "fc")],
        "SCAT.PIC",
    );
    let found = assignments(&Geometry::parse(&scattered.shape).unwrap());
    assert_eq!(found.len(), 3);
    assert_eq!(scattered.faces.len(), 3);
    assert_eq!(
        model_face(&scattered.shape, &Pose::new(), face(&l, "fb")).texture,
        "BASE.PIC"
    );
    let back = restore_texture_assignment(&scattered.shape, &scattered.faces).unwrap();
    assert_eq!(drawn(&back.shape, &Pose::new()), drawn(&src, &Pose::new()));
    // Different textures on neighbours split the run.
    let one = keep(&src, &[face(&l, "fa")], "ONE.PIC");
    let two = keep(&one.shape, &[face(&l, "fb")], "TWO.PIC");
    assert_eq!(assignments(&Geometry::parse(&two.shape).unwrap()).len(), 2);
}
#[test]
fn flat_faces_take_projected_uvs_and_keep_or_scale_refuse_them() {
    let (src, l) = fixture();
    let fd = face(&l, "fd");
    let e = assign_texture(&src, &[fd], "NEW.PIC", UvMode::Keep).unwrap_err();
    assert!(e.contains("untextured"), "{e}");
    let mode = UvMode::Project {
        plane: Plane::Auto,
        size: [64, 32],
    };
    let out = assign_texture(&src, &[fd], "NEW.PIC", mode).unwrap();
    let f = model_face(&out.shape, &Pose::new(), out.faces[0]);
    assert_eq!(f.texture, "NEW.PIC");
    assert_eq!(f.sub & 4, 4);
    assert_eq!(f.uv.len(), 4);
    // A 20 x 20 square into 64 x 32: square texels, fitted to the height
    // (the far edge rounds onto the last texel).
    let u = f.uv.iter().map(|p| p[0]).max().unwrap();
    let v = f.uv.iter().map(|p| p[1]).max().unwrap();
    assert_eq!((u, v), (32, 31));
    let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
    assert_eq!(drawn(&back.shape, &Pose::new()), drawn(&src, &Pose::new()));
    let g = Geometry::parse(&back.shape).unwrap();
    let i = g.face_at(fd).unwrap();
    assert_eq!(g.faces[i].content, 0x63);
}
#[test]
fn scale_respects_byte_and_word_uvs_and_widens_when_needed() {
    let (src, l) = fixture();
    let scale = |from: [u32; 2], to: [u32; 2]| UvMode::Scale { from, to };
    let up = assign_texture(
        &src,
        &[face(&l, "fa")],
        "BIG.PIC",
        scale([64, 64], [512, 512]),
    )
    .unwrap();
    let f = model_face(&up.shape, &Pose::new(), up.faces[0]);
    assert_eq!(f.uv, [[0, 0], [504, 0], [0, 504]]);
    assert_eq!(f.flags & 1, 0, "widened to word UVs");
    let g = Geometry::parse(&up.shape).unwrap();
    assert_eq!(g.faces[g.face_at(up.faces[0]).unwrap()].layout & 1, 0);
    // A word-UV face stays word even when its UVs would fit bytes.
    let down = assign_texture(
        &src,
        &[face(&l, "fw")],
        "SMALL.PIC",
        scale([512, 512], [128, 128]),
    )
    .unwrap();
    let f = model_face(&down.shape, &Pose::new(), down.faces[0]);
    assert_eq!(f.uv, [[0, 0], [75, 0], [0, 50]]);
    assert_eq!(f.flags & 1, 0);
    // A byte face that still fits keeps bytes; the reverse is exact.
    let half = assign_texture(
        &src,
        &[face(&l, "fa")],
        "HALF.PIC",
        scale([64, 64], [32, 32]),
    )
    .unwrap();
    let f = model_face(&half.shape, &Pose::new(), half.faces[0]);
    assert_eq!(f.uv, [[0, 0], [32, 0], [0, 32]]);
    assert_eq!(f.flags & 1, 1);
    for out in [&up, &down, &half] {
        let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
        assert_eq!(drawn(&back.shape, &Pose::new()), drawn(&src, &Pose::new()));
    }
    assert!(assign_texture(&src, &[face(&l, "fa")], "X.PIC", scale([0, 64], [8, 8])).is_err());
}
#[test]
fn project_with_fixed_planes_keeps_square_texels() {
    let (src, l) = fixture();
    let fc = face(&l, "fc");
    for (plane, expect) in [
        (Plane::Top, [[0, 0], [63, 0], [63, 63], [0, 63]]),
        (Plane::Auto, [[0, 63], [63, 63], [63, 0], [0, 0]]),
    ] {
        let mode = UvMode::Project {
            plane,
            size: [64, 64],
        };
        let out = assign_texture(&src, &[fc], "P.PIC", mode).unwrap();
        let f = model_face(&out.shape, &Pose::new(), out.faces[0]);
        assert_eq!(f.uv, expect, "{plane:?}");
    }
    // The side plane sees the 40 x 20 x 0 face edge-on.
    let mode = UvMode::Project {
        plane: Plane::Side,
        size: [64, 64],
    };
    let out = assign_texture(&src, &[fc], "P.PIC", mode).unwrap();
    let f = model_face(&out.shape, &Pose::new(), out.faces[0]);
    assert!(f.uv.iter().all(|p| p[1] == 0));
}
#[test]
fn refusals_name_their_reason() {
    let (src, l) = build(Opt {
        inner_c: true,
        ..Opt::default()
    });
    let e = keep_err(&src, &[face(&l, "fc")]);
    assert!(e.contains("points into the middle"), "{e}");
    assert!(keep_err(&src, &[face(&l, "fc") + 1]).contains("No FC face"));
    let (resumed, l2) = build(Opt {
        resume_on_late: true,
        ..Opt::default()
    });
    let e = keep_err(&resumed, &[face(&l2, "late")]);
    assert!(e.contains("resumes drawing"), "{e}");
    assert!(assign_texture(&src, &[face(&l, "fa")], "BAD NAME.PIC", UvMode::Keep).is_err());
    assert!(assign_texture(&src, &[face(&l, "fa")], "NEW.SH", UvMode::Keep).is_err());
    assert!(assign_texture(&src, &[], "NEW.PIC", UvMode::Keep).is_err());
    assert_eq!(
        restore_texture_assignment(&src, &[face(&l, "fa")]).unwrap_err(),
        "No Hangar texture assignment to remove"
    );
    let out = keep(&src, &[face(&l, "fa")], "NEW.PIC");
    let e = restore_texture_assignment(&out.shape, &[out.faces[0], face(&l, "fb")]).unwrap_err();
    assert!(e.contains("No Hangar texture assignment to remove"), "{e}");
    // Stored originals are never drawn and cannot be selected.
    let g = Geometry::parse(&out.shape).unwrap();
    let a = &assignments(&g)[0];
    let e = keep_err(&out.shape, &[g.inventory.code_start + a.originals[0]]);
    assert!(e.contains("never drawn"), "{e}");
    // Keep refusal text for the UI.
    assert!(keep_refusal(Some([64, 64]), [128, 64]).is_some());
    assert!(keep_refusal(Some([64, 64]), [64, 64]).is_none());
    assert!(keep_refusal(None, [64, 64]).is_some());
}
fn keep_err(src: &[u8], faces: &[usize]) -> String {
    assign_texture(src, faces, "NEW.PIC", UvMode::Keep).unwrap_err()
}
#[test]
fn keep_on_the_current_texture_is_an_exact_no_op() {
    let (src, l) = fixture();
    let same = keep(&src, &[face(&l, "fa"), face(&l, "fb")], "base.pic");
    assert_eq!(same.shape, src);
    assert_eq!(same.faces, [face(&l, "fa"), face(&l, "fb")]);
    let out = keep(&src, &[face(&l, "fa")], "NEW.PIC");
    let again = keep(&out.shape, &out.faces, "NEW.PIC");
    assert_eq!(again.shape, out.shape);
    assert_eq!(again.faces, out.faces);
}
#[test]
fn faces_in_a_part_keep_their_frame() {
    let (src, l) = fixture();
    let pose = gear_down();
    let gear = Model::with_pose(&src, &pose)
        .unwrap()
        .faces
        .into_iter()
        .find(|f| f.part.is_some())
        .unwrap();
    assert!(gear.offset > face(&l, "gearblock"));
    let out = keep(&src, &[gear.offset], "GEAR.PIC");
    let m = Model::with_pose(&out.shape, &pose).unwrap();
    let f = m.faces.iter().find(|f| f.offset == out.faces[0]).unwrap();
    assert_eq!(f.texture, "GEAR.PIC");
    assert_eq!(f.part, gear.part);
    let before = Model::with_pose(&src, &pose).unwrap();
    let points = |m: &Model, f: &crate::model::Face| -> Vec<[i32; 3]> {
        f.indices.iter().map(|i| m.vertices[*i].point).collect()
    };
    assert_eq!(points(&m, f), points(&before, &gear));
    // Moving the part (another pose) still moves the assigned face.
    let mut up = pose.clone();
    up.insert("_PLgearPos".into(), 0);
    let a = Model::with_pose(&src, &up).unwrap();
    let b = Model::with_pose(&out.shape, &up).unwrap();
    let fa = a.faces.iter().find(|f| f.offset == gear.offset).unwrap();
    let fb = b.faces.iter().find(|f| f.offset == out.faces[0]).unwrap();
    assert_eq!(points(&a, fa), points(&b, fb));
    let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
    assert_eq!(drawn(&back.shape, &pose), drawn(&src, &pose));
}
#[test]
fn a_second_assignment_moves_faces_without_nesting() {
    let (src, l) = fixture();
    let first = keep(&src, &[face(&l, "fa"), face(&l, "fb")], "ONE.PIC");
    let second = keep(&first.shape, &first.faces[1..], "TWO.PIC");
    let g = Geometry::parse(&second.shape).unwrap();
    let mut found = assignments(&g);
    found.sort_unstable_by_key(|a| a.sites[0]);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].texture, "ONE.PIC");
    assert_eq!(found[0].sites, [(l.at("fa"), l.at("fb"))]);
    assert_eq!(found[1].texture, "TWO.PIC");
    assert_eq!(found[1].sites, [(l.at("fb"), l.at("fc"))]);
    let fa = model_face(
        &second.shape,
        &Pose::new(),
        g.inventory.code_start + found[0].copies[0],
    );
    assert_eq!(fa.texture, "ONE.PIC");
    // Back to the shape texture in two steps, then all at once.
    let one = restore_texture_assignment(&second.shape, &second.faces).unwrap();
    assert_eq!(assignments(&Geometry::parse(&one.shape).unwrap()).len(), 1);
    let fa_now = *assigned_faces(&one.shape).unwrap().keys().next().unwrap();
    let both = restore_texture_assignment(&one.shape, &[fa_now]).unwrap();
    assert_eq!(drawn(&both.shape, &Pose::new()), drawn(&src, &Pose::new()));
    let (a, e) = (face(&l, "fa"), face(&l, "fc"));
    assert_eq!(src[a..e], both.shape[a..e]);
}
#[test]
fn restore_keeps_later_flips_of_the_copy() {
    let (src, l) = fixture();
    let out = keep(&src, &[face(&l, "fa")], "NEW.PIC");
    let flipped = flip_faces(&out.shape, &out.faces).unwrap();
    let back = restore_texture_assignment(&flipped, &out.faces).unwrap();
    let direct = flip_faces(&src, &[face(&l, "fa")]).unwrap();
    assert_eq!(
        drawn(&back.shape, &Pose::new()),
        drawn(&direct, &Pose::new())
    );
}
#[test]
fn truncated_or_corrupted_shapes_never_panic() {
    let (src, l) = fixture();
    let out = keep(&src, &[face(&l, "fa"), face(&l, "fb")], "NEW.PIC");
    for b in [&src, &out.shape] {
        for n in (0..b.len()).step_by(97).chain([b.len() - 1]) {
            let cut = &b[..n];
            let _ = assign_texture(cut, &[face(&l, "fa")], "NEW.PIC", UvMode::Keep);
            let _ = restore_texture_assignment(cut, &out.faces);
            let _ = assigned_faces(cut);
        }
    }
    // A damaged continuation is no longer recognised.
    let g = Geometry::parse(&out.shape).unwrap();
    let a = assignments(&g).remove(0);
    let mut bad = out.shape.clone();
    bad[g.inventory.code_start + a.originals[0] + 15] ^= 0x07;
    let mut bad2 = out.shape.clone();
    bad2[g.inventory.code_start + a.sites[1].0 + 6] = 0;
    for b in [bad, bad2] {
        if let Ok(g) = Geometry::parse(&b) {
            assert!(assignments(&g)
                .iter()
                .all(|x| x.copies.len() != 2 || x != &a));
        }
    }
}
fn quad(p: [[i32; 3]; 4]) -> (Vec<[i32; 3]>, Option<[i32; 3]>) {
    (p.to_vec(), None)
}
#[test]
fn panel_sheets_follow_the_panel_aspect_with_square_texels() {
    let d = 1 << 16; // one texel per unit
                     // 4:1 rectangle lying flat: 128 x 32 units.
    let r = planar(
        &[quad([[0, 0, 0], [128, 0, 0], [128, 32, 0], [0, 32, 0]])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(r.size, [128, 32]);
    let u: Vec<i32> = r.uv[0].iter().map(|p| p[0]).collect();
    let v: Vec<i32> = r.uv[0].iter().map(|p| p[1]).collect();
    assert_eq!((u.iter().max(), v.iter().max()), (Some(&127), Some(&31)));
    // Standing upright along the forward axis: the long side is still U.
    let r2 = planar(
        &[quad([[0, 0, 0], [0, 0, 32], [0, 128, 32], [0, 128, 0]])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(r2.size, [128, 32]);
    // Rotated 30 degrees in its plane (cos 0.866, sin 0.5), same size.
    let rot = |x: i32, y: i32| [(x * 866 - y * 500) / 1000, (x * 500 + y * 866) / 1000, 0];
    let r3 = planar(
        &[quad([rot(0, 0), rot(128, 0), rot(128, 32), rot(0, 32)])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert!(
        (r3.size[0] as i32 - 128).abs() <= 2 && (r3.size[1] as i32 - 32).abs() <= 2,
        "{:?}",
        r3.size
    );
    // A skewed parallelogram: U follows its longest edge.
    let r4 = planar(
        &[quad([[0, 0, 0], [100, 0, 0], [120, 20, 0], [20, 20, 0]])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(r4.size, [120, 20]);
    // A triangle.
    let r5 = planar(
        &[(alloc::vec![[0, 0, 0], [64, 0, 0], [0, 16, 0]], None)],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    // U runs along the hypotenuse (66 units); the height is 64*16/66.
    assert_eq!(r5.size, [66, 16]);
    // Clamping scales both sides: 1024 x 256 -> 256 x 64; 16 x 2 -> 64 x 8.
    let big = planar(
        &[quad([[0, 0, 0], [1024, 0, 0], [1024, 256, 0], [0, 256, 0]])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(big.size, [256, 64]);
    let small = planar(
        &[quad([[0, 0, 0], [16, 0, 0], [16, 2, 0], [0, 2, 0]])],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(small.size, [64, 8]);
    // Two coplanar neighbours share one sheet over their combined extent.
    let pair = planar(
        &[
            quad([[0, 0, 0], [64, 0, 0], [64, 32, 0], [0, 32, 0]]),
            quad([[64, 0, 0], [128, 0, 0], [128, 32, 0], [64, 32, 0]]),
        ],
        Plane::Auto,
        Fit::Density(d),
    )
    .unwrap();
    assert_eq!(pair.size, [128, 32]);
    assert_eq!(pair.uv[0][1], pair.uv[1][0]);
    // Degenerate panels are refused.
    assert!(planar(
        &[(alloc::vec![[0, 0, 0], [1, 0, 0], [2, 0, 0]], None)],
        Plane::Auto,
        Fit::Density(d)
    )
    .is_err());
}
#[test]
fn atlas_density_measures_texels_per_unit() {
    let (src, _) = fixture();
    let m = Model::parse(&src).unwrap();
    let d = atlas_density(&m).unwrap();
    assert!(d > 1 << 16 && d < 4 << 16, "{d}");
    let flat = crate::model::Model::parse(&crate::model::demo_shape()).unwrap();
    assert_eq!(atlas_density(&flat), None);
}
#[test]
fn every_face_of_the_textured_kit_can_be_assigned_and_restored() {
    let src = crate::shape_testkit::demo_textured_kit();
    for pose in [Pose::new(), gear_down()] {
        let m = Model::with_pose(&src, &pose).unwrap();
        assert!(m
            .faces
            .iter()
            .any(|f| f.texture == "KIT.PIC" && f.sub & 4 != 0));
        for f in &m.faces {
            let mode = if f.sub & 4 != 0 {
                UvMode::Keep
            } else {
                UvMode::Project {
                    plane: Plane::Auto,
                    size: [32, 32],
                }
            };
            let out = assign_texture(&src, &[f.offset], "KITT1.PIC", mode)
                .unwrap_or_else(|e| panic!("face {:X}: {e}", f.offset));
            let back = restore_texture_assignment(&out.shape, &out.faces).unwrap();
            assert_eq!(drawn(&back.shape, &pose), drawn(&src, &pose));
        }
    }
}
