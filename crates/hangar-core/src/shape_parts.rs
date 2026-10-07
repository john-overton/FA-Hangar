//! Part settings: in-place parameter edits on census-recognised F0 stubs.
//!
//! Moving parts are gated or driven by small native stubs. This module never
//! authors stub code: it finds the parameter fields of stubs the inventory's
//! evaluator fully understands (`Stub::unrecognised` is `None`) for reviewed
//! aircraft variables, and changes one field at the same size. Allowed values
//! come from the FA_2.LIB stub census; anything without a same-size encoding
//! in that census is reported read-only with its reason. Every edit re-parses
//! the inventory and checks that the binding now reports the new value.
use crate::{
    invalid,
    model::Model,
    shape_code::{
        x86::{self, Insn, Mem, Rm},
        Binding, BindingKind, Condition, Inventory, Kind, LawOp, PointerKind, Rel, Target,
    },
    slice, u16_at, Result,
};
use alloc::{string::String, vec::Vec};

/// Rotation word written by an xform store, by its offset from C4+2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    /// r0 (+6), about up.
    Yaw,
    /// r1 (+8), about right.
    Pitch,
    /// r2 (+0A), about forward.
    Roll,
}
impl Axis {
    pub const ALL: [Axis; 3] = [Axis::Yaw, Axis::Pitch, Axis::Roll];
    fn disp(self) -> u8 {
        match self {
            Axis::Yaw => 6,
            Axis::Pitch => 8,
            Axis::Roll => 10,
        }
    }
    fn of(disp: i32) -> Option<Self> {
        Axis::ALL.into_iter().find(|a| a.disp() as i32 == disp)
    }
    pub fn label(self) -> &'static str {
        match self {
            Axis::Yaw => "yaw (r0)",
            Axis::Pitch => "pitch (r1)",
            Axis::Roll => "roll (r2)",
        }
    }
}
/// One editable or read-only parameter value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setting {
    /// Immediate of the stub's `index`-th variable compare.
    Compare { index: usize, value: i32 },
    /// Whether that compare's branch is taken when equal (je) or not (jne).
    Branch { index: usize, equal: bool },
    /// Amount of the stub's `index`-th arithmetic right shift.
    Shift { index: usize, amount: u8 },
    /// Whether the law negates its value; never editable in place.
    Direction { negated: bool },
    /// Rotation word of the stub's `index`-th store.
    Axis { index: usize, axis: Axis },
    /// C4/C6 translation in model order (right, forward, up).
    Pivot([i32; 3]),
}
/// What a control may be set to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Allowed {
    /// Any of these values (compare immediates, shift amounts).
    Values(Vec<i32>),
    /// Either branch sense.
    Either,
    Axes(Vec<Axis>),
    /// Each component within this inclusive range.
    Range(i32, i32),
    /// Read-only, with the reason.
    Fixed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    pub label: String,
    /// Current value.
    pub setting: Setting,
    pub allowed: Allowed,
    /// File offset of the field.
    pub at: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Gear,
    GearMesh,
    Flap,
    Rudder,
    SpeedBrake,
    Hook,
    Afterburner,
    BayDoor,
    SwingWing,
    Canard,
    ThrustVector,
    Slats,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Side {
    Left,
    Right,
    Nose,
    Centre,
}
/// A stub-driven part: the stub's file offset and the SH record it resumes at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PartId {
    pub stub: usize,
    pub target: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartInfo {
    pub id: PartId,
    pub name: String,
    pub role: Role,
    pub side: Option<Side>,
    /// True for a transform (law stored into the C4 words), false for a toggle.
    pub xform: bool,
    pub variables: Vec<String>,
    pub when: Vec<Vec<Condition>>,
    /// C4/C6 translation in model order, for C4/C6 targets.
    pub pivot: Option<[i32; 3]>,
    /// File offset of the called block.
    pub block: Option<usize>,
    pub controls: Vec<Control>,
    /// Why the stub's fields cannot be edited at all, if they cannot.
    pub locked: Option<String>,
}
/// Compare immediates observed per variable in the FA_2.LIB stub census.
const COMPARES: &[(&str, &[i32])] = &[
    ("_PLgearDown", &[0, 1, 4]),
    ("_PLleftFlap", &[-2, -1, 0, 1]),
    ("_PLrightFlap", &[-2, -1, 0, 1]),
    ("_PLrudder", &[-1, 0, 1]),
    ("_PLbrake", &[0, 1]),
    ("_PLhook", &[0, 1]),
    ("_PLafterBurner", &[0, 1]),
    ("_PLbayOpen", &[0, 1]),
    ("_PLvtOn", &[0]),
    ("_PLslats", &[0]),
];
/// Variables whose xform laws drive aircraft parts.
const LAWS: &[&str] = &[
    "_PLgearPos",
    "_PLswingWing",
    "_PLcanardPos",
    "_PLbayDoorPos",
    "_PLvtAngle",
];
fn reviewed(variable: &str) -> bool {
    COMPARES.iter().any(|(v, _)| *v == variable) || LAWS.contains(&variable)
}
fn insns(code: &[u8], blocks: &[(usize, usize)]) -> Result<Vec<Insn>> {
    let mut out = Vec::new();
    for (s, e) in blocks {
        let mut p = *s;
        while p < *e {
            let i = x86::decode(code, p)?;
            p = i.end();
            out.push(i);
            if out.len() > 4096 {
                return Err(invalid("Stub too long"));
            }
        }
    }
    out.sort_unstable_by_key(|i| i.at);
    Ok(out)
}
struct Fields {
    controls: Vec<Control>,
    locked: Option<String>,
}
/// Controls of one stub for the binding resuming at `target` (CODE offsets).
fn fields(inv: &Inventory, code: &[u8], stub: usize, b: &Binding) -> Result<Fields> {
    let cs = inv.code_start;
    let s = inv
        .stubs
        .iter()
        .find(|s| s.offset == stub)
        .ok_or("No such stub")?;
    let mut controls = Vec::new();
    if let Some((at, why)) = &s.unrecognised {
        return Ok(Fields {
            controls,
            locked: Some(format!(
                "The stub's code is outside the reviewed subset at CODE+{at:X} ({why}); it is never edited"
            )),
        });
    }
    let vars = b.variables();
    if let Some(v) = vars.iter().find(|v| !reviewed(v)) {
        return Ok(Fields {
            controls,
            locked: Some(format!("{v} is not a reviewed aircraft part variable")),
        });
    }
    let law_var = match &b.kind {
        BindingKind::Xform { writes } => writes.iter().flat_map(|(_, l)| l).find_map(|o| match o {
            LawOp::Var(v) => Some(v.clone()),
            _ => None,
        }),
        BindingKind::Toggle => None,
    };
    let two_term = law_var.as_deref() == Some("_PLswingWing");
    let list = insns(code, &s.blocks)?;
    let (mut compares, mut shifts, mut stores) = (0, 0, 0);
    for (k, i) in list.iter().enumerate() {
        let last = cs + i.end() - 1;
        match (i.op, i.ext, i.rm) {
            (0x83, 7, Some(Rm::Mem(Mem::Abs(va)))) if i.word => {
                let Some(name) = inv.alias_name(va as usize) else {
                    continue;
                };
                let index = compares;
                compares += 1;
                let value = slice(code, i.end() - 1, 1)?[0] as i8 as i32;
                let allowed = match COMPARES.iter().find(|(v, _)| *v == name) {
                    Some((_, values)) if values.len() > 1 => Allowed::Values(values.to_vec()),
                    Some(_) => Allowed::Fixed(format!("The census shows only {value} for {name}")),
                    None => Allowed::Fixed(format!("{name} compares are not reviewed")),
                };
                controls.push(Control {
                    label: format!("{name} compare"),
                    setting: Setting::Compare { index, value },
                    allowed,
                    at: last,
                });
                if let Some(j) = list.get(k + 1).filter(|j| j.at == i.end()) {
                    let op = slice(code, j.at, 1)?[0];
                    controls.push(Control {
                        label: format!("{name} branch"),
                        setting: Setting::Branch {
                            index,
                            equal: op == 0x74,
                        },
                        allowed: if matches!(op, 0x74 | 0x75) {
                            Allowed::Either
                        } else {
                            Allowed::Fixed(
                                "A ranged branch (<, >, >=); only = and != swap in place".into(),
                            )
                        },
                        at: cs + j.at,
                    });
                }
            }
            (0xc1, 7, Some(Rm::Reg(_))) => {
                let amount = i.imm.unwrap_or(0) as u8;
                let range: Vec<i32> = if two_term { (1..=7).collect() } else { (1..=3).collect() };
                controls.push(Control {
                    label: "Shift".into(),
                    setting: Setting::Shift {
                        index: shifts,
                        amount,
                    },
                    allowed: Allowed::Values(range),
                    at: last,
                });
                shifts += 1;
            }
            (0xd1, 7, Some(Rm::Reg(_))) => {
                controls.push(Control {
                    label: "Shift".into(),
                    setting: Setting::Shift {
                        index: shifts,
                        amount: 1,
                    },
                    allowed: Allowed::Fixed(
                        "Shift 1 uses the 3-byte D1 form; 2 or 3 need the 4-byte C1 form".into(),
                    ),
                    at: cs + i.at,
                });
                shifts += 1;
            }
            (0xf7, 3, Some(Rm::Reg(_))) => controls.push(Control {
                label: "Direction".into(),
                setting: Setting::Direction { negated: true },
                allowed: Allowed::Fixed(
                    "Reversing needs a 3-byte NEG added or removed; no same-size form is in the census".into(),
                ),
                at: cs + i.at,
            }),
            (
                0x89,
                _,
                Some(Rm::Mem(Mem::Reg {
                    base: Some(3),
                    index: None,
                    disp,
                })),
            ) if i.word && matches!(b.target_op, 0xc4 | 0xc6) => {
                let index = stores;
                stores += 1;
                let Some(axis) = Axis::of(disp) else {
                    controls.push(Control {
                        label: "Store".into(),
                        setting: Setting::Axis {
                            index,
                            axis: Axis::Yaw,
                        },
                        allowed: Allowed::Fixed("The law writes a translation word".into()),
                        at: last,
                    });
                    continue;
                };
                controls.push(Control {
                    label: "Rotation axis".into(),
                    setting: Setting::Axis { index, axis },
                    allowed: Allowed::Axes(Axis::ALL.to_vec()),
                    at: last,
                });
            }
            _ => {}
        }
    }
    if matches!(b.kind, BindingKind::Xform { .. })
        && !controls
            .iter()
            .any(|c| matches!(c.setting, Setting::Direction { .. }))
    {
        controls.push(Control {
            label: "Direction".into(),
            setting: Setting::Direction { negated: false },
            allowed: Allowed::Fixed(
                "Reversing needs a 3-byte NEG added or removed; no same-size form is in the census"
                    .into(),
            ),
            at: cs + stub,
        });
    }
    if let (Some(p), true) = (b.pivot, matches!(b.target_op, 0xc4 | 0xc6)) {
        let t = b.target;
        // Only recognised stores may write the C4 words; anything else that
        // addresses them (an unrecognised stub's self-offset) locks the pivot.
        let foreign = inv
            .records
            .iter()
            .filter(|r| matches!(r.kind, Kind::X86 { .. }))
            .any(|r| {
                r.pointers.iter().any(|q| {
                    q.kind == PointerKind::SelfOffset
                        && matches!(q.target, Target::Code(x) if (t + 2..t + 8).contains(&x))
                        && inv.stubs.iter().any(|s| {
                            s.unrecognised.is_some()
                                && s.blocks
                                    .iter()
                                    .any(|(a, e)| *a <= r.offset && r.offset < *e)
                        })
                })
            });
        let stored = inv
            .stubs
            .iter()
            .flat_map(|s| &s.outcomes)
            .flat_map(|o| &o.stores)
            .any(|st| st.at < t + 8 && st.at + 2 > t + 2);
        controls.push(Control {
            label: "Pivot".into(),
            setting: Setting::Pivot(p),
            allowed: if foreign || stored {
                Allowed::Fixed("Native code writes this part's translation".into())
            } else {
                Allowed::Range(-32768, 32767)
            },
            at: cs + t + 2,
        });
    }
    Ok(Fields {
        controls,
        locked: None,
    })
}
fn side_of(x: i32) -> Side {
    match x {
        x if x < -1 => Side::Left,
        x if x > 1 => Side::Right,
        _ => Side::Centre,
    }
}
fn role_of(vars: &[String], law: Option<&str>) -> Role {
    let has = |n: &str| vars.iter().any(|v| v == n);
    match law {
        Some("_PLgearPos") => Role::Gear,
        Some("_PLswingWing") => Role::SwingWing,
        Some("_PLcanardPos") => Role::Canard,
        Some("_PLbayDoorPos") => Role::BayDoor,
        Some("_PLvtAngle") => Role::ThrustVector,
        _ if has("_PLleftFlap") || has("_PLrightFlap") => Role::Flap,
        _ if has("_PLrudder") => Role::Rudder,
        _ if has("_PLbrake") => Role::SpeedBrake,
        _ if has("_PLhook") => Role::Hook,
        _ if has("_PLbayOpen") => Role::BayDoor,
        _ if has("_PLgearDown") => Role::GearMesh,
        _ if has("_PLafterBurner") => Role::Afterburner,
        _ if has("_PLvtOn") => Role::ThrustVector,
        _ if has("_PLslats") => Role::Slats,
        _ => Role::Other,
    }
}
fn role_name(r: Role) -> &'static str {
    match r {
        Role::Gear => "Gear",
        Role::GearMesh => "Gear mesh",
        Role::Flap => "Flap",
        Role::Rudder => "Rudder",
        Role::SpeedBrake => "Speed brake",
        Role::Hook => "Hook",
        Role::Afterburner => "Afterburner",
        Role::BayDoor => "Bay door",
        Role::SwingWing => "Swing wing",
        Role::Canard => "Canard",
        Role::ThrustVector => "Thrust vector",
        Role::Slats => "Slats",
        Role::Other => "Part",
    }
}
/// Every stub-driven part of a shape: toggled blocks (12/6E/C4/C6 calls)
/// and transformed C4/C6 parts, with semantic names and their controls.
///
/// Naming: the role comes from the law variable (gearPos, swingWing,
/// canardPos, bayDoorPos, vtAngle) or else the gate variable (flaps, rudder,
/// brake, hook, bayOpen, gearDown, afterBurner, vtOn, slats). The side comes
/// from the variable (left/right flap), else the pivot's right coordinate
/// (below -1 left, above 1 right), else the centroid of a toggled block's
/// first vertex buffer. Gear transforms farthest from the centreline are the
/// main legs; gear pivots more than halfway from them to the most forward
/// gear pivot are the nose gear (this also names the A-10's offset nose
/// leg). Doors and legs driven by the same variables are not told apart. Toggle names carry the
/// gate state (`state 1`), since retail state meanings are not all verified.
pub fn parts(source: &[u8]) -> Result<Vec<PartInfo>> {
    let inv = Inventory::parse(source)?;
    let code = slice(source, inv.code_start, inv.code_len)?;
    let cs = inv.code_start;
    let mut out = Vec::new();
    for b in &inv.bindings {
        let xform = matches!(b.kind, BindingKind::Xform { .. });
        if !xform && !matches!(b.target_op, 0x12 | 0x6e | 0xc4 | 0xc6) {
            continue;
        }
        let f = fields(&inv, code, b.stub, b)?;
        let variables: Vec<String> = b.variables().into_iter().collect();
        let law = match &b.kind {
            BindingKind::Xform { writes } => {
                writes.iter().flat_map(|(_, l)| l).find_map(|o| match o {
                    LawOp::Var(v) => Some(v.as_str()),
                    _ => None,
                })
            }
            BindingKind::Toggle => None,
        };
        let role = role_of(&variables, law);
        let side = if variables.iter().any(|v| v == "_PLleftFlap") {
            Some(Side::Left)
        } else if variables.iter().any(|v| v == "_PLrightFlap") {
            Some(Side::Right)
        } else if let Some(p) = b.pivot {
            Some(side_of(p[0]))
        } else {
            b.block.and_then(|t| {
                let r = inv.records.get(inv.starting_at(t)?)?;
                if r.kind != Kind::Sh(0x82) {
                    return None;
                }
                let n = u16_at(code, t + 2).ok()?;
                if n == 0 {
                    return None;
                }
                let sum: i64 = (0..n)
                    .map(|k| u16_at(code, t + 6 + 6 * k).map_or(0, |v| v as u16 as i16 as i64))
                    .sum();
                Some(side_of((sum / n as i64) as i32))
            })
        };
        out.push(PartInfo {
            id: PartId {
                stub: cs + b.stub,
                target: cs + b.target,
            },
            name: String::new(),
            role,
            side,
            xform,
            variables,
            when: b.when.clone(),
            pivot: b.pivot,
            block: b.block.map(|t| cs + t),
            controls: f.controls,
            locked: f.locked,
        });
    }
    // Nose gear: the main legs are the gear transforms farthest from the
    // centreline; gear pivots more than halfway from them to the most forward
    // gear pivot are the nose gear and its doors.
    let gear: Vec<usize> = (0..out.len())
        .filter(|i| out[*i].role == Role::Gear && out[*i].pivot.is_some())
        .collect();
    let pivots: Vec<[i32; 3]> = out.iter().map(|x| x.pivot.unwrap_or([0; 3])).collect();
    let p = |i: usize| pivots[i];
    if let Some(wide) = gear.iter().map(|i| p(*i)[0].abs()).max().filter(|w| *w > 1) {
        let mains: Vec<i32> = gear
            .iter()
            .filter(|i| p(**i)[0].abs() == wide)
            .map(|i| p(*i)[1])
            .collect();
        let y_main = mains.iter().sum::<i32>() / mains.len().max(1) as i32;
        let y_max = gear.iter().map(|i| p(*i)[1]).max().unwrap_or(y_main);
        if y_max - y_main > 4 {
            let threshold = y_main + (y_max - y_main) / 2;
            for i in &gear {
                if p(*i)[1] > threshold {
                    out[*i].side = Some(Side::Nose);
                }
            }
        }
    }
    let mut seen = alloc::collections::BTreeMap::<String, usize>::new();
    for p in &mut out {
        let mut name = String::from(role_name(p.role));
        match p.side {
            Some(Side::Nose) => name = format!("Nose {}", name.to_ascii_lowercase()),
            Some(Side::Left) => name.push_str(" left"),
            Some(Side::Right) => name.push_str(" right"),
            Some(Side::Centre) if p.role == Role::Gear => name.push_str(" centre"),
            _ => {}
        }
        if !p.xform {
            if let Some(c) = p
                .when
                .first()
                .and_then(|w| w.iter().find(|c| c.rel == Rel::Eq))
            {
                name.push_str(&format!(" (state {})", c.value));
            }
        }
        let n = seen.entry(name.clone()).or_default();
        *n += 1;
        if *n > 1 {
            name.push_str(&format!(" {n}"));
        }
        p.name = name;
    }
    Ok(out)
}
fn decode_shape(inv: &Inventory, code: &[u8], stub: usize) -> Result<Vec<(usize, u16, u8)>> {
    let s = inv
        .stubs
        .iter()
        .find(|s| s.offset == stub)
        .ok_or("Stub lost")?;
    Ok(insns(code, &s.blocks)?
        .into_iter()
        // Same instruction boundaries and kinds; a jcc keeps its class.
        .map(|i| {
            let op = if (0x70..=0x7f).contains(&i.op) {
                0x70
            } else {
                i.op
            };
            (i.at, op, i.ext)
        })
        .collect())
}
/// Change one part control in place and verify the binding reports it.
pub fn apply_part_setting(source: &[u8], part: PartId, setting: &Setting) -> Result<Vec<u8>> {
    let list = parts(source)?;
    let p = list
        .iter()
        .find(|p| p.id == part)
        .ok_or("No such part in this shape")?;
    if let Some(why) = &p.locked {
        return Err(why.clone());
    }
    let same_kind = |c: &Control| match (&c.setting, setting) {
        (Setting::Compare { index: a, .. }, Setting::Compare { index: b, .. })
        | (Setting::Branch { index: a, .. }, Setting::Branch { index: b, .. })
        | (Setting::Shift { index: a, .. }, Setting::Shift { index: b, .. })
        | (Setting::Axis { index: a, .. }, Setting::Axis { index: b, .. }) => a == b,
        (Setting::Direction { .. }, Setting::Direction { .. })
        | (Setting::Pivot(_), Setting::Pivot(_)) => true,
        _ => false,
    };
    let c = p
        .controls
        .iter()
        .find(|c| same_kind(c))
        .ok_or("This part has no such control")?;
    if c.setting == *setting {
        return Ok(source.to_vec());
    }
    let mut out = source.to_vec();
    let mut put = |at: usize, bytes: &[u8]| -> Result<()> {
        out.get_mut(at..at + bytes.len())
            .ok_or("Field outside the shape")?
            .copy_from_slice(bytes);
        Ok(())
    };
    match (&c.allowed, setting) {
        (Allowed::Fixed(why), _) => return Err(why.clone()),
        (Allowed::Values(v), Setting::Compare { value, .. }) if v.contains(value) => {
            put(c.at, &[*value as i8 as u8])?
        }
        (Allowed::Values(v), Setting::Shift { amount, .. }) if v.contains(&(*amount as i32)) => {
            put(c.at, &[*amount])?
        }
        (Allowed::Either, Setting::Branch { equal, .. }) => {
            put(c.at, &[if *equal { 0x74 } else { 0x75 }])?
        }
        (Allowed::Axes(a), Setting::Axis { axis, .. }) if a.contains(axis) => {
            let taken = p.controls.iter().any(|o| {
                matches!((&o.setting, setting), (Setting::Axis { index: i, axis: x }, Setting::Axis { index: j, .. }) if i != j && x == axis)
            });
            if taken {
                return Err(invalid(
                    "Another store of this stub already writes that axis",
                ));
            }
            put(c.at, &[axis.disp()])?
        }
        (Allowed::Range(lo, hi), Setting::Pivot(v))
            if v.iter().all(|x| (*lo..=*hi).contains(x)) =>
        {
            for (k, axis) in [0, 2, 1].into_iter().enumerate() {
                put(c.at + 2 * k, &(v[axis] as i16).to_le_bytes())?;
            }
        }
        _ => {
            return Err(invalid(
                "Value outside the census-backed range for this control",
            ))
        }
    }
    // Verify from scratch.
    let before = Inventory::parse(source)?;
    let after = Inventory::parse(&out)?;
    if !after.contiguous() || after.opaque_bytes() != before.opaque_bytes() {
        return Err(invalid("The edit changed how much of CODE is explained"));
    }
    let cs = after.code_start;
    let (stub, target) = (part.stub - cs, part.target - cs);
    let s = after
        .stubs
        .iter()
        .find(|s| s.offset == stub)
        .ok_or("The stub was lost")?;
    if s.unrecognised.is_some() || after.stubs.len() != before.stubs.len() {
        return Err(invalid("The edited stub is no longer recognised"));
    }
    let shape_before = decode_shape(&before, slice(source, cs, before.code_len)?, stub)?;
    let shape_after = decode_shape(&after, slice(&out, cs, after.code_len)?, stub)?;
    if shape_before != shape_after {
        return Err(invalid("The edit changed the stub's instructions"));
    }
    let b = after
        .bindings
        .iter()
        .find(|b| b.stub == stub && b.target == target)
        .ok_or("The edited part no longer resumes at its target")?;
    let conditions = || b.when.iter().flatten();
    let reported = match setting {
        Setting::Compare { value, .. } => conditions().any(|c| c.value == *value),
        Setting::Branch { .. } => {
            let old = before
                .bindings
                .iter()
                .find(|x| x.stub == stub && x.target == target)
                .map(|x| x.when.clone())
                .unwrap_or_default();
            b.when != old
        }
        Setting::Shift { amount, .. } => match &b.kind {
            BindingKind::Xform { writes } => writes
                .iter()
                .flat_map(|(_, l)| l)
                .any(|o| matches!(o, LawOp::Sar { n, .. } if n == amount)),
            BindingKind::Toggle => false,
        },
        Setting::Axis { axis, .. } => match &b.kind {
            BindingKind::Xform { writes } => writes.iter().any(|(w, _)| *w == 3 + *axis as usize),
            BindingKind::Toggle => false,
        },
        Setting::Pivot(v) => b.pivot == Some(*v),
        Setting::Direction { .. } => false,
    };
    if !reported {
        return Err(invalid("The binding does not report the new value"));
    }
    let others = |x: &Inventory| -> Vec<Binding> {
        x.bindings
            .iter()
            .filter(|b| b.stub != stub)
            .cloned()
            .collect()
    };
    if others(&before) != others(&after) && !matches!(setting, Setting::Pivot(_)) {
        return Err(invalid("The edit changed another part"));
    }
    if Model::parse(source).is_ok() && Model::parse(&out).is_err() {
        return Err(invalid("The edited shape no longer parses as a model"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pose;
    use crate::shape_testkit::{Asm, Shift};
    fn shape(shift: Shift, neg: bool) -> Vec<u8> {
        let mut a = Asm::default();
        a.b(&[0xff, 0xff, 0, 0, 0x10, 0, 8, 0, 0x40, 0, 0x40, 0, 0x40, 0]);
        a.verts(0, &[[0, 0, 0], [10, 0, 0], [0, 10, 0]]);
        a.face(0, 32, None, &[0, 1, 2], &[]);
        a.xform(
            "main",
            Some(("_PLgearDown", 1)),
            "_PLgearPos",
            shift,
            neg,
            0x0a,
            [-6, 0, 2],
            "leg",
            "a1",
        );
        a.label("a1");
        a.xform(
            "other",
            Some(("_PLgearDown", 1)),
            "_PLgearPos",
            Shift::One,
            false,
            0x0a,
            [6, 0, 2],
            "leg2",
            "a2",
        );
        a.label("a2");
        a.xform(
            "nose",
            Some(("_PLgearDown", 1)),
            "_PLgearPos",
            Shift::One,
            true,
            0x08,
            [0, 0, 30],
            "leg3",
            "a3",
        );
        a.label("a3");
        a.xform(
            "wing",
            None,
            "_PLswingWing",
            Shift::Imm(2),
            false,
            0x06,
            [-8, 1, 4],
            "wingblock",
            "a4",
        );
        a.label("a4");
        a.toggle("_PLhook", 1, 0x75, "hook", "hookblock", "a5");
        a.label("a5");
        a.toggle("_PLleftFlap", -1, 0x75, "flap", "flapblock", "a6");
        a.label("a6");
        a.jump("end");
        for (name, slot) in [
            ("leg", 3u16),
            ("leg2", 6),
            ("leg3", 9),
            ("wingblock", 12),
            ("hookblock", 15),
            ("flapblock", 18),
        ] {
            a.label(name)
                .verts(slot, &[[0, 0, 0], [0, 0, -10], [1, 0, -10]])
                .face(0, 40, None, &[slot, slot + 1, slot + 2], &[])
                .b(&[0x1e]);
        }
        a.label("end")
            .b(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 0]);
        a.finish()
    }
    fn find<'a>(parts: &'a [PartInfo], name: &str) -> &'a PartInfo {
        parts.iter().find(|p| p.name == name).unwrap_or_else(|| {
            panic!(
                "no {name}: {:?}",
                parts.iter().map(|p| &p.name).collect::<Vec<_>>()
            )
        })
    }
    #[test]
    fn parts_are_named_from_variables_and_pivot_sides() {
        let b = shape(Shift::Imm(2), true);
        let parts = parts(&b).unwrap();
        let names: Vec<_> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Gear left",
                "Gear right",
                "Nose gear",
                "Swing wing left",
                "Hook (state 1)",
                "Flap left (state -1)"
            ]
        );
        let gear = find(&parts, "Gear left");
        assert!(gear.xform && gear.locked.is_none());
        assert_eq!(gear.pivot, Some([-6, 2, 0]));
        let labels: Vec<_> = gear.controls.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "_PLgearDown compare",
                "_PLgearDown branch",
                "Shift",
                "Direction",
                "Rotation axis",
                "Pivot"
            ]
        );
        assert_eq!(gear.controls[2].allowed, Allowed::Values(vec![1, 2, 3]));
        assert!(matches!(gear.controls[3].allowed, Allowed::Fixed(_)));
        // The D1 form keeps shift 1 read-only.
        let right = find(&parts, "Gear right");
        assert!(right.controls.iter().any(|c| c.setting
            == Setting::Shift {
                index: 0,
                amount: 1
            }
            && matches!(c.allowed, Allowed::Fixed(_))));
        assert!(right
            .controls
            .iter()
            .any(|c| c.setting == Setting::Direction { negated: false }));
        let wing = find(&parts, "Swing wing left");
        assert!(wing
            .controls
            .iter()
            .any(|c| c.allowed == Allowed::Values((1..=7).collect())));
    }
    #[test]
    fn settings_change_one_field_and_the_binding_reports_them() {
        let b = shape(Shift::Imm(2), true);
        let list = parts(&b).unwrap();
        let gear = find(&list, "Gear left").clone();
        let law = |bytes: &[u8]| -> (Vec<LawOp>, usize) {
            let inv = Inventory::parse(bytes).unwrap();
            let b = inv
                .bindings
                .iter()
                .find(|b| {
                    b.stub + inv.code_start == gear.id.stub
                        && b.target + inv.code_start == gear.id.target
                })
                .unwrap();
            match &b.kind {
                BindingKind::Xform { writes } => (writes[0].1.clone(), writes[0].0),
                _ => unreachable!(),
            }
        };
        // Shift 2 -> 3: one byte, and the law reports sar 3.
        let out = apply_part_setting(
            &b,
            gear.id,
            &Setting::Shift {
                index: 0,
                amount: 3,
            },
        )
        .unwrap();
        let diff: Vec<_> = (0..b.len()).filter(|i| b[*i] != out[*i]).collect();
        assert_eq!(diff.len(), 1);
        assert!(law(&out).0.contains(&LawOp::Sar { n: 3, w16: true }));
        // Axis r2 -> r1.
        let out = apply_part_setting(
            &b,
            gear.id,
            &Setting::Axis {
                index: 0,
                axis: Axis::Pitch,
            },
        )
        .unwrap();
        assert_eq!(law(&out).1, 4);
        // The preview follows the new axis.
        let pose = Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -8192)]);
        let m = Model::with_pose(&out, &pose).unwrap();
        let part = m.parts.iter().find(|p| p.offset == gear.id.target).unwrap();
        assert_eq!(part.posed_rotation, [0, 2048, 0]);
        // Pivot words.
        let out = apply_part_setting(&b, gear.id, &Setting::Pivot([-7, 3, 1])).unwrap();
        assert_eq!(
            find(&parts(&out).unwrap(), "Gear left").pivot,
            Some([-7, 3, 1])
        );
        // Gate compare and branch sense.
        let out =
            apply_part_setting(&b, gear.id, &Setting::Compare { index: 0, value: 4 }).unwrap();
        let inv = Inventory::parse(&out).unwrap();
        assert!(inv
            .bindings
            .iter()
            .flat_map(|b| b.when.iter().flatten())
            .any(|c| c.variable == "_PLgearDown" && c.value == 4));
        let hook = find(&list, "Hook (state 1)").clone();
        let out = apply_part_setting(
            &b,
            hook.id,
            &Setting::Branch {
                index: 0,
                equal: true,
            },
        )
        .unwrap();
        let after = parts(&out).unwrap();
        let h = after.iter().find(|p| p.id == hook.id).unwrap();
        assert_eq!(
            h.when,
            vec![vec![Condition {
                variable: "_PLhook".into(),
                rel: Rel::Ne,
                value: 1,
                w16: true
            }]]
        );
        // No-op is byte-identical; refusals leave nothing half-written.
        assert_eq!(
            apply_part_setting(
                &b,
                gear.id,
                &Setting::Shift {
                    index: 0,
                    amount: 2
                }
            )
            .unwrap(),
            b
        );
        assert!(apply_part_setting(
            &b,
            gear.id,
            &Setting::Shift {
                index: 0,
                amount: 4
            }
        )
        .is_err());
        assert!(apply_part_setting(&b, gear.id, &Setting::Compare { index: 0, value: 2 }).is_err());
        assert!(
            apply_part_setting(&b, gear.id, &Setting::Direction { negated: false })
                .unwrap_err()
                .contains("NEG")
        );
        let right = find(&list, "Gear right").id;
        assert!(apply_part_setting(
            &b,
            right,
            &Setting::Shift {
                index: 0,
                amount: 2
            }
        )
        .unwrap_err()
        .contains("D1"));
        let flap = find(&list, "Flap left (state -1)").id;
        let out = apply_part_setting(
            &b,
            flap,
            &Setting::Compare {
                index: 0,
                value: -2,
            },
        )
        .unwrap();
        assert!(parts(&out)
            .unwrap()
            .iter()
            .any(|p| p.name == "Flap left (state -2)"));
        assert!(
            apply_part_setting(&b, PartId { stub: 0, target: 0 }, &Setting::Pivot([0; 3])).is_err()
        );
    }
    #[test]
    fn unrecognised_stubs_are_locked() {
        let mut b = shape(Shift::Imm(2), true);
        let inv = Inventory::parse(&b).unwrap();
        let cs = inv.code_start;
        // Replace the swing wing's register move with a pushad: outside the subset.
        let wing = inv
            .stubs
            .iter()
            .find(|s| s.signature.contains("_PLswingWing"))
            .unwrap();
        let at = wing.blocks[0].0 + 8 + 4;
        assert_eq!(&b[cs + at..cs + at + 2], &[0x66, 0xa1]);
        b[cs + at] = 0x60;
        b[cs + at + 1] = 0x61;
        let list = parts(&b).unwrap_or_default();
        assert!(list
            .iter()
            .all(|p| p.role != Role::SwingWing || p.locked.is_some()));
        for n in 0..b.len() {
            let _ = parts(&b[..n]);
        }
    }
}
