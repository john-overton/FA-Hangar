use crate::{
    platform,
    ui::{App, FileAction, Key},
};
#[path = "cli_shape.rs"]
mod shape;
use hangar_core::{archive::Archive, brf::Brf, document::Document, model::Model, Result};
fn argument(args: &[String], n: usize) -> Result<&str> {
    args.get(n)
        .map(String::as_str)
        .ok_or_else(|| "Missing argument; run --help".into())
}
/// Source LIBs are read like the GUI wizard reads them: directory only, then
/// bounded range reads of the payloads the export needs, so no LIB is loaded whole.
fn export_object(args: &[String]) -> Result<()> {
    use hangar_core::{
        archive::{decode_payload, IndexedEntry},
        clone_aircraft::{build_with, Policy, Resolution},
        dependencies::Evidence,
    };
    let mut policy = Policy::default();
    let mut positional = Vec::new();
    let (mut short, mut long) = (None, None);
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--keep-unresolved" => policy.keep_unresolved = true,
            "--short" => short = Some(rest.next().ok_or("--short needs NAME")?.as_str()),
            "--long" => long = Some(rest.next().ok_or("--long needs NAME")?.as_str()),
            "--substitute" => {
                let value = rest.next().ok_or("--substitute needs OLD.PIC=NEW.PIC")?;
                let (old, new) = value
                    .split_once('=')
                    .ok_or("--substitute needs OLD.PIC=NEW.PIC")?;
                let key = (String::new(), old.trim().to_ascii_uppercase());
                if policy
                    .substitutes
                    .insert(key, new.trim().to_ascii_uppercase())
                    .is_some()
                {
                    return Err(format!("{old} has more than one --substitute"));
                }
            }
            _ => positional.push(arg.as_str()),
        }
    }
    if positional.len() < 5 {
        return Err("Missing argument; run --help".into());
    }
    let mut libraries: Vec<(&str, Vec<IndexedEntry>)> = Vec::new();
    let mut catalog = std::collections::BTreeSet::new();
    let mut providers = std::collections::BTreeMap::new();
    for path in std::iter::once(positional[0]).chain(positional[5..].iter().copied()) {
        if libraries.len() >= 64 {
            return Err("More than 64 source LIBs".into());
        }
        let entries = crate::ui::library_index(path)?
            .ok_or_else(|| format!("{path}: not an EALIB archive"))?;
        for (i, e) in entries.iter().enumerate() {
            catalog.insert(e.name.clone());
            // Earlier LIBs win, as in the input-first source order.
            providers
                .entry(e.name.clone())
                .or_insert((libraries.len(), i));
        }
        libraries.push((path, entries));
    }
    let package = build_with(
        &catalog,
        positional[1],
        positional[2],
        // TITLE fills both names unless --short or --long replaces one.
        hangar_core::clone_aircraft::Names {
            short: short.unwrap_or(positional[3]),
            long: long.unwrap_or(positional[3]),
        },
        &policy,
        |name| {
            let (l, i) = providers
                .get(name)
                .ok_or_else(|| format!("Missing referenced resource {name}"))?;
            let (path, entries) = &libraries[*l];
            let e = &entries[*i];
            decode_payload(e.flag, platform::read_range(path, e.offset, e.size)?)
        },
    )
    .map_err(|e| {
        if !e.starts_with("Unresolved in source") {
            return e;
        }
        let mut e = if e.contains("substitute a packaged texture") {
            format!("{e}. Use --keep-unresolved or --substitute OLD.PIC=NEW.PIC")
        } else {
            format!("{e}. Use --keep-unresolved")
        };
        if positional.len() == 5 {
            e.push_str("; no source LIBs were given, so list the LIBs that provide shared resources after OUTPUT.LIB");
        }
        e
    })?;
    save_lib(positional[4], &package.archive.bytes()?)?;
    for (old, new) in &package.mapping {
        println!("{old:13} -> {new}");
    }
    for u in &package.unresolved {
        let drawn = if u.drawn() == Some(false) {
            "; not drawn by any pose"
        } else {
            ""
        };
        let label = u.evidence.label();
        match &u.resolution {
            Resolution::Keep if u.evidence == Evidence::Convention => println!(
                "{:13} absent, as in source ({}, {label})",
                u.target, u.resource
            ),
            Resolution::Keep => println!(
                "{:13} kept unresolved, as in source ({}, {label}{drawn})",
                u.target, u.resource
            ),
            Resolution::Substitute(new) => println!(
                "{:13} -> {new} in {} ({label}{drawn})",
                u.target, u.resource
            ),
        }
    }
    println!(
        "{} private resources; {} bytes",
        package.archive.entries.len(),
        package.archive.bytes()?.len()
    );
    Ok(())
}
/// FA's loader limits when `path` is in a game folder: a LIB write that
/// would break them is refused with the numbers (the GUI asks instead).
fn folder_guard(path: &str, bytes: &[u8]) -> Result<Option<hangar_core::save::GameFolder>> {
    let entries = Archive::parse(bytes.to_vec())?.entries.len();
    let folder = crate::saving::game_folder(path, entries);
    if let Some(f) = &folder {
        let problems = f.problems();
        if !problems.is_empty() {
            return Err(format!(
                "Refused: saving here would break FA's loader limits. {} {}",
                problems.join(" "),
                f.summary()
            ));
        }
    }
    Ok(folder)
}
fn folder_report(folder: Option<hangar_core::save::GameFolder>) {
    if let Some(f) = folder {
        println!("{}", f.summary());
        for w in f.warnings() {
            println!("WARN {w}");
        }
    }
}
/// `saving::library` behind `folder_guard`, printing the folder summary.
fn save_lib(path: &str, bytes: &[u8]) -> Result<Option<String>> {
    let folder = folder_guard(path, bytes)?;
    let backup = crate::saving::library(path, bytes)?;
    folder_report(folder);
    Ok(backup)
}
/// PIC header fields that decide whether FA's texture mapper can read it.
fn texture_header(bytes: &[u8]) -> String {
    let field = |at: usize| {
        bytes
            .get(at..at + 4)
            .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
    };
    let kind = bytes
        .get(..2)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
    format!(
        "kind {kind}, {} x {}, row table {}, palette {} B, span size {}",
        field(2),
        field(6),
        if field(38) == 0 {
            "none".into()
        } else {
            format!("@{} ({} B)", field(34), field(38))
        },
        field(22),
        field(30)
    )
}
/// `repair-textures INPUT.LIB NEW_OUTPUT.LIB [PALETTE.PAL|PALETTE_SOURCE.LIB]`:
/// every PIC a textured SH face draws that is not a retail texture is
/// rewritten in that layout; no SH changes. The game palette comes from the
/// input's PALETTE.PAL, else the optional third argument.
fn repair_textures(args: &[String]) -> Result<()> {
    use hangar_core::{dependencies::Index, picture, picture::PaletteCheck, validation};
    let input = argument(args, 1)?;
    let output = argument(args, 2)?;
    hangar_core::save::validate_destination(output)?;
    let mut doc = Document::new(Archive::parse(platform::read(input)?)?);
    let pal_of = |archive: &Archive| -> Option<Result<[[u8; 3]; 256]>> {
        let at = archive.find("PALETTE.PAL")?;
        Some(
            archive.entries[at]
                .read()
                .and_then(|b| picture::palette(&b)),
        )
    };
    let (game, source) = match pal_of(&doc.archive) {
        Some(p) => (Some(p?), format!("{input} PALETTE.PAL")),
        None => match args.get(3) {
            Some(path) => {
                let bytes = platform::read(path)?;
                let p = if bytes.starts_with(b"EALIB") {
                    pal_of(&Archive::parse(bytes)?)
                        .ok_or_else(|| format!("{path} has no PALETTE.PAL"))??
                } else {
                    picture::palette(&bytes)?
                };
                (Some(p), path.clone())
            }
            None => (None, "none".into()),
        },
    };
    println!("Game palette: {source}");
    let shapes: Vec<(String, Vec<u8>)> = doc
        .archive
        .entries
        .iter()
        .filter(|e| e.name.ends_with(".SH"))
        .map(|e| Ok((e.name.clone(), e.read()?)))
        .collect::<Result<_>>()?;
    let before: Vec<(String, String)> = {
        let mut index = Index::default();
        index.update(&doc.archive);
        validation::texture_layout_problems(&doc.archive, &index)
            .into_iter()
            .map(|p| (p.pic, p.reason))
            .collect()
    };
    if before.is_empty() {
        println!("Every texture a textured SH face draws is already in the retail layout; nothing written");
        return Ok(());
    }
    let mut index = Index::default();
    let repair = validation::repair_textures(&doc.archive, &mut index, game.as_ref())?;
    let read = |doc: &Document, name: &str| -> Result<Vec<u8>> {
        doc.archive.entries[doc.archive.find(name).ok_or("Entry missing")?].read()
    };
    let old: Vec<(String, Vec<u8>)> = repair
        .entries
        .iter()
        .map(|e| Ok((e.name.clone(), read(&doc, &e.name)?)))
        .collect::<Result<_>>()?;
    doc.transaction(repair.entries, &[])?;
    for (name, bytes) in &old {
        let new = read(&doc, name)?;
        println!("{name}");
        println!("  before: {}", texture_header(bytes));
        println!("  after:  {}", texture_header(&new));
    }
    for (name, check) in &repair.repaired {
        let note = match check {
            PaletteCheck::None => "no embedded palette".to_string(),
            PaletteCheck::Same => "embedded palette equals the game palette; indices kept".into(),
            PaletteCheck::Remapped(n) => format!(
                "FLAG: embedded palette differs from the game palette; {n} used indices mapped to the nearest game color"
            ),
            PaletteCheck::Unverified => {
                "FLAG: no game palette given; indices kept unverified".into()
            }
        };
        println!("{name}: {note}");
    }
    for (name, why) in &repair.refused {
        println!("{name}: not repaired: {why}");
    }
    for (name, bytes) in &shapes {
        if read(&doc, name)? != *bytes {
            return Err(format!("{name} changed; repair stopped"));
        }
    }
    let mut index = Index::default();
    let left = hangar_core::validation::inspect(&doc, &mut index);
    let remaining = left
        .checks
        .iter()
        .filter(|c| c.message.starts_with(validation::TEXTURE_LAYOUT_ERROR))
        .count();
    let bytes = doc.archive.bytes()?;
    let folder = folder_guard(output, &bytes)?;
    platform::write_new(output, &bytes)?;
    folder_report(folder);
    println!(
        "Repaired {} of {} textures ({} stored originals); {} SH entries unchanged; {remaining} texture-layout errors remain; wrote {output}",
        repair.repaired.len(),
        before.len(),
        repair.originals.len(),
        shapes.len()
    );
    Ok(())
}
pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = App::new();
    match args.first().map(String::as_str) {
        Some("--help")=>println!("TORE Hangar\n  tore-hangar [FILE.LIB]\n  --demo\n  --snapshot OUTPUT.svg [FILE.LIB [ENTRY]]\n  --smoke-test\n  demo-lib OUTPUT.LIB\n  export-object INPUT.LIB ENTRY ID \"TITLE\" OUTPUT.LIB [SOURCE_LIBS...] [--keep-unresolved] [--substitute OLD.PIC=NEW.PIC]... [--short NAME] [--long NAME]\n  clone-aircraft INPUT.LIB DONOR.PT ID \"TITLE\" OUTPUT.LIB [SOURCE_LIBS...] [--keep-unresolved] [--substitute OLD.PIC=NEW.PIC]...\n  variant INPUT.LIB DONOR.PT INPUT.SH ID \"TITLE\" OUTPUT.LIB [TEXTURES...]\n  list INPUT.LIB\n  inspect INPUT.LIB ENTRY\n  references INPUT.LIB ENTRY\n  validate INPUT.LIB\n  extract INPUT.LIB ENTRY OUTPUT\n  repack INPUT.LIB OUTPUT.LIB\n  repair-panels INPUT.LIB ENTRY.SH NEW_OUTPUT.LIB\n  repair-textures INPUT.LIB NEW_OUTPUT.LIB [PALETTE.PAL|PALETTE_SOURCE.LIB]\n  --texture-repair-check INPUT.LIB ENTRY.SH [PALETTE.PAL|PALETTE_SOURCE.LIB]\n  --shape-inventory INPUT.LIB [ENTRY.SH]\n  --shape-pose INPUT.LIB ENTRY.SH [NAME=VALUE...]\n  --stub-census NEW_OUTPUT.txt INPUT.LIB...\n  --geometry-check OUTPUT_DIR INPUT.LIB [ENTRY.SH...]\n  --proof-census INPUT.LIB NEW_OUTPUT.txt [BASELINE.txt]\n  --edit-check INPUT.LIB ENTRY.SH...\n  --face-texture-check INPUT.LIB NEW_OUTPUT.LIB ENTRY.SH...\n  --replace-check INPUT.LIB ENTRY.SH NEW_OUTPUT_DIR\n  --remap-check INPUT.LIB NEW_OUTPUT_DIR ENTRY.SH...\n  --identity-check INPUT.LIB RENAME.PT NEW_ID DUPLICATE.PT DUPLICATE_ID NEW_OUTPUT.LIB\n  --palette-check INPUT.LIB NEW_OUTPUT.LIB COPY.PT [ENTRY...]\n  replace INPUT.LIB ENTRY RESOURCE OUTPUT.LIB\n  set INPUT.LIB ENTRY FIELD_INDEX VALUE OUTPUT.LIB\nRetail LIB names are protected. Custom LIB saves keep numbered <STEM>.BAK/.B01-.B99 backups. Resource exports require new files. Windows launches the native GUI."),
        Some("--smoke-test")=>{
            crate::ui::NO_PALETTE_MEMORY.store(true, core::sync::atomic::Ordering::Relaxed);
            app.demo();let before=app.doc.archive.bytes()?;
            app.key(Key::Char('g'),false,false);app.key(Key::Char('1'),false,false);app.key(Key::Char('q'),false,false);app.key(Key::Enter,false,false);
            assert!(!app.doc.dirty());assert_eq!(app.doc.archive.bytes()?,before);app.key(Key::Escape,false,false);
            app.key(Key::Char('g'),false,false);app.key(Key::Char('1'),false,false);app.key(Key::Char('0'),false,false);app.key(Key::Enter,false,false);assert!(app.doc.dirty());assert_ne!(app.doc.archive.bytes()?,before);app.key(Key::Char('z'),true,false);assert!(!app.doc.dirty());assert_eq!(app.doc.archive.bytes()?,before);
            app.select_entry(1);assert_eq!(app.selected,1);app.key(Key::Enter,false,false);app.key(Key::Char('a'),true,false);app.key(Key::Char('6'),false,false);app.key(Key::Enter,false,false);assert!(app.doc.dirty());assert!(!app.draw().commands.is_empty());
            app.key(Key::Char('z'),true,false);
            let temp=std::env::temp_dir().join(format!("hangar-smoke-{}",std::process::id()));
            std::fs::create_dir(&temp).map_err(|e|e.to_string())?;
            let shape=temp.join("MODEL.SH");let output=temp.join("NEW.LIB");
            platform::write_new(shape.to_str().unwrap(), &hangar_core::model::demo_shape())?;
            app.file_prompt(FileAction::VariantSh);
            for value in [shape.to_str().unwrap(),"NEWJET","New test aircraft","CREATE"] {
                app.key(Key::Char('a'),true,false);
                for c in value.chars(){app.key(Key::Char(c),false,false);}app.key(Key::Enter,false,false);
            }
            assert!(app.doc.archive.find("NEWJET.PT").is_some(),"{}",app.status);
            assert!(app.doc.dirty());
            app.file_prompt(FileAction::Save);app.key(Key::Char('a'),true,false);
            for c in temp.to_str().unwrap().chars(){app.key(Key::Char(c),false,false);}app.key(Key::Enter,false,false);app.key(Key::Char('a'),true,false);
            for c in "NEW.LIB".chars(){app.key(Key::Char(c),false,false);}app.key(Key::Enter,false,false);
            assert!(!app.doc.dirty(),"{}",app.status);
            app=App::new();app.open(output.to_str().unwrap())?;assert_eq!(app.doc.archive.entries.len(),7);
            std::fs::remove_dir_all(temp).map_err(|e|e.to_string())?;
            app.smoke_layout();
            app.smoke_media();
            app.smoke_clone();
            app.smoke_palette();
            app.smoke_texture_repair();
            crate::saving::smoke();
            app.smoke_save_policy();
            app.smoke_game_folder();
            println!("App size: {} bytes",core::mem::size_of::<App>());
            println!("PASS: shared UI selection, transform, undo, BRF edit, draw commands, donor wizard, packaging, reopening");
        },
        Some("--stub-census")=>{let report=shape::census(&args[2..])?;platform::write_new(argument(&args,1)?,report.as_bytes())?;println!("Wrote {}",argument(&args,1)?);},
        Some("--shape-pose")=>print!("{}",shape::pose(argument(&args,1)?,argument(&args,2)?,&args[3..])?),
        Some("--proof-census")=>{let (summary,lines)=shape::proof_census(argument(&args,1)?,args.get(3).map(String::as_str))?;platform::write_new(argument(&args,2)?,lines.as_bytes())?;print!("{summary}");},
        Some("--geometry-check")=>print!("{}",shape::geometry_check(argument(&args,1)?,argument(&args,2)?,&args[3..])?),
        Some("--shape-inventory")=>print!("{}",shape::inventory(argument(&args,1)?,args.get(2).map(String::as_str))?),
        Some("--texture-repair-check")=>{app.open(argument(&args,1)?)?;print!("{}",app.check_texture_repair(argument(&args,2)?,args.get(3).map(String::as_str))?);},
        Some("--repair-check")=>{
            app.open(argument(&args,1)?)?;
            let at=app.doc.archive.find(argument(&args,2)?).ok_or("SH not found")?;
            app.select_entry(at);
            app.check_panel_repair()?;
            println!("PASS: repair preserves rendered pixels; one undo restores the exact archive; redo restores the repaired SH");
        },
        Some("repair-textures")=>repair_textures(&args)?,
        Some("repair-panels")=>{
            let mut doc=Document::new(Archive::parse(platform::read(argument(&args,1)?)?)?);
            let name=argument(&args,2)?;
            let at=doc.archive.find(name).ok_or("SH not found")?;
            let before=doc.archive.entries[at].read()?;
            let repaired=hangar_core::shape_edit::repair_panel_layout(&before)?.ok_or("No legacy generated panel layout found")?;
            let count=repaired.panels;
            doc.replace(at,repaired.shape)?;
            let output=argument(&args,3)?;
            if hangar_core::save::protected_name(output).is_some(){return Err("Choose a new custom LIB name".into());}
            platform::write_new(output,&doc.archive.bytes()?)?;
            println!("Repaired {count} generated panels in {name}; all other entries unchanged; wrote {output}");
        },
        Some("--panel-check")=>{
            let bytes=platform::read(argument(&args,1)?)?;let model=Model::parse(&bytes)?;
            let summary=model.faces.iter().filter(|f|f.sub&4==0).count();let known=model.faces.iter().filter(|f|f.sub&4==0&&!f.material_selector.is_empty()).count();
            println!("{} vertices, {} faces, {summary} flat faces, {known} known material states, {} decoded records",model.vertices.len(),model.faces.len(),model.records.len());
            let mut types=std::collections::BTreeMap::new();for f in &model.faces{*types.entry(f.sub).or_insert(0)+=1;}println!("Face subtypes: {types:?}");
            let face:usize=if argument(&args,2)?=="auto"{model.faces.iter().position(|f|f.sub&4==0&&f.sub&!0x67==0&&(!f.material_selector.is_empty()||model.writable)).ok_or("No supported panel")?}else{argument(&args,2)?.parse().map_err(|_|"Face index")?};
            let result=hangar_core::shape_edit::texture_panel(&bytes,face,argument(&args,3)?,64)?;
            platform::write_new(argument(&args,4)?,&result.shape)?;platform::write_new(argument(&args,5)?,&result.picture)?;
            println!("Generated panel {}. Geometry retained; texture mapped.",result.face);
        },
        Some("--decal-check")=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("PIC not found")?;app.select_entry(at);println!("{}",app.check_decal_import(argument(&args,3)?,argument(&args,4)?)?);},
        Some("--paint-check")=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("Shape not found")?;app.select_entry(at);println!("{}",app.check_real_paint()?);},
        Some("--edit-check")=>{app.open(argument(&args,1)?)?;for name in &args[2..]{let at=app.doc.archive.find(name).ok_or("SH not found")?;app.select_entry(at);print!("{name}:\n{}",app.check_edit()?);}},
        Some("--face-texture-check")=>{let output=argument(&args,2)?;if hangar_core::save::protected_name(output).is_some(){return Err("Choose a new custom LIB name".into());}app.open(argument(&args,1)?)?;print!("{}",app.check_face_textures(&args[3..],output)?);},
        Some("--remap-check")=>{app.open(argument(&args,1)?)?;print!("{}",app.check_remap(argument(&args,2)?,&args[3..])?);println!("PASS stretch census, Remap from view with Bake, retail texture layout, coverage, bindings, Use shape texture and undo");},
        Some("--replace-check")=>{app.open(argument(&args,1)?)?;print!("{}",app.check_real_replace(argument(&args,2)?,argument(&args,3)?)?);println!("PASS whole-texture and tail-panel Replace color, raster bytes only, stored original, undo, redo and Restore texture");},
        Some("--restore-check")=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("Shape not found")?;app.select_entry(at);println!("{}",app.check_real_restore()?);},
        Some(cmd @ ("export-png"|"export-wav"))=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("Entry not found")?;app.select_entry(at);app.file_prompt(if cmd=="export-png"{FileAction::Png}else{FileAction::Wav});app.key(Key::Char('a'),true,false);for c in argument(&args,3)?.chars(){app.key(Key::Char(c),false,false);}app.key(Key::Enter,false,false);if app.status.starts_with("Error:"){return Err(app.status);}println!("{}",app.status);},
        Some("--native-snapshot") => {
            if let Some(path)=args.get(2).filter(|p|p.as_str()!="-") { app.open(path)?; if let Some(name)=args.get(3) { let at=app.doc.archive.find(name).ok_or("Entry not found")?;app.select_entry(at); } } else { app.demo(); }
            if let Some(workspace)=args.get(4){app.workspace(workspace)?;}
            if let Some(size)=args.get(5){if let Some((w,h))=size.split_once('x'){app.width=w.parse().map_err(|_|"Invalid width")?;app.height=h.parse().map_err(|_|"Invalid height")?;}}
            platform::capture(app,argument(&args,1)?)?;
        },
        Some("--demo")=>{app.demo();platform::run(app)?;},
        Some("demo-lib")=>{app.demo();save_lib(argument(&args,1)?,&app.doc.archive.bytes()?)?;},
        Some("--snapshot")=>{if let Some(path)=args.get(2).filter(|p|p.as_str()!="-"){app.open(path)?;if let Some(name)=args.get(3){let at=app.doc.archive.find(name).ok_or("Entry not found")?;app.select_entry(at);}}else{app.demo();}
            if let Some(workspace)=args.get(4){app.workspace(workspace)?;}
            if let Some(size)=args.get(5){if let Some((w,h))=size.split_once('x'){app.width=w.parse().map_err(|_|"Invalid width")?;app.height=h.parse().map_err(|_|"Invalid height")?;}}
            let s=app.draw().svg(app.width,app.height);platform::write_new(argument(&args,1)?,s.as_bytes())?;
        },
        Some("--palette-check")=>{app.open(argument(&args,1)?)?;print!("{}",app.palette_check(argument(&args,2)?,argument(&args,3)?,&args[4..])?);},
        Some("--identity-check")=>{app.open(argument(&args,1)?)?;print!("{}",app.identity_check(argument(&args,2)?,argument(&args,3)?,argument(&args,4)?,argument(&args,5)?,argument(&args,6)?)?);println!("PASS rename and same-LIB duplicate through the review, undo byte identity, create-new save");},
        Some("--clone-check")=>{app.open(argument(&args,1)?)?;app.clone_export_check(argument(&args,2)?,argument(&args,3)?,argument(&args,4)?,argument(&args,5)?)?;println!("PASS selected-object wizard, automatic sources, review, named LIB export and reopen");},
        Some("clone-aircraft"|"export-object") => export_object(&args)?,
        Some("variant") => {
            let source=Archive::parse(platform::read(argument(&args,1)?)?)?;
            let mut variant=hangar_core::authoring::create(&source,argument(&args,2)?,platform::read(argument(&args,3)?)?,argument(&args,4)?,argument(&args,5)?)?;
            for path in args.iter().skip(7) {
                let name=path.rsplit(['/', '\\']).next().unwrap();
                if variant.archive.find(name).is_some(){return Err(format!("Resource name already present: {name}"));}
                variant.archive.entries.push(hangar_core::archive::Entry::new(name,platform::read(path)?)?);
            }
            for name in &variant.missing_textures {if variant.archive.find(name).is_none(){return Err(format!("Missing imported-shape texture {name}; supply its path after OUTPUT.LIB"));}}
            save_lib(argument(&args,6)?,&variant.archive.bytes()?)?;
            println!("Created {} entries from {}. Donor damage/shadow retained. Shared stock references: {}. Game test still required.",variant.archive.entries.len(),variant.donor,variant.shared.join(", "));
        },
        Some("references") => {
            let archive = Archive::parse(platform::read(argument(&args, 1)?)?)?;
            let at = archive.find(argument(&args, 2)?).ok_or("Entry not found")?;
            let name = &archive.entries[at].name;
            let mut index = hangar_core::dependencies::Index::default();
            index.update(&archive);
            println!("{name}: observed stored names in the current LIB only.");
            if let Some(scan) = index.get(name) {
                for link in &scan.links { println!("  -> {} [{}; {}]", link.target, link.evidence, if archive.find(&link.target).is_some() { "local" } else { "not in this LIB" }); }
                if let Some(error) = &scan.unavailable { println!("  Unverified: {error}"); }
                for note in &scan.notes { println!("  {note}"); }
            }
            for source in index.incoming(name) { println!("  <- {source}"); }
            for source in index.aircraft_users(name) { println!("  Aircraft user: {source}"); }
            println!("{} resources have unavailable dependency scans; implicit families and runtime lookups are unverified", index.unavailable_count());
        },
        Some("validate") => {
            let doc = Document::new(Archive::parse(platform::read(argument(&args, 1)?)?)?);
            let report = hangar_core::validation::inspect(&doc, &mut Default::default());
            println!("{}", report.summary());
            for check in &report.checks { println!("{} {}: {}", check.level.label(), check.entry.as_deref().unwrap_or("Package"), check.message); }
            if report.omitted > 0 { println!("{} further results omitted", report.omitted); }
            if report.errors > 0 { return Err("Package validation failed".into()); }
        },
        Some(cmd @ ("list"|"inspect"|"extract"|"repack"|"replace"|"set"))=>{
            let a=Archive::parse(platform::read(argument(&args,1)?)?)?;
            if cmd=="list"{for (i,e) in a.entries.iter().enumerate(){println!("{i:5} {:13} {:9} bytes  flag {}",e.name,e.stored_len(),e.flag());}}
            else if cmd=="repack"{save_lib(argument(&args,2)?,&a.bytes()?)?;}
            else{let at=a.find(argument(&args,2)?).ok_or("Entry not found")?;let bytes=a.entries[at].read()?;let ext=a.entries[at].name.rsplit('.').next().unwrap();
                match cmd{
                    "inspect"=>{if ext=="SH"{let m=Model::parse(&bytes)?;println!("{} vertices, {} faces, writable={} {}",m.vertices.len(),m.faces.len(),m.writable,m.reason);}else{let b=Brf::parse(&bytes,ext)?;for (i,f) in b.fields.iter().enumerate(){println!("{i:4} {:36} {:7} {}{}",f.label,f.kind,if f.scaled{"^"}else{""},f.value);}}},
                    "extract"=>platform::write_new(argument(&args,3)?,&bytes)?,
                    "replace"=>{let mut d=Document::new(a);let name=d.archive.entries[at].name.clone();let new=platform::read(argument(&args,3)?)?;if new!=bytes{let entries=hangar_core::originals::with_originals(&d,vec![hangar_core::archive::Entry::new(&name,new)?],&[]);if let Some(org)=entries.get(1){println!("Original {name} kept as {}",org.name);}d.transaction(entries,&[])?;}save_lib(argument(&args,4)?,&d.archive.bytes()?)?;},
                    "set"=>{let b=Brf::parse(&bytes,ext)?;let index=argument(&args,3)?.parse().map_err(|_|"Invalid field index")?;let new=b.edit(&bytes,index,argument(&args,4)?,ext)?;let mut d=Document::new(a);d.replace(at,new)?;save_lib(argument(&args,5)?,&d.archive.bytes()?)?;},_=>{}
                }
            }
        },
        Some(path)=>{app.open(path)?;platform::run(app)?;},None=>platform::run(app)?,
    }
    Ok(())
}
