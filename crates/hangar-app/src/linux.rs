//! Small Xlib backend for local Linux development. No toolkit or GPU dependency.
use crate::ui::{App, Draw, Key};
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
    fn XLoadFont(d: *mut c_void, name: *const c_char) -> c_ulong;
    fn XSetFont(d: *mut c_void, gc: *mut c_void, font: c_ulong) -> c_int;
    fn XUnloadFont(d: *mut c_void, font: c_ulong) -> c_int;
}
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(data: *mut c_void);
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
            0x1b1f23,
        );
        let title = CString::new(format!("TORE Hangar {}", env!("CARGO_PKG_VERSION"))).unwrap();
        XStoreName(d, w, title.as_ptr());
        XSelectInput(d, w, 1 | 4 | 8 | 64 | 32768 | 131072);
        let mut delete = XInternAtom(d, c"WM_DELETE_WINDOW".as_ptr(), 0);
        XSetWMProtocols(d, w, &mut delete, 1);
        let gc = XCreateGC(d, w, 0, ptr::null_mut());
        let font = XLoadFont(d, c"fixed".as_ptr());
        XSetFont(d, gc, font);
        if capture.is_none() {
            XMapWindow(d, w);
        }
        let mut event = [0 as c_long; 24];
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
                    if let Some(k) = key {
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
                    app.motion(e.x, e.y, e.state & 1 != 0);
                    if kind == 4 && (e.keycode == 4 || e.keycode == 5) {
                        app.wheel(if e.keycode == 4 { 1 } else { -1 });
                    } else {
                        app.click(e.x, e.y, e.keycode as u8, kind == 4);
                    }
                }
                6 => {
                    let e = &*(event.as_ptr().cast::<XKeyEvent>());
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
                    Draw::Text(x, y, s, color) | Draw::Label(x, y, s, color) => {
                        let s = CString::new(s.replace('\0', "?")).unwrap();
                        XSetForeground(d, gc, color as c_ulong);
                        XDrawString(d, pix, gc, x, y, s.as_ptr(), s.as_bytes().len() as i32);
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
        XUnloadFont(d, font);
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
fn recent_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("tore-hangar-recent.txt")
}
pub fn load_recent() -> Vec<String> {
    std::fs::read_to_string(recent_path())
        .ok()
        .filter(|s| s.len() <= 16384)
        .map(|s| s.lines().take(8).map(str::to_owned).collect())
        .unwrap_or_default()
}
pub fn save_recent(paths: &[String]) -> Result<()> {
    std::fs::write(recent_path(), paths.join("\n")).map_err(|e| e.to_string())
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
