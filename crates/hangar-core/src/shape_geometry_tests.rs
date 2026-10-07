//! Synthetic fixtures for region-scoped geometry edits. Never retail data.
use super::*;
use crate::shape_testkit::{Asm, Shift};

#[derive(Default, Clone, Copy)]
struct Opt {
    /// An unreached 48 jump into the middle of face `fb`.
    inner_pointer: bool,
    /// A HIGHLOW relocation site inside face `fc`.
    reloc_in_face: bool,
    /// The hook block rewrites body slots 2..4 before face `late`.
    hook_reuses: bool,
    /// A buffer and face above slot 255.
    wide: bool,
    /// About 40 KiB of padding, jumped over, between the body and the parts.
    far: bool,
}
const BODY: [[i16; 3]; 6] = [
    [0, 0, 0],
    [20, 0, 0],
    [0, 20, 0],
    [20, 20, 0],
    [0, 0, 20],
    [20, 0, 20],
];
/// Retail normal and centre for corner points.
fn lit(points: &[[i16; 3]]) -> Option<([i16; 3], [i16; 3])> {
    let p: Vec<[i32; 3]> = points.iter().map(|p| p.map(|v| v as i32)).collect();
    let n = face_normal(&p)?;
    let c = average(&p);
    Some((n.map(|v| v as i16), c.map(|v| v as i16)))
}
fn pts(slots: &[u16]) -> Vec<[i16; 3]> {
    slots.iter().map(|s| BODY[*s as usize]).collect()
}
fn build(o: Opt) -> (Vec<u8>, Asm) {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
    a.b(&[0xf2, 0]).rel16("end", 2);
    a.b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
    a.label("body").verts(0, &BODY);
    a.label("fa")
        .face(0x23, 32, lit(&pts(&[0, 1, 2])), &[0, 1, 2], &[]);
    a.label("fb")
        .face(0x23, 33, lit(&pts(&[1, 3, 2])), &[1, 3, 2], &[]);
    a.label("fc")
        .face(0x23, 34, lit(&pts(&[0, 4, 5, 1])), &[0, 4, 5, 1], &[]);
    a.label("ft").face(
        0x28,
        35,
        lit(&pts(&[2, 3, 5])),
        &[2, 3, 5],
        &[[0, 0], [63, 0], [0, 63]],
    );
    if o.wide {
        let w = [[0, 0, 40], [20, 0, 40], [0, 20, 40]];
        a.label("wide").verts(300, &w);
        a.label("fw").face(0x23, 36, lit(&w), &[300, 301, 302], &[]);
    }
    a.xform(
        "gear",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        // Both retail shift encodings: D1 (sar 1) and C1 imm8.
        if o.wide { Shift::Imm(1) } else { Shift::One },
        true,
        0x0a,
        [5, 2, -3],
        "gearblock",
        "afterg",
    );
    a.label("afterg");
    a.toggle("_PLhook", 1, 0x75, "hook", "hookblock", "afterh");
    a.label("afterh");
    a.label("late")
        .face(0x23, 37, lit(&pts(&[0, 1, 4])), &[0, 1, 4], &[]);
    a.jump("end");
    if o.far {
        a.label("far1").jump("far2");
        a.b(&[0x1e; 20000]);
        a.label("far2").jump("far3");
        a.b(&[0x1e; 20000]);
        a.label("far3");
    }
    if o.inner_pointer {
        let at = a.at("fb") + 3;
        a.mark("fb+3", at).jump("fb+3");
    }
    if o.reloc_in_face {
        let at = a.at("fc") + 6;
        a.reloc(at);
    }
    a.label("orphan").b(&[0xca, 0, 0, 0]);
    let gear = [[0, 0, 0], [0, 0, -10], [1, 0, -10]];
    a.label("gearblock")
        .verts(6, &gear)
        .face(0x23, 40, lit(&gear), &[6, 7, 8], &[])
        .b(&[0x1e]);
    let hook = [[0, -5, 0], [0, -9, 0], [0, -9, -2]];
    let base = if o.hook_reuses { 2 } else { 9 };
    a.label("hookblock")
        .verts(base, &hook)
        .face(0x23, 50, lit(&hook), &[base, base + 1, base + 2], &[])
        .b(&[0x1e]);
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    let labels = Asm::default();
    let mut copy = Asm::default();
    for name in [
        "body",
        "fa",
        "fb",
        "fc",
        "ft",
        "late",
        "gearblock",
        "hookblock",
        "end",
    ] {
        copy.mark(name, a.at(name));
    }
    if o.wide {
        copy.mark("wide", a.at("wide")).mark("fw", a.at("fw"));
    }
    let _ = labels;
    (a.finish(), copy)
}
const CS: usize = 1024;
fn fixture() -> (Vec<u8>, Asm) {
    build(Opt::default())
}
fn face(l: &Asm, name: &str) -> usize {
    CS + l.at(name)
}
fn vertex(l: &Asm, buffer: &str, i: usize) -> usize {
    CS + l.at(buffer) + 6 + 6 * i
}
fn changed(a: &[u8], b: &[u8]) -> Vec<usize> {
    (0..a.len().min(b.len()))
        .filter(|i| a[*i] != b[*i])
        .collect()
}
fn gear_down() -> Pose {
    Pose::from([("_PLgearDown".into(), 1), ("_PLhook".into(), 1)])
}

#[test]
fn analysis_tags_frames_and_proves_every_slot() {
    let (b, l) = fixture();
    let g = Geometry::parse(&b).unwrap();
    assert_eq!(g.buffers.len(), 3);
    assert_eq!(g.faces.len(), 7);
    let frame = |n: &str| g.faces[g.face_at(face(&l, n)).unwrap()].frame;
    assert_eq!(frame("fa"), Frame::Root);
    assert_eq!(frame("late"), Frame::Root);
    // The hook block is called by a 12 inside the root frame.
    let hook_face = g.faces.iter().find(|f| f.color == 50).unwrap();
    assert_eq!(hook_face.frame, Frame::Root);
    let gear_face = g.faces.iter().find(|f| f.color == 40).unwrap();
    let Frame::Part(c4) = gear_face.frame else {
        panic!("gear face frame {:?}", gear_face.frame)
    };
    assert_eq!(b[c4], 0xc4);
    assert_eq!(g.buffers[1].frame, Frame::Part(c4));
    for (i, f) in g.faces.iter().enumerate() {
        for s in &f.slots {
            let w = g.writer(i, *s).unwrap();
            assert!(g.buffers[w].slot <= *s);
        }
    }
    let report = g.vertex_report();
    assert_eq!(report.len(), 12);
    assert!(report.iter().all(|v| v.refusal.is_none()), "{report:?}");
    // Body vertex 0 drives fa, fc and late; vertex 3 drives fb and ft.
    assert_eq!(
        report[0].faces,
        [face(&l, "fa"), face(&l, "fc"), face(&l, "late")]
    );
    assert_eq!(report[3].faces, [face(&l, "fb"), face(&l, "ft")]);
    assert_eq!(report[6].local, [0, 0, 0]);
    assert_eq!(g.vertex_at(vertex(&l, "body", 5)), Some((0, 5)));
    assert_eq!(g.vertex_at(vertex(&l, "body", 5) + 2), None);
}

#[test]
fn unprovable_slots_and_inner_pointers_are_refused_with_reasons() {
    let (b, l) = build(Opt {
        hook_reuses: true,
        ..Opt::default()
    });
    let g = Geometry::parse(&b).unwrap();
    let late = g.face_at(face(&l, "late")).unwrap();
    let e = g.writer(late, 4).unwrap_err();
    assert!(e.contains("rewrites slot 4"), "{e}");
    let status = g.vertex_status(0, 4);
    assert!(
        status.refusal.as_ref().unwrap().contains("slot 4"),
        "{status:?}"
    );
    assert!(g.vertex_status(0, 0).refusal.is_none());
    assert!(write_vertices(&b, &[(vertex(&l, "body", 4), [1, 1, 1])]).is_err());
    assert!(write_vertices(&b, &[(vertex(&l, "body", 3), [1, 1, 1])]).is_ok());

    let (b, l) = build(Opt {
        inner_pointer: true,
        reloc_in_face: true,
        ..Opt::default()
    });
    let g = Geometry::parse(&b).unwrap();
    let fb = g.face_at(face(&l, "fb")).unwrap();
    assert!(g
        .face_refusal(fb)
        .unwrap()
        .contains("points into the middle"));
    let fc = g.face_at(face(&l, "fc")).unwrap();
    assert!(g.face_refusal(fc).unwrap().contains("relocation field"));
    for op in [delete_faces, flip_faces] {
        assert!(op(&b, &[face(&l, "fb")]).is_err());
        assert!(op(&b, &[face(&l, "fc")]).is_err());
        assert!(op(&b, &[face(&l, "fa")]).is_ok());
    }
    // Vertex 3 drives fb, whose normal cannot be rewritten.
    assert!(g
        .vertex_status(0, 3)
        .refusal
        .unwrap()
        .contains("cannot be updated"));
}

#[test]
fn delete_is_a_same_size_jump_stub_and_noops_are_exact() {
    let (b, l) = fixture();
    assert_eq!(delete_faces(&b, &[]).unwrap(), b);
    assert_eq!(flip_faces(&b, &[]).unwrap(), b);
    assert_eq!(write_vertices(&b, &[]).unwrap(), b);
    assert_eq!(
        write_vertices(&b, &[(vertex(&l, "body", 1), [20, 0, 0])]).unwrap(),
        b
    );
    let before = Model::parse(&b).unwrap();
    let out = delete_faces(&b, &[face(&l, "fb"), face(&l, "fa")]).unwrap();
    assert_eq!(out.len(), b.len());
    let (fa, fb) = (face(&l, "fa"), face(&l, "fb"));
    let end = face(&l, "fc");
    assert!(changed(&b, &out).iter().all(|i| (fa..end).contains(i)));
    assert_eq!(out[fa], 0x48);
    assert_eq!(out[fb], 0x48);
    let after = Model::parse(&out).unwrap();
    assert_eq!(after.faces.len() + 2, before.faces.len());
    assert!(after.faces.iter().all(|f| f.offset != fa && f.offset != fb));
    let g = Geometry::parse(&out).unwrap();
    assert!(g.inventory.contiguous() && g.inventory.opaque_bytes() == 0);
    assert!(delete_faces(&b, &[face(&l, "fa") + 1]).is_err());
    // Pure function: same input, same output; the source is untouched.
    assert_eq!(delete_faces(&b, &[fb, fa]).unwrap(), out);
}

#[test]
fn flip_reverses_corners_and_normal_and_is_its_own_inverse() {
    let (b, l) = fixture();
    let ft = face(&l, "ft");
    let out = flip_faces(&b, &[ft]).unwrap();
    let g = Geometry::parse(&b).unwrap();
    let h = Geometry::parse(&out).unwrap();
    let (x, y) = (
        &g.faces[g.face_at(ft).unwrap()],
        &h.faces[h.face_at(ft).unwrap()],
    );
    assert_eq!(y.slots, [5, 3, 2]);
    assert_eq!(y.uv, [[0, 63], [63, 0], [0, 0]]);
    assert_eq!(y.normal, x.normal.map(|n| n.map(|v| -v)));
    assert_eq!(y.centre, x.centre);
    assert_eq!(
        face_normal(&[[0, 20, 0], [20, 20, 0], [20, 0, 20]].map(|p| p)).map(|n| n.map(|v| -v)),
        {
            let p: Vec<[i32; 3]> = [5, 3, 2]
                .iter()
                .map(|s: &usize| BODY[*s].map(|v| v as i32))
                .collect();
            face_normal(&p)
        }
    );
    assert_eq!(flip_faces(&out, &[ft]).unwrap(), b);
    assert!(changed(&b, &out)
        .iter()
        .all(|i| (ft..face(&l, "ft") + x.len).contains(i)));
}

#[test]
fn vertex_writes_update_exactly_the_faces_using_the_slot() {
    let (b, l) = fixture();
    let g = Geometry::parse(&b).unwrap();
    let out = write_vertices(&b, &[(vertex(&l, "body", 2), [0, 30, 0])]).unwrap();
    let h = Geometry::parse(&out).unwrap();
    assert_eq!(h.buffers[0].points[2], [0, 30, 0]);
    for (i, f) in g.faces.iter().enumerate() {
        let j = h.face_at(f.offset).unwrap();
        let uses = f.slots.contains(&2) && f.frame == Frame::Root && f.color < 40;
        assert_eq!(same_face(&g, i, &h, j), !uses, "face {:X}", f.offset);
        if uses {
            let p = h.face_points(j).unwrap();
            assert_eq!(h.faces[j].normal, face_normal(&p));
            assert_eq!(h.faces[j].centre, Some(average(&p)));
        }
    }
    // Only the coordinate and the three faces' normal/centre bytes changed.
    let v = vertex(&l, "body", 2);
    let spans: Vec<(usize, usize)> = ["fa", "fb", "ft"]
        .iter()
        .map(|n| {
            let f = &g.faces[g.face_at(face(&l, n)).unwrap()];
            (f.offset + 5, f.offset + 17)
        })
        .collect();
    assert!(changed(&b, &out)
        .iter()
        .all(|i| (v..v + 6).contains(i) || spans.iter().any(|(a, e)| (*a..*e).contains(i))));
    // A byte centre that would overflow is refused, never widened.
    let e = write_vertices(&b, &[(vertex(&l, "body", 2), [0, 600, 0])]).unwrap_err();
    assert!(e.contains("byte width"), "{e}");
    assert!(write_vertices(&b, &[(vertex(&l, "body", 2), [0, 40000, 0])]).is_err());
}

#[test]
fn part_vertices_are_written_in_local_coordinates() {
    let (b, l) = fixture();
    let before = Model::with_pose(&b, &gear_down()).unwrap();
    let at = vertex(&l, "gearblock", 1);
    let i = before.vertices.iter().position(|v| v.offset == at).unwrap();
    assert_eq!(before.vertex_tags[i].local, [0, 0, -10]);
    let out = write_vertices(&b, &[(at, [0, 4, -10])]).unwrap();
    let after = Model::with_pose(&out, &gear_down()).unwrap();
    let d: [i32; 3] =
        core::array::from_fn(|k| after.vertices[i].point[k] - before.vertices[i].point[k]);
    assert_eq!(d, [0, 4, 0]);
    assert_eq!(after.vertex_tags[i].local, [0, 4, -10]);
    // The model-space mover reaches the same bytes through an unrotated part.
    let moved = move_model_vertices(&b, &before, &[i], [0, 4, 0]).unwrap();
    assert_eq!(moved, out);
    // Pre-existing preview path: non-writable shapes now use the region writer.
    let neutral = Model::parse(&b).unwrap();
    assert!(!neutral.writable);
    let o = crate::shape_edit::move_vertices(&b, &[0], [0, 0, 1]).unwrap();
    assert_eq!(Geometry::parse(&o).unwrap().buffers[0].points[0], [0, 0, 1]);
    let face = face(&l, "fa");
    assert_eq!(frame_translation(&neutral, face).unwrap(), [0, 0, 0]);
}

#[test]
fn scale_moves_corners_about_their_centroid() {
    let (b, l) = fixture();
    let out = scale_faces(&b, &[face(&l, "fa")], 200).unwrap();
    let g = Geometry::parse(&out).unwrap();
    // Corners 0, 1, 2 average to (6, 6, 0).
    assert_eq!(g.buffers[0].points[0], [-6, -6, 0]);
    assert_eq!(g.buffers[0].points[1], [34, -6, 0]);
    assert_eq!(scale_faces(&b, &[face(&l, "fa")], 100).unwrap(), b);
    assert!(scale_faces(&b, &[face(&l, "fa")], 0).is_err());
}

#[test]
fn add_face_detours_a_host_in_the_same_frame() {
    let (b, l) = fixture();
    let before = Model::parse(&b).unwrap();
    let corners = [
        vertex(&l, "body", 3),
        vertex(&l, "body", 5),
        vertex(&l, "body", 1),
    ];
    let added = add_face(&b, &corners, &FaceStyle::flat(77)).unwrap();
    let out = &added.shape;
    let g = Geometry::parse(out).unwrap();
    assert!(g.inventory.contiguous() && g.inventory.opaque_bytes() == 0);
    let j = g.face_at(added.faces[0]).unwrap();
    let f = &g.faces[j];
    assert_eq!(
        (f.slots.clone(), f.color, f.content, f.frame),
        (vec![3, 5, 1], 77, 0x63, Frame::Root)
    );
    let p = g.face_points(j).unwrap();
    assert_eq!(f.normal, face_normal(&p));
    assert_eq!(f.centre, Some(average(&p)));
    // The new face follows its host's copy before the end marker.
    let host = added.host.unwrap();
    assert!(host < added.faces[0]);
    assert!(added.faces[0] < CS + g.inventory.end_marker.unwrap());
    let after = Model::parse(out).unwrap();
    assert_eq!(after.faces.len(), before.faces.len() + 1);
    let pts = |m: &Model| m.vertices.iter().map(|v| v.point).collect::<Vec<_>>();
    assert_eq!(pts(&after), pts(&before));
    // The host record is now a jump, the rest of the body is untouched.
    let original = Geometry::parse(&b).unwrap();
    let h = original
        .faces
        .iter()
        .find(|f| f.slots == g.faces[g.face_at(host).unwrap()].slots)
        .unwrap();
    assert_eq!(out[h.offset], 0x48);
    // The bindings and part preview survive the tail move.
    let gear = Model::with_pose(out, &gear_down()).unwrap();
    assert_eq!(gear.parts.len(), 1);
    // Pure function.
    assert_eq!(
        add_face(&b, &corners, &FaceStyle::flat(77)).unwrap().shape,
        added.shape
    );
    // Follow-up edits work on the continuation: delete and flip the new face.
    let deleted = delete_faces(out, &[added.faces[0]]).unwrap();
    assert_eq!(
        Model::parse(&deleted).unwrap().faces.len(),
        before.faces.len()
    );
    flip_faces(out, &[added.faces[0]]).unwrap();
    let moved = write_vertices(out, &[(vertex(&l, "body", 5), [20, 0, 30])]).unwrap();
    let m = Geometry::parse(&moved).unwrap();
    let k = m.face_at(added.faces[0]).unwrap();
    assert_eq!(m.faces[k].normal, face_normal(&m.face_points(k).unwrap()));
}

#[test]
fn add_face_refusals_are_explicit() {
    let (b, l) = fixture();
    let body = |i| vertex(&l, "body", i);
    let gear = vertex(&l, "gearblock", 0);
    let e = add_face(&b, &[body(0), body(1), gear], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("another part") || e.contains("host"), "{e}");
    let e = add_face(&b, &[body(0), body(1)], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("3 to 64"), "{e}");
    let e = add_face(&b, &[body(0), body(0), body(1)], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("twice"), "{e}");
    let e = add_face(&b, &[body(0), body(1), body(1) + 1], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("No stored vertex"), "{e}");
    // Collinear corners (0, 1 and the hook block's... ) use 0, 1 and a point on that line.
    let line = write_vertices(&b, &[(body(4), [10, 0, 0])]).unwrap();
    let e = add_face(&line, &[body(0), body(4), body(1)], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("collinear"), "{e}");
    let mut shaded = FaceStyle::flat(1);
    shaded.content = 0xee;
    assert!(add_face(&b, &[body(0), body(1), body(3)], &shaded).is_err());
    // Rewritten slot: the hook block rewrites 2..4, so no host after it sees
    // body vertex 4, and hosts before it are in the same frame.
    let (r, rl) = build(Opt {
        hook_reuses: true,
        ..Opt::default()
    });
    let host = face(&rl, "late");
    let e = add_vertices(
        &r,
        host,
        &[],
        &[NewFace {
            corners: vec![
                Corner::Vertex(vertex(&rl, "body", 0)),
                Corner::Vertex(vertex(&rl, "body", 4)),
                Corner::Vertex(vertex(&rl, "body", 1)),
            ],
            style: FaceStyle::flat(1),
        }],
    )
    .unwrap_err();
    assert!(e.contains("not provably current"), "{e}");
    // A textured face in the host's state needs UVs.
    let mut tex = FaceStyle::flat(1);
    tex.content = 0x6c;
    assert!(add_face(&b, &[body(2), body(3), body(5)], &tex).is_err());
    tex.uv = vec![[0, 0], [10, 0], [0, 10]];
    let ok = add_face(&b, &[body(2), body(3), body(5)], &tex).unwrap();
    let g = Geometry::parse(&ok.shape).unwrap();
    assert_eq!(g.faces[g.face_at(ok.faces[0]).unwrap()].uv, tex.uv);
}

#[test]
fn textures_switch_and_restore_around_new_faces() {
    let (b, l) = fixture();
    let mut tex = FaceStyle::flat(1);
    tex.content = 0x6c;
    tex.uv = vec![[0, 0], [300, 0], [0, 10]];
    tex.texture = Some("NEW.PIC".into());
    let added = add_face(
        &b,
        &[
            vertex(&l, "body", 0),
            vertex(&l, "body", 1),
            vertex(&l, "body", 2),
        ],
        &tex,
    )
    .unwrap();
    let m = Model::parse(&added.shape).unwrap();
    let f = m.faces.iter().find(|f| f.offset == added.faces[0]).unwrap();
    assert_eq!(f.texture, "NEW.PIC");
    assert_eq!(f.uv, [[0, 0], [300, 0], [0, 10]]);
    // Faces drawn after the continuation are back in BASE.PIC.
    let late = m.faces.iter().find(|f| f.offset == face(&l, "ft")).unwrap();
    assert_eq!(late.texture, "BASE.PIC");
    let g = Geometry::parse(&added.shape).unwrap();
    assert_eq!(
        g.faces[g.face_at(added.faces[0]).unwrap()].layout & 1,
        0,
        "word UVs"
    );
}

#[test]
fn wide_slots_choose_word_indices() {
    let (b, l) = build(Opt {
        wide: true,
        ..Opt::default()
    });
    let corners = [
        vertex(&l, "wide", 2),
        vertex(&l, "wide", 1),
        vertex(&l, "wide", 0),
    ];
    let added = add_face(&b, &corners, &FaceStyle::flat(9)).unwrap();
    let g = Geometry::parse(&added.shape).unwrap();
    let f = &g.faces[g.face_at(added.faces[0]).unwrap()];
    assert_eq!(f.slots, [302, 301, 300]);
    assert_eq!(f.layout & 4, 4);
    // New vertices take the first free run: slots 12.. (0-11 and 300-302 are used).
    let dup = duplicate_faces(&b, &[face(&l, "fw")], [0, 0, 5]).unwrap();
    assert_eq!(dup.slots, [12, 13, 14]);
}

#[test]
fn duplicate_and_add_vertices_use_free_slots_nothing_else_reads() {
    let (b, l) = fixture();
    let before = Model::parse(&b).unwrap();
    let dup = duplicate_faces(&b, &[face(&l, "fa"), face(&l, "fb")], [0, 0, 7]).unwrap();
    assert_eq!(dup.slots, [12, 13, 14, 15]);
    assert_eq!(dup.faces.len(), 2);
    let g = Geometry::parse(&dup.shape).unwrap();
    let new = g.vertex_at(dup.vertices[0]).unwrap();
    assert_eq!(
        g.buffers[new.0].points,
        [[0, 0, 7], [20, 0, 7], [0, 20, 7], [20, 20, 7]]
    );
    for o in &dup.faces {
        let j = g.face_at(*o).unwrap();
        for s in &g.faces[j].slots {
            assert_eq!(g.writer(j, *s).unwrap(), new.0);
        }
    }
    let after = Model::parse(&dup.shape).unwrap();
    assert_eq!(after.faces.len(), before.faces.len() + 2);
    assert_eq!(after.vertices.len(), before.vertices.len() + 4);
    // Shared corners of the two faces become one new vertex each.
    let shared = &g.faces[g.face_at(dup.faces[1]).unwrap()].slots;
    assert_eq!(shared, &[13, 15, 14]);
    // Explicit new geometry in the gear part is stored pivot-local.
    let gear_face = Geometry::parse(&b)
        .unwrap()
        .faces
        .iter()
        .find(|f| f.color == 40)
        .unwrap()
        .offset;
    let added = add_vertices(
        &b,
        gear_face,
        &[[0, 3, 0]],
        &[NewFace {
            corners: vec![
                Corner::Vertex(vertex(&l, "gearblock", 0)),
                Corner::Vertex(vertex(&l, "gearblock", 1)),
                Corner::New(0),
            ],
            style: FaceStyle::flat(41),
        }],
    )
    .unwrap();
    let m = Model::with_pose(&added.shape, &gear_down()).unwrap();
    let v = m
        .vertices
        .iter()
        .position(|v| v.offset == added.vertices[0])
        .unwrap();
    let base = Model::with_pose(&b, &gear_down()).unwrap();
    let pivot = base.parts[0].posed_position;
    assert_eq!(m.vertex_tags[v].local, [0, 3, 0]);
    assert_eq!(m.vertices[v].point, [pivot[0], pivot[1] + 3, pivot[2]]);
    let g = Geometry::parse(&added.shape).unwrap();
    assert!(matches!(
        g.faces[g.face_at(added.faces[0]).unwrap()].frame,
        Frame::Part(_)
    ));
    // Ceiling: more vertices than any retail shape holds are refused.
    let many = vec![[0, 0, 0]; 700];
    let e = add_vertices(&b, face(&l, "fa"), &many, &[]).unwrap_err();
    assert!(e.contains("ceiling"), "{e}");
}

#[test]
fn extrude_adds_caps_and_outward_sides_and_handles_the_base() {
    let (b, l) = fixture();
    let fa = face(&l, "fa");
    let g = Geometry::parse(&b).unwrap();
    let n = g.faces[g.face_at(fa).unwrap()].normal.unwrap();
    // Extrude along the stored normal (down): three side quads plus the cap.
    let up = [0, 0, -5];
    assert!(n[2] < 0);
    let out = extrude_faces(&b, &[fa], up, Base::Keep).unwrap();
    assert_eq!(out.faces.len(), 4);
    assert_eq!(out.slots.len(), 3);
    let h = Geometry::parse(&out.shape).unwrap();
    let centroid = [20 / 3, 20 / 3, -2];
    for o in &out.faces[1..] {
        let j = h.face_at(*o).unwrap();
        let p = h.face_points(j).unwrap();
        let nn = h.faces[j].normal.unwrap();
        let c = average(&p);
        let dot: i64 = (0..3)
            .map(|k| nn[k] as i64 * (c[k] - centroid[k]) as i64)
            .sum();
        assert!(dot > 0, "side {o:X} faces inward");
        assert_eq!(p.len(), 4);
    }
    // Two adjacent faces share an edge, which gets no side quad.
    let two = extrude_faces(&b, &[fa, face(&l, "fb")], up, Base::Keep).unwrap();
    assert_eq!(two.faces.len(), 2 + 4);
    assert_eq!(two.slots.len(), 4);
    // Flip and Remove apply to every original; the host is handled in its copy.
    let flipped = extrude_faces(&b, &[fa, face(&l, "fb")], up, Base::Flip).unwrap();
    let f = Geometry::parse(&flipped.shape).unwrap();
    let other = f.faces.iter().find(|x| x.offset == fa).unwrap();
    assert_eq!(other.slots, [2, 1, 0]);
    let removed = extrude_faces(&b, &[fa, face(&l, "fb")], up, Base::Remove).unwrap();
    let m = Model::parse(&removed.shape).unwrap();
    assert!(m
        .faces
        .iter()
        .all(|x| x.offset != fa && x.offset != face(&l, "fb")));
    assert!(removed.host.is_none());
    assert!(extrude_faces(&b, &[fa], [0; 3], Base::Keep).is_err());
}

#[test]
fn layout_limits_fail_with_clear_messages() {
    // No virtual-address room after CODE.
    let (mut b, l) = fixture();
    let len = u32::from_le_bytes(b[384..388].try_into().unwrap()) as usize;
    // Move .reloc's RVA directly behind CODE (its raw data stays put).
    b[456 + 12..456 + 16].copy_from_slice(&(0x1000 + len as u32).to_le_bytes());
    let e = add_face(
        &b,
        &[
            vertex(&l, "body", 3),
            vertex(&l, "body", 5),
            vertex(&l, "body", 1),
        ],
        &FaceStyle::flat(1),
    )
    .unwrap_err();
    assert!(e.contains("room"), "{e}");
    // Jump reach: a body face more than 32 KiB before the continuation.
    let (far, l) = build(Opt {
        far: true,
        ..Opt::default()
    });
    let e = add_face(
        &far,
        &[
            vertex(&l, "body", 3),
            vertex(&l, "body", 5),
            vertex(&l, "body", 1),
        ],
        &FaceStyle::flat(1),
    )
    .unwrap_err();
    assert!(e.contains("jump reach"), "{e}");
    // In-place edits work at any distance.
    assert!(delete_faces(&far, &[face(&l, "fa")]).is_ok());
    // A module without the native tail cannot show a continuation.
    let d = crate::model::demo_shape();
    let g = Geometry::parse(&d).unwrap();
    let v = |i: usize| g.buffers[0].vertex(i);
    let e = add_face(&d, &[v(1), v(2), v(4)], &FaceStyle::flat(1)).unwrap_err();
    assert!(e.contains("end marker"), "{e}");
    assert!(delete_faces(&d, &[g.faces[0].offset]).is_ok());
}

#[test]
fn edits_are_one_undo_step_in_a_document() {
    let (b, l) = fixture();
    let mut archive = crate::archive::Archive::empty();
    archive
        .entries
        .push(crate::archive::Entry::new("JET.SH", b.clone()).unwrap());
    let mut doc = crate::document::Document::new(archive);
    let added = add_face(
        &b,
        &[
            vertex(&l, "body", 3),
            vertex(&l, "body", 5),
            vertex(&l, "body", 1),
        ],
        &FaceStyle::flat(5),
    )
    .unwrap();
    doc.replace(0, added.shape.clone()).unwrap();
    assert!(doc.dirty());
    assert!(doc.undo());
    assert_eq!(doc.archive.entries[0].read().unwrap(), b);
    assert!(doc.redo());
    assert_eq!(doc.archive.entries[0].read().unwrap(), added.shape);
}

#[test]
fn padding_inside_a_38_scope_continues_the_walk() {
    // 06 node (count 5: rel16 + 38 trailer) whose scope covers a 1E pad.
    let mut a = Asm::default();
    a.verts(0, &[[0, 0, 0], [10, 0, 0], [0, 10, 0]]);
    a.b(&[6, 0]).b(&[0; 12]).w(5).rel16("after", 2);
    a.b(&[0x38]).rel16("scope", 2);
    a.label("first").face(
        0x23,
        1,
        lit(&[[0, 0, 0], [10, 0, 0], [0, 10, 0]]),
        &[0, 1, 2],
        &[],
    );
    a.b(&[0x1e]);
    a.label("after").face(
        0x23,
        2,
        lit(&[[0, 10, 0], [10, 0, 0], [0, 0, 0]]),
        &[2, 1, 0],
        &[],
    );
    a.label("scope").b(&[0x1e]);
    a.b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    let after = CS + a.at("after");
    let b = a.finish();
    let g = Geometry::parse(&b).unwrap();
    let j = g.face_at(after).unwrap();
    assert_eq!(g.faces[j].frame, Frame::Root);
    assert_eq!(g.writer(j, 1), Ok(0));
    assert!(g.vertex_report().iter().all(|v| v.refusal.is_none()));
}

#[test]
fn truncated_and_corrupted_shapes_never_panic() {
    let (b, l) = fixture();
    for n in (0..b.len()).step_by(7) {
        let t = &b[..n];
        let _ = Geometry::parse(t);
        let _ = delete_faces(t, &[face(&l, "fa")]);
        let _ = write_vertices(t, &[(vertex(&l, "body", 0), [1, 1, 1])]);
    }
    let end = CS + l.at("end") + 18;
    for at in (CS..end).step_by(3) {
        for v in [0x00, 0x1e, 0x48, 0x82, 0xfc, 0xff] {
            let mut m = b.clone();
            m[at] = v;
            if let Ok(g) = Geometry::parse(&m) {
                let _ = g.vertex_report();
            }
            let _ = flip_faces(&m, &[face(&l, "fb")]);
            let _ = add_face(
                &m,
                &[
                    vertex(&l, "body", 3),
                    vertex(&l, "body", 5),
                    vertex(&l, "body", 1),
                ],
                &FaceStyle::flat(1),
            );
        }
    }
}
