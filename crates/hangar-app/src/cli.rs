use crate::{
    platform,
    ui::{App, Draw, FileAction, Key},
};
use hangar_core::{archive::Archive, brf::Brf, document::Document, model::Model, Result};
fn argument(args: &[String], n: usize) -> Result<&str> {
    args.get(n)
        .map(String::as_str)
        .ok_or_else(|| "Missing argument; run --help".into())
}
pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut app = App::new();
    match args.first().map(String::as_str) {
        Some("--help")=>println!("TORE Hangar\n  tore-hangar [FILE.LIB]\n  --demo\n  --snapshot OUTPUT.svg [FILE.LIB [ENTRY]]\n  --smoke-test\n  demo-lib OUTPUT.LIB\n  clone-aircraft INPUT.LIB DONOR.PT ID \"TITLE\" OUTPUT.LIB [SOURCE_LIBS...]\n  variant INPUT.LIB DONOR.PT INPUT.SH ID \"TITLE\" OUTPUT.LIB [TEXTURES...]\n  list INPUT.LIB\n  inspect INPUT.LIB ENTRY\n  extract INPUT.LIB ENTRY OUTPUT\n  repack INPUT.LIB OUTPUT.LIB\n  replace INPUT.LIB ENTRY RESOURCE OUTPUT.LIB\n  set INPUT.LIB ENTRY FIELD_INDEX VALUE OUTPUT.LIB\nRetail LIB names are protected. Custom LIB saves keep numbered .bak backups. Resource exports require new files. Windows launches the native GUI."),
        Some("--smoke-test")=>{
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
            app.open(output.to_str().unwrap())?;assert_eq!(app.doc.archive.entries.len(),7);
            std::fs::remove_dir_all(temp).map_err(|e|e.to_string())?;
            app.smoke_layout();
            app.smoke_media();
            app.smoke_clone();
            crate::saving::smoke();
            app.smoke_save_policy();
            println!("PASS: shared UI selection, transform, undo, BRF edit, draw commands, donor wizard, packaging, reopening");
        },
        Some("--paint-check")=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("Shape not found")?;app.select_entry(at);println!("{}",app.check_real_paint()?);},
        Some(cmd @ ("export-png"|"export-wav"))=>{app.open(argument(&args,1)?)?;let at=app.doc.archive.find(argument(&args,2)?).ok_or("Entry not found")?;app.select_entry(at);app.file_prompt(if cmd=="export-png"{FileAction::Png}else{FileAction::Wav});app.key(Key::Char('a'),true,false);for c in argument(&args,3)?.chars(){app.key(Key::Char(c),false,false);}app.key(Key::Enter,false,false);if app.status.starts_with("Error:"){return Err(app.status);}println!("{}",app.status);},
        Some("--native-snapshot") => {
            if let Some(path)=args.get(2).filter(|p|p.as_str()!="-") { app.open(path)?; if let Some(name)=args.get(3) { let at=app.doc.archive.find(name).ok_or("Entry not found")?;app.select_entry(at); } } else { app.demo(); }
            if let Some(workspace)=args.get(4){app.workspace(workspace)?;}
            if let Some(size)=args.get(5){if let Some((w,h))=size.split_once('x'){app.width=w.parse().map_err(|_|"Invalid width")?;app.height=h.parse().map_err(|_|"Invalid height")?;}}
            platform::capture(app,argument(&args,1)?)?;
        },
        Some("--demo")=>{app.demo();platform::run(app)?;},
        Some("demo-lib")=>{app.demo();crate::saving::library(argument(&args,1)?,&app.doc.archive.bytes()?)?;},
        Some("--snapshot")=>{if let Some(path)=args.get(2).filter(|p|p.as_str()!="-"){app.open(path)?;if let Some(name)=args.get(3){let at=app.doc.archive.find(name).ok_or("Entry not found")?;app.select_entry(at);}}else{app.demo();}
            if let Some(workspace)=args.get(4){app.workspace(workspace)?;}
            if let Some(size)=args.get(5){if let Some((w,h))=size.split_once('x'){app.width=w.parse().map_err(|_|"Invalid width")?;app.height=h.parse().map_err(|_|"Invalid height")?;}}
            let mut s=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\">",app.width,app.height);
            fn escape(s:&str)->String{s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;")}
            for d in app.draw().commands{match d{Draw::Bitmap(x,y,w,h,pixels)=>{for yy in 0..h {let mut xx=0;while xx<w {let color=pixels[yy*w+xx];let mut end=xx+1;while end<w&&pixels[yy*w+end]==color{end+=1;}s.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"1\" fill=\"#{color:06x}\"/>",x+xx as i32,y+yy as i32,end-xx));xx=end;}}},Draw::Rect(x,y,w,h,c)=>s.push_str(&format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"#{c:06x}\"/>")),Draw::Line(x,y,a,b,c)=>s.push_str(&format!("<path d=\"M{x} {y} L{a} {b}\" stroke=\"#{c:06x}\"/>")),Draw::Label(x,y,t,c)=>s.push_str(&format!("<text x=\"{x}\" y=\"{y}\" font-family=\"DejaVu Sans,sans-serif\" font-size=\"12\" fill=\"#{c:06x}\">{}</text>",escape(&t))),Draw::Text(x,y,t,c)=>s.push_str(&format!("<text x=\"{x}\" y=\"{y}\" font-family=\"monospace\" font-size=\"12\" fill=\"#{c:06x}\">{}</text>",escape(&t)))}}s.push_str("</svg>");platform::write_new(argument(&args,1)?,s.as_bytes())?;
        },
        Some("--clone-check")=>{app.open(argument(&args,1)?)?;app.clone_export_check(argument(&args,2)?,argument(&args,3)?,argument(&args,4)?,argument(&args,5)?)?;println!("PASS selected-PT wizard, automatic sources, review, named LIB export and reopen");},
        Some("clone-aircraft") => {
            let mut sources=vec![Archive::parse(platform::read(argument(&args,1)?)?)?];
            for path in args.iter().skip(6){sources.push(Archive::parse(platform::read(path)?)?);}
            let refs:Vec<_>=sources.iter().collect();let package=hangar_core::clone_aircraft::build(&refs,argument(&args,2)?,argument(&args,3)?,argument(&args,4)?)?;
            crate::saving::library(argument(&args,5)?,&package.archive.bytes()?)?;
            for (old,new) in &package.mapping{println!("{old:13} -> {new}");}
            println!("{} private resources; {} bytes",package.archive.entries.len(),package.archive.bytes()?.len());
        },
        Some("variant") => {
            let source=Archive::parse(platform::read(argument(&args,1)?)?)?;
            let mut variant=hangar_core::authoring::create(&source,argument(&args,2)?,platform::read(argument(&args,3)?)?,argument(&args,4)?,argument(&args,5)?)?;
            for path in args.iter().skip(7) {
                let name=path.rsplit(['/', '\\']).next().unwrap();
                if variant.archive.find(name).is_some(){return Err(format!("Resource name already present: {name}"));}
                variant.archive.entries.push(hangar_core::archive::Entry::new(name,platform::read(path)?)?);
            }
            for name in &variant.missing_textures {if variant.archive.find(name).is_none(){return Err(format!("Missing imported-shape texture {name}; supply its path after OUTPUT.LIB"));}}
            crate::saving::library(argument(&args,6)?,&variant.archive.bytes()?)?;
            println!("Created {} entries from {}. Donor damage/shadow retained. Shared stock references: {}. Game test still required.",variant.archive.entries.len(),variant.donor,variant.shared.join(", "));
        },
        Some(cmd @ ("list"|"inspect"|"extract"|"repack"|"replace"|"set"))=>{
            let a=Archive::parse(platform::read(argument(&args,1)?)?)?;
            if cmd=="list"{for (i,e) in a.entries.iter().enumerate(){println!("{i:5} {:13} {:9} bytes  flag {}",e.name,e.stored_len(),e.flag());}}
            else if cmd=="repack"{crate::saving::library(argument(&args,2)?,&a.bytes()?)?;}
            else{let at=a.find(argument(&args,2)?).ok_or("Entry not found")?;let bytes=a.entries[at].read()?;let ext=a.entries[at].name.rsplit('.').next().unwrap();
                match cmd{
                    "inspect"=>{if ext=="SH"{let m=Model::parse(&bytes)?;println!("{} vertices, {} faces, writable={} {}",m.vertices.len(),m.faces.len(),m.writable,m.reason);}else{let b=Brf::parse(&bytes,ext)?;for (i,f) in b.fields.iter().enumerate(){println!("{i:4} {:36} {:7} {}{}",f.label,f.kind,if f.scaled{"^"}else{""},f.value);}}},
                    "extract"=>platform::write_new(argument(&args,3)?,&bytes)?,
                    "replace"=>{let mut d=Document::new(a);d.replace(at,platform::read(argument(&args,3)?)?)?;crate::saving::library(argument(&args,4)?,&d.archive.bytes()?)?;},
                    "set"=>{let b=Brf::parse(&bytes,ext)?;let index=argument(&args,3)?.parse().map_err(|_|"Invalid field index")?;let new=b.edit(&bytes,index,argument(&args,4)?,ext)?;let mut d=Document::new(a);d.replace(at,new)?;crate::saving::library(argument(&args,5)?,&d.archive.bytes()?)?;},_=>{}
                }
            }
        },
        Some(path)=>{app.open(path)?;platform::run(app)?;},None=>platform::run(app)?,
    }
    Ok(())
}
