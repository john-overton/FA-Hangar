//! Synthetic fixtures for runtime markings. Never retail data.
use super::*;
use crate::picture::{is_retail_texture, Pic};
use crate::shape_testkit::{demo_markings_kit, Asm};

/// The synthetic module with its relocations padded as native PL files
/// pad them (to the next 4 KiB), which is the layout Hangar writes: retail
/// shapes already have it, so hiding and showing gives their bytes back.
fn native(src: Vec<u8>) -> Vec<u8> {
    let slot = markings(&src).unwrap().last().unwrap().slot;
    let out = show_slot(&hide_slot(&src, slot).unwrap(), slot).unwrap();
    assert_eq!(drawn(&out), drawn(&src));
    out
}
fn kit() -> Vec<u8> {
    native(demo_markings_kit(false))
}
fn row(src: &[u8], slot: u16) -> Marking {
    markings(src)
        .unwrap()
        .into_iter()
        .find(|m| m.slot == slot)
        .unwrap_or_else(|| panic!("slot {slot}"))
}
/// Every drawn face in the neutral pose: (CODE offset, texture, points, uv).
#[allow(clippy::type_complexity)]
fn drawn(src: &[u8]) -> Vec<(usize, String, Vec<[i32; 3]>, Vec<[i32; 2]>)> {
    let cs = Geometry::parse(src).unwrap().inventory.code_start;
    let m = Model::parse(src).unwrap();
    let mut out: Vec<_> = m
        .faces
        .iter()
        .map(|f| {
            (
                f.offset - cs,
                f.texture.clone(),
                f.indices.iter().map(|i| m.vertices[*i].point).collect(),
                f.uv.clone(),
            )
        })
        .collect();
    out.sort_unstable();
    out
}
/// Byte identity, reporting where the shapes differ.
fn exact(a: &[u8], b: &[u8]) {
    let diff: Vec<usize> = (0..a.len().max(b.len()))
        .filter(|i| a.get(*i) != b.get(*i))
        .collect();
    assert!(
        diff.is_empty(),
        "lengths {} {}, {} bytes differ: {:?}",
        a.len(),
        b.len(),
        diff.len(),
        &diff[..diff.len().min(24)]
    );
}
fn record(src: &[u8], offset: usize) -> Vec<u8> {
    let g = Geometry::parse(src).unwrap();
    let f = &g.faces[g.face_at(offset).unwrap()];
    src[f.offset..f.end()].to_vec()
}

#[test]
fn finds_each_slot_with_its_face_and_e0_record() {
    let src = kit();
    let rows = markings(&src).unwrap();
    assert_eq!(rows.iter().map(|m| m.slot).collect::<Vec<_>>(), [2, 3, 4]);
    let g = Geometry::parse(&src).unwrap();
    for m in &rows {
        assert_eq!(m.records.len(), 1);
        assert_eq!(&g.code[m.records[0]..m.records[0] + 2], [0xe0, 0]);
        assert_eq!(u16_at(&g.code, m.records[0] + 2).unwrap(), m.slot as usize);
        assert_eq!(m.faces.len(), 1);
        assert!(m.hidden.is_empty() && m.painted.is_empty() && m.contested.is_empty());
        // The face follows its E0 directly and draws no named texture.
        assert_eq!(m.faces[0], g.inventory.code_start + m.records[0] + 4);
        let model = Model::parse(&src).unwrap();
        let f = model.faces.iter().find(|f| f.offset == m.faces[0]).unwrap();
        assert!(f.texture.is_empty() && f.sub & 4 != 0);
    }
    assert_eq!(slot_name(0), "Tail art left");
    assert_eq!(slot_name(4), "Wing marking right");
    // A shape without E0 records has no rows.
    let plain = crate::shape_testkit::demo_textured_kit();
    assert!(markings(&plain).unwrap().is_empty());
}

#[test]
fn hide_and_show_a_slot_round_trip_exactly() {
    let src = kit();
    let face = row(&src, 4).faces[0];
    let original = record(&src, face);
    let hidden = hide_slot(&src, 4).unwrap();
    // The face is gone from every drawn view; everything else is unchanged.
    let before = drawn(&src);
    let after = drawn(&hidden);
    let cs = Geometry::parse(&src).unwrap().inventory.code_start;
    let expect: Vec<_> = before
        .iter()
        .filter(|f| f.0 != face - cs)
        .cloned()
        .collect();
    assert_eq!(after, expect);
    let r = row(&hidden, 4);
    assert_eq!((r.faces.len(), r.hidden.as_slice()), (0, [face].as_slice()));
    assert_eq!(row(&hidden, 3).faces.len(), 1);
    // The stub is the same size, and the block stores the exact record.
    let g = Geometry::parse(&hidden).unwrap();
    let blocks = hidden_blocks(&g);
    assert_eq!(blocks.len(), 1);
    let o = blocks[0].originals[0];
    assert_eq!(g.code[o..o + original.len()], original[..]);
    assert_eq!(
        hide_slot(&hidden, 4).unwrap_err(),
        "Slot 4 is already hidden"
    );
    // Show from the bytes alone (as after save and reopen).
    let shown = show_slot(&hidden, 4).unwrap();
    assert_eq!(record(&shown, face), original);
    assert_eq!(drawn(&shown), before);
    assert!(hidden_blocks(&Geometry::parse(&shown).unwrap()).is_empty());
    assert_eq!(
        show_slot(&shown, 4).unwrap_err(),
        "Slot 4 has no hidden faces"
    );
    // The block ended the appended records, so the layout comes back too.
    exact(&shown, &src);
    // Every face record outside the dissolved block is byte-identical.
    let g0 = Geometry::parse(&src).unwrap();
    for f in &g0.faces {
        assert_eq!(src[f.offset..f.end()], shown[f.offset..f.end()]);
    }
}

#[test]
fn hiding_two_slots_and_showing_one_keeps_the_other_hidden() {
    let src = kit();
    let (f3, f4) = (row(&src, 3).faces[0], row(&src, 4).faces[0]);
    let both = hide_faces(&src, &[f3, f4]).unwrap();
    assert_eq!(row(&both, 3).hidden, [f3]);
    assert_eq!(row(&both, 4).hidden, [f4]);
    let one = show_faces(&both, &[f4]).unwrap();
    assert_eq!(row(&one, 4).faces, [f4]);
    assert_eq!(row(&one, 3).hidden, [f3]);
    let all = show_faces(&one, &[f3]).unwrap();
    assert_eq!(drawn(&all), drawn(&src));
    exact(&all, &src);
    // Hidden one at a time and shown in the other order: still exact.
    let h = hide_slot(&hide_slot(&src, 4).unwrap(), 3).unwrap();
    let s = show_slot(&show_slot(&h, 4).unwrap(), 3).unwrap();
    exact(&s, &src);
    assert_eq!(record(&all, f3), record(&src, f3));
    assert!(show_faces(&all, &[f3])
        .unwrap_err()
        .contains("no faces Hangar hid start there"));
}

#[test]
fn hide_refuses_faces_that_are_not_markings() {
    let src = kit();
    let g = Geometry::parse(&src).unwrap();
    let skin = g.faces[0].offset;
    assert!(hide_faces(&src, &[skin])
        .unwrap_err()
        .contains("draws a named texture, not a runtime marking"));
    let flat = g.faces.iter().find(|f| f.content & 4 == 0).unwrap().offset;
    assert!(hide_faces(&src, &[flat])
        .unwrap_err()
        .contains("untextured"));
    assert!(hide_faces(&src, &[skin + 1])
        .unwrap_err()
        .contains("No FC face record"));
    assert!(hide_slot(&src, 0)
        .unwrap_err()
        .contains("No E0 record selects slot 0"));
}

#[test]
fn reassign_stays_within_named_slots_and_reverses_exactly() {
    let src = kit();
    let moved = reassign_slot(&src, 4, 1).unwrap();
    assert_eq!(moved.len(), src.len());
    assert_eq!(row(&moved, 1).faces, row(&src, 4).faces);
    assert!(markings(&moved).unwrap().iter().all(|m| m.slot != 4));
    assert_eq!(drawn(&moved), drawn(&src));
    assert_eq!(reassign_slot(&moved, 1, 4).unwrap(), src);
    assert!(reassign_slot(&src, 4, 5)
        .unwrap_err()
        .contains("outside 0 to 4"));
    assert!(reassign_slot(&src, 4, 3)
        .unwrap_err()
        .contains("would merge"));
    assert!(reassign_slot(&src, 0, 1)
        .unwrap_err()
        .contains("No E0 record selects slot 0"));
    assert_eq!(reassign_slot(&src, 4, 4).unwrap(), src);
    // A hidden slot moves with its E0: its block follows the state.
    let hidden = hide_slot(&src, 4).unwrap();
    let both = reassign_slot(&hidden, 4, 0).unwrap();
    assert_eq!(row(&both, 0).hidden, row(&hidden, 4).hidden);
}

#[test]
fn make_paintable_draws_a_retail_texture_and_restores_the_slot() {
    let src = kit();
    let face = row(&src, 4).faces[0];
    let p = make_paintable(&src, 4, "KITM4.PIC", None, Some(7)).unwrap();
    assert!(is_retail_texture(&p.picture));
    let pic = Pic::parse(&p.picture).unwrap();
    assert_eq!(pic.width, 256);
    assert!(pic.pixels.iter().all(|v| *v == 7));
    assert!(p.size[0] >= 8 && p.size[1] >= 8);
    let r = row(&p.shape, 4);
    assert!(r.faces.is_empty());
    assert_eq!(r.painted, [(p.faces[0], String::from("KITM4.PIC"))]);
    let m = Model::parse(&p.shape).unwrap();
    let f = m.faces.iter().find(|f| f.offset == p.faces[0]).unwrap();
    assert_eq!(f.texture, "KITM4.PIC");
    assert!(
        f.uv.iter()
            .all(|uv| (0..p.size[0] as i32).contains(&uv[0])
                && (0..p.size[1] as i32).contains(&uv[1]))
    );
    // The other faces still draw what they drew.
    let cs = Geometry::parse(&src).unwrap().inventory.code_start;
    let others = |b: &[u8], skip: usize| -> Vec<_> {
        drawn(b).into_iter().filter(|f| f.0 != skip - cs).collect()
    };
    assert_eq!(others(&p.shape, p.faces[0]), others(&src, face));
    assert!(make_paintable(&p.shape, 4, "X.PIC", None, None)
        .unwrap_err()
        .contains("already paintable"));
    let (back, names) = restore_slot(&p.shape, 4).unwrap();
    assert_eq!(names, ["KITM4.PIC"]);
    assert_eq!(row(&back, 4).faces, [face]);
    assert_eq!(record(&back, face), record(&src, face));
    assert_eq!(drawn(&back), drawn(&src));
    exact(&back, &src);
    assert!(restore_slot(&back, 4)
        .unwrap_err()
        .contains("no paintable faces"));
    // Hidden faces are shown first.
    let hidden = hide_slot(&src, 4).unwrap();
    assert!(make_paintable(&hidden, 4, "X.PIC", None, None)
        .unwrap_err()
        .contains("show it first"));
    // The damage family shares one sheet size.
    let size = paint_size(&src, 3).unwrap();
    let d = native(demo_markings_kit(true));
    let q = make_paintable(&d, 3, "KITM3.PIC", Some(size), None).unwrap();
    assert_eq!(q.size, size);
    assert_eq!(q.color, 150);
}

/// `E2 BASE; top: [82; face X]; E0 slot 1; [82; face Y]; AC top`: face X
/// draws BASE.PIC on the first pass and slot 1 after the loop branch, face
/// Y always draws slot 1. Then `E0 slot 2` with face Z inside a second loop
/// that restores BASE.PIC before it branches back: proved through the loop.
fn looped() -> (Vec<u8>, usize, usize, usize) {
    let quad = |z: i16| [[0, 0, z], [10, 0, z], [10, 10, z], [0, 10, z]];
    let uv = [[0, 0], [9, 0], [9, 9], [0, 9]];
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x10, 0]);
    a.b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
    a.label("top").verts(0, &quad(0));
    a.label("x").face(0x28, 5, None, &[0, 1, 2, 3], &uv);
    a.b(&[0xe0, 0]).w(1);
    a.verts(4, &quad(2));
    a.label("y").face(0x28, 5, None, &[4, 5, 6, 7], &uv);
    a.call(0xac, "top");
    a.label("again").b(&[0xe2, 0]).b(b"BASE.PIC\0\0\0\0\0\0");
    a.b(&[0xe0, 0]).w(2);
    a.verts(8, &quad(4));
    a.label("z").face(0x28, 5, None, &[8, 9, 10, 11], &uv);
    a.call(0xac, "again");
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    let (x, y, z) = (a.at("x"), a.at("y"), a.at("z"));
    let cs = 1024;
    (a.finish(), cs + x, cs + y, cs + z)
}

#[test]
fn loops_prove_markings_and_a_face_under_two_states_is_refused() {
    let (src, x, y, z) = looped();
    let src = native(src);
    let one = row(&src, 1);
    assert_eq!(one.faces, [y]);
    assert_eq!(one.contested.len(), 1);
    assert_eq!(one.contested[0].0, x);
    assert!(one.contested[0].1.contains("different textures"));
    assert_eq!(row(&src, 2).faces, [z]);
    let e = hide_faces(&src, &[x]).unwrap_err();
    assert!(e.contains("draws slot 1 on some paths"), "{e}");
    assert!(hide_slot(&src, 1)
        .unwrap_err()
        .contains("hiding it would hide both"));
    assert!(make_paintable(&src, 1, "X.PIC", None, None)
        .unwrap_err()
        .contains("cannot be made paintable"));
    // Faces proved through the loops hide and show exactly.
    let hidden = hide_faces(&src, &[y, z]).unwrap();
    assert_eq!(row(&hidden, 1).hidden, [y]);
    assert_eq!(row(&hidden, 2).hidden, [z]);
    let shown = show_faces(&hidden, &[y, z]).unwrap();
    assert_eq!(drawn(&shown), drawn(&src));
    exact(&shown, &src);
    assert_eq!(record(&shown, y), record(&src, y));
}

#[test]
fn truncated_shapes_fail_without_panicking() {
    let src = kit();
    let hidden = hide_slot(&src, 4).unwrap();
    for bytes in [&src, &hidden] {
        for n in (0..bytes.len()).step_by(37) {
            let cut = &bytes[..n];
            let _ = markings(cut);
            let _ = hide_slot(cut, 3);
            let _ = show_slot(cut, 4);
            let _ = reassign_slot(cut, 3, 0);
            let _ = make_paintable(cut, 3, "X.PIC", None, None);
            let _ = restore_slot(cut, 3);
        }
    }
    assert!(markings(&src[..src.len() / 2]).is_err());
}
