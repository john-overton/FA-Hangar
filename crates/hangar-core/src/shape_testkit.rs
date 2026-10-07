//! Labelled assembler for synthetic SH modules: SH records, the retail stub
//! idioms (toggle and xform), end marker, import trampolines, `.idata` and
//! HIGHLOW relocations. Used by core tests and the app's smoke fixtures
//! (`demo_parts`). Never retail data.
use alloc::{collections::BTreeMap, string::String, vec::Vec};

pub const IMPORTS: [&str; 10] = [
    "_PLgearDown",
    "_PLgearPos",
    "_PLhook",
    "_PLleftFlap",
    "_PLswingWing",
    "_PLrightFlap",
    "do_start_interp",
    "_PLrudder",
    "_PLbrake",
    "_PLafterBurner",
];
/// `.idata` layout: import lookup table, address table, DLL name, names.
const ILT: usize = 0x40;
const IAT: usize = ILT + 4 * (IMPORTS.len() + 1);
const DLL: usize = IAT + 4 * (IMPORTS.len() + 1);
const NAMES: usize = DLL + 8;
const CODE_VA: usize = 0x1000;
#[derive(Default)]
pub struct Asm {
    pub c: Vec<u8>,
    relocs: Vec<usize>,
    labels: BTreeMap<String, usize>,
    rel16: Vec<(usize, usize, String)>,
    rel8: Vec<(usize, String)>,
    abs: Vec<(usize, String)>,
    k32: Vec<(usize, String, String)>,
}
pub enum Shift {
    /// `66 D1 F8`: sar ax, 1.
    One,
    /// `66 C1 F8 n`.
    Imm(u8),
}
impl Asm {
    pub fn label(&mut self, name: &str) -> &mut Self {
        self.labels.insert(name.into(), self.c.len());
        self
    }
    pub fn at(&self, name: &str) -> usize {
        self.labels[name]
    }
    pub fn try_at(&self, name: &str) -> Option<usize> {
        self.labels.get(name).copied()
    }
    /// A label at an explicit CODE offset (for pointers into a record).
    pub fn mark(&mut self, name: &str, at: usize) -> &mut Self {
        self.labels.insert(name.into(), at);
        self
    }
    /// A HIGHLOW relocation site at an explicit CODE offset.
    pub fn reloc(&mut self, at: usize) -> &mut Self {
        self.relocs.push(at);
        self
    }
    pub fn b(&mut self, b: &[u8]) -> &mut Self {
        self.c.extend(b);
        self
    }
    pub fn w(&mut self, v: i16) -> &mut Self {
        self.c.extend(v.to_le_bytes());
        self
    }
    /// Signed 16-bit displacement to `label`, measured from `field + base`.
    pub fn rel16(&mut self, label: &str, base: usize) -> &mut Self {
        let at = self.c.len();
        self.rel16.push((at, at + base, label.into()));
        self.c.extend([0, 0]);
        self
    }
    pub fn rel8(&mut self, label: &str) -> &mut Self {
        self.rel8.push((self.c.len(), label.into()));
        self.c.push(0);
        self
    }
    /// HIGHLOW virtual address of a label (`alias:NAME` for import trampolines).
    pub fn abs(&mut self, label: &str) -> &mut Self {
        self.relocs.push(self.c.len());
        self.abs.push((self.c.len(), label.into()));
        self.c.extend([0; 4]);
        self
    }
    pub fn call(&mut self, op: u8, label: &str) -> &mut Self {
        self.b(&[op, 0]).rel16(label, 2)
    }
    pub fn jump(&mut self, label: &str) -> &mut Self {
        self.call(0x48, label)
    }
    pub fn c4(&mut self, t: [i16; 3], r: [i16; 3], label: &str) -> &mut Self {
        self.b(&[0xc4, 0]);
        for v in t.into_iter().chain(r) {
            self.w(v);
        }
        self.rel16(label, 2)
    }
    pub fn verts(&mut self, slot: u16, v: &[[i16; 3]]) -> &mut Self {
        self.b(&[0x82, 0]).w(v.len() as i16).w((slot * 8) as i16);
        for p in v {
            for x in p {
                self.w(*x);
            }
        }
        self
    }
    /// FC with an optional stored normal/centre (model order right, forward,
    /// up; stored right, up, forward) and optional UVs.
    pub fn face(
        &mut self,
        content: u8,
        color: u8,
        lit: Option<([i16; 3], [i16; 3])>,
        slots: &[u16],
        uv: &[[u16; 2]],
    ) -> &mut Self {
        let wide = slots.iter().any(|s| *s > 255);
        let byte_centre = lit.is_some_and(|(_, c)| c.iter().all(|v| (-128..=127).contains(v)));
        let byte_uv = uv.iter().flatten().all(|v| *v <= 255);
        let content =
            content | if lit.is_some() { 0x40 } else { 0 } | if uv.is_empty() { 0 } else { 4 };
        let layout =
            (wide as u8) << 2 | (byte_centre as u8) << 1 | (byte_uv && !uv.is_empty()) as u8;
        self.b(&[0xfc, content, layout, color, 0]);
        if let Some((n, c)) = lit {
            for k in [0, 2, 1] {
                self.w(n[k]);
            }
            for k in [0, 2, 1] {
                if byte_centre {
                    self.c.push(c[k] as i8 as u8);
                } else {
                    self.w(c[k]);
                }
            }
        }
        self.c.push(slots.len() as u8);
        for s in slots {
            if wide {
                self.c.extend(s.to_le_bytes());
            } else {
                self.c.push(*s as u8);
            }
        }
        for p in uv {
            for v in p {
                if byte_uv {
                    self.c.push(*v as u8);
                } else {
                    self.c.extend(v.to_le_bytes());
                }
            }
        }
        self
    }
    /// Retail toggle: `cmp word [var], imm8; jcc skip; push call; push dsi;
    /// ret`, then the 12 call record to `block`, then the skip resume to `after`.
    pub fn toggle(&mut self, var: &str, value: i8, jcc: u8, name: &str, block: &str, after: &str) {
        let skip = format!("{name}.skip");
        let call = format!("{name}.call");
        self.b(&[0xf0, 0, 0x66, 0x83, 0x3d])
            .abs(&format!("alias:{var}"))
            .b(&[value as u8, jcc])
            .rel8(&skip)
            .b(&[0x68])
            .abs(&call)
            .b(&[0x68])
            .abs("alias:do_start_interp")
            .b(&[0xc3]);
        self.label(&call).call(0x12, block);
        let at = self.c.len();
        self.b(&[0xf0, 0]);
        self.labels.insert(skip, at + 2);
        self.b(&[0x68])
            .abs(after)
            .b(&[0x68])
            .abs("alias:do_start_interp")
            .b(&[0xc3]);
    }
    /// Retail gear xform: `cmp word [gate], value; jne skip; call $+5; pop ebx;
    /// add ebx, C4+2; mov ax, [law]; shift; [neg]; mov [ebx+disp], ax; push C4;
    /// push dsi; ret`, the C4 record calling `block`, then the skip resume.
    #[allow(clippy::too_many_arguments)]
    pub fn xform(
        &mut self,
        name: &str,
        gate: Option<(&str, i8)>,
        law: &str,
        shift: Shift,
        neg: bool,
        disp: u8,
        pivot: [i16; 3],
        block: &str,
        after: &str,
    ) {
        let skip = format!("{name}.skip");
        let c4 = format!("{name}.c4");
        let pop = format!("{name}.pop");
        let words = format!("{name}.words");
        self.b(&[0xf0, 0]);
        if let Some((var, value)) = gate {
            self.b(&[0x66, 0x83, 0x3d])
                .abs(&format!("alias:{var}"))
                .b(&[value as u8, 0x75])
                .rel8(&skip);
        }
        self.b(&[0xe8, 0, 0, 0, 0])
            .label(&pop)
            .b(&[0x5b, 0x81, 0xc3]);
        self.k32.push((self.c.len(), words.clone(), pop));
        self.c.extend([0; 4]);
        self.b(&[0x66, 0xa1]).abs(&format!("alias:{law}"));
        match shift {
            Shift::One => self.b(&[0x66, 0xd1, 0xf8]),
            Shift::Imm(n) => self.b(&[0x66, 0xc1, 0xf8, n]),
        };
        if neg {
            self.b(&[0x66, 0xf7, 0xd8]);
        }
        self.b(&[0x66, 0x89, 0x43, disp])
            .b(&[0x68])
            .abs(&c4)
            .b(&[0x68])
            .abs("alias:do_start_interp")
            .b(&[0xc3]);
        self.label(&c4);
        self.labels.insert(words, self.c.len() + 2);
        self.c4(pivot, [0; 3], block);
        let at = self.c.len();
        self.b(&[0xf0, 0]);
        self.labels.insert(skip, at + 2);
        self.b(&[0x68])
            .abs(after)
            .b(&[0x68])
            .abs("alias:do_start_interp")
            .b(&[0xc3]);
    }
    /// Pad, append the end marker and trampolines, resolve labels and wrap the
    /// CODE in a PL module with `.idata` and `.reloc`.
    pub fn finish(mut self) -> Vec<u8> {
        while !self.c.len().is_multiple_of(16) {
            self.c.push(0x1e);
        }
        self.c.extend(crate::shape_code::END_MARKER);
        while !self.c.len().is_multiple_of(16) {
            self.c.push(0);
        }
        let idata = idata_rva(self.c.len() + 6 * IMPORTS.len());
        for (i, name) in IMPORTS.iter().enumerate() {
            self.labels.insert(format!("alias:{name}"), self.c.len());
            self.relocs.push(self.c.len() + 2);
            self.c.extend([0xff, 0x25]);
            self.c.extend(((idata + IAT + 4 * i) as u32).to_le_bytes());
        }
        let get = |labels: &BTreeMap<String, usize>, l: &str| -> usize {
            *labels.get(l).unwrap_or_else(|| panic!("label {l}"))
        };
        for (at, base, l) in &self.rel16 {
            let d = get(&self.labels, l) as i64 - *base as i64;
            self.c[*at..*at + 2].copy_from_slice(&(d as i16).to_le_bytes());
        }
        for (at, l) in &self.rel8 {
            let d = get(&self.labels, l) as i64 - (*at as i64 + 1);
            assert!((-128..128).contains(&d), "rel8 {l}");
            self.c[*at] = d as i8 as u8;
        }
        for (at, l) in &self.abs {
            let v = (CODE_VA + get(&self.labels, l)) as u32;
            self.c[*at..*at + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (at, l, base) in &self.k32 {
            let k = get(&self.labels, l) as i64 - get(&self.labels, base) as i64;
            self.c[*at..*at + 4].copy_from_slice(&(k as u32).to_le_bytes());
        }
        module(&self.c, &self.relocs)
    }
}
fn put(b: &mut [u8], at: usize, v: usize) {
    b[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
}
fn align(n: usize, a: usize) -> usize {
    n.div_ceil(a) * a
}
/// `.idata` RVA for a CODE length: at least 0x5000, with 16 KiB of virtual
/// room after CODE for continuations.
fn idata_rva(code_len: usize) -> usize {
    align(CODE_VA + code_len, 0x1000).max(0x1000) + 0x4000
}
/// PL module: CODE at RVA 0x1000 (file 1024), then `.idata` and `.reloc`,
/// with the native 224-byte optional header and per-page relocation blocks.
pub fn module(code: &[u8], relocs: &[usize]) -> Vec<u8> {
    let code_raw = align(code.len().max(1), 512);
    let idata = idata_rva(code.len());
    let reloc = idata + 0x1000;
    let mut pages = BTreeMap::<usize, Vec<u16>>::new();
    for site in relocs {
        let rva = CODE_VA + site;
        pages
            .entry(rva & !0xfff)
            .or_default()
            .push(0x3000 | (rva & 0xfff) as u16);
    }
    let mut table = Vec::new();
    for (page, mut entries) in pages {
        entries.sort_unstable();
        if entries.len() % 2 != 0 {
            entries.push(0);
        }
        table.extend((page as u32).to_le_bytes());
        table.extend(((8 + 2 * entries.len()) as u32).to_le_bytes());
        for e in entries {
            table.extend(e.to_le_bytes());
        }
    }
    let reloc_raw = align(table.len() + 1, 512);
    let (idata_file, reloc_file) = (1024 + code_raw, 1024 + code_raw + 512);
    let mut b = vec![0; reloc_file + reloc_raw];
    b[..2].copy_from_slice(b"MZ");
    put(&mut b, 60, 128);
    b[128..132].copy_from_slice(b"PL\0\0");
    b[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
    b[134..136].copy_from_slice(&3u16.to_le_bytes());
    b[148..150].copy_from_slice(&224u16.to_le_bytes());
    b[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
    for (at, v) in [
        (156, code.len()),
        (184, 4096),
        (188, 512),
        (208, reloc + 0x1000),
        (212, 1024),
        (244, 16),
        (256, idata),
        (260, 40),
        (288, reloc),
        (292, table.len()),
    ] {
        put(&mut b, at, v);
    }
    for (n, (name, rva, virt, raw, size)) in [
        ("CODE", CODE_VA, code.len(), 1024, code_raw),
        (".idata", idata, 0x200, idata_file, 512),
        (".reloc", reloc, reloc_raw, reloc_file, reloc_raw),
    ]
    .into_iter()
    .enumerate()
    {
        let h = 376 + n * 40;
        b[h..h + name.len()].copy_from_slice(name.as_bytes());
        for (off, value) in [(8, virt), (12, rva), (16, size), (20, raw)] {
            put(&mut b, h + off, value);
        }
    }
    b[1024..1024 + code.len()].copy_from_slice(code);
    put(&mut b, idata_file, idata + ILT);
    put(&mut b, idata_file + 12, idata + DLL);
    put(&mut b, idata_file + 16, idata + IAT);
    b[idata_file + DLL..idata_file + DLL + 3].copy_from_slice(b"FA\0");
    for (i, name) in IMPORTS.iter().enumerate() {
        let off = NAMES + 18 * i;
        put(&mut b, idata_file + ILT + 4 * i, idata + off);
        put(&mut b, idata_file + IAT + 4 * i, idata + off);
        let at = idata_file + off + 2;
        b[at..at + name.len()].copy_from_slice(name.as_bytes());
    }
    b[reloc_file..reloc_file + table.len()].copy_from_slice(&table);
    b
}
impl Asm {
    /// An 82 buffer at `slot` and lit flat faces over it (0x63), each wound
    /// so its retail normal points away from `inside` (model order).
    pub fn solid(
        &mut self,
        slot: u16,
        points: &[[i16; 3]],
        faces: &[(&[u16], [i32; 3])],
        color: u8,
    ) -> &mut Self {
        self.solid_uv(slot, points, faces, color, &[])
    }
    /// `solid` with UVs: face `k` takes `uvs[k]` (one per corner, in the
    /// given corner order) and is drawn textured (0x6C); without UVs flat.
    pub fn solid_uv(
        &mut self,
        slot: u16,
        points: &[[i16; 3]],
        faces: &[(&[u16], [i32; 3])],
        color: u8,
        uvs: &[Vec<[u16; 2]>],
    ) -> &mut Self {
        self.verts(slot, points);
        for (k, (corners, inside)) in faces.iter().enumerate() {
            let mut order: Vec<u16> = corners.to_vec();
            let mut uv: Vec<[u16; 2]> = uvs.get(k).cloned().unwrap_or_default();
            let at = |order: &[u16]| -> Vec<[i32; 3]> {
                order
                    .iter()
                    .map(|k| points[*k as usize].map(|v| v as i32))
                    .collect()
            };
            let mut p = at(&order);
            let mut n = crate::model::face_normal(&p).unwrap_or([0, 0, 32765]);
            let c: [i32; 3] =
                core::array::from_fn(|k| p.iter().map(|q| q[k]).sum::<i32>() / p.len() as i32);
            if (0..3)
                .map(|k| n[k] as i64 * (c[k] - inside[k]) as i64)
                .sum::<i64>()
                < 0
            {
                order.reverse();
                uv.reverse();
                p = at(&order);
                n = crate::model::face_normal(&p).unwrap_or([0, 0, 32765]);
            }
            let slots: Vec<u16> = order.iter().map(|k| slot + k).collect();
            self.face(
                if uv.is_empty() { 0x23 } else { 0x28 },
                color,
                Some((n.map(|v| v as i16), c.map(|v| v as i16))),
                &slots,
                &uv,
            );
        }
        self
    }
}
/// A synthetic textured aircraft for per-face texture smoke tests: a box
/// fuselage whose six faces map separate regions of the 32 x 32 KIT.PIC,
/// flat wings, a textured fin, and a gear leg on a C4 transform with a
/// textured face. Native end marker and import tail. Not a game asset.
pub fn demo_textured_kit() -> Vec<u8> {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x10, 0]);
    a.b(&[0xe2, 0]).b(b"KIT.PIC\0\0\0\0\0\0\0");
    let box_points: [[i16; 3]; 8] = core::array::from_fn(|i| {
        [
            if i & 1 == 0 { -5 } else { 5 },
            if i & 2 == 0 { -40 } else { 40 },
            if i & 4 == 0 { -4 } else { 4 },
        ]
    });
    let inside = [0, 0, 0];
    let region = |k: u16| -> Vec<[u16; 2]> {
        let (u, v) = ((k % 2) * 16, (k / 2) * 10);
        alloc::vec![[u, v], [u + 15, v], [u + 15, v + 9], [u, v + 9]]
    };
    a.solid_uv(
        0,
        &box_points,
        &[
            (&[0, 1, 3, 2], inside),
            (&[4, 5, 7, 6], inside),
            (&[0, 1, 5, 4], inside),
            (&[2, 3, 7, 6], inside),
            (&[0, 2, 6, 4], inside),
            (&[1, 3, 7, 5], inside),
        ],
        150,
        &(0..6).map(region).collect::<Vec<_>>(),
    );
    plate(
        &mut a,
        8,
        [[-5, -6, 0], [-40, -12, 0], [-40, -2, 0], [-5, 10, 0]],
        151,
    );
    plate(
        &mut a,
        12,
        [[5, -6, 0], [40, -12, 0], [40, -2, 0], [5, 10, 0]],
        151,
    );
    let fin: [[i16; 3]; 4] = [[0, -30, 4], [0, -40, 4], [0, -40, 16], [0, -35, 16]];
    a.solid_uv(
        16,
        &fin,
        &[
            (&[0, 1, 2, 3], [1, -35, 10]),
            (&[0, 1, 2, 3], [-1, -35, 10]),
        ],
        152,
        &[
            alloc::vec![[0, 30], [9, 30], [9, 31], [0, 31]],
            alloc::vec![[10, 30], [19, 30], [19, 31], [10, 31]],
        ],
    );
    a.xform(
        "gl",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        Shift::One,
        true,
        0x0a,
        [-12, -4, 2],
        "legl",
        "s1",
    );
    a.label("s1").b(&[0xca, 0, 0, 0]);
    a.jump("end");
    a.label("legl");
    let leg = [[0, 0, 0], [0, 0, -10], [1, 0, -10], [1, 0, 0]];
    a.solid_uv(
        20,
        &leg,
        &[(&[0, 1, 2, 3], [0, 1, -5]), (&[0, 1, 2, 3], [0, -1, -5])],
        153,
        &[
            alloc::vec![[20, 30], [23, 30], [23, 31], [20, 31]],
            alloc::vec![[24, 30], [27, 30], [27, 31], [24, 31]],
        ],
    );
    a.b(&[0x1e]);
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    a.finish()
}
/// The textured kit's box drawn in a loop that switches from KIT.PIC to
/// SAME.PIC halfway (an AC branch back to the start): the first three faces
/// may be drawn from either PIC, so no texture edit can prove their state;
/// the last three always draw from SAME.PIC. Not a game asset.
pub fn demo_looped_kit() -> Vec<u8> {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x10, 0]);
    a.b(&[0xe2, 0]).b(b"KIT.PIC\0\0\0\0\0\0\0");
    let points: [[i16; 3]; 8] = core::array::from_fn(|i| {
        [
            if i & 1 == 0 { -5 } else { 5 },
            if i & 2 == 0 { -40 } else { 40 },
            if i & 4 == 0 { -4 } else { 4 },
        ]
    });
    let sides: [(&[u16], [i32; 3]); 6] = [
        (&[0, 1, 3, 2], [0; 3]),
        (&[4, 5, 7, 6], [0; 3]),
        (&[0, 1, 5, 4], [0; 3]),
        (&[2, 3, 7, 6], [0; 3]),
        (&[0, 2, 6, 4], [0; 3]),
        (&[1, 3, 7, 5], [0; 3]),
    ];
    let region = |k: u16| -> Vec<[u16; 2]> {
        let (u, v) = ((k % 2) * 16, (k / 2) * 10);
        alloc::vec![[u, v], [u + 15, v], [u + 15, v + 9], [u, v + 9]]
    };
    a.label("top");
    a.solid_uv(
        0,
        &points,
        &sides[..3],
        150,
        &(0..3).map(region).collect::<Vec<_>>(),
    );
    a.b(&[0xe2, 0]).b(b"SAME.PIC\0\0\0\0\0\0");
    a.solid_uv(
        8,
        &points,
        &sides[3..],
        150,
        &(3..6).map(region).collect::<Vec<_>>(),
    );
    a.call(0xac, "top");
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    a.finish()
}
/// A flat quad drawn from both sides.
fn plate(a: &mut Asm, slot: u16, p: [[i16; 3]; 4], color: u8) {
    let c: [i32; 3] = core::array::from_fn(|k| p.iter().map(|q| q[k] as i32).sum::<i32>() / 4);
    let n = crate::model::face_normal(&p.map(|q| q.map(|v| v as i32))).unwrap_or([0, 0, 1]);
    let off = |s: i32| -> [i32; 3] { core::array::from_fn(|k| c[k] + s * n[k].signum()) };
    a.solid(
        slot,
        &p,
        &[(&[0, 1, 2, 3], off(-1)), (&[0, 1, 2, 3], off(1))],
        color,
    );
}
/// A synthetic aircraft with animated parts, for smoke tests and demos:
/// a lit flat body, gear legs on C4 transforms (a D1 + NEG slot on the left
/// leg, D1 alone on the right, a C1 shift on the nose leg), and toggled
/// meshes for flaps, rudder, hook, speed brake and afterburner. Not a game
/// asset; coordinates are arbitrary source units.
pub fn demo_parts() -> Vec<u8> {
    let mut a = Asm::default();
    a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x10, 0]);
    // Fuselage box (slots 0..8), wings (8..16) and fin (16..20).
    let box_points: [[i16; 3]; 8] = core::array::from_fn(|i| {
        [
            if i & 1 == 0 { -5 } else { 5 },
            if i & 2 == 0 { -40 } else { 40 },
            if i & 4 == 0 { -4 } else { 4 },
        ]
    });
    let inside = [0, 0, 0];
    a.solid(
        0,
        &box_points,
        &[
            (&[0, 1, 3, 2], inside),
            (&[4, 5, 7, 6], inside),
            (&[0, 1, 5, 4], inside),
            (&[2, 3, 7, 6], inside),
            (&[0, 2, 6, 4], inside),
            (&[1, 3, 7, 5], inside),
        ],
        150,
    );
    plate(
        &mut a,
        8,
        [[-5, -6, 0], [-40, -12, 0], [-40, -2, 0], [-5, 10, 0]],
        151,
    );
    plate(
        &mut a,
        12,
        [[5, -6, 0], [40, -12, 0], [40, -2, 0], [5, 10, 0]],
        151,
    );
    plate(
        &mut a,
        16,
        [[0, -30, 4], [0, -40, 4], [0, -40, 16], [0, -35, 16]],
        152,
    );
    // Stubs: gear transforms (pivots stored right, up, forward), then toggles.
    let leg = [[0, 0, 0], [0, 0, -10], [1, 0, -10], [1, 0, 0]];
    a.xform(
        "gl",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        Shift::One,
        true,
        0x0a,
        [-12, -4, 2],
        "legl",
        "s1",
    );
    a.label("s1");
    a.xform(
        "gr",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        Shift::One,
        false,
        0x0a,
        [12, -4, 2],
        "legr",
        "s2",
    );
    a.label("s2");
    a.xform(
        "gn",
        Some(("_PLgearDown", 1)),
        "_PLgearPos",
        Shift::Imm(3),
        false,
        0x08,
        [0, -4, 30],
        "legn",
        "s3",
    );
    a.label("s3");
    let toggles: [(&str, i8, &str, [[i16; 3]; 4]); 8] = [
        (
            "_PLleftFlap",
            -1,
            "fl1",
            [[-10, -6, 0], [-30, -10, 0], [-30, -14, -4], [-10, -10, -4]],
        ),
        (
            "_PLleftFlap",
            0,
            "fl0",
            [[-10, -6, 0], [-30, -10, 0], [-30, -15, 0], [-10, -11, 0]],
        ),
        (
            "_PLrightFlap",
            -1,
            "fr1",
            [[10, -6, 0], [30, -10, 0], [30, -14, -4], [10, -10, -4]],
        ),
        (
            "_PLrightFlap",
            0,
            "fr0",
            [[10, -6, 0], [30, -10, 0], [30, -15, 0], [10, -11, 0]],
        ),
        (
            "_PLrudder",
            0,
            "rd0",
            [[0, -40, 4], [0, -44, 4], [0, -44, 16], [0, -40, 16]],
        ),
        (
            "_PLhook",
            1,
            "hk",
            [[0, -36, -4], [0, -46, -9], [1, -46, -9], [1, -36, -4]],
        ),
        (
            "_PLbrake",
            1,
            "bk",
            [[-3, 0, 4], [3, 0, 4], [3, -4, 9], [-3, -4, 9]],
        ),
        (
            "_PLafterBurner",
            1,
            "ab",
            [[-3, -40, -2], [3, -40, -2], [3, -52, 0], [-3, -52, 0]],
        ),
    ];
    for (k, (var, value, name, _)) in toggles.iter().enumerate() {
        let after = format!("t{k}");
        a.toggle(var, *value, 0x75, name, &format!("{name}.mesh"), &after);
        a.label(&after);
    }
    a.jump("end");
    for (k, name) in ["legl", "legr", "legn"].iter().enumerate() {
        a.label(name);
        plate(&mut a, 20 + 4 * k as u16, leg, 153);
        a.b(&[0x1e]);
    }
    for (k, (_, _, name, quad)) in toggles.iter().enumerate() {
        a.label(&format!("{name}.mesh"));
        plate(&mut a, 32 + 4 * k as u16, *quad, 154 + (k % 3) as u8);
        a.b(&[0x1e]);
    }
    a.label("end")
        .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
    a.finish()
}
