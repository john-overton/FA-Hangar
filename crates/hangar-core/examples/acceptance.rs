//! Local-only handoff generator. Reads user-owned LIBs and writes a fresh directory.
//! No source payloads are included in the repository or CI artifacts.
use hangar_core::{
    animation,
    archive::{Archive, Entry},
    brf::Brf,
    clone_aircraft,
    document::Document,
    hardpoints,
    model::Model,
    picture::{self, Pic},
    shape_edit, Result,
};
use std::{collections::BTreeMap, fs, io::Write, path::Path};
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes).map_err(|e| e.to_string())
}
fn save(dir: &Path, name: &str, archive: &Archive) -> Result<()> {
    let bytes = archive.bytes()?;
    let reopened = Archive::parse(bytes.clone())?;
    for e in &archive.entries {
        let other = reopened.find(&e.name).ok_or("Missing entry after reopen")?;
        if e.read()? != reopened.entries[other].read()? {
            return Err("Reopened payload differs".into());
        }
    }
    write(&dir.join(name), &bytes)
}
fn read(archive: &Archive, name: &str) -> Result<Vec<u8>> {
    archive.entries[archive.find(name).ok_or(format!("Missing {name}"))?].read()
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: acceptance FA_2.LIB FA_1.LIB NEW_OUTPUT_DIRECTORY".into());
    }
    let sources = [
        Archive::parse(fs::read(&args[0]).map_err(|e| e.to_string())?)?,
        Archive::parse(fs::read(&args[1]).map_err(|e| e.to_string())?)?,
    ];
    let out = Path::new(&args[2]);
    fs::create_dir(out).map_err(|e| e.to_string())?;
    let mut report=String::from("TORE HANGAR LOCAL ACCEPTANCE CANDIDATES\nOriginal-game results: PENDING USER TEST\n\nUse one candidate folder at a time. Baseline and edited folders deliberately share private filenames; do not load both together. Keep stock LIBs intact. New IDs may require mission/loadout selection; presence in a package does not prove runtime registration. Record game version, enabled LIB order, object selection, views/LOD/damage and results in RESULTS.tsv.\n\n");
    let cases = [
        ("01-weapon", "AIM9M.JT", "HGWPN", "Hangar test missile"),
        ("02-static", "BLDG1.OT", "HGBLD", "Hangar test building"),
        ("03-ship", "TICON.NT", "HGSHIP", "Hangar test ship"),
        ("04-tank", "M1.NT", "HGTANK", "Hangar test tank"),
        ("05-aaa", "ZSU23.NT", "HGAAA", "Hangar test AAA"),
        ("06-sam", "SA2A.NT", "HGSAM", "Hangar test SAM"),
        (
            "07-mobile-launcher",
            "CHAP.NT",
            "HGCHAP",
            "Hangar test launcher",
        ),
    ];
    let mut results = String::from(
        "case\tbaseline_load\tedited_load\tvisuals_and_damage\tlaunch_and_slew\tnotes\n",
    );
    for (folder, donor, id, title) in cases {
        let package = clone_aircraft::build(&[&sources[0], &sources[1]], donor, id, title)?;
        let ext = donor.rsplit('.').next().unwrap();
        let name = format!("{id}.{ext}");
        let original = Brf::parse(&read(&sources[0], donor)?, ext)?;
        let bytes = read(&package.archive, &name)?;
        let b = Brf::parse(&bytes, ext)?;
        let values = |b: &Brf| {
            b.fields
                .iter()
                .filter(|f| matches!(f.kind.as_str(), "word" | "byte" | "dword"))
                .map(|f| (&f.label, &f.value, f.scaled))
                .map(|(a, b, c)| (a.clone(), b.clone(), c))
                .collect::<Vec<_>>()
        };
        if values(&original) != values(&b) {
            return Err(format!("{donor}: numerical operands changed during clone"));
        }
        let dir = out.join(folder);
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        for sub in ["baseline", "edited"] {
            fs::create_dir(dir.join(sub)).map_err(|e| e.to_string())?;
        }
        save(
            &dir.join("baseline"),
            &format!("{id}.LIB"),
            &package.archive,
        )?;
        let mut doc = Document::new(package.archive);
        let at = doc.archive.find(&name).unwrap();
        let (edited, change) = if ext == "NT" {
            let station = hardpoints::read(&b)?
                .into_iter()
                .next()
                .ok_or("NPC has no station")?;
            let mut pos = station.position;
            pos[0] += 10;
            let moved = hardpoints::position(&bytes, 0, pos)?;
            let parsed = Brf::parse(&moved, ext)?;
            let pitch = hardpoints::read(&parsed)?[0].fields[5];
            let changed = parsed.edit(&moved, pitch, "1024", ext)?;
            (changed,format!("Station 1 X: {} -> {}; pitch: {} -> 1024 (stored units). Verify launch point, firing direction and slew limits; do not infer degree or model-axis equivalence.",station.position[0],pos[0],b.fields[station.fields[5]].value))
        } else {
            let label = if ext == "JT" {
                "projectile.fuelT"
            } else {
                "object.hitPoints"
            };
            let (i, f) = b
                .fields
                .iter()
                .enumerate()
                .find(|(_, f)| f.label == label)
                .ok_or("Missing named edit field")?;
            let before: i32 = f.value.parse().map_err(|_| "Nondecimal test operand")?;
            (
                b.edit(&bytes, i, &(before + 1).to_string(), ext)?,
                format!("{label}: {before} -> {} (stored units).", before + 1),
            )
        };
        doc.replace(at, edited)?;
        save(&dir.join("edited"), &format!("{id}.LIB"), &doc.archive)?;
        if !doc.undo() || doc.archive.entries[at].read()? != bytes {
            return Err("Variant edit undo failed".into());
        }
        report.push_str(&format!("{folder}: {donor} -> {name}; {} resources. Numeric clone preservation, edit/reopen/undo PASS.\n  {change}\n",doc.archive.entries.len()));
        for note in package.notes {
            report.push_str(&format!("  Dependency note: {note}\n"));
        }
        if let Some(i) = doc.archive.find(&format!("{id}.SH")) {
            match Model::parse(&doc.archive.entries[i].read()?) {
                Ok(m) => report.push_str(&format!(
                    "  Preview: {} vertices / {} faces / {} C4 parts.\n",
                    m.vertices.len(),
                    m.faces.len(),
                    m.parts.len()
                )),
                Err(e) => report.push_str(&format!("  Preview limitation: {e}\n")),
            }
        }
        results.push_str(&format!("{folder}\tPENDING\tPENDING\tPENDING\tPENDING\t\n"));
    }
    for (stem, id) in [("F18", "HG18"), ("A10", "HG10")] {
        let dir = out.join(format!("texture-{stem}"));
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        for sub in ["baseline", "generated-panel", "part-placement"] {
            fs::create_dir(dir.join(sub)).map_err(|e| e.to_string())?;
        }
        let package = clone_aircraft::build(
            &[&sources[0], &sources[1]],
            &format!("{stem}.PT"),
            id,
            &format!("Hangar {stem} texture test"),
        )?;
        let name = format!("{id}.SH");
        let bytes = read(&package.archive, &name)?;
        let model = Model::parse(&bytes)?;
        let palette = picture::palette(&read(&package.archive, &format!("{id}.PAL"))?)?;
        let (face, mut panel) = model
            .faces
            .iter()
            .enumerate()
            .find_map(|(i, f)| {
                (f.sub & 4 == 0)
                    .then(|| {
                        shape_edit::texture_panel(&bytes, i, "HGPNL.PIC", 64, &palette)
                            .ok()
                            .map(|p| (i, p))
                    })
                    .flatten()
            })
            .ok_or("No convertible panel")?;
        let nearest = |rgb: [i32; 3]| {
            (0..255)
                .min_by_key(|i| {
                    (0..3)
                        .map(|k| (palette[*i][k] as i32 - rgb[k]).pow(2))
                        .sum::<i32>()
                })
                .unwrap() as u8
        };
        let colors = [
            nearest([255, 0, 255]),
            nearest([255, 255, 0]),
            nearest([0, 0, 0]),
        ];
        let mut pic = Pic::parse(&panel.picture)?;
        let pixels = (0..4096)
            .map(|i| {
                let (x, y) = (i % 64, i / 64);
                if ((8..14).contains(&x) && (8..56).contains(&y))
                    || ((14..46).contains(&x) && (8..14).contains(&y))
                    || ((14..36).contains(&x) && (28..34).contains(&y))
                {
                    colors[2]
                } else {
                    colors[((x / 8) + (y / 8)) % 2]
                }
            })
            .collect::<Vec<_>>();
        pic.patch_indices(&mut panel.picture, &pixels)?;
        write(&dir.join("PANEL-PREVIEW.png"), &pic.png(&palette))?;
        let mut doc = Document::new(package.archive);
        save(&dir.join("baseline"), &format!("{id}.LIB"), &doc.archive)?;
        let at = doc.archive.find(&name).unwrap();
        doc.transaction(
            vec![
                Entry::new(&name, panel.shape)?,
                Entry::new("HGPNL.PIC", panel.picture)?,
            ],
            &[],
        )?;
        save(
            &dir.join("generated-panel"),
            &format!("{id}.LIB"),
            &doc.archive,
        )?;
        if !doc.undo()
            || doc.archive.entries[at].read()? != bytes
            || doc.archive.find("HGPNL.PIC").is_some()
        {
            return Err("Panel undo failed".into());
        }
        let mut part_state = BTreeMap::new();
        let mut part_model = model.clone();
        if part_model.parts.is_empty() {
            for address in &model.state_words {
                let candidate = BTreeMap::from([(*address, 1)]);
                if let Ok(m) = Model::with_state(&bytes, &candidate) {
                    if !m.parts.is_empty() {
                        part_state = candidate;
                        part_model = m;
                        break;
                    }
                }
            }
        }
        let part_note = if let Some(part) = part_model.parts.first() {
            let modified =
                animation::place_part(&bytes, &part_state, part.offset, 0, part.position[0] + 2)?;
            doc.replace(at, modified)?;
            save(
                &dir.join("part-placement"),
                &format!("{id}.LIB"),
                &doc.archive,
            )?;
            doc.undo();
            format!(
                "Part at file {:X}, X {} -> {}. Check gear and all related animation states.",
                part.offset,
                part.position[0],
                part.position[0] + 2
            )
        } else {
            "No neutral C4 part; no part-placement candidate.".into()
        };
        report.push_str(&format!("texture-{stem}: panel {} (source FC file {:X}), asymmetric F/checker indices {:?}. Geometry/reopen/one-step undo PASS. CODE continuation retains original RVAs. Native-game texture/state/LOD/damage acceptance is PENDING. Panel records precede the SH terminator and import stubs are relocated; original-game acceptance still requires this test.\n  {part_note}\n",face+1,model.faces[face].offset,colors));
        report.push_str(&format!("  Part-edit preview state: {part_state:?}\n"));
        let symbols = animation::symbols(&bytes)?;
        for addr in model.state_words {
            report.push_str(&format!(
                "  Preview state {addr:08X}: {}\n",
                symbols
                    .get(&addr)
                    .map(String::as_str)
                    .unwrap_or("(unnamed)")
            ));
        }
        results.push_str(&format!(
            "texture-{stem}\tPENDING\tPENDING\tPENDING\tPENDING\t\n"
        ));
    }
    report.push_str("\nWINDOWS CHECK ORDER\n1. Open each baseline in Hangar; inspect definition, model, dependencies and stations. Then try it in FA with the other candidate LIBs disabled.\n2. Replace baseline with its edited counterpart. Select/place the same asset in a mission or loadout; test spawn, model/damage, weapon firing, reload and turret arcs. Confirm unchanged assets still behave normally.\n3. For textures, use baseline then generated-panel on the named HG18/HG10 aircraft. Locate the bright F/checker panel from several angles; test gear, flaps, hook, LOD distance and damage. Compare palette/transparency and watch for missing neighboring panels. Do not combine HG18 and HG10 checker packages (both use HGPNL.PIC).\n4. Test part-placement separately from generated-panel. Confirm the moved C4 part still switches/animates, and inspect clipping/visibility/collision. Native angle arithmetic has not been rewritten.\n5. Record exact game edition, load order, result, failure steps and screenshots in RESULTS.tsv. Baseline failures and edited-only failures are different findings. Remove candidate LIBs after testing.\n");
    write(&out.join("READ-ME.txt"), report.as_bytes())?;
    write(&out.join("RESULTS.tsv"), results.as_bytes())?;
    println!("Exported acceptance candidates to {}", out.display());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
