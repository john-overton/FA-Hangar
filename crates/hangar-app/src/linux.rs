//! Small Xlib backend for local Linux development. No toolkit or GPU dependency.
use crate::ui::{App, Draw, Key, Style};
use hangar_core::Result;
use std::{
    ffi::{c_char, c_int, c_long, c_uint, c_ulong, c_void, CString},
    io::{Read, Write},
    ptr,
};
pub fn read(path: &str) -> Result<Vec<u8>> {
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let n = f.metadata().map_err(|e| e.to_string())?.len();
    if n > hangar_core::archive::ARCHIVE_LIMIT as u64 {
        return Err("File exceeds 128 MiB limit".into());
    }
    let mut b = Vec::new();
    Read::by_ref(&mut f)
        .take(hangar_core::archive::ARCHIVE_LIMIT as u64 + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > hangar_core::archive::ARCHIVE_LIMIT {
        return Err("File grew beyond limit".into());
    }
    Ok(b)
}
pub fn write_new(path: &str, bytes: &[u8]) -> Result<()> {
    hangar_core::save::guard_output(path)?;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let result = f.write_all(bytes).and_then(|_| f.sync_all());
    drop(f);
    if let Err(e) = result {
        let _ = std::fs::remove_file(path);
        return Err(e.to_string());
    }
    Ok(())
}
pub fn save_exists(path: &str) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(true),
        Ok(_) => Err(
            "LIB output, backup and temporary paths must be regular files, not links/directories"
                .into(),
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}
pub fn move_new(from: &str, to: &str) -> Result<()> {
    save_exists(from)?;
    let m = std::fs::metadata(from).map_err(|e| e.to_string())?;
    if m.permissions().readonly() {
        return Err("Cannot replace a read-only LIB".into());
    }
    // Same-directory hard link + unlink gives no-clobber moves without a crate
    // or Linux-only rename flags. A failed unlink leaves both copies intact.
    std::fs::hard_link(from, to).map_err(|e| e.to_string())?;
    std::fs::remove_file(from).map_err(|e| e.to_string())
}
#[repr(C)]
struct XKeyEvent {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    window: c_ulong,
    root: c_ulong,
    subwindow: c_ulong,
    time: c_ulong,
    x: c_int,
    y: c_int,
    x_root: c_int,
    y_root: c_int,
    state: c_uint,
    keycode: c_uint,
    same: c_int,
}
#[repr(C)]
struct XConfigureEvent {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    event: c_ulong,
    window: c_ulong,
    x: c_int,
    y: c_int,
    width: c_int,
    height: c_int,
    border: c_int,
    above: c_ulong,
    override_redirect: c_int,
}
#[repr(C)]
struct XClientMessage {
    kind: c_int,
    serial: c_ulong,
    send: c_int,
    display: *mut c_void,
    window: c_ulong,
    message_type: c_ulong,
    format: c_int,
    data: [c_long; 5],
}
#[link(name = "X11")]
unsafe extern "C" {
    fn XGetImage(
        d: *mut c_void,
        w: c_ulong,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
        planes: c_ulong,
        format: c_int,
    ) -> *mut c_void;
    fn XGetPixel(image: *mut c_void, x: c_int, y: c_int) -> c_ulong;
    fn XDestroyImage(image: *mut c_void) -> c_int;
    fn XDefaultVisual(d: *mut c_void, screen: c_int) -> *mut c_void;
    fn XCreateImage(
        d: *mut c_void,
        visual: *mut c_void,
        depth: c_uint,
        format: c_int,
        offset: c_int,
        data: *mut c_char,
        w: c_uint,
        h: c_uint,
        pad: c_int,
        stride: c_int,
    ) -> *mut c_void;
    fn XPutImage(
        d: *mut c_void,
        w: c_ulong,
        gc: *mut c_void,
        image: *mut c_void,
        sx: c_int,
        sy: c_int,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XOpenDisplay(name: *const c_char) -> *mut c_void;
    fn XDefaultScreen(d: *mut c_void) -> c_int;
    fn XRootWindow(d: *mut c_void, s: c_int) -> c_ulong;
    fn XDefaultDepth(d: *mut c_void, s: c_int) -> c_int;
    fn XCreateSimpleWindow(
        d: *mut c_void,
        p: c_ulong,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        border: c_uint,
        bc: c_ulong,
        bg: c_ulong,
    ) -> c_ulong;
    fn XStoreName(d: *mut c_void, w: c_ulong, name: *const c_char) -> c_int;
    fn XChangeProperty(
        d: *mut c_void,
        w: c_ulong,
        property: c_ulong,
        kind: c_ulong,
        format: c_int,
        mode: c_int,
        data: *const u8,
        n: c_int,
    ) -> c_int;
    fn XSelectInput(d: *mut c_void, w: c_ulong, mask: c_long) -> c_int;
    fn XMapWindow(d: *mut c_void, w: c_ulong) -> c_int;
    fn XCreateGC(d: *mut c_void, w: c_ulong, mask: c_ulong, values: *mut c_void) -> *mut c_void;
    fn XFreeGC(d: *mut c_void, gc: *mut c_void) -> c_int;
    fn XSetForeground(d: *mut c_void, gc: *mut c_void, color: c_ulong) -> c_int;
    fn XFillRectangle(
        d: *mut c_void,
        w: c_ulong,
        gc: *mut c_void,
        x: c_int,
        y: c_int,
        width: c_uint,
        height: c_uint,
    ) -> c_int;
    fn XDrawLine(
        d: *mut c_void,
        w: c_ulong,
        gc: *mut c_void,
        x: c_int,
        y: c_int,
        a: c_int,
        b: c_int,
    ) -> c_int;
    fn XDrawString(
        d: *mut c_void,
        w: c_ulong,
        gc: *mut c_void,
        x: c_int,
        y: c_int,
        s: *const c_char,
        len: c_int,
    ) -> c_int;
    fn XNextEvent(d: *mut c_void, event: *mut c_void) -> c_int;
    fn XLookupString(
        event: *mut XKeyEvent,
        buf: *mut c_char,
        n: c_int,
        keysym: *mut c_ulong,
        compose: *mut c_void,
    ) -> c_int;
    fn XInternAtom(d: *mut c_void, name: *const c_char, only: c_int) -> c_ulong;
    fn XSetWMProtocols(d: *mut c_void, w: c_ulong, atoms: *mut c_ulong, n: c_int) -> c_int;
    fn XCreatePixmap(
        d: *mut c_void,
        w: c_ulong,
        width: c_uint,
        height: c_uint,
        depth: c_uint,
    ) -> c_ulong;
    fn XFreePixmap(d: *mut c_void, p: c_ulong) -> c_int;
    fn XCopyArea(
        d: *mut c_void,
        src: c_ulong,
        dst: c_ulong,
        gc: *mut c_void,
        x: c_int,
        y: c_int,
        w: c_uint,
        h: c_uint,
        dx: c_int,
        dy: c_int,
    ) -> c_int;
    fn XFlush(d: *mut c_void) -> c_int;
    fn XDestroyWindow(d: *mut c_void, w: c_ulong) -> c_int;
    fn XCloseDisplay(d: *mut c_void) -> c_int;
    fn XLoadQueryFont(d: *mut c_void, name: *const c_char) -> *mut XFontStruct;
    fn XSetFont(d: *mut c_void, gc: *mut c_void, font: c_ulong) -> c_int;
    fn XFreeFont(d: *mut c_void, font: *mut XFontStruct) -> c_int;
}
/// Leading fields of Xlib's XFontStruct; only `fid` is read.
#[repr(C)]
struct XFontStruct {
    ext_data: *mut c_void,
    fid: c_ulong,
}
/// One X core font per text style: the closest of a few common XLFD families
/// by size and weight, falling back to `fixed`. `fake_bold` marks styles that
/// want weight 600+ but got a medium font; they are drawn twice, 1px apart.
struct Fonts {
    loaded: Vec<*mut XFontStruct>,
    fake_bold: Vec<bool>,
}
impl Fonts {
    unsafe fn load(d: *mut c_void) -> Self {
        let mut loaded = Vec::new();
        let mut fake_bold = Vec::new();
        for style in Style::ALL {
            let spec = style.spec();
            let bold = spec.weight >= 600;
            let families: &[&str] = if style.mono() {
                &["-*-lucidatypewriter", "-*-courier", "-misc-fixed"]
            } else {
                &["-*-helvetica", "-*-lucida", "-*-dejavu sans", "-misc-fixed"]
            };
            let mut font = ptr::null_mut();
            let mut got_bold = false;
            let weights: &[&str] = if bold {
                &["bold", "medium"]
            } else {
                &["medium"]
            };
            'search: for weight in weights {
                for family in families {
                    let name = CString::new(format!(
                        "{family}-{weight}-r-*--{}-*-*-*-*-*-iso8859-1",
                        spec.size
                    ))
                    .unwrap();
                    font = XLoadQueryFont(d, name.as_ptr());
                    if !font.is_null() {
                        got_bold = *weight == "bold";
                        break 'search;
                    }
                }
            }
            if font.is_null() {
                font = XLoadQueryFont(d, c"fixed".as_ptr());
            }
            loaded.push(font);
            fake_bold.push(bold && !got_bold);
        }
        Fonts { loaded, fake_bold }
    }
    unsafe fn free(&self, d: *mut c_void) {
        for font in &self.loaded {
            if !font.is_null() {
                XFreeFont(d, *font);
            }
        }
    }
}
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(data: *mut c_void);
}
/// `_NET_WM_ICON` cardinals (width, height, ARGB rows top-down) for the 16,
/// 32 and 48px 32-bit entries of the committed app icon. A 32-bit DIB pixel
/// read as a little-endian u32 is already 0xAARRGGBB.
fn wm_icon() -> Vec<c_ulong> {
    const ICO: &[u8] = include_bytes!("../../../tore-hangar-design/icons/app/tore-hangar.ico");
    let int = |at: usize, len: usize| {
        ICO.get(at..at + len)
            .map_or(0, |b| b.iter().rev().fold(0, |v, &x| v << 8 | x as usize))
    };
    let mut out = Vec::new();
    for entry in (0..int(4, 2)).map(|i| 6 + i * 16) {
        let (n, offset) = (int(entry, 1), int(entry + 12, 4));
        if !matches!(n, 16 | 32 | 48) || int(entry + 6, 2) != 32 || int(offset, 4) != 40 {
            continue;
        }
        let Some(pixels) = ICO.get(offset + 40..offset + 40 + n * n * 4) else {
            continue;
        };
        out.extend([n as c_ulong, n as c_ulong]);
        for row in pixels.chunks_exact(n * 4).rev() {
            out.extend(
                row.chunks_exact(4)
                    .map(|p| u32::from_le_bytes([p[0], p[1], p[2], p[3]]) as c_ulong),
            );
        }
    }
    out
}
pub fn run(app: App) -> Result<()> {
    run_surface(app, None)
}
pub fn capture(app: App, path: &str) -> Result<()> {
    run_surface(app, Some(path))
}
fn run_surface(mut app: App, capture: Option<&str>) -> Result<()> {
    unsafe {
        let d = XOpenDisplay(ptr::null());
        if d.is_null() {
            return Err(
                "No X11 display. Run under X11/Xwayland or use --snapshot / CLI commands.".into(),
            );
        }
        let screen = XDefaultScreen(d);
        let w = XCreateSimpleWindow(
            d,
            XRootWindow(d, screen),
            0,
            0,
            app.width as u32,
            app.height as u32,
            0,
            0,
            crate::ui::theme::color::GM_900.0 as c_ulong,
        );
        let title = CString::new(format!("TORE Hangar {}", env!("CARGO_PKG_VERSION"))).unwrap();
        XStoreName(d, w, title.as_ptr());
        let icon = wm_icon();
        if !icon.is_empty() {
            let atom = XInternAtom(d, c"_NET_WM_ICON".as_ptr(), 0);
            // XA_CARDINAL, format 32 (C longs), PropModeReplace.
            XChangeProperty(
                d,
                w,
                atom,
                6,
                32,
                0,
                icon.as_ptr().cast(),
                icon.len() as c_int,
            );
        }
        XSelectInput(d, w, 1 | 4 | 8 | 64 | 32768 | 131072);
        let mut delete = XInternAtom(d, c"WM_DELETE_WINDOW".as_ptr(), 0);
        XSetWMProtocols(d, w, &mut delete, 1);
        let gc = XCreateGC(d, w, 0, ptr::null_mut());
        let fonts = Fonts::load(d);
        if capture.is_none() {
            XMapWindow(d, w);
        }
        let mut event = [0 as c_long; 24];
        // Last left press (time ms, x, y) for double-click detection.
        let mut last_press: (c_ulong, c_int, c_int) = (0, i32::MIN, i32::MIN);
        while !app.quit {
            if capture.is_none() {
                XNextEvent(d, event.as_mut_ptr().cast());
            }
            let kind = *(event.as_ptr().cast::<c_int>());
            match kind {
                2 => {
                    let e = &mut *(event.as_mut_ptr().cast::<XKeyEvent>());
                    let mut buf = [0i8; 32];
                    let mut sym = 0;
                    let n = XLookupString(e, buf.as_mut_ptr(), 32, &mut sym, ptr::null_mut());
                    let ctrl = e.state & 4 != 0;
                    let shift = e.state & 1 != 0;
                    // Mod1 (Alt): Alt+N flips normals in Edit Mesh.
                    let alt = e.state & 8 != 0;
                    let key = match sym {
                        0xff0d | 0xff8d => Some(Key::Enter),
                        0xff1b => Some(Key::Escape),
                        0xff08 => Some(Key::Backspace),
                        0xffff => Some(Key::Delete),
                        0xff52 => Some(Key::Up),
                        0xff54 => Some(Key::Down),
                        0xff50 => Some(Key::Home),
                        0xff09 => Some(Key::Tab),
                        0xffbe => Some(Key::F1),
                        0xffb0..=0xffb9 => Some(Key::Num((sym - 0xffb0) as u8)),
                        _ => None,
                    };
                    if alt {
                        if sym == b'n' as c_ulong || sym == b'N' as c_ulong {
                            app.alt_key('n');
                        } else if sym == b'z' as c_ulong || sym == b'Z' as c_ulong {
                            // Alt+Z: X-ray.
                            app.alt_key('z');
                        }
                    } else if let Some(k) = key {
                        app.key(k, ctrl, shift);
                    } else if ctrl && (32..127).contains(&sym) {
                        app.key(Key::Char(sym as u8 as char), true, shift);
                    } else if n > 0 {
                        let b = std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), n as usize);
                        if let Ok(s) = std::str::from_utf8(b) {
                            for c in s.chars() {
                                app.key(Key::Char(c), ctrl, shift);
                            }
                        }
                    }
                }
                4 | 5 => {
                    let e = &*(event.as_ptr().cast::<XKeyEvent>());
                    app.modifiers(e.state & 4 != 0);
                    app.alt_modifier(e.state & 8 != 0);
                    app.motion(e.x, e.y, e.state & 1 != 0);
                    if kind == 4 && (e.keycode == 4 || e.keycode == 5) {
                        app.wheel(if e.keycode == 4 { 1 } else { -1 });
                    } else {
                        app.pointer(e.x, e.y, e.keycode as u8, kind == 4, e.state & 1 != 0);
                        if kind == 4 && e.keycode == 1 {
                            let (time, x, y) = last_press;
                            if e.time.wrapping_sub(time) < 400
                                && (e.x - x).abs() < 4
                                && (e.y - y).abs() < 4
                            {
                                app.double_click(e.x, e.y);
                                last_press = (0, i32::MIN, i32::MIN);
                            } else {
                                last_press = (e.time, e.x, e.y);
                            }
                        }
                    }
                }
                6 => {
                    let e = &*(event.as_ptr().cast::<XKeyEvent>());
                    app.modifiers(e.state & 4 != 0);
                    app.alt_modifier(e.state & 8 != 0);
                    app.motion(e.x, e.y, e.state & 1 != 0);
                }
                22 => {
                    let e = &*(event.as_ptr().cast::<XConfigureEvent>());
                    app.width = e.width.max(800);
                    app.height = e.height.max(600);
                }
                33 => {
                    let e = &*(event.as_ptr().cast::<XClientMessage>());
                    if e.data[0] as c_ulong == delete {
                        app.close();
                    }
                }
                _ => {}
            }
            let pix = XCreatePixmap(
                d,
                w,
                app.width as u32,
                app.height as u32,
                XDefaultDepth(d, screen) as u32,
            );
            for cmd in app.draw().commands {
                match cmd {
                    Draw::Rect(x, y, width, height, color) => {
                        XSetForeground(d, gc, color as c_ulong);
                        XFillRectangle(d, pix, gc, x, y, width as u32, height as u32);
                    }
                    Draw::Line(x, y, a, b, color) => {
                        XSetForeground(d, gc, color as c_ulong);
                        XDrawLine(d, pix, gc, x, y, a, b);
                    }
                    Draw::Icon(x, y, g, small, full, half) => {
                        let mask = g.mask(small);
                        for (runs, color) in [(mask.full, full), (mask.half, half)] {
                            XSetForeground(d, gc, color as c_ulong);
                            for (row, at, len) in runs {
                                XFillRectangle(
                                    d,
                                    pix,
                                    gc,
                                    x + *at as i32,
                                    y + *row as i32,
                                    *len as u32,
                                    1,
                                );
                            }
                        }
                    }
                    Draw::Bitmap(x, y, width, height, pixels) => {
                        let memory = malloc(pixels.len() * 4);
                        if !memory.is_null() {
                            std::ptr::copy_nonoverlapping(
                                pixels.as_ptr(),
                                memory.cast(),
                                pixels.len(),
                            );
                            let image = XCreateImage(
                                d,
                                XDefaultVisual(d, screen),
                                XDefaultDepth(d, screen) as u32,
                                2,
                                0,
                                memory.cast(),
                                width as u32,
                                height as u32,
                                32,
                                0,
                            );
                            if !image.is_null() {
                                XPutImage(
                                    d,
                                    pix,
                                    gc,
                                    image,
                                    0,
                                    0,
                                    x,
                                    y,
                                    width as u32,
                                    height as u32,
                                );
                                XDestroyImage(image);
                            } else {
                                free(memory);
                            }
                        }
                    }
                    Draw::Text(x, y, s, color, style) => {
                        let bytes = crate::ui::native_text(&s, false);
                        let font = fonts.loaded[style.index()];
                        if !font.is_null() {
                            XSetFont(d, gc, (*font).fid);
                        }
                        XSetForeground(d, gc, color as c_ulong);
                        for dx in 0..=i32::from(fonts.fake_bold[style.index()]) {
                            XDrawString(
                                d,
                                pix,
                                gc,
                                x + dx,
                                y,
                                bytes.as_ptr().cast(),
                                bytes.len() as i32,
                            );
                        }
                    }
                }
            }
            if let Some(path) = capture {
                let image = XGetImage(
                    d,
                    pix,
                    0,
                    0,
                    app.width as u32,
                    app.height as u32,
                    c_ulong::MAX,
                    2,
                );
                if image.is_null() {
                    return Err("XGetImage failed".into());
                }
                let mut bytes = format!("P6\n{} {}\n255\n", app.width, app.height).into_bytes();
                for y in 0..app.height {
                    for x in 0..app.width {
                        let pixel = XGetPixel(image, x, y);
                        bytes.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
                    }
                }
                XDestroyImage(image);
                write_new(path, &bytes)?;
                app.quit = true;
            }
            XCopyArea(
                d,
                pix,
                w,
                gc,
                0,
                0,
                app.width as u32,
                app.height as u32,
                0,
                0,
            );
            XFreePixmap(d, pix);
            XFlush(d);
        }
        stop_audio();
        fonts.free(d);
        XFreeGC(d, gc);
        XDestroyWindow(d, w);
        XCloseDisplay(d);
        Ok(())
    }
}

pub fn current_dir() -> String {
    std::env::current_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/".into())
}
pub fn roots() -> Vec<String> {
    vec![
        "/".into(),
        std::env::var("HOME").unwrap_or_else(|_| "/".into()),
    ]
}
pub fn is_dir(path: &str) -> bool {
    std::path::Path::new(path).is_dir()
}
pub fn list_dir(path: &str) -> Result<Vec<crate::ui::FileItem>> {
    let mut files = Vec::new();
    for e in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        files.push(crate::ui::FileItem {
            name,
            path: e.path().to_string_lossy().into_owned(),
            directory: e.path().is_dir(),
        });
        if files.len() > 8192 {
            return Err("Directory exceeds 8192 items; choose a smaller folder".into());
        }
    }
    files.sort_unstable_by(|a, b| {
        b.directory.cmp(&a.directory).then_with(|| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        })
    });
    Ok(files)
}
fn sidecar_path(name: &str) -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name(name)
}
fn load_paths(name: &str, count: usize) -> Vec<String> {
    std::fs::read_to_string(sidecar_path(name))
        .ok()
        .filter(|s| s.len() <= 16384)
        .map(|s| s.lines().take(count).map(str::to_owned).collect())
        .unwrap_or_default()
}
fn save_paths(name: &str, paths: &[String]) -> Result<()> {
    std::fs::write(sidecar_path(name), paths.join("\n")).map_err(|e| e.to_string())
}
pub fn load_recent() -> Vec<String> {
    load_paths("tore-hangar-recent.txt", 8)
}
pub fn save_recent(paths: &[String]) -> Result<()> {
    save_paths("tore-hangar-recent.txt", paths)
}
/// The last game palette Hangar resolved: its source label, then 1,536 hex
/// digits. Read-only locations keep it for the session only.
pub fn load_palette_memory() -> Vec<String> {
    load_paths("tore-hangar-palette.txt", 2)
}
pub fn save_palette_memory(lines: &[String]) -> Result<()> {
    save_paths("tore-hangar-palette.txt", lines)
}
pub fn load_decals() -> Vec<String> {
    load_paths("tore-hangar-decals.txt", 16)
}
pub fn save_decals(paths: &[String]) -> Result<()> {
    save_paths("tore-hangar-decals.txt", paths)
}
std::thread_local! {static AUDIO:std::cell::RefCell<Option<std::process::Child>>=const{std::cell::RefCell::new(None)};}
pub fn stop_audio() {
    AUDIO.with(|slot| {
        if let Some(mut child) = slot.borrow_mut().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    });
}
pub fn play_audio(wav: Vec<u8>) -> Result<()> {
    stop_audio();
    let mut child = std::process::Command::new("aplay")
        .arg("-q")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Audio preview needs ALSA aplay: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("Audio pipe unavailable")?;
    std::thread::spawn(move || {
        let _ = stdin.write_all(&wav);
    });
    AUDIO.with(|slot| *slot.borrow_mut() = Some(child));
    Ok(())
}

pub fn file_size(path: &str) -> Result<usize> {
    let n = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if n > i32::MAX as u64 {
        return Err("Source LIB exceeds 2 GiB".into());
    }
    Ok(n as usize)
}
pub fn read_range(path: &str, at: usize, size: usize) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    if size > hangar_core::archive::RESOURCE_LIMIT * 2 + 4 {
        return Err("Source range exceeds limit".into());
    }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(at as u64))
        .map_err(|e| e.to_string())?;
    let mut out = vec![0; size];
    file.read_exact(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

pub fn remove_file(path: &str) -> Result<()> {
    std::fs::remove_file(path).map_err(|e| e.to_string())
}
