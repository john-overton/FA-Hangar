//! Manual SH inventory and stub census checks against user-supplied LIBs (not CI).
use hangar_core::{
    archive::Archive,
    shape_code::{
        x86::Outcome, Binding, BindingKind, Condition, Inventory, Kind, LawOp, PointerKind, Rel,
        Target,
    },
    Result,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

pub fn rel(r: Rel) -> &'static str {
    match r {
        Rel::Eq => "==",
        Rel::Ne => "!=",
        Rel::Lt => "<",
        Rel::Ge => ">=",
        Rel::Le => "<=",
        Rel::Gt => ">",
        Rel::Below => "<u",
        Rel::AboveEq => ">=u",
        Rel::BelowEq => "<=u",
        Rel::Above => ">u",
    }
}
pub fn conditions(c: &[Condition]) -> String {
    if c.is_empty() {
        return "always".into();
    }
    c.iter()
        .map(|c| format!("{}{}{}", c.variable, rel(c.rel), c.value))
        .collect::<Vec<_>>()
        .join(" & ")
}
pub fn law(l: &[LawOp]) -> String {
    l.iter()
        .map(|o| match o {
            LawOp::Var(v) => v.clone(),
            LawOp::Const(n) => n.to_string(),
            LawOp::Neg { w16 } => format!("neg{}", if *w16 { "" } else { "32" }),
            LawOp::Add { w16 } => format!("+{}", if *w16 { "" } else { "32" }),
            LawOp::Sub { w16 } => format!("-{}", if *w16 { "" } else { "32" }),
            LawOp::Mul { w16 } => format!("*{}", if *w16 { "" } else { "32" }),
            LawOp::Sar { n, w16 } => format!("sar{n}{}", if *w16 { "" } else { "/32" }),
            LawOp::Shl { n, w16 } => format!("shl{n}{}", if *w16 { "" } else { "/32" }),
            LawOp::Shr { n, w16 } => format!("shr{n}{}", if *w16 { "" } else { "/32" }),
        })
        .collect::<Vec<_>>()
        .join(" ")
}
const WORDS: [&str; 6] = [
    "tx",
    "ty(up)",
    "tz(fwd)",
    "r0(yaw)",
    "r1(pitch)",
    "r2(roll)",
];
pub fn binding(b: &Binding) -> String {
    let when = b
        .when
        .iter()
        .map(|c| conditions(c))
        .collect::<Vec<_>>()
        .join(" | ");
    let mut s = format!(
        "stub {:05X} -> {:02X}@{:05X}",
        b.stub, b.target_op, b.target
    );
    if let Some(t) = b.block {
        let _ = write!(s, " block {t:05X}");
    }
    if let Some(p) = b.pivot {
        let _ = write!(s, " pivot {p:?}");
    }
    match &b.kind {
        BindingKind::Toggle => {
            let _ = write!(s, " toggle when {when}");
        }
        BindingKind::Xform { writes } => {
            let _ = write!(s, " xform when {when}:");
            for (w, l) in writes {
                let _ = write!(s, " {}={}", WORDS[*w], law(l));
            }
        }
    }
    s
}
fn kind(k: Kind) -> String {
    match k {
        Kind::Sh(op) => format!("{op:02X}"),
        Kind::Pad => "pad".into(),
        Kind::EndObject => "end-object".into(),
        Kind::X86 { header: true } => "x86".into(),
        Kind::X86 { header: false } => "x86+".into(),
        Kind::X86Data => "x86-data".into(),
        Kind::EndShape => "end-shape".into(),
        Kind::Trampoline => "trampoline".into(),
        Kind::Opaque => "OPAQUE".into(),
    }
}
fn outcome(o: &Outcome) -> String {
    let mut s = format!("{} -> {:05X}", conditions(&o.conditions), o.resume);
    for st in &o.stores {
        let _ = write!(s, " [{:05X}]={}", st.at, law(&st.law));
    }
    s
}
fn shapes(path: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let archive = Archive::parse(crate::platform::read(path)?)?;
    let mut out = Vec::new();
    for e in &archive.entries {
        if e.name.ends_with(".SH") {
            out.push((e.name.clone(), e.read()?));
        }
    }
    Ok(out)
}
fn add(
    map: &mut BTreeMap<String, BTreeMap<String, Vec<String>>>,
    group: &str,
    key: String,
    shape: &str,
) {
    map.entry(group.to_string())
        .or_default()
        .entry(key)
        .or_default()
        .push(shape.to_string());
}
fn table(out: &mut String, title: &str, map: &BTreeMap<String, BTreeMap<String, Vec<String>>>) {
    let _ = writeln!(out, "\n== {title}");
    for (group, rows) in map {
        let total: usize = rows.values().map(Vec::len).sum();
        let _ = writeln!(out, "\n-- {group}: {} distinct, {total} total", rows.len());
        let mut rows: Vec<_> = rows.iter().collect();
        rows.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        for (key, shapes) in rows {
            let mut names: Vec<_> = shapes.iter().map(|s| s.trim_end_matches(".SH")).collect();
            names.dedup();
            let _ = writeln!(out, "{:5}  {key}\n       {}", shapes.len(), names.join(" "));
        }
    }
}
/// Address-masked F0 stub signatures and binding parameters over several LIBs.
pub fn census(libs: &[String]) -> Result<String> {
    let mut signatures = BTreeMap::new();
    let mut params = BTreeMap::new();
    let mut unrecognised = BTreeMap::new();
    let mut imports = BTreeMap::<String, (usize, Vec<String>)>::new();
    let mut reloc_room = BTreeMap::<String, usize>::new();
    let mut animated = 0;
    let mut out = String::new();
    for lib in libs {
        let list = match shapes(lib) {
            Ok(l) => l,
            Err(e) => {
                let _ = writeln!(out, "{lib}: {e}");
                continue;
            }
        };
        let _ = writeln!(out, "{lib}: {} SH entries", list.len());
        for (name, bytes) in &list {
            let Ok(inv) = Inventory::parse(bytes) else {
                continue;
            };
            let code = &bytes[inv.code_start..inv.code_start + inv.code_len];
            for s in &inv.stubs {
                let mut vars: BTreeSet<String> = BTreeSet::new();
                for o in &s.outcomes {
                    vars.extend(o.conditions.iter().map(|c| c.variable.clone()));
                    for st in &o.stores {
                        vars.extend(st.law.iter().filter_map(|l| match l {
                            LawOp::Var(v) => Some(v.clone()),
                            _ => None,
                        }));
                    }
                }
                let group = if vars.is_empty() {
                    "(no variable)".to_string()
                } else {
                    vars.into_iter().collect::<Vec<_>>().join("+")
                };
                if let Some((_, why)) = &s.unrecognised {
                    add(
                        &mut unrecognised,
                        why.split(" at ").next().unwrap_or(why),
                        s.signature.clone(),
                        name,
                    );
                } else if group != "(no variable)" {
                    add(&mut signatures, &group, s.signature.clone(), name);
                }
            }
            if inv.bindings.is_empty() {
                continue;
            }
            animated += 1;
            let mut used = BTreeSet::new();
            for b in &inv.bindings {
                let target = inv.starting_at(b.target).map(|i| &inv.records[i]);
                let stored = target
                    .filter(|r| matches!(r.kind, Kind::Sh(0xc4 | 0xc6)))
                    .map(|r| {
                        (0..3)
                            .map(|k| {
                                i16::from_le_bytes([
                                    code[r.offset + 8 + 2 * k],
                                    code[r.offset + 9 + 2 * k],
                                ])
                            })
                            .collect::<Vec<_>>()
                    });
                let when = b
                    .when
                    .iter()
                    .map(|c| conditions(c))
                    .collect::<Vec<_>>()
                    .join(" | ");
                let key = match &b.kind {
                    BindingKind::Toggle => format!(
                        "toggle {:02X} when {when}{}",
                        b.target_op,
                        stored
                            .map(|r| format!(" stored rot {r:?}"))
                            .unwrap_or_default()
                    ),
                    BindingKind::Xform { writes } => {
                        let w: Vec<_> = writes
                            .iter()
                            .map(|(k, l)| format!("{}={}", WORDS[*k], law(l)))
                            .collect();
                        format!("xform {} when {when}", w.join(" "))
                    }
                };
                let pivot = b.pivot.map(|p| format!(" pivot {p:?}")).unwrap_or_default();
                for v in b.variables() {
                    used.insert(v.clone());
                    add(&mut params, &v, key.clone(), name);
                    if !pivot.is_empty() {
                        add(
                            &mut params,
                            &format!("{v} pivots"),
                            format!("{}{pivot}", key.split(" when ").next().unwrap_or("")),
                            name,
                        );
                    }
                }
            }
            for v in used {
                let e = imports.entry(v).or_default();
                e.0 += 1;
            }
            // Which known animation inputs this animated shape does not import.
            for v in [
                "_PLgearDown",
                "_PLgearPos",
                "_PLleftFlap",
                "_PLrightFlap",
                "_PLbrake",
                "_PLrudder",
                "_PLhook",
                "_PLafterBurner",
                "_PLbayOpen",
                "_PLbayDoorPos",
                "_PLswingWing",
                "_PLcanardPos",
                "do_start_interp",
            ] {
                if !inv.imports.iter().any(|i| i.name == v && i.alias.is_some()) {
                    imports
                        .entry(v.to_string())
                        .or_default()
                        .1
                        .push(name.trim_end_matches(".SH").to_string());
                }
            }
            let room = reloc_slack(bytes).map_or("unknown".to_string(), |n| match n {
                0 => "0".into(),
                1..=15 => "1-15".into(),
                16..=63 => "16-63".into(),
                64..=511 => "64-511".into(),
                _ => "512+".into(),
            });
            *reloc_room.entry(room).or_default() += 1;
        }
    }
    let _ = writeln!(out, "{animated} shapes with at least one part binding");
    table(&mut out, "F0 stub signatures by variable set (addresses masked: <NAME> import alias, <sh> CODE address, <j> branch, <k> self-offset)", &signatures);
    table(&mut out, "Binding parameters by variable (C4 words: tx, ty(up), tz(fwd), r0 about up, r1 about right, r2 about forward; 8192 units = 180 degrees)", &params);
    table(
        &mut out,
        "Unrecognised stubs by first unsupported step",
        &unrecognised,
    );
    let _ = writeln!(out, "\n== Imports among animated shapes (aliased = has an FF 25 trampoline the stub can reference)");
    for (v, (n, missing)) in &imports {
        let _ = writeln!(
            out,
            "{v}: bound in {n} shapes; not imported by {} animated shapes: {}",
            missing.len(),
            missing.join(" ")
        );
    }
    let _ = writeln!(out, "\n== Free bytes in .reloc raw data after the relocation directory (animated shapes): {reloc_room:?}");
    Ok(out)
}
/// Raw `.reloc` bytes beyond the relocation directory size.
fn reloc_slack(data: &[u8]) -> Option<usize> {
    let u32_at = |at: usize| {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let u16_at = |at: usize| {
        data.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let pe = u32_at(60)?;
    let opt = pe + 24;
    let table = opt + u16_at(pe + 20)?;
    let used = u32_at(opt + 96 + 5 * 8 + 4)?;
    for i in 0..u16_at(pe + 6)?.min(32) {
        let h = table + i * 40;
        if data.get(h..h + 6)? == b".reloc" {
            return u32_at(h + 16)?.checked_sub(used);
        }
    }
    None
}
/// Per-part vertex extents for a symbolic pose, e.g. `_PLgearDown=1 _PLgearPos=-8192`.
pub fn pose(path: &str, entry: &str, settings: &[String]) -> Result<String> {
    let archive = Archive::parse(crate::platform::read(path)?)?;
    let at = archive.find(entry).ok_or("Entry not found")?;
    let bytes = archive.entries[at].read()?;
    let mut pose = hangar_core::model::Pose::new();
    for s in settings {
        let (k, v) = s.split_once('=').ok_or("Use NAME=VALUE")?;
        pose.insert(k.to_string(), v.parse().map_err(|_| "Invalid value")?);
    }
    let m = hangar_core::model::Model::with_pose(&bytes, &pose)?;
    let mut out = format!(
        "{entry} pose {pose:?}: {} vertices, {} faces, {} parts, {} groups\n",
        m.vertices.len(),
        m.faces.len(),
        m.parts.len(),
        m.groups.len()
    );
    for (i, p) in m.parts.iter().enumerate() {
        let pts: Vec<_> = m
            .vertices
            .iter()
            .zip(&m.vertex_tags)
            .filter(|(_, t)| t.part == Some(i))
            .map(|(v, _)| v.point)
            .collect();
        let lo: Vec<_> = (0..3)
            .map(|k| pts.iter().map(|p| p[k]).min().unwrap_or(0))
            .collect();
        let hi: Vec<_> = (0..3)
            .map(|k| pts.iter().map(|p| p[k]).max().unwrap_or(0))
            .collect();
        let _ = writeln!(
            out,
            "  part {i} C4@{:X} pivot {:?} stored rot {:?} posed rot {:?}: {} vertices, min {lo:?} max {hi:?}",
            p.offset, p.posed_position, p.rotation, p.posed_rotation, pts.len()
        );
    }
    Ok(out)
}
pub fn inventory(path: &str, entry: Option<&str>) -> Result<String> {
    let mut out = String::new();
    let mut covered = 0;
    let mut with_opaque = 0;
    let mut failed = 0;
    let mut opaque_ops = BTreeMap::<u8, usize>::new();
    let mut misaligned = BTreeMap::<String, usize>::new();
    let mut examples = Vec::new();
    let mut grammar = BTreeMap::<String, usize>::new();
    let mut self_offsets = BTreeMap::<String, usize>::new();
    let mut unrec = BTreeMap::<String, usize>::new();
    let mut vars = BTreeMap::<String, usize>::new();
    let list = shapes(path)?;
    for (name, bytes) in &list {
        if entry.is_some_and(|e| !e.eq_ignore_ascii_case(name)) {
            continue;
        }
        let inv = match std::panic::catch_unwind(|| Inventory::parse(bytes)) {
            Ok(Ok(inv)) => inv,
            Ok(Err(e)) => {
                failed += 1;
                let _ = writeln!(out, "{name}: ERROR {e}");
                continue;
            }
            Err(_) => {
                failed += 1;
                let _ = writeln!(out, "{name}: PANIC");
                continue;
            }
        };
        let code = &bytes[inv.code_start..inv.code_start + inv.code_len];
        let opaque = inv.opaque_bytes();
        if opaque == 0 && inv.contiguous() {
            covered += 1;
        } else {
            with_opaque += 1;
        }
        for s in &inv.stubs {
            for o in &s.outcomes {
                let k = inv.starting_at(o.resume).map(|i| inv.records[i].kind);
                if !o.conditions.is_empty() || !o.stores.is_empty() {
                    let key = match k {
                        Some(Kind::Sh(op @ (0x12 | 0x6e | 0xc4 | 0xc6))) => {
                            format!("resume {op:02X}")
                        }
                        Some(Kind::X86 { .. }) => "resume x86".into(),
                        Some(k) => format!("resume {}", kind(k)),
                        None => "resume none".into(),
                    };
                    *grammar.entry(key).or_default() += 1;
                }
            }
        }
        for r in &inv.records {
            if r.kind == Kind::Sh(0xfc) && code[r.offset + 1] & 0x60 == 0x20 {
                *grammar
                    .entry("FC content 0x20 without 0x40".into())
                    .or_default() += 1;
            }
            if r.kind == Kind::EndObject {
                *grammar
                    .entry(format!(
                        "end-object {} bytes",
                        if r.len == 18 {
                            "18".into()
                        } else {
                            format!("{}", r.len)
                        }
                    ))
                    .or_default() += 1;
            }
            if let Kind::Sh(op @ (0x06 | 0x0c | 0x0e | 0x10 | 0x6c)) = r.kind {
                *grammar
                    .entry(format!(
                        "{op:02X} len {} next {:02X}",
                        r.len,
                        code.get(r.end()).copied().unwrap_or(0)
                    ))
                    .or_default() += 1;
            }
            if r.kind == Kind::Opaque {
                *opaque_ops.entry(code[r.offset]).or_default() += 1;
            }
            for p in &r.pointers {
                if let Target::Code(t) = p.target {
                    // x86 branches land on instructions inside x86 records, and
                    // self-offsets address C4 words or x86 data.
                    let holder = inv.record_index(t).map(|q| inv.records[q].kind);
                    let inside_x86 = matches!(p.kind, PointerKind::X86Rel8 | PointerKind::X86Rel32)
                        && matches!(holder, Some(Kind::X86 { .. }));
                    if p.kind == PointerKind::SelfOffset && inv.starting_at(t).is_none() {
                        let what = match holder {
                            Some(Kind::Sh(0xc4 | 0xc6)) => "C4/C6 words".to_string(),
                            Some(k) => kind(k),
                            None => "none".into(),
                        };
                        *self_offsets.entry(what).or_default() += 1;
                    } else if inv.starting_at(t).is_none() && !inside_x86 {
                        let key = format!("{} {:?}", kind(r.kind), p.kind);
                        *misaligned.entry(key).or_default() += 1;
                        if examples.len() < 12 {
                            examples.push(format!(
                                "{name} {:05X}->{t:05X} in {}",
                                p.field,
                                inv.record_index(t)
                                    .map_or("none".into(), |q| kind(inv.records[q].kind))
                            ));
                        }
                    }
                }
            }
        }
        let x86data: usize = inv
            .records
            .iter()
            .filter(|r| r.kind == Kind::X86Data)
            .map(|r| r.len)
            .sum();
        let unreached = inv
            .records
            .iter()
            .filter(|r| !r.reached && matches!(r.kind, Kind::Sh(_)))
            .count();
        let mut names = BTreeSet::new();
        for b in &inv.bindings {
            let k = if matches!(b.kind, BindingKind::Toggle) {
                "toggle"
            } else {
                "xform"
            };
            for v in b.variables() {
                names.insert(format!("{v}:{k}"));
            }
        }
        for n in &names {
            *vars.entry(n.clone()).or_default() += 1;
        }
        let bad: Vec<_> = inv
            .stubs
            .iter()
            .filter_map(|s| s.unrecognised.as_ref().map(|u| (s.offset, u.1.clone())))
            .collect();
        for (_, why) in &bad {
            let key = why.split(" at ").next().unwrap_or(why).to_string();
            *unrec.entry(key).or_default() += 1;
        }
        let _ = writeln!(
            out,
            "{name}: {} records, opaque {opaque}, x86-data {x86data}, unreached SH {unreached}, {} stubs ({} unrecognised), {} slots; {}",
            inv.records.len(),
            inv.stubs.len(),
            bad.len(),
            inv.slots.len(),
            names.into_iter().collect::<Vec<_>>().join(" ")
        );
        if entry.is_some() {
            for i in &inv.imports {
                let _ = writeln!(
                    out,
                    "  import {:24} iat {:08X} alias {:?}",
                    i.name,
                    i.iat,
                    i.alias.map(|a| format!("{a:05X}"))
                );
            }
            for r in &inv.records {
                let mut line = format!(
                    "  @{:05X} {:5} {:10} {}",
                    r.offset,
                    r.len,
                    kind(r.kind),
                    if r.reached { "R" } else { "-" }
                );
                for p in &r.pointers {
                    let t = match p.target {
                        Target::Code(t) => format!(
                            "{t:05X}{}",
                            if inv.starting_at(t).is_some() {
                                ""
                            } else {
                                "!"
                            }
                        ),
                        Target::External(v) => format!("ext {v:08X}"),
                    };
                    let _ = write!(line, " {:?}@{:05X}->{t}", p.kind, p.field);
                }
                let _ = writeln!(out, "{line}");
            }
            for s in &inv.stubs {
                let _ = writeln!(out, "  stub {:05X}: {}", s.offset, s.signature);
                for o in &s.outcomes {
                    let _ = writeln!(out, "    {}", outcome(o));
                }
                if let Some((at, why)) = &s.unrecognised {
                    let _ = writeln!(out, "    UNRECOGNISED at {at:05X}: {why}");
                }
            }
            for b in &inv.bindings {
                let _ = writeln!(out, "  binding {}", binding(b));
            }
        }
    }
    let _ = writeln!(
        out,
        "TOTAL {path}: {} shapes, {covered} fully covered, {with_opaque} with opaque spans, {failed} failed",
        covered + with_opaque + failed
    );
    let _ = writeln!(out, "Opaque span first bytes: {opaque_ops:02X?}");
    let _ = writeln!(
        out,
        "Pointer targets not at a record start: {misaligned:?} e.g. {examples:?}"
    );
    let _ = writeln!(out, "x86 self-relative data targets: {self_offsets:?}");
    let _ = writeln!(out, "Grammar evidence: {grammar:?}");
    let _ = writeln!(out, "Unrecognised stub reasons: {unrec:?}");
    let _ = writeln!(out, "Shapes per variable binding: {vars:?}");
    Ok(out)
}
/// Manual real-data check of region geometry edits on in-memory copies of
/// retail shapes. Each edit is re-parsed (whole-CODE inventory coverage,
/// geometry analysis, model reader) and written create-new to `out_dir`.
pub fn geometry_check(out_dir: &str, lib: &str, names: &[String]) -> Result<String> {
    use hangar_core::{
        model::Model,
        shape_geometry::{self as geo, Base, FaceStyle, Frame, Geometry},
    };
    let archive = Archive::parse(crate::platform::read(lib)?)?;
    let mut out = String::new();
    let mut failed = 0;
    if names.is_empty() {
        // Analysis only, over every SH: writability and slot proofs.
        let (mut count, mut errors, mut verts, mut writable, mut faces, mut editable) =
            (0, 0, 0, 0, 0, 0);
        let mut parts = (0, 0);
        let mut reasons = BTreeMap::<String, usize>::new();
        for e in archive.entries.iter().filter(|e| e.name.ends_with(".SH")) {
            let (name, bytes) = (&e.name, e.read()?);
            count += 1;
            let g = match Geometry::parse(&bytes) {
                Ok(g) => g,
                Err(e) => {
                    errors += 1;
                    let _ = writeln!(out, "{name}: {e}");
                    continue;
                }
            };
            let report = g.vertex_report();
            verts += report.len();
            writable += report.iter().filter(|v| v.refusal.is_none()).count();
            faces += g.faces.len();
            // Reasons keyed without offsets, so like refusals group together.
            let key = |r: &str| -> String {
                r.split(' ')
                    .map(|w| {
                        if w.contains(|c: char| c.is_ascii_digit()) {
                            "#"
                        } else {
                            w
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            for r in report.iter().filter_map(|v| v.refusal.as_deref()) {
                *reasons.entry(format!("vertex: {}", key(r))).or_default() += 1;
            }
            for f in 0..g.faces.len() {
                match g.face_refusal(f) {
                    None => editable += 1,
                    Some(r) => *reasons.entry(format!("face: {}", key(&r))).or_default() += 1,
                }
            }
            if let Ok(list) = hangar_core::shape_parts::parts(&bytes) {
                parts.0 += list.len();
                parts.1 += list.iter().filter(|p| p.locked.is_some()).count();
                for p in list.iter().filter(|p| p.locked.is_some()) {
                    let r = p.locked.as_deref().unwrap_or("");
                    *reasons.entry(format!("part: {}", key(r))).or_default() += 1;
                }
            }
        }
        let _ = writeln!(
            out,
            "{count} SH, {errors} not analysable; vertices {writable}/{verts} writable; faces {editable}/{faces} editable; parts {} ({} locked)",
            parts.0, parts.1
        );
        for (r, n) in &reasons {
            let _ = writeln!(out, "  {n:6} {r}");
        }
        return Ok(out);
    }
    for name in names {
        let at = archive
            .find(name)
            .ok_or_else(|| format!("{name} not found"))?;
        let bytes = archive.entries[at].read()?;
        let g = Geometry::parse(&bytes)?;
        let inv = &g.inventory;
        let report = g.vertex_report();
        let writable = report.iter().filter(|v| v.refusal.is_none()).count();
        let editable = (0..g.faces.len())
            .filter(|f| g.face_refusal(*f).is_none())
            .count();
        let frames: BTreeSet<_> = g.buffers.iter().map(|b| b.frame).collect();
        let _ = writeln!(
            out,
            "{name}: CODE {} bytes, {} records, opaque {}, contiguous {}; {} faces ({editable} editable), {} vertices ({writable} writable), {} frames",
            inv.code_len,
            inv.records.len(),
            inv.opaque_bytes(),
            inv.contiguous(),
            g.faces.len(),
            report.len(),
            frames.len()
        );
        let mut reasons = BTreeMap::<String, usize>::new();
        for v in &report {
            if let Some(r) = &v.refusal {
                *reasons.entry(r.clone()).or_default() += 1;
            }
        }
        for (r, n) in reasons.iter().take(4) {
            let _ = writeln!(out, "  refused vertices {n}: {r}");
        }
        let model = Model::parse(&bytes).ok();
        let face = (0..g.faces.len())
            .filter(|f| {
                let r = &g.faces[*f];
                r.frame == Frame::Root
                    && r.normal.is_some()
                    && r.content & 0x80 == 0
                    && g.face_refusal(*f).is_none()
                    && model
                        .as_ref()
                        .is_none_or(|m| m.faces.iter().any(|x| x.offset == r.offset))
            })
            .nth(3)
            .ok_or("No editable root face")?;
        let f = g.faces[face].clone();
        let corners: Vec<usize> = f
            .slots
            .iter()
            .map(|s| {
                let b = g.writer(face, *s)?;
                Ok(g.buffers[b].vertex(*s - g.buffers[b].slot))
            })
            .collect::<Result<_>>()?;
        let n = f.normal.unwrap_or([0, 0, 1]);
        let axis = (0..3).max_by_key(|k| n[*k].abs()).unwrap_or(2);
        let mut push: [i32; 3] = [0; 3];
        push[axis] = 2 * n[axis].signum();
        let part_vertex = report
            .iter()
            .find(|v| matches!(v.frame, Frame::Part(_)) && v.refusal.is_none())
            .or_else(|| report.iter().find(|v| v.refusal.is_none()))
            .ok_or("No writable vertex")?;
        let mut moved = part_vertex.local;
        moved[2] += 1;
        let mut reversed = corners.clone();
        reversed.reverse();
        let mut style = FaceStyle::like(&f);
        style.uv.reverse();
        let edits: Vec<(&str, Result<Vec<u8>>)> = vec![
            ("delete", geo::delete_faces(&bytes, &[f.offset])),
            ("flip", geo::flip_faces(&bytes, &[f.offset])),
            (
                "add",
                geo::add_face(&bytes, &reversed, &style).map(|a| a.shape),
            ),
            (
                "duplicate",
                geo::duplicate_faces(&bytes, &[f.offset], push).map(|a| a.shape),
            ),
            (
                "extrude",
                geo::extrude_faces(&bytes, &[f.offset], push, Base::Flip).map(|a| a.shape),
            ),
            (
                "move",
                geo::write_vertices(&bytes, &[(part_vertex.offset, moved)]),
            ),
            ("scale", geo::scale_faces(&bytes, &[f.offset], 110)),
        ];
        for (op, result) in edits {
            let line = match result
                .and_then(|shape| check_shape(&shape, inv, model.as_ref(), out_dir, name, op))
            {
                Ok(s) => format!("PASS {s}"),
                Err(e) => {
                    failed += 1;
                    format!("FAIL {e}")
                }
            };
            let _ = writeln!(out, "  {op:9} face {:X}: {line}", f.offset);
        }
        failed += part_check(&mut out, &bytes, name, out_dir)?;
    }
    let _ = writeln!(out, "{failed} failed");
    Ok(out)
}
/// Re-parse an edited shape: whole-CODE coverage unchanged, every drawn
/// face slot provable, and the model reader still accepting it.
fn check_shape(
    shape: &[u8],
    before: &Inventory,
    model: Option<&hangar_core::model::Model>,
    out_dir: &str,
    name: &str,
    op: &str,
) -> Result<String> {
    use hangar_core::shape_geometry::{Frame, Geometry};
    let after = Geometry::parse(shape)?;
    let ai = &after.inventory;
    if !ai.contiguous() || ai.opaque_bytes() != before.opaque_bytes() {
        return Err("inventory coverage changed".into());
    }
    if ai.bindings != before.bindings {
        return Err("part bindings changed".into());
    }
    let unresolved = (0..after.faces.len())
        .filter(|j| after.faces[*j].frame != Frame::Unreached)
        .flat_map(|j| after.faces[j].slots.iter().map(move |s| (j, *s)))
        .filter(|(j, s)| after.writer(*j, *s).is_err())
        .count();
    let faces = match (hangar_core::model::Model::parse(shape), model) {
        (Ok(m), Some(b)) => format!("model faces {} -> {}", b.faces.len(), m.faces.len()),
        (Err(e), Some(_)) => return Err(format!("model reader fails: {e}")),
        (_, None) => "model reader n/a".into(),
    };
    let stem = name.trim_end_matches(".SH");
    crate::platform::write_new(&format!("{out_dir}/{stem}-{op}.SH"), shape)?;
    Ok(format!(
        "{} bytes, inventory 100% ({} records), {} stubs, unresolved slots {unresolved}, {faces}",
        shape.len(),
        ai.records.len(),
        ai.stubs.len()
    ))
}
/// Part names and controls, then one in-place edit per editable control kind,
/// each re-parsed and written create-new to `out_dir`.
fn part_check(out: &mut String, bytes: &[u8], name: &str, out_dir: &str) -> Result<usize> {
    use hangar_core::shape_parts::{self as parts, Allowed, Axis, Setting};
    let list = parts::parts(bytes)?;
    let locked = list.iter().filter(|p| p.locked.is_some()).count();
    let _ = writeln!(out, "  parts {} ({locked} locked)", list.len());
    for p in &list {
        let controls: Vec<String> = p
            .controls
            .iter()
            .map(|c| {
                let state = match &c.allowed {
                    Allowed::Fixed(_) => "fixed",
                    _ => "edit",
                };
                format!("{} {state}", c.label)
            })
            .collect();
        let _ = writeln!(
            out,
            "    {:24} stub {:X} -> {:X} pivot {:?}: {}",
            p.name,
            p.id.stub,
            p.id.target,
            p.pivot,
            if let Some(l) = &p.locked {
                l.clone()
            } else {
                controls.join(", ")
            }
        );
    }
    let mut failed = 0;
    let mut tried = BTreeSet::new();
    let inv = Inventory::parse(bytes)?;
    let model = hangar_core::model::Model::parse(bytes).ok();
    for p in &list {
        for c in &p.controls {
            let kind = c.label.clone();
            if tried.contains(&(p.xform, kind.clone())) {
                continue;
            }
            let next = match (&c.allowed, &c.setting) {
                (Allowed::Values(v), Setting::Compare { index, value }) => {
                    v.iter().find(|x| *x != value).map(|x| Setting::Compare {
                        index: *index,
                        value: *x,
                    })
                }
                (Allowed::Values(v), Setting::Shift { index, amount }) => v
                    .iter()
                    .find(|x| **x != *amount as i32)
                    .map(|x| Setting::Shift {
                        index: *index,
                        amount: *x as u8,
                    }),
                (Allowed::Either, Setting::Branch { index, equal }) => Some(Setting::Branch {
                    index: *index,
                    equal: !equal,
                }),
                (Allowed::Either, Setting::Direction { index, negated }) => {
                    Some(Setting::Direction {
                        index: *index,
                        negated: !negated,
                    })
                }
                (Allowed::Axes(_), Setting::Axis { index, axis }) => Some(Setting::Axis {
                    index: *index,
                    axis: if *axis == Axis::Roll {
                        Axis::Pitch
                    } else {
                        Axis::Roll
                    },
                }),
                (Allowed::Range(..), Setting::Pivot(v)) => {
                    Some(Setting::Pivot([v[0] + 1, v[1], v[2]]))
                }
                _ => None,
            };
            let Some(next) = next else { continue };
            tried.insert((p.xform, kind));
            let result = parts::apply_part_setting(bytes, p.id, &next).and_then(|shape| {
                let after = Inventory::parse(&shape)?;
                if !after.contiguous() || after.opaque_bytes() != inv.opaque_bytes() {
                    return Err("inventory coverage changed".into());
                }
                if model.is_some() {
                    hangar_core::model::Model::parse(&shape)?;
                }
                let changed = bytes.iter().zip(&shape).filter(|(a, b)| a != b).count();
                let stem = name.trim_end_matches(".SH");
                let label = format!("{}-{}", p.name, c.label).replace([' ', '(', ')'], "_");
                crate::platform::write_new(&format!("{out_dir}/{stem}-part-{label}.SH"), &shape)?;
                Ok(format!(
                    "{changed} bytes changed, inventory 100%, binding reports {next:?}"
                ))
            });
            let line = match result {
                Ok(s) => format!("PASS {s}"),
                Err(e) => {
                    failed += 1;
                    format!("FAIL {e}")
                }
            };
            let _ = writeln!(out, "  setting {} / {}: {line}", p.name, c.label);
        }
    }
    failed += gear_forms(out, bytes, name, out_dir)?;
    Ok(failed)
}
/// Every same-size gear law form reachable from each gear part: each state
/// re-parses with the expected law, previews, and the census bytes return.
fn gear_forms(out: &mut String, bytes: &[u8], name: &str, out_dir: &str) -> Result<usize> {
    use hangar_core::{
        model::{Model, Pose},
        shape_parts::{self as parts, Allowed, Role, Setting},
    };
    let mut failed = 0;
    let state = |b: &[u8], id| -> Result<(u8, bool, bool)> {
        let list = parts::parts(b)?;
        let p = list.iter().find(|p| p.id == id).ok_or("part lost")?;
        let mut s = (0, false, p.retail());
        for c in &p.controls {
            match c.setting {
                Setting::Shift { amount, .. } => s.0 = amount,
                Setting::Direction { negated, .. } => s.1 = negated,
                _ => {}
            }
        }
        Ok(s)
    };
    let list = parts::parts(bytes)?;
    let base = Inventory::parse(bytes)?;
    let stem = name.trim_end_matches(".SH");
    for p in list
        .iter()
        .filter(|p| p.role == Role::Gear && p.locked.is_none())
    {
        let result = (|| -> Result<String> {
            let start = state(bytes, p.id)?;
            let mut seen = BTreeMap::<(u8, bool), Vec<u8>>::new();
            seen.insert((start.0, start.1), bytes.to_vec());
            let mut work = vec![bytes.to_vec()];
            let mut unseen = 0;
            while let Some(b) = work.pop() {
                let now = parts::parts(&b)?;
                let part = now.iter().find(|x| x.id == p.id).ok_or("part lost")?;
                for c in &part.controls {
                    let next: Vec<Setting> = match (&c.allowed, &c.setting) {
                        (Allowed::Values(v), Setting::Shift { index, .. }) => v
                            .iter()
                            .map(|n| Setting::Shift {
                                index: *index,
                                amount: *n as u8,
                            })
                            .collect(),
                        (Allowed::Either, Setting::Direction { index, negated }) => {
                            vec![Setting::Direction {
                                index: *index,
                                negated: !negated,
                            }]
                        }
                        _ => Vec::new(),
                    };
                    for s in next {
                        let shape = parts::apply_part_setting(&b, p.id, &s)?;
                        let after = Inventory::parse(&shape)?;
                        if !after.contiguous()
                            || after.opaque_bytes() != base.opaque_bytes()
                            || shape.len() != bytes.len()
                        {
                            return Err("inventory changed".into());
                        }
                        let (n, neg, retail) = state(&shape, p.id)?;
                        // Preview: the part turns by the evaluated law.
                        let pose =
                            Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -8192)]);
                        if Model::parse(bytes).is_ok() {
                            Model::with_pose(&shape, &pose)?;
                        }
                        if let Some(known) = seen.get(&(n, neg)) {
                            // Includes a step back to the census state that
                            // does not reproduce the original bytes.
                            if *known != shape {
                                return Err(format!("two encodings for sar {n} neg {neg}"));
                            }
                        } else {
                            if !retail {
                                unseen += 1;
                                let label =
                                    format!("{}-sar{n}{}", p.name, if neg { "-neg" } else { "" })
                                        .replace([' ', '(', ')'], "_");
                                crate::platform::write_new(
                                    &format!("{out_dir}/{stem}-form-{label}.SH"),
                                    &shape,
                                )?;
                            }
                            seen.insert((n, neg), shape.clone());
                            work.push(shape);
                        }
                    }
                }
            }
            let states: Vec<String> = seen
                .keys()
                .map(|(n, neg)| format!("sar{n}{}", if *neg { "+neg" } else { "" }))
                .collect();
            Ok(format!(
                "{} states [{}], {unseen} not seen in retail, census bytes restored",
                seen.len(),
                states.join(" ")
            ))
        })();
        let line = match result {
            Ok(s) => format!("PASS {s}"),
            Err(e) => {
                failed += 1;
                format!("FAIL {e}")
            }
        };
        let _ = writeln!(out, "  gear forms {}: {line}", p.name);
    }
    Ok(failed)
}
/// Texture-state and vertex-slot proofs for every face the neutral model of
/// each decodable SH draws: a summary, and one line per face and proof so a
/// later run can be compared with this one (`baseline`, an earlier list).
pub fn proof_census(lib: &str, baseline: Option<&str>) -> Result<(String, String)> {
    use hangar_core::{model::Model, shape_geometry::Geometry};
    let archive = Archive::parse(crate::platform::read(lib)?)?;
    let mut lines = String::new();
    let (mut shapes, mut decodable, mut faces, mut complete) = (0, 0, 0, 0);
    let (mut material, mut slots, mut slot_total) = (0, 0, 0);
    let mut reasons = BTreeMap::<String, usize>::new();
    for e in archive.entries.iter().filter(|e| e.name.ends_with(".SH")) {
        shapes += 1;
        let bytes = e.read()?;
        let (Ok(g), Ok(m)) = (Geometry::parse(&bytes), Model::parse(&bytes)) else {
            continue;
        };
        decodable += 1;
        let drawn: BTreeSet<usize> = m.faces.iter().filter_map(|f| g.face_at(f.offset)).collect();
        let mut all = true;
        for i in drawn {
            faces += 1;
            let at = g.faces[i].offset - g.inventory.code_start;
            let line = match g.material(i) {
                Ok(s) => {
                    material += 1;
                    let hex: String = g.selector(s).iter().map(|b| format!("{b:02X}")).collect();
                    format!("OK {hex}")
                }
                Err(r) => {
                    all = false;
                    let key: String = r
                        .split(' ')
                        .map(|w| if w.contains("CODE+") { "CODE+#" } else { w })
                        .collect::<Vec<_>>()
                        .join(" ");
                    *reasons.entry(key).or_default() += 1;
                    format!("ERR {r}")
                }
            };
            let _ = writeln!(lines, "{} {at:05X} material {line}", e.name);
            for s in &g.faces[i].slots {
                slot_total += 1;
                let line = match g.writer(i, *s) {
                    Ok(b) => {
                        slots += 1;
                        format!("OK {:05X}", g.buffers[b].offset - g.inventory.code_start)
                    }
                    Err(r) => format!("ERR {r}"),
                };
                let _ = writeln!(lines, "{} {at:05X} slot{s} {line}", e.name);
            }
        }
        complete += all as usize;
    }
    let mut out = format!(
        "{shapes} SH, {decodable} decodable; texture state proved for {material} of {faces} drawn faces (every face in {complete} shapes); vertex slots proved {slots} of {slot_total}\n"
    );
    let mut sorted: Vec<_> = reasons.into_iter().collect();
    sorted.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (r, n) in sorted.iter().take(12) {
        let _ = writeln!(out, "  {n:6} {r}");
    }
    if let Some(path) = baseline {
        let old = String::from_utf8(crate::platform::read(path)?).map_err(|e| e.to_string())?;
        let parse = |text: &str| -> BTreeMap<String, String> {
            text.lines()
                .filter_map(|l| {
                    let mut p = l.splitn(4, ' ');
                    let key = format!("{} {} {}", p.next()?, p.next()?, p.next()?);
                    Some((key, p.next()?.to_string()))
                })
                .collect()
        };
        let (old, new) = (parse(&old), parse(&lines));
        for what in ["material", "slot"] {
            let (mut before, mut after, mut gained, mut differ, mut missing) = (0, 0, 0, 0, 0);
            let mut examples = Vec::new();
            for (k, v) in &new {
                if !k.split(' ').nth(2).is_some_and(|w| w.starts_with(what)) {
                    continue;
                }
                after += v.starts_with("OK") as usize;
                match old.get(k) {
                    Some(w) if w.starts_with("OK") => {
                        before += 1;
                        if w != v {
                            differ += 1;
                            if examples.len() < 8 {
                                examples.push(format!("{k}: {w} -> {v}"));
                            }
                        }
                    }
                    Some(_) => gained += v.starts_with("OK") as usize,
                    None => missing += 1,
                }
            }
            let _ = writeln!(
                out,
                "{what}: old proved {before}, new proves {after} (+{gained} newly proved), {differ} disagreements, {missing} not in the baseline"
            );
            for x in examples {
                let _ = writeln!(out, "  {x}");
            }
        }
    }
    Ok((out, lines))
}
/// `--decal-census`: which shapes select which runtime texture slots (E0
/// records), how many textured faces each slot draws, and which aircraft
/// (PT) use the shape.
pub fn decal_census(lib: &str) -> Result<String> {
    use hangar_core::{
        dependencies::Index,
        shape_markings::{markings, slot_name},
    };
    let archive = Archive::parse(crate::platform::read(lib)?)?;
    let mut index = Index::default();
    index.update(&archive);
    let mut out = String::new();
    let (mut shapes, mut with, mut aircraft, mut failed) = (0, 0, 0, 0);
    let mut per_slot = BTreeMap::<u16, (usize, usize, usize, usize)>::new();
    let mut odd = Vec::new();
    let mut lines = String::new();
    for e in archive.entries.iter().filter(|e| e.name.ends_with(".SH")) {
        shapes += 1;
        let bytes = e.read()?;
        let rows = match markings(&bytes) {
            Ok(r) => r,
            Err(why) => {
                failed += 1;
                let _ = writeln!(lines, "{} not analysed: {why}", e.name);
                continue;
            }
        };
        if rows.is_empty() {
            continue;
        }
        with += 1;
        let users = index.aircraft_users(&e.name);
        aircraft += usize::from(!users.is_empty());
        let mut parts = Vec::new();
        for r in &rows {
            let t = per_slot.entry(r.slot).or_default();
            t.0 += 1;
            t.1 += r.faces.len();
            t.2 += r.contested.len();
            t.3 += r.records.len();
            if r.slot > 4 {
                odd.push(format!("{} selects slot {}", e.name, r.slot));
            }
            let mut p = format!(
                "slot {} {}x E0 {} face{}",
                r.slot,
                r.records.len(),
                r.faces.len(),
                if r.faces.len() == 1 { "" } else { "s" }
            );
            if !r.contested.is_empty() {
                p += &format!(" ({} contested)", r.contested.len());
            }
            if !r.hidden.is_empty() || !r.painted.is_empty() {
                p += &format!(
                    " ({} hidden, {} paintable)",
                    r.hidden.len(),
                    r.painted.len()
                );
            }
            parts.push(p);
        }
        let who = if users.is_empty() {
            "-".to_string()
        } else {
            users.join(",")
        };
        let _ = writeln!(lines, "{} [{who}]: {}", e.name, parts.join("; "));
    }
    let _ = writeln!(
        out,
        "{lib}: {shapes} SH, {with} select runtime slots ({aircraft} of them used by a PT), {failed} not analysed"
    );
    for (slot, (n, faces, contested, records)) in &per_slot {
        let _ = writeln!(
            out,
            "  slot {slot} {:<18} {n:4} shapes, {records:4} E0 records, {faces:5} faces, {contested} contested",
            slot_name(*slot)
        );
    }
    for o in &odd {
        let _ = writeln!(out, "  note: {o}");
    }
    out.push_str(&lines);
    Ok(out)
}
/// `--markings-check` core pass: every runtime-marking operation on every
/// shape that selects a slot, each reversed and compared byte for byte.
pub fn markings_round_trip(lib: &str) -> Result<String> {
    use hangar_core::shape_fill::{FillMode, FillSource};
    use hangar_core::shape_markings::{
        hide_slot, make_paintable, markings, reassign_slot, restore_slot, show_slot,
    };
    let archive = Archive::parse(crate::platform::read(lib)?)?;
    let textures = |n: &str| archive.find(n).and_then(|i| archive.entries[i].read().ok());
    let (mut shapes, mut passed, mut ops) = (0, 0, 0);
    let mut failures = Vec::new();
    let mut fills = [0usize; 3];
    let mut unfound = Vec::new();
    for e in archive.entries.iter().filter(|e| e.name.ends_with(".SH")) {
        let src = e.read()?;
        let Ok(rows) = markings(&src) else { continue };
        if rows.is_empty() {
            continue;
        }
        shapes += 1;
        let mut trials: Vec<(String, Result<Vec<u8>>)> = Vec::new();
        let used: Vec<u16> = rows.iter().map(|r| r.slot).collect();
        for r in &rows {
            let s = r.slot;
            trials.push((
                format!("hide/show slot {s}"),
                hide_slot(&src, s).and_then(|h| show_slot(&h, s)),
            ));
            if let Some(to) = (0..5).find(|t| !used.contains(t)) {
                trials.push((
                    format!("reassign slot {s} to {to} and back"),
                    reassign_slot(&src, s, to).and_then(|m| reassign_slot(&m, to, s)),
                ));
            }
            trials.push((
                format!("make paintable/restore slot {s}"),
                make_paintable(&src, s, "ZZPAINT.PIC", None, FillMode::Surface, &textures)
                    .inspect(|p| {
                        let k = match p.fill {
                            FillSource::Surface(_) => 0,
                            FillSource::Skin => 1,
                            _ => 2,
                        };
                        fills[k] += 1;
                        if k > 0 {
                            unfound.push(format!("{} slot {s}", e.name));
                        }
                    })
                    .and_then(|p| restore_slot(&p.shape, s).map(|r| r.0)),
            ));
        }
        let mut all = Ok(src.clone());
        for r in &rows {
            all = all.and_then(|b| hide_slot(&b, r.slot));
        }
        for r in rows.iter().rev() {
            all = all.and_then(|b| show_slot(&b, r.slot));
        }
        trials.push(("hide every slot, show every slot".into(), all));
        let mut ok = true;
        for (what, r) in trials {
            ops += 1;
            let why = match r {
                Ok(b) if b == src => continue,
                Ok(b) => format!("not byte-identical ({} vs {} bytes)", b.len(), src.len()),
                Err(why) => why,
            };
            ok = false;
            failures.push(format!("{} {what}: {why}", e.name));
        }
        passed += ok as usize;
    }
    let mut out = format!(
        "{lib}: {shapes} shapes select runtime slots; {passed} pass all of their operations byte for byte ({ops} operations)\n"
    );
    let _ = writeln!(
        out,
        "  fill from the surface under the marking: {} slots; shape skin index: {}; stored colour: {}",
        fills[0], fills[1], fills[2]
    );
    for u in unfound.iter().take(24) {
        let _ = writeln!(out, "    no surface found under {u}");
    }
    for f in &failures {
        let _ = writeln!(out, "  FAIL {f}");
    }
    Ok(out)
}
