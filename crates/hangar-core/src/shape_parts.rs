//! Part settings: in-place parameter edits on census-recognised F0 stubs.
//!
//! Moving parts are gated or driven by small native stubs. This module never
//! authors stub code: it finds the parameter fields of stubs the inventory's
//! evaluator fully understands (`Stub::unrecognised` is `None`) for reviewed
//! aircraft variables, and changes one field at the same size. Allowed values
//! come from the FA_2.LIB stub census. Gear laws additionally take a small set
//! of same-size encodings of their shift and NEG slot that the census does
//! not contain; those carry `retail: false`. Every edit re-parses the
//! inventory and checks that the binding now reports the new value.
use crate::{
    invalid,
    model::Model,
    shape_code::{
        x86::{self, Insn, Mem, Rm},
        Binding, BindingKind, Condition, Inventory, Kind, LawOp, PointerKind, Rel, Target,
    },
    slice, u16_at, Result,
};
use alloc::{collections::BTreeSet, string::String, vec::Vec};

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
    /// Whether the `index`-th store's law ends in a NEG.
    Direction { index: usize, negated: bool },
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
    /// Either branch sense, or either direction.
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
    /// The current encoding occurs in the FA_2.LIB stub census. False for
    /// Hangar's same-size forms, which the stub evaluator proves but which
    /// are not yet verified in the game.
    pub retail: bool,
    /// Allowed values whose encoding the census does not contain: shift
    /// amounts, 0/1 for a direction (negated), axis numbers 0..2.
    pub unseen: Vec<i32>,
}
impl Control {
    fn new(label: &str, setting: Setting, allowed: Allowed, at: usize) -> Self {
        Self {
            label: label.into(),
            setting,
            allowed,
            at,
            retail: true,
            unseen: Vec::new(),
        }
    }
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
impl PartInfo {
    /// False when some control currently uses an encoding the retail
    /// census does not contain.
    pub fn retail(&self) -> bool {
        self.controls.iter().all(|c| c.retail)
    }
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
/// Variables whose xform laws drive aircraft parts, with the C1 shift
/// amounts and rotation words (Axis order) the census shows for them.
type LawCensus = (&'static str, &'static [u8], &'static [usize]);
const LAWS: &[LawCensus] = &[
    ("_PLgearPos", &[2, 3], &[0, 1, 2]),
    ("_PLswingWing", &[2, 5], &[0]),
    ("_PLcanardPos", &[], &[1]),
    ("_PLbayDoorPos", &[], &[2]),
    ("_PLvtAngle", &[], &[1]),
];
const GEAR_LAW: &str = "_PLgearPos";
/// Compare values the FA_2.LIB census shows for a toggle variable.
pub fn census_values(variable: &str) -> &'static [i32] {
    COMPARES
        .iter()
        .find(|(v, _)| *v == variable)
        .map_or(&[], |(_, values)| values)
}
fn reviewed(variable: &str) -> bool {
    COMPARES.iter().any(|(v, _)| *v == variable) || LAWS.iter().any(|(v, ..)| *v == variable)
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

// ---------------------------------------------------------------- gear law slots

/// A gear law slot: the `sar ax` right before a C4 word store plus the NEG
/// or no-op that may follow it. Its size never changes; within it Hangar
/// writes one of the forms of `slot_bytes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot {
    /// CODE offset of the sar.
    at: usize,
    size: usize,
    shift: u8,
    neg: bool,
    /// Index of the sar among the stub's shifts and of the store among its stores.
    shift_index: usize,
    store_index: usize,
    /// C4 word the store writes (3 + axis).
    word: usize,
}
const NEG_AX: [u8; 3] = [0x66, 0xf7, 0xd8];
/// `mov ax, ax`: the 3-byte no-op that takes NEG's place.
const MOV_AX: [u8; 3] = [0x66, 0x89, 0xc0];
/// `xchg ax, ax`: the 2-byte filler after a 4-byte C1 shift in a 6-byte slot.
const NOP_AX: [u8; 2] = [0x66, 0x90];
/// The bytes of a `size`-byte gear law slot for `sar ax, n` and an optional
/// NEG, or `None` when that combination does not fit. Shift 1 keeps the
/// 3-byte D1 form where it can, so a census slot returns to its exact bytes.
///
/// | size | census form         | reachable                                       |
/// |------|---------------------|-------------------------------------------------|
/// | 3    | `D1 F8`             | sar 1                                           |
/// | 4    | `C1 F8 n`           | sar 1..3                                        |
/// | 6    | `D1 F8`, NEG        | sar 1 with NEG or `mov ax,ax`; sar 2..3, `66 90` |
/// | 7    | `C1 F8 n`, NEG      | sar 1..3 with NEG or `mov ax,ax`                |
fn slot_bytes(size: usize, n: u8, neg: bool) -> Option<Vec<u8>> {
    if !(1..=3).contains(&n) {
        return None;
    }
    let tail: &[u8] = if neg { &NEG_AX } else { &MOV_AX };
    let c1 = [0x66, 0xc1, 0xf8, n];
    let d1 = [0x66, 0xd1, 0xf8];
    match size {
        3 => (n == 1 && !neg).then(|| d1.to_vec()),
        4 => (!neg).then(|| c1.to_vec()),
        6 if n == 1 => Some([&d1[..], tail].concat()),
        6 => (!neg).then(|| [&c1[..], &NOP_AX[..]].concat()),
        7 => Some([&c1[..], tail].concat()),
        _ => None,
    }
}
/// Whether that form occurs in the FA_2.LIB gear census.
fn slot_retail(size: usize, n: u8, neg: bool) -> bool {
    match size {
        3 => true,
        4 => n >= 2,
        6 => n == 1 && neg,
        7 => n >= 2 && neg,
        _ => false,
    }
}
fn is_sar(i: &Insn) -> bool {
    matches!((i.op, i.ext, i.rm), (0xc1 | 0xd1, 7, Some(Rm::Reg(_))))
}
/// A C4 word store `mov [ebx+d], r16`; returns d (from C4+2).
fn store_disp(i: &Insn) -> Option<i32> {
    match (i.op, i.rm) {
        (
            0x89,
            Some(Rm::Mem(Mem::Reg {
                base: Some(3),
                index: None,
                disp,
            })),
        ) if i.word => Some(disp),
        _ => None,
    }
}
/// The gear law slots among a stub's instructions (sorted by address).
fn law_slots(list: &[Insn]) -> Vec<Slot> {
    let mut out = Vec::new();
    let mut stores = 0;
    // The instruction before `list[j]`, when it ends exactly where that starts.
    let before = |j: usize| {
        j.checked_sub(1)
            .map(|p| &list[p])
            .filter(|p| p.end() == list[j].at)
    };
    for (k, i) in list.iter().enumerate() {
        let Some(disp) = store_disp(i) else { continue };
        let store_index = stores;
        stores += 1;
        let (Some(axis), 0) = (Axis::of(disp), i.reg) else {
            continue;
        };
        let Some(p) = before(k) else { continue };
        let tail = match (p.op, p.ext, p.rm, p.len, p.word) {
            (0xf7, 3, Some(Rm::Reg(0)), 3, true) => Some(true),
            (0x89, _, Some(Rm::Reg(0)), 3, true) if p.reg == 0 => Some(false),
            (0x90, _, None, 2, true) => Some(false),
            _ => None,
        };
        let sar_k = if tail.is_some() { k - 1 } else { k };
        let Some(sar) = before(sar_k) else { continue };
        let shift = match (sar.op, sar.ext, sar.rm, sar.len, sar.word) {
            (0xd1, 7, Some(Rm::Reg(0)), 3, true) => 1,
            (0xc1, 7, Some(Rm::Reg(0)), 4, true) => sar.imm.unwrap_or(0).clamp(0, 31) as u8,
            _ => continue,
        };
        if p.op == 0x90 && sar.op != 0xc1 {
            continue;
        }
        // A branch into the slot would split a rewritten instruction.
        if list
            .iter()
            .any(|j| j.rel.is_some_and(|t| t > sar.at as i64 && t < i.at as i64))
        {
            continue;
        }
        out.push(Slot {
            at: sar.at,
            size: i.at - sar.at,
            shift,
            neg: tail == Some(true),
            shift_index: list[..sar_k - 1].iter().filter(|x| is_sar(x)).count(),
            store_index,
            word: 3 + axis as usize,
        });
    }
    out
}
fn shift_fixed(slot: &Slot) -> String {
    match slot.size {
        3 => "Shift 1 uses the 3-byte D1 form; 2 or 3 need the 4-byte C1 form and this law has no NEG slot to give up".into(),
        6 => "Shift 2 or 3 with NEG needs 7 bytes and this law slot has 6; reverse the direction first".into(),
        n => format!("No other shift fits this {n}-byte law slot"),
    }
}
fn direction_fixed(slot: &Slot) -> String {
    match slot.size {
        6 => format!(
            "NEG with shift {} needs 7 bytes and this law slot has 6; set shift 1 first",
            slot.shift
        ),
        n => format!("Reversing needs a 3-byte NEG and this law slot has only its {n}-byte shift"),
    }
}
/// Shift and direction controls of a gear law slot.
fn slot_controls(slot: &Slot, cs: usize) -> [Control; 2] {
    let fits: Vec<i32> = (1..=3)
        .filter(|n| slot_bytes(slot.size, *n as u8, slot.neg).is_some())
        .collect();
    let retail = slot_retail(slot.size, slot.shift, slot.neg);
    let mut shift = Control::new(
        "Shift",
        Setting::Shift {
            index: slot.shift_index,
            amount: slot.shift,
        },
        if fits.len() > 1 {
            Allowed::Values(fits.clone())
        } else {
            Allowed::Fixed(shift_fixed(slot))
        },
        cs + slot.at,
    );
    shift.retail = retail;
    shift.unseen = fits
        .into_iter()
        .filter(|n| !slot_retail(slot.size, *n as u8, slot.neg))
        .collect();
    let flip = slot_bytes(slot.size, slot.shift, !slot.neg).is_some();
    let mut direction = Control::new(
        "Direction",
        Setting::Direction {
            index: slot.store_index,
            negated: slot.neg,
        },
        if flip {
            Allowed::Either
        } else {
            Allowed::Fixed(direction_fixed(slot))
        },
        cs + slot.at,
    );
    direction.retail = retail;
    direction.unseen = [false, true]
        .into_iter()
        .filter(|neg| {
            slot_bytes(slot.size, slot.shift, *neg).is_some()
                && !slot_retail(slot.size, slot.shift, *neg)
        })
        .map(i32::from)
        .collect();
    [shift, direction]
}
/// The first variable an xform binding's laws read.
fn law_variable(b: &Binding) -> Option<&str> {
    match &b.kind {
        BindingKind::Xform { writes } => writes.iter().flat_map(|(_, l)| l).find_map(|o| match o {
            LawOp::Var(v) => Some(v.as_str()),
            _ => None,
        }),
        BindingKind::Toggle => None,
    }
}
/// The bindings of one stub resuming at one target.
fn group_of(inv: &Inventory, stub: usize, target: usize) -> Vec<&Binding> {
    inv.bindings
        .iter()
        .filter(|b| b.stub == stub && b.target == target)
        .collect()
}
/// The binding whose law reads a variable, else the first.
fn representative<'a>(group: &[&'a Binding]) -> Option<&'a Binding> {
    group
        .iter()
        .find(|b| law_variable(b).is_some())
        .or(group.first())
        .copied()
}
/// Gear law slots of the stub behind binding `b`.
fn binding_slots(b: &Binding, list: &[Insn]) -> Vec<Slot> {
    if law_variable(b) == Some(GEAR_LAW) && matches!(b.target_op, 0xc4 | 0xc6) {
        law_slots(list)
    } else {
        Vec::new()
    }
}
struct Fields {
    controls: Vec<Control>,
    locked: Option<String>,
}
const DIRECTION_GEAR_ONLY: &str =
    "Reversing needs a NEG added or removed; same-size forms are reviewed for gear laws only";
/// Controls of one stub for the binding resuming at `target` (CODE offsets).
fn fields(
    inv: &Inventory,
    code: &[u8],
    stub: usize,
    b: &Binding,
    vars: &BTreeSet<String>,
) -> Result<Fields> {
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
    if let Some(v) = vars.iter().find(|v| !reviewed(v)) {
        return Ok(Fields {
            controls,
            locked: Some(format!("{v} is not a reviewed aircraft part variable")),
        });
    }
    let law_var = law_variable(b);
    let census = LAWS.iter().find(|(v, ..)| Some(*v) == law_var);
    let two_term = law_var == Some("_PLswingWing");
    let list = insns(code, &s.blocks)?;
    let slots = binding_slots(b, &list);
    let xform_target = matches!(b.target_op, 0xc4 | 0xc6);
    let (mut compares, mut shifts, mut stores) = (0, 0, 0);
    for (k, i) in list.iter().enumerate() {
        let last = cs + i.end() - 1;
        if let Some(slot) = slots.iter().find(|x| (x.at..x.at + x.size).contains(&i.at)) {
            // A gear law slot: shift and direction, once, at its sar.
            if slot.at == i.at {
                controls.extend(slot_controls(slot, cs));
                shifts += 1;
            }
            continue;
        }
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
                controls.push(Control::new(
                    &format!("{name} compare"),
                    Setting::Compare { index, value },
                    allowed,
                    last,
                ));
                if let Some(j) = list.get(k + 1).filter(|j| j.at == i.end()) {
                    let op = slice(code, j.at, 1)?[0];
                    controls.push(Control::new(
                        &format!("{name} branch"),
                        Setting::Branch {
                            index,
                            equal: op == 0x74,
                        },
                        if matches!(op, 0x74 | 0x75) {
                            Allowed::Either
                        } else {
                            Allowed::Fixed(
                                "A ranged branch (<, >, >=); only = and != swap in place".into(),
                            )
                        },
                        cs + j.at,
                    ));
                }
            }
            (0xc1, 7, Some(Rm::Reg(_))) => {
                let amount = i.imm.unwrap_or(0) as u8;
                let range: Vec<i32> = if two_term {
                    (1..=7).collect()
                } else {
                    (1..=3).collect()
                };
                let seen = census.map_or(&[][..], |(_, s, _)| *s);
                let mut c = Control::new(
                    "Shift",
                    Setting::Shift {
                        index: shifts,
                        amount,
                    },
                    Allowed::Values(range.clone()),
                    last,
                );
                c.retail = seen.contains(&amount);
                c.unseen = range
                    .into_iter()
                    .filter(|n| !seen.contains(&(*n as u8)))
                    .collect();
                controls.push(c);
                shifts += 1;
            }
            (0xd1, 7, Some(Rm::Reg(_))) => {
                controls.push(Control::new(
                    "Shift",
                    Setting::Shift {
                        index: shifts,
                        amount: 1,
                    },
                    Allowed::Fixed(
                        "Shift 1 uses the 3-byte D1 form; 2 or 3 need the 4-byte C1 form".into(),
                    ),
                    cs + i.at,
                ));
                shifts += 1;
            }
            _ => {}
        }
        let Some(disp) = store_disp(i).filter(|_| xform_target) else {
            continue;
        };
        let index = stores;
        stores += 1;
        if !slots.iter().any(|x| x.store_index == index) {
            let negated = k
                .checked_sub(1)
                .map(|p| &list[p])
                .is_some_and(|p| p.end() == i.at && (p.op, p.ext) == (0xf7, 3));
            controls.push(Control::new(
                "Direction",
                Setting::Direction { index, negated },
                Allowed::Fixed(DIRECTION_GEAR_ONLY.into()),
                cs + i.at,
            ));
        }
        let Some(axis) = Axis::of(disp) else {
            controls.push(Control::new(
                "Store",
                Setting::Axis {
                    index,
                    axis: Axis::Yaw,
                },
                Allowed::Fixed("The law writes a translation word".into()),
                last,
            ));
            continue;
        };
        let seen = census.map_or(&[][..], |(_, _, a)| *a);
        let mut c = Control::new(
            "Rotation axis",
            Setting::Axis { index, axis },
            Allowed::Axes(Axis::ALL.to_vec()),
            last,
        );
        c.retail = seen.contains(&(axis as usize));
        c.unseen = (0..3).filter(|a| !seen.contains(&(*a as usize))).collect();
        controls.push(c);
    }
    if let (Some(p), true) = (b.pivot, xform_target) {
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
        controls.push(Control::new(
            "Pivot",
            Setting::Pivot(p),
            if foreign || stored {
                Allowed::Fixed("Native code writes this part's translation".into())
            } else {
                Allowed::Range(-32768, 32767)
            },
            cs + t + 2,
        ));
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
    for (k, first) in inv.bindings.iter().enumerate() {
        // One part per stub and target: paths with different laws (a ranged
        // gear stub that stores 0 below a threshold) are one part.
        if inv.bindings[..k]
            .iter()
            .any(|x| x.stub == first.stub && x.target == first.target)
        {
            continue;
        }
        let group = group_of(&inv, first.stub, first.target);
        let b = representative(&group).unwrap_or(first);
        let xform = group
            .iter()
            .any(|b| matches!(b.kind, BindingKind::Xform { .. }));
        if !xform && !matches!(b.target_op, 0x12 | 0x6e | 0xc4 | 0xc6) {
            continue;
        }
        // Inserted one by one: collecting sorts on a 4 KiB stack buffer,
        // which the CRT-free x86_64 build cannot probe.
        let mut vars = BTreeSet::new();
        for v in group.iter().flat_map(|b| b.variables()) {
            vars.insert(v);
        }
        let mut when = Vec::new();
        for c in group.iter().flat_map(|b| &b.when) {
            if !when.contains(c) {
                when.push(c.clone());
            }
        }
        let f = fields(&inv, code, b.stub, b, &vars)?;
        let variables: Vec<String> = vars.into_iter().collect();
        let law = law_variable(b);
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
            when,
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
fn stub_insns(inv: &Inventory, code: &[u8], stub: usize) -> Result<Vec<Insn>> {
    let s = inv
        .stubs
        .iter()
        .find(|s| s.offset == stub)
        .ok_or("Stub lost")?;
    insns(code, &s.blocks)
}
/// Instruction boundaries and kinds outside `skip`; a jcc keeps its class.
fn decode_shape(list: &[Insn], skip: (usize, usize)) -> Vec<(usize, u16, u8)> {
    list.iter()
        .filter(|i| !(skip.0..skip.1).contains(&i.at))
        .map(|i| {
            let op = if (0x70..=0x7f).contains(&i.op) {
                0x70
            } else {
                i.op
            };
            (i.at, op, i.ext)
        })
        .collect()
}
fn xform_law(b: &Binding, word: usize) -> Option<&Vec<LawOp>> {
    match &b.kind {
        BindingKind::Xform { writes } => writes.iter().find(|(w, _)| *w == word).map(|(_, l)| l),
        BindingKind::Toggle => None,
    }
}
/// What the re-parsed binding must report after an edit.
enum Expect {
    Setting,
    /// A gear law slot rewritten to (shift, neg): the slot re-parses so and
    /// the law stored into `word` is exactly `law`.
    Slot {
        slot: Slot,
        laws: Vec<Option<Vec<LawOp>>>,
    },
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
        | (Setting::Direction { index: a, .. }, Setting::Direction { index: b, .. })
        | (Setting::Axis { index: a, .. }, Setting::Axis { index: b, .. }) => a == b,
        (Setting::Pivot(_), Setting::Pivot(_)) => true,
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
    if let Allowed::Fixed(why) = &c.allowed {
        return Err(why.clone());
    }
    let before = Inventory::parse(source)?;
    let cs = before.code_start;
    let (stub, target) = (part.stub - cs, part.target - cs);
    let code = slice(source, cs, before.code_len)?;
    let group = group_of(&before, stub, target);
    let binding = representative(&group).ok_or("No such part in this shape")?;
    let old_list = stub_insns(&before, code, stub)?;
    let slots = binding_slots(binding, &old_list);
    let slot = slots.iter().find(|x| match setting {
        Setting::Shift { index, .. } => x.shift_index == *index,
        Setting::Direction { index, .. } => x.store_index == *index,
        _ => false,
    });
    let mut out = source.to_vec();
    let mut put = |at: usize, bytes: &[u8]| -> Result<()> {
        out.get_mut(at..at + bytes.len())
            .ok_or("Field outside the shape")?
            .copy_from_slice(bytes);
        Ok(())
    };
    let mut skip = (0, 0);
    let expect = if let Some(slot) = slot {
        let (shift, neg) = match setting {
            Setting::Shift { amount, .. } => (*amount, slot.neg),
            Setting::Direction { negated, .. } => (slot.shift, *negated),
            _ => (slot.shift, slot.neg),
        };
        let bytes = slot_bytes(slot.size, shift, neg).ok_or_else(|| {
            if let Setting::Shift { .. } = setting {
                if (1..=3).contains(&shift) {
                    shift_fixed(slot)
                } else {
                    invalid("Gear shifts are 1 to 3")
                }
            } else {
                direction_fixed(slot)
            }
        })?;
        put(cs + slot.at, &bytes)?;
        skip = (slot.at, slot.at + slot.size);
        // Expected laws, one per path: each old law with its trailing NEG
        // and last shift replaced.
        let mut laws = Vec::new();
        for b in &group {
            let Some(old) = xform_law(b, slot.word) else {
                laws.push(None);
                continue;
            };
            let mut law = old.clone();
            if slot.neg && law.pop() != Some(LawOp::Neg { w16: true }) {
                return Err(invalid("The gear law does not end in its NEG"));
            }
            if law.pop()
                != Some(LawOp::Sar {
                    n: slot.shift,
                    w16: true,
                })
            {
                return Err(invalid("The gear law does not end in its shift"));
            }
            law.push(LawOp::Sar {
                n: shift,
                w16: true,
            });
            if neg {
                law.push(LawOp::Neg { w16: true });
            }
            laws.push(Some(law));
        }
        Expect::Slot {
            slot: Slot {
                shift,
                neg,
                ..*slot
            },
            laws,
        }
    } else {
        match (&c.allowed, setting) {
            (Allowed::Values(v), Setting::Compare { value, .. }) if v.contains(value) => {
                put(c.at, &[*value as i8 as u8])?
            }
            (Allowed::Values(v), Setting::Shift { amount, .. })
                if v.contains(&(*amount as i32)) =>
            {
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
        Expect::Setting
    };
    // Verify from scratch.
    let after = Inventory::parse(&out)?;
    if !after.contiguous() || after.opaque_bytes() != before.opaque_bytes() {
        return Err(invalid("The edit changed how much of CODE is explained"));
    }
    let s = after
        .stubs
        .iter()
        .find(|s| s.offset == stub)
        .ok_or("The stub was lost")?;
    if s.unrecognised.is_some() || after.stubs.len() != before.stubs.len() {
        return Err(invalid("The edited stub is no longer recognised"));
    }
    let new_list = stub_insns(&after, slice(&out, cs, after.code_len)?, stub)?;
    if decode_shape(&old_list, skip) != decode_shape(&new_list, skip) {
        return Err(invalid("The edit changed the stub's instructions"));
    }
    let new_group = group_of(&after, stub, target);
    let b = representative(&new_group).ok_or("The edited part no longer resumes at its target")?;
    let conditions = || new_group.iter().flat_map(|b| b.when.iter().flatten());
    let reported = match (&expect, setting) {
        (Expect::Slot { slot, laws }, _) => {
            binding_slots(b, &new_list).contains(slot)
                && new_group.len() == group.len()
                && new_group.iter().zip(&group).zip(laws).all(|((a, o), law)| {
                    a.when == o.when && xform_law(a, slot.word) == law.as_ref()
                })
        }
        (_, Setting::Compare { value, .. }) => conditions().any(|c| c.value == *value),
        (_, Setting::Branch { .. }) => b.when != binding.when,
        (_, Setting::Shift { amount, .. }) => match &b.kind {
            BindingKind::Xform { writes } => writes
                .iter()
                .flat_map(|(_, l)| l)
                .any(|o| matches!(o, LawOp::Sar { n, .. } if n == amount)),
            BindingKind::Toggle => false,
        },
        (_, Setting::Axis { axis, .. }) => match &b.kind {
            BindingKind::Xform { writes } => writes.iter().any(|(w, _)| *w == 3 + *axis as usize),
            BindingKind::Toggle => false,
        },
        (_, Setting::Pivot(v)) => b.pivot == Some(*v),
        (_, Setting::Direction { .. }) => false,
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
        // A 7-byte C1 + NEG slot reverses in place.
        assert_eq!(gear.controls[3].allowed, Allowed::Either);
        assert!(gear.controls.iter().all(|c| c.retail));
        // The D1 form keeps shift 1 read-only.
        let right = find(&parts, "Gear right");
        assert!(right.controls.iter().any(|c| c.setting
            == Setting::Shift {
                index: 0,
                amount: 1
            }
            && matches!(c.allowed, Allowed::Fixed(_))));
        assert!(right.controls.iter().any(|c| c.setting
            == Setting::Direction {
                index: 0,
                negated: false
            }
            && matches!(c.allowed, Allowed::Fixed(_))));
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
        assert!(apply_part_setting(
            &b,
            find(&list, "Gear right").id,
            &Setting::Direction {
                index: 0,
                negated: true
            }
        )
        .unwrap_err()
        .contains("NEG"));
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
    /// (shift, negated) of the Gear left law in a shape.
    fn gear_state(bytes: &[u8]) -> (u8, bool, bool) {
        let gear = find(&parts(bytes).unwrap(), "Gear left").clone();
        let shift = gear
            .controls
            .iter()
            .find_map(|c| match c.setting {
                Setting::Shift { amount, .. } => Some((amount, c.retail)),
                _ => None,
            })
            .unwrap();
        let neg = gear
            .controls
            .iter()
            .find_map(|c| match c.setting {
                Setting::Direction { negated, .. } => Some(negated),
                _ => None,
            })
            .unwrap();
        (shift.0, neg, shift.1)
    }
    #[test]
    fn gear_law_slots_reach_exactly_the_forms_that_fit() {
        use alloc::collections::{BTreeMap, BTreeSet};
        // Every census gear signature: D1, D1+NEG, C1 2/3, C1 2/3 + NEG.
        type Case = (Shift, bool, &'static [(u8, bool)]);
        let cases: [Case; 6] = [
            (Shift::One, false, &[(1, false)]),
            (
                Shift::One,
                true,
                &[(1, false), (1, true), (2, false), (3, false)],
            ),
            (Shift::Imm(2), false, &[(1, false), (2, false), (3, false)]),
            (Shift::Imm(3), false, &[(1, false), (2, false), (3, false)]),
            (
                Shift::Imm(2),
                true,
                &[
                    (1, false),
                    (1, true),
                    (2, false),
                    (2, true),
                    (3, false),
                    (3, true),
                ],
            ),
            (
                Shift::Imm(3),
                true,
                &[
                    (1, false),
                    (1, true),
                    (2, false),
                    (2, true),
                    (3, false),
                    (3, true),
                ],
            ),
        ];
        for (shift, neg, reachable) in cases {
            let size = match (&shift, neg) {
                (Shift::One, false) => 3,
                (Shift::One, true) => 6,
                (Shift::Imm(_), false) => 4,
                (Shift::Imm(_), true) => 7,
            };
            let original = shape(shift, neg);
            let start = gear_state(&original);
            assert!(start.2, "census form reads as retail");
            let id = find(&parts(&original).unwrap(), "Gear left").id;
            // Breadth-first over every allowed shift and direction change.
            let mut seen = BTreeMap::<(u8, bool), Vec<u8>>::new();
            let mut work = alloc::vec![original.clone()];
            seen.insert((start.0, start.1), original.clone());
            while let Some(bytes) = work.pop() {
                let gear = find(&parts(&bytes).unwrap(), "Gear left").clone();
                for c in &gear.controls {
                    let next: Vec<Setting> = match (&c.allowed, &c.setting) {
                        (Allowed::Values(v), Setting::Shift { index, .. }) => v
                            .iter()
                            .map(|n| Setting::Shift {
                                index: *index,
                                amount: *n as u8,
                            })
                            .collect(),
                        (Allowed::Either, Setting::Direction { index, negated }) => {
                            alloc::vec![Setting::Direction {
                                index: *index,
                                negated: !negated
                            }]
                        }
                        _ => Vec::new(),
                    };
                    for setting in next {
                        let out = apply_part_setting(&bytes, id, &setting).unwrap();
                        // Same size, only the slot's bytes change.
                        assert_eq!(out.len(), bytes.len());
                        let diff: Vec<_> =
                            (0..out.len()).filter(|i| out[*i] != bytes[*i]).collect();
                        assert!(diff.iter().all(|i| (c.at..c.at + 7).contains(i)));
                        let state = gear_state(&out);
                        let key = (state.0, state.1);
                        // Each state has one canonical encoding.
                        match seen.get(&key) {
                            Some(known) => assert_eq!(known, &out, "{key:?}"),
                            None => {
                                seen.insert(key, out.clone());
                                work.push(out);
                            }
                        }
                    }
                    // No-op identity.
                    assert_eq!(apply_part_setting(&bytes, id, &c.setting).unwrap(), bytes);
                }
            }
            let got: BTreeSet<_> = seen.keys().copied().collect();
            let want: BTreeSet<_> = reachable.iter().copied().collect();
            assert_eq!(got, want, "{:?}", (start.0, neg));
            // The census state is the original bytes, reached back from every
            // other state; every other state is flagged as not seen in retail.
            assert_eq!(seen[&(start.0, start.1)], original);
            for (key, bytes) in &seen {
                let (n, negated, retail) = gear_state(bytes);
                assert_eq!((n, negated), *key);
                let census = slot_retail(size, n, negated);
                assert_eq!(retail, census, "{key:?}");
                // The evaluator proves the law: the preview turns by
                // -(gearPos >> n), negated when the NEG is present.
                let pose = Pose::from([("_PLgearDown".into(), 1), ("_PLgearPos".into(), -8192)]);
                let m = Model::with_pose(bytes, &pose).unwrap();
                let part = m.parts.iter().find(|p| p.offset == id.target).unwrap();
                let turn = (-8192i32 >> n) * if negated { -1 } else { 1 };
                assert_eq!(part.posed_rotation, [0, 0, turn], "{key:?}");
                // Every other part is untouched.
                let others: Vec<_> = parts(bytes)
                    .unwrap()
                    .into_iter()
                    .filter(|p| p.id != id)
                    .collect();
                let base: Vec<_> = parts(&original)
                    .unwrap()
                    .into_iter()
                    .filter(|p| p.id != id)
                    .collect();
                assert_eq!(others, base);
            }
        }
    }
    #[test]
    fn gear_law_slots_refuse_forms_that_do_not_fit() {
        let gear = |b: &[u8]| find(&parts(b).unwrap(), "Gear left").clone();
        let shift = |n| Setting::Shift {
            index: 0,
            amount: n,
        };
        let dir = |negated| Setting::Direction { index: 0, negated };
        // D1 only: neither a NEG nor a C1 shift fits.
        let d1 = shape(Shift::One, false);
        let id = gear(&d1).id;
        assert!(apply_part_setting(&d1, id, &dir(true))
            .unwrap_err()
            .contains("3-byte shift"));
        assert!(apply_part_setting(&d1, id, &shift(2))
            .unwrap_err()
            .contains("D1"));
        // C1 only: no NEG.
        let c1 = shape(Shift::Imm(3), false);
        let id = gear(&c1).id;
        assert!(apply_part_setting(&c1, id, &dir(true))
            .unwrap_err()
            .contains("4-byte shift"));
        assert!(apply_part_setting(&c1, id, &shift(4)).is_err());
        // D1 + NEG: shift 2 or 3 only without the NEG.
        let d1n = shape(Shift::One, true);
        let g = gear(&d1n);
        let id = g.id;
        assert!(g
            .controls
            .iter()
            .any(|c| c.label == "Shift" && matches!(c.allowed, Allowed::Fixed(_))));
        assert!(apply_part_setting(&d1n, id, &shift(2))
            .unwrap_err()
            .contains("reverse the direction first"));
        let flipped = apply_part_setting(&d1n, id, &dir(false)).unwrap();
        let g = gear(&flipped);
        let direction = g.controls.iter().find(|c| c.label == "Direction").unwrap();
        assert!(!direction.retail);
        assert_eq!(direction.unseen, alloc::vec![0]);
        let wide = apply_part_setting(&flipped, id, &shift(3)).unwrap();
        let at = direction.at;
        assert_eq!(&wide[at..at + 6], &[0x66, 0xc1, 0xf8, 3, 0x66, 0x90]);
        assert!(apply_part_setting(&wide, id, &dir(true))
            .unwrap_err()
            .contains("set shift 1 first"));
        // Shift 1 returns to the D1 form, then the NEG restores the census bytes.
        let back = apply_part_setting(&wide, id, &shift(1)).unwrap();
        assert_eq!(&back[at..at + 6], &[0x66, 0xd1, 0xf8, 0x66, 0x89, 0xc0]);
        assert_eq!(apply_part_setting(&back, id, &dir(true)).unwrap(), d1n);
        // A swing wing's direction stays read-only.
        let wing = find(&parts(&d1n).unwrap(), "Swing wing left").clone();
        let d = wing
            .controls
            .iter()
            .find(|c| c.label == "Direction")
            .unwrap();
        assert!(matches!(&d.allowed, Allowed::Fixed(r) if r.contains("gear laws only")));
        assert!(apply_part_setting(&d1n, wing.id, &dir(true)).is_err());
        // Truncations never panic.
        for n in (0..flipped.len()).step_by(7) {
            let _ = parts(&flipped[..n]);
        }
    }
    #[test]
    fn demo_parts_fixture_is_fully_editable() {
        let b = crate::shape_testkit::demo_parts();
        let list = parts(&b).unwrap();
        let names: Vec<_> = list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Gear left",
                "Gear right",
                "Nose gear",
                "Flap left (state -1)",
                "Flap left (state 0)",
                "Flap right (state -1)",
                "Flap right (state 0)",
                "Rudder (state 0)",
                "Hook (state 1)",
                "Speed brake (state 1)",
                "Afterburner (state 1)"
            ]
        );
        assert!(list.iter().all(|p| p.locked.is_none() && p.retail()));
        let g = crate::shape_geometry::Geometry::parse(&b).unwrap();
        assert!(g.vertex_report().iter().all(|v| v.refusal.is_none()));
        assert!((0..g.faces.len()).all(|f| g.face_refusal(f).is_none()));
        let neutral = Model::parse(&b).unwrap();
        let down = Model::with_pose(
            &b,
            &Pose::from([
                ("_PLgearDown".into(), 1),
                ("_PLleftFlap".into(), -1),
                ("_PLafterBurner".into(), 1),
            ]),
        )
        .unwrap();
        assert!(down.faces.len() > neutral.faces.len());
        // The left leg's direction flips in place.
        let gear = find(&list, "Gear left");
        let out = apply_part_setting(
            &b,
            gear.id,
            &Setting::Direction {
                index: 0,
                negated: false,
            },
        )
        .unwrap();
        assert!(!find(&parts(&out).unwrap(), "Gear left").retail());
    }
}
