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
    Ok(failed)
}
