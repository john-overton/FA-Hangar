//! Reviewed BRF record views and transactional characteristic grafts.
//! Values stay in source storage units; names are never mapped by field index.
use crate::{
    brf::{Brf, Field},
    invalid, schema, Result,
};
use alloc::{string::String, vec::Vec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aspect {
    Envelope,
    Propulsion,
    Handling,
    Weights,
    Damage,
    Hardpoints,
    Systems,
    Seeker,
    Motor,
    Warhead,
    Movement,
    Engagement,
}
pub const ASPECTS: [Aspect; 12] = [
    Aspect::Envelope,
    Aspect::Propulsion,
    Aspect::Handling,
    Aspect::Weights,
    Aspect::Damage,
    Aspect::Hardpoints,
    Aspect::Systems,
    Aspect::Seeker,
    Aspect::Motor,
    Aspect::Warhead,
    Aspect::Movement,
    Aspect::Engagement,
];
impl Aspect {
    pub fn label(self) -> &'static str {
        match self {
            Self::Envelope => "Flight envelope",
            Self::Propulsion => "Propulsion",
            Self::Handling => "Handling",
            Self::Weights => "Weights",
            Self::Damage => "Damage",
            Self::Hardpoints => "Hardpoint values",
            Self::Systems => "Systems",
            Self::Seeker => "Seeker",
            Self::Motor => "Motor",
            Self::Warhead => "Warhead",
            Self::Movement => "Movement",
            Self::Engagement => "Engagement / firing",
        }
    }
    pub fn bit(self) -> u16 {
        1 << self as u16
    }
}
pub fn aspect(label: &str) -> Option<Aspect> {
    if matches!(label, "object._acc" | "object._dacc") {
        return Some(Aspect::Movement);
    }
    if matches!(
        label,
        "npc.searchFrequencyT"
            | "npc.unreadyAttackT"
            | "npc.attackT"
            | "npc.retargetT"
            | "npc.zoneDist"
    ) || label.starts_with("projectile.offsetFire")
        || label.starts_with("projectile.gameRound")
        || label.starts_with("projectile.gameBurst")
        || matches!(
            label,
            "projectile.reloadT"
                | "projectile.startupShots"
                | "projectile.actualRoundsPerGame"
                | "projectile.randomFirePercent"
                | "projectile.launchRetard"
        )
    {
        return Some(Aspect::Engagement);
    }

    if label.starts_with("hardpoint[") || label == "npc.numHards" {
        return Some(Aspect::Hardpoints);
    }
    if label.starts_with("envelope[")
        || matches!(
            label,
            "plane.envMin"
                | "plane.envMax"
                | "object._minSpeed"
                | "object._cornerSpeed"
                | "object._maxSpeed"
                | "object.minAlt"
                | "object.maxAlt"
        )
    {
        return Some(Aspect::Envelope);
    }
    if matches!(
        label,
        "plane.engines"
            | "plane.thrust"
            | "plane.aftThrust"
            | "plane.throttleAcc"
            | "plane.throttleDacc"
            | "plane.fuelConsumption"
            | "plane.aftFuelConsumption"
            | "plane.internalFuel"
    ) {
        return Some(Aspect::Propulsion);
    }
    if matches!(
        label,
        "object.weight" | "plane.maxTakeoffWeight" | "sensor.weight" | "ecm.weight"
    ) {
        return Some(Aspect::Weights);
    }
    if label.starts_with("object.damage[")
        || label.starts_with("plane.systemDamage[")
        || label.starts_with("plane.structure")
        || label == "object.hitPoints"
    {
        return Some(Aspect::Damage);
    }
    if label.starts_with("object.sigs[")
        || label.starts_with("sensor.")
        || label.starts_with("ecm.")
    {
        return (!label.ends_with("structType")).then_some(Aspect::Systems);
    }
    if let Some(key) = label.strip_prefix("projectile.") {
        if key.starts_with("zone")
            || matches!(
                key,
                "lookDown"
                    | "dopplerSpeedAbove"
                    | "dopplerSpeedBelow"
                    | "dopplerMinRange"
                    | "allAspect"
                    | "chaffFlareChance"
                    | "deceptionChance"
                    | "trackT"
                    | "trackMaxG"
                    | "targetSunChance"
                    | "sig"
            )
        {
            return Some(Aspect::Seeker);
        }
        if matches!(
            key,
            "initialSpeed"
                | "finalSpeed"
                | "igniteT"
                | "fuelT"
                | "removeT"
                | "poweredTurnRate"
                | "unpoweredTurnRate"
                | "performanceAt0"
                | "performanceAt20"
        ) {
            return Some(Aspect::Motor);
        }
        if key.starts_with("fuze")
            || matches!(
                key,
                "collateralDamageRadius"
                    | "collateralDamagePercent"
                    | "sideHitFuzeFailure"
                    | "expTypeForLand"
                    | "expTypeForWater"
            )
        {
            return Some(Aspect::Warhead);
        }
    }
    if label.starts_with("plane._bv")
        || label.starts_with("plane._brv")
        || label.starts_with("plane.rudder")
        || label.starts_with("plane.stall")
        || label.starts_with("plane.spin")
        || matches!(
            label,
            "object._turnRate"
                | "object._bankRate"
                | "object.maxClimb"
                | "object.maxDive"
                | "object.maxBank"
                | "object._acc"
                | "object._dacc"
                | "plane.gpullAOA"
                | "plane.lowAOASpeed"
                | "plane.lowAOAPitch"
                | "plane.negGLimit"
                | "plane.coefDrag"
                | "plane.flapsLift"
                | "plane.flapsDrag"
                | "plane.gearDrag"
                | "plane.airBrakesDrag"
        )
    {
        return Some(Aspect::Handling);
    }
    None
}
fn integer(field: &Field) -> Option<i64> {
    if let Some(h) = field.value.strip_prefix('$') {
        let n = i64::from_str_radix(h, 16).ok()?;
        Some(if field.kind == "word" {
            n as u16 as i16 as i64
        } else {
            n
        })
    } else {
        field.value.parse().ok()
    }
}
fn value(fields: &[Field], name: &str) -> Option<i64> {
    fields.iter().find(|f| f.label == name).and_then(integer)
}
fn record_matches(fields: &[Field], indices: &[usize], layout: &[(&str, &str)]) -> bool {
    indices.len().is_multiple_of(layout.len())
        && indices.iter().enumerate().all(|(n, i)| {
            let (kind, _) = layout[n % layout.len()];
            let f = &fields[*i];
            f.kind == kind || (kind == "ptr" && f.kind == "dword" && f.value == "0")
        })
}
/// Only annotate linked blocks after exact kind/count matching. Unrecognized
/// records keep their original indexed labels and a visible diagnostic.
pub(crate) fn annotate(fields: &mut [Field]) -> Vec<String> {
    let mut issues = Vec::new();
    for (pointer, prefix, layout, count) in [
        (
            "npc.hards",
            "hardpoint",
            schema::HARDPOINT,
            value(fields, "npc.numHards").filter(|n| (0..=64).contains(n)),
        ),
        (
            "plane.env",
            "envelope",
            schema::ENVELOPE,
            value(fields, "plane.envMin")
                .zip(value(fields, "plane.envMax"))
                .and_then(|(min, max)| {
                    (min >= -20 && max <= 30 && max >= min).then_some(max - min + 1)
                }),
        ),
    ] {
        let Some(ptr) = fields.iter().find(|f| f.label == pointer) else {
            continue;
        };
        if ptr.kind == "dword" && ptr.value == "0" {
            if pointer == "npc.hards" && count.is_some_and(|n| n != 0) {
                issues.push("Hardpoint count is nonzero but the station pointer is null".into());
            }
            continue;
        }
        let block = ptr.value.clone();
        let indices: Vec<_> = fields
            .iter()
            .enumerate()
            .filter(|(_, f)| f.block == block)
            .map(|(i, _)| i)
            .collect();
        if count.is_none()
            || indices.len() != count.unwrap_or(0) as usize * layout.len()
            || !record_matches(fields, &indices, layout)
        {
            issues.push(format!(
                "{prefix}: linked record count or field kinds do not match the reviewed schema"
            ));
            continue;
        }
        for (n, i) in indices.iter().enumerate() {
            fields[*i].label = format!(
                "{prefix}[{}].{}",
                n / layout.len(),
                layout[n % layout.len()].1
            );
        }
        if prefix == "envelope" {
            let min = value(fields, "plane.envMin").unwrap();
            for (n, row) in indices.chunks_exact(layout.len()).enumerate() {
                if integer(&fields[row[0]]) != Some(min + n as i64)
                    || !integer(&fields[row[1]]).is_some_and(|n| (3..=20).contains(&n))
                {
                    issues.push(format!(
                        "envelope[{n}]: G row or active point count is inconsistent"
                    ));
                }
            }
        }
    }
    issues
}
#[derive(Clone, Debug)]
pub struct Delta {
    pub index: usize,
    pub label: String,
    pub before: String,
    pub after: String,
    pub kind: String,
    pub scaled: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Graft {
    pub edits: Vec<Delta>,
    pub conflicts: Vec<String>,
}
impl Graft {
    pub fn review(target: &Brf, source: &Brf, mask: u16) -> Self {
        let mut result = Self::default();
        for group in ASPECTS {
            if mask & group.bit() == 0 {
                continue;
            }
            let target_fields: Vec<_> = target
                .fields
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    aspect(&f.label) == Some(group)
                        && matches!(f.kind.as_str(), "byte" | "word" | "dword")
                })
                .collect();
            let source_count = source
                .fields
                .iter()
                .filter(|f| {
                    aspect(&f.label) == Some(group)
                        && matches!(f.kind.as_str(), "byte" | "word" | "dword")
                })
                .count();
            if target_fields.is_empty() || source_count != target_fields.len() {
                result.conflicts.push(format!(
                    "{}: record layouts/counts differ or are unrecognized",
                    group.label()
                ));
                continue;
            }
            if matches!(group, Aspect::Hardpoints | Aspect::Envelope)
                && (!target.issues.is_empty() || !source.issues.is_empty())
            {
                result.conflicts.push(format!(
                    "{}: resolve record diagnostics before grafting",
                    group.label()
                ));
                continue;
            }
            for (index, f) in target_fields {
                if let Some(other) = source.fields.iter().find(|other| {
                    other.label == f.label && other.kind == f.kind && other.scaled == f.scaled
                }) {
                    if f.value != other.value {
                        result.edits.push(Delta {
                            index,
                            label: f.label.clone(),
                            before: f.value.clone(),
                            after: other.value.clone(),
                            kind: f.kind.clone(),
                            scaled: f.scaled,
                        });
                    }
                } else {
                    result
                        .conflicts
                        .push(format!("{}: field kind or scaling differs", f.label));
                }
            }
        }
        result
    }
    pub fn apply(&self, target: &[u8], extension: &str) -> Result<Vec<u8>> {
        if !self.conflicts.is_empty() {
            return Err(invalid("Resolve selected graft conflicts before applying"));
        }
        let brf = Brf::parse(target, extension)?;
        for edit in &self.edits {
            let f = brf.fields.get(edit.index).ok_or("Graft target changed")?;
            if f.label != edit.label
                || f.value != edit.before
                || f.kind != edit.kind
                || f.scaled != edit.scaled
            {
                return Err(invalid("Graft target changed; review again"));
            }
        }
        brf.edit_many(
            target,
            &self
                .edits
                .iter()
                .map(|e| (e.index, e.after.clone()))
                .collect::<Vec<_>>(),
            extension,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_labels_and_multi_field_graft_preserve_target_identity_and_other_bytes() {
        let target = crate::brf::demo_with_records();
        let brf = Brf::parse(&target, "PT").unwrap();
        assert!(brf.issues.is_empty());
        assert!(brf.fields.iter().any(|f| f.label == "hardpoint[1].pos.x"));
        assert!(brf.fields.iter().any(|f| f.label == "envelope[2].alt[19]"));
        let weight = brf
            .fields
            .iter()
            .position(|f| f.label == "object.weight")
            .unwrap();
        let thrust = brf
            .fields
            .iter()
            .position(|f| f.label == "plane.thrust")
            .unwrap();
        let donor_bytes = brf
            .edit_many(
                &target,
                &[(weight, "12345".into()), (thrust, "6789".into())],
                "PT",
            )
            .unwrap();
        let donor = Brf::parse(&donor_bytes, "PT").unwrap();
        let graft = Graft::review(&brf, &donor, Aspect::Weights.bit());
        assert!(graft.conflicts.is_empty());
        assert_eq!(graft.edits.len(), 1);
        let output = graft.apply(&target, "PT").unwrap();
        assert_eq!(output, brf.edit(&target, weight, "12345", "PT").unwrap());
        let graft = Graft::review(
            &brf,
            &donor,
            Aspect::Weights.bit() | Aspect::Propulsion.bit(),
        );
        assert_eq!(graft.apply(&target, "PT").unwrap(), donor_bytes);
        assert!(graft.apply(&output, "PT").is_err());
    }
    #[test]
    fn incompatible_records_and_scaled_fields_are_conflicts_not_index_matches() {
        let data = crate::brf::demo_with_records();
        let target = Brf::parse(&data, "PT").unwrap();
        let incompatible = String::from_utf8(data.clone())
            .unwrap()
            .replace("byte 2 ; numHards", "byte 3 ; numHards");
        let donor = Brf::parse(incompatible.as_bytes(), "PT").unwrap();
        assert!(!donor.issues.is_empty());
        assert!(!Graft::review(&target, &donor, Aspect::Hardpoints.bit())
            .conflicts
            .is_empty());
        let scaled = String::from_utf8(data.clone())
            .unwrap()
            .replace("dword 10000 ; weight", "dword ^10000 ; weight");
        let donor = Brf::parse(scaled.as_bytes(), "PT").unwrap();
        assert!(!Graft::review(&target, &donor, Aspect::Weights.bit())
            .conflicts
            .is_empty());
        let gload = String::from_utf8(data)
            .unwrap()
            .replace("word -1 ; gload", "word 3 ; gload");
        assert!(!Brf::parse(gload.as_bytes(), "PT")
            .unwrap()
            .issues
            .is_empty());
    }
}
