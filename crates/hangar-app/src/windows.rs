//! ANSI Win32/GDI only. No CRT, standard library, Unicode shim, GPU or installer.
#![allow(non_snake_case)]
use crate::ui::{App, Draw, Key, Style};
use alloc::{boxed::Box, ffi::CString, format, string::String, vec, vec::Vec};
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::{c_char, c_void},
    ptr,
};
use hangar_core::Result;
type Handle = *mut c_void;
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Msg {
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
}
#[repr(C)]
struct Paint {
    dc: Handle,
    erase: i32,
    rect: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct WndClass {
    style: u32,
    proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> isize>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu: *const c_char,
    name: *const c_char,
}
#[repr(C)]
struct MinMax {
    reserved: Point,
    max_size: Point,
    max_position: Point,
    min_track: Point,
    max_track: Point,
}
#[cfg_attr(not(target_arch = "x86"), link(name = "kernel32", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "kernel32",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
unsafe extern "system" {
    fn GetProcessHeap() -> Handle;
    fn HeapAlloc(heap: Handle, flags: u32, size: usize) -> *mut c_void;
    fn HeapFree(heap: Handle, flags: u32, mem: *mut c_void) -> i32;
    fn ExitProcess(code: u32) -> !;
    fn GetModuleHandleA(name: *const c_char) -> Handle;
    fn GetCommandLineA() -> *const c_char;
    fn CreateFileA(
        name: *const c_char,
        access: u32,
        share: u32,
        security: *mut c_void,
        creation: u32,
        flags: u32,
        template: Handle,
    ) -> Handle;
    fn GetFileSize(file: Handle, high: *mut u32) -> u32;
    fn ReadFile(
        file: Handle,
        buf: *mut c_void,
        size: u32,
        read: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn WriteFile(
        file: Handle,
        buf: *const c_void,
        size: u32,
        written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn FlushFileBuffers(file: Handle) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn DeleteFileA(name: *const c_char) -> i32;
    fn MoveFileA(from: *const c_char, to: *const c_char) -> i32;
    fn GetLastError() -> u32;
}
#[cfg_attr(not(target_arch = "x86"), link(name = "user32", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")
)]
unsafe extern "system" {
    fn RegisterClassA(class: *const WndClass) -> u16;
    fn CreateWindowExA(
        ex: u32,
        class: *const c_char,
        title: *const c_char,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *mut c_void,
    ) -> Handle;
    fn DefWindowProcA(hwnd: Handle, message: u32, wp: usize, lp: isize) -> isize;
    fn ShowWindow(hwnd: Handle, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: Handle) -> i32;
    fn GetMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageA(msg: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn DestroyWindow(hwnd: Handle) -> i32;
    fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
    fn BeginPaint(hwnd: Handle, paint: *mut Paint) -> Handle;
    fn EndPaint(hwnd: Handle, paint: *const Paint) -> i32;
    fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
    fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
    fn LoadCursorA(instance: Handle, name: *const c_char) -> Handle;
    fn LoadIconA(instance: Handle, name: *const c_char) -> Handle;
    fn GetKeyState(key: i32) -> i16;
    fn SetCapture(hwnd: Handle) -> Handle;
    fn ReleaseCapture() -> i32;
    fn MessageBoxA(hwnd: Handle, text: *const c_char, caption: *const c_char, flags: u32) -> i32;
}
#[cfg_attr(not(target_arch = "x86"), link(name = "gdi32", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(name = "gdi32", kind = "raw-dylib", import_name_type = "undecorated")
)]
unsafe extern "system" {
    fn StretchDIBits(
        dc: Handle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        sx: i32,
        sy: i32,
        sw: i32,
        sh: i32,
        bits: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
    fn CreateSolidBrush(color: u32) -> Handle;
    fn DeleteObject(obj: Handle) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> Handle;
    fn SelectObject(dc: Handle, obj: Handle) -> Handle;
    fn MoveToEx(dc: Handle, x: i32, y: i32, previous: *mut Point) -> i32;
    fn LineTo(dc: Handle, x: i32, y: i32) -> i32;
    fn TextOutA(dc: Handle, x: i32, y: i32, text: *const c_char, len: i32) -> i32;
    fn SetTextColor(dc: Handle, color: u32) -> u32;
    fn SetBkMode(dc: Handle, mode: i32) -> i32;
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn CreateCompatibleBitmap(dc: Handle, w: i32, h: i32) -> Handle;
    fn DeleteDC(dc: Handle) -> i32;
    fn BitBlt(
        dst: Handle,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: Handle,
        sx: i32,
        sy: i32,
        rop: u32,
    ) -> i32;
    fn CreateFontA(
        height: i32,
        width: i32,
        escape: i32,
        orient: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        family: u32,
        name: *const c_char,
    ) -> Handle;
}
struct Heap;
unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(core::mem::size_of::<usize>());
        let Some(size) = layout
            .size()
            .checked_add(align)
            .and_then(|n| n.checked_add(core::mem::size_of::<usize>()))
        else {
            return ptr::null_mut();
        };
        let base = HeapAlloc(GetProcessHeap(), 0, size) as usize;
        if base == 0 {
            return ptr::null_mut();
        }
        let aligned = (base + core::mem::size_of::<usize>() + align - 1) & !(align - 1);
        *((aligned - core::mem::size_of::<usize>()) as *mut usize) = base;
        aligned as *mut u8
    }
    unsafe fn dealloc(&self, p: *mut u8, _layout: Layout) {
        let base = *((p as usize - core::mem::size_of::<usize>()) as *const usize);
        HeapFree(GetProcessHeap(), 0, base as *mut c_void);
    }
}
#[global_allocator]
static ALLOCATOR: Heap = Heap;
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        MessageBoxA(
            ptr::null_mut(),
            c"Hangar stopped after an internal error. If saving, check the LIB and its .bak/.tmp files.".as_ptr(),
            c"TORE Hangar".as_ptr(),
            0x10,
        );
        ExitProcess(1)
    }
}
fn path(path: &str) -> Result<CString> {
    if !path.is_ascii() {
        return Err("This build requires ASCII file paths".into());
    }
    CString::new(path.replace('/', "\\")).map_err(|_| "Path contains a NUL byte".into())
}
fn error(action: &str) -> String {
    format!("{} (Windows error {})", action, unsafe { GetLastError() })
}
pub fn read(name: &str) -> Result<Vec<u8>> {
    let name = path(name)?;
    unsafe {
        let file = CreateFileA(
            name.as_ptr(),
            0x80000000,
            1,
            ptr::null_mut(),
            3,
            0x80,
            ptr::null_mut(),
        );
        if file as isize == -1 {
            return Err(error("Cannot open file"));
        }
        let mut high = 0;
        let size = GetFileSize(file, &mut high);
        if high != 0 || size as usize > hangar_core::archive::ARCHIVE_LIMIT {
            CloseHandle(file);
            return Err("File exceeds 128 MiB limit".into());
        }
        let mut b = vec![0; size as usize];
        let mut at = 0;
        while at < b.len() {
            let mut n = 0;
            if ReadFile(
                file,
                b[at..].as_mut_ptr().cast(),
                (b.len() - at) as u32,
                &mut n,
                ptr::null_mut(),
            ) == 0
                || n == 0
            {
                let e = error("Cannot read file");
                CloseHandle(file);
                return Err(e);
            }
            at += n as usize;
        }
        CloseHandle(file);
        Ok(b)
    }
}
pub fn write_new(name: &str, bytes: &[u8]) -> Result<()> {
    hangar_core::save::guard_output(name)?;
    let name = path(name)?;
    unsafe {
        let file = CreateFileA(
            name.as_ptr(),
            0x40000000,
            0,
            ptr::null_mut(),
            1,
            0x80,
            ptr::null_mut(),
        );
        if file as isize == -1 {
            return Err(error("Cannot create output; choose a new file path"));
        }
        let mut at = 0;
        let mut err = None;
        while at < bytes.len() {
            let mut n = 0;
            if WriteFile(
                file,
                bytes[at..].as_ptr().cast(),
                (bytes.len() - at) as u32,
                &mut n,
                ptr::null_mut(),
            ) == 0
                || n == 0
            {
                err = Some(error("Cannot write output"));
                break;
            }
            at += n as usize;
        }
        if err.is_none() && FlushFileBuffers(file) == 0 {
            err = Some(error("Cannot flush output"));
        }
        CloseHandle(file);
        if let Some(e) = err {
            DeleteFileA(name.as_ptr());
            Err(e)
        } else {
            Ok(())
        }
    }
}
pub fn save_exists(name: &str) -> Result<bool> {
    let name = path(name)?;
    unsafe {
        let attr = GetFileAttributesA(name.as_ptr());
        if attr == u32::MAX {
            return match GetLastError() {
                2 | 3 => Ok(false),
                _ => Err(error("Cannot inspect output path")),
            };
        }
        if attr & (0x10 | 0x400) != 0 {
            return Err("LIB output, backup and temporary paths must be regular files, not links/directories".into());
        }
        Ok(true)
    }
}
pub fn move_new(from: &str, to: &str) -> Result<()> {
    save_exists(from)?;
    let from = path(from)?;
    let to = path(to)?;
    unsafe {
        if GetFileAttributesA(from.as_ptr()) & 1 != 0 {
            return Err("Cannot replace a read-only LIB".into());
        }
        // MoveFileA fails if the destination exists; no NT-only replace API.
        if MoveFileA(from.as_ptr(), to.as_ptr()) == 0 {
            return Err(error("Cannot move LIB"));
        }
    }
    Ok(())
}
static mut APP: *mut App = ptr::null_mut();
fn color(rgb: u32) -> u32 {
    (rgb & 255) << 16 | (rgb & 0xff00) | (rgb >> 16) & 255
}
#[repr(C)]
struct BitmapInfo {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bpp: u16,
    compression: u32,
    bytes: u32,
    xppm: i32,
    yppm: i32,
    used: u32,
    important: u32,
}
/// One cached GDI font per text style, created on first paint and kept for
/// the life of the process: Tahoma for UI styles, Lucida Console for data.
/// Negative heights select the em size from `theme::text`; Win9x maps weight
/// 500 to regular and 600 to bold.
static mut FONTS: [Handle; 9] = [ptr::null_mut(); 9];
unsafe fn fonts() -> [Handle; 9] {
    let fonts = &mut *ptr::addr_of_mut!(FONTS);
    for style in Style::ALL {
        if fonts[style.index()].is_null() {
            let spec = style.spec();
            fonts[style.index()] = CreateFontA(
                -spec.size,
                0,
                0,
                0,
                spec.weight as i32,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                u32::from(style.mono()),
                if style.mono() {
                    c"Lucida Console".as_ptr()
                } else {
                    c"Tahoma".as_ptr()
                },
            );
        }
    }
    *fonts
}
/// Baseline offset from the TextOutA cell top: Tahoma's ascent is one em,
/// Lucida Console's is 1616/2048 em.
fn ascent(style: Style) -> i32 {
    let size = style.spec().size;
    if style.mono() {
        (size * 1616 + 1024) / 2048
    } else {
        size
    }
}
unsafe fn paint(hwnd: Handle) {
    let mut ps: Paint = core::mem::zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    // BeginPaint can synchronously deliver WM_ERASEBKGND. Borrow state afterward.
    let app = &*APP;
    let back = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, app.width, app.height);
    if back.is_null() || bitmap.is_null() {
        if !back.is_null() {
            DeleteDC(back);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        EndPaint(hwnd, &ps);
        return;
    }
    let oldbitmap = SelectObject(back, bitmap);
    let fonts = fonts();
    let oldfont = SelectObject(back, fonts[0]);
    SetBkMode(back, 1);
    for cmd in app.draw().commands {
        match cmd {
            Draw::Rect(x, y, w, h, c) => {
                let brush = CreateSolidBrush(color(c));
                FillRect(
                    back,
                    &Rect {
                        left: x,
                        top: y,
                        right: x + w,
                        bottom: y + h,
                    },
                    brush,
                );
                DeleteObject(brush);
            }
            Draw::Icon(x, y, g, small, full, half) => {
                let mask = g.mask(small);
                for (runs, c) in [(mask.full, full), (mask.half, half)] {
                    let brush = CreateSolidBrush(color(c));
                    for (row, at, len) in runs {
                        let left = x + *at as i32;
                        let top = y + *row as i32;
                        FillRect(
                            back,
                            &Rect {
                                left,
                                top,
                                right: left + *len as i32,
                                bottom: top + 1,
                            },
                            brush,
                        );
                    }
                    DeleteObject(brush);
                }
            }
            Draw::Line(x, y, a, b, c) => {
                let pen = CreatePen(0, 1, color(c));
                let old = SelectObject(back, pen);
                MoveToEx(back, x, y, ptr::null_mut());
                LineTo(back, a, b);
                SelectObject(back, old);
                DeleteObject(pen);
            }
            Draw::Text(x, y, s, c, style) => {
                let bytes = crate::ui::native_text(&s, true);
                SelectObject(back, fonts[style.index()]);
                SetTextColor(back, color(c));
                TextOutA(
                    back,
                    x,
                    y - ascent(style),
                    bytes.as_ptr().cast(),
                    bytes.len() as i32,
                );
            }
            Draw::Bitmap(x, y, w, h, pixels) => {
                let info = BitmapInfo {
                    size: 40,
                    width: w as i32,
                    height: -(h as i32),
                    planes: 1,
                    bpp: 32,
                    compression: 0,
                    bytes: 0,
                    xppm: 0,
                    yppm: 0,
                    used: 0,
                    important: 0,
                };
                StretchDIBits(
                    back,
                    x,
                    y,
                    w as i32,
                    h as i32,
                    0,
                    0,
                    w as i32,
                    h as i32,
                    pixels.as_ptr().cast(),
                    &info,
                    0,
                    0x00cc0020,
                );
            }
        }
    }
    BitBlt(dc, 0, 0, app.width, app.height, back, 0, 0, 0x00cc0020);
    SelectObject(back, oldfont);
    SelectObject(back, oldbitmap);
    DeleteObject(bitmap);
    DeleteDC(back);
    EndPaint(hwnd, &ps);
}
unsafe extern "system" fn wndproc(hwnd: Handle, msg: u32, wp: usize, lp: isize) -> isize {
    if APP.is_null() {
        return DefWindowProcA(hwnd, msg, wp, lp);
    }
    // These messages may be delivered synchronously by GDI or mouse capture.
    // Handle them before taking an exclusive reference to editor state.
    match msg {
        0x24 => {
            let m = &mut *(lp as *mut MinMax);
            m.min_track = Point { x: 816, y: 639 };
            return 0;
        }
        0x0f => {
            paint(hwnd);
            return 0;
        }
        0x14 | 0x215 => return 1,
        0x02 => {
            PostQuitMessage(0);
            return 0;
        }
        _ => {}
    }
    let app = &mut *APP;
    let shift = GetKeyState(0x10) < 0;
    let ctrl = GetKeyState(0x11) < 0;
    match msg {
        0x05 => {
            let mut rect: Rect = core::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            app.width = rect.right.max(800);
            app.height = rect.bottom.max(600);
        }
        0x10 => {
            app.close();
            if app.quit {
                DestroyWindow(hwnd);
            } else {
                InvalidateRect(hwnd, ptr::null(), 0);
            }
            return 0;
        }
        0x100 => {
            let k = match wp {
                0x0d => Some(Key::Enter),
                0x1b => Some(Key::Escape),
                8 => Some(Key::Backspace),
                0x2e => Some(Key::Delete),
                0x26 => Some(Key::Up),
                0x28 => Some(Key::Down),
                0x24 => Some(Key::Home),
                9 => Some(Key::Tab),
                0x70 => Some(Key::F1),
                0x60..=0x69 => Some(Key::Num((wp - 0x60) as u8)),
                0x41..=0x5a if ctrl => Some(Key::Char((wp as u8 as char).to_ascii_lowercase())),
                _ => None,
            };
            if let Some(k) = k {
                app.key(k, ctrl, shift);
            }
        }
        // Alt+N (WM_SYSKEYDOWN, then its WM_SYSCHAR) flips normals in Edit Mesh.
        0x104 if wp == 0x4e => app.alt_key('n'),
        0x106 if wp == b'n' as usize || wp == b'N' as usize => return 0,
        0x102 => {
            if !ctrl && (32..127).contains(&wp) {
                app.key(Key::Char(wp as u8 as char), false, shift);
            }
        }
        0x201 | 0x202 | 0x203 | 0x204 | 0x205 | 0x207 | 0x208 => {
            let x = lp as u16 as i16 as i32;
            let y = (lp >> 16) as u16 as i16 as i32;
            app.modifiers(ctrl);
            let (button, down) = match msg {
                0x201 | 0x203 => (1, true),
                0x202 => (1, false),
                0x204 => (3, true),
                0x205 => (3, false),
                0x207 => (2, true),
                _ => (2, false),
            };
            app.pointer(x, y, button, down, shift);
            // With CS_DBLCLKS the second press arrives as WM_LBUTTONDBLCLK.
            if msg == 0x203 {
                app.double_click(x, y);
            }
        }
        0x200 => {
            app.modifiers(ctrl);
            app.motion(
                lp as u16 as i16 as i32,
                (lp >> 16) as u16 as i16 as i32,
                shift,
            )
        }
        0x20a => app.wheel((wp >> 16) as u16 as i16 as i32 / 120),
        _ => return DefWindowProcA(hwnd, msg, wp, lp),
    }
    let quit = app.quit;
    // Capture can reenter the window procedure; the editor borrow ends above.
    if msg == 0x207 || msg == 0x201 || msg == 0x203 {
        SetCapture(hwnd);
    }
    if msg == 0x208 || msg == 0x202 {
        ReleaseCapture();
    }
    if quit {
        DestroyWindow(hwnd);
    } else {
        InvalidateRect(hwnd, ptr::null(), 0);
    }
    0
}
#[no_mangle]
pub extern "C" fn mainCRTStartup() -> ! {
    unsafe {
        let mut app = Box::new(App::new());
        // A single optional quoted path, plus --demo and --smoke-test for Windows CI.
        let command = core::ffi::CStr::from_ptr(GetCommandLineA()).to_string_lossy();
        let rest = if let Some(s) = command.strip_prefix('"') {
            s.split_once('"').map_or("", |(_, r)| r)
        } else {
            command.split_once(' ').map_or("", |(_, r)| r)
        }
        .trim();
        if rest == "--smoke-test" {
            app.demo();
            let before = app.doc.archive.bytes().unwrap();
            app.key(Key::Char('g'), false, false);
            app.key(Key::Char('1'), false, false);
            app.key(Key::Enter, false, false);
            assert!(app.doc.dirty());
            assert!(app.draw().commands.len() > 100);
            app.key(Key::Char('z'), true, false);
            assert_eq!(app.doc.archive.bytes().unwrap(), before);
            app.smoke_layout();
            app.smoke_media();
            app.smoke_clone();
            crate::saving::smoke();
            app.smoke_save_policy();
            ExitProcess(0);
        }
        if rest == "--demo" {
            app.demo();
        } else if !rest.is_empty() {
            if let Err(e) = app.open(rest.trim_matches('"')) {
                app.status = e;
            }
        }
        APP = Box::into_raw(app);
        let instance = GetModuleHandleA(ptr::null());
        let class = WndClass {
            // CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS
            style: 3 | 8,
            proc: Some(wndproc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            // MAKEINTRESOURCE(1): RT_GROUP_ICON 1, linked in by build.rs.
            icon: LoadIconA(instance, ptr::without_provenance(1)),
            cursor: LoadCursorA(ptr::null_mut(), 32512usize as *const c_char),
            background: ptr::null_mut(),
            menu: ptr::null(),
            name: c"ToreHangar".as_ptr(),
        };
        if RegisterClassA(&class) == 0 {
            ExitProcess(1);
        }
        let hwnd = CreateWindowExA(
            0,
            class.name,
            concat!("TORE Hangar ", env!("CARGO_PKG_VERSION"), "\0")
                .as_ptr()
                .cast(),
            0x00cf0000,
            0x80000000u32 as i32,
            0x80000000u32 as i32,
            1296,
            839,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if hwnd.is_null() {
            ExitProcess(1);
        }
        ShowWindow(hwnd, 1);
        UpdateWindow(hwnd);
        let mut msg: Msg = core::mem::zeroed();
        loop {
            let result = GetMessageA(&mut msg, ptr::null_mut(), 0, 0);
            if result <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
        stop_audio();
        drop(Box::from_raw(APP));
        ExitProcess(0)
    }
}
// LLVM may emit these intrinsics. Volatile loops prevent self-recursive lowering.
#[no_mangle]
unsafe extern "C" fn memcpy(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    for i in 0..n {
        dst.add(i).write_volatile(src.add(i).read_volatile());
    }
    dst
}
#[no_mangle]
unsafe extern "C" fn memmove(dst: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dst as usize) < src as usize {
        memcpy(dst, src, n);
    } else {
        for i in (0..n).rev() {
            dst.add(i).write_volatile(src.add(i).read_volatile());
        }
    }
    dst
}
#[no_mangle]
unsafe extern "C" fn memset(dst: *mut u8, value: i32, n: usize) -> *mut u8 {
    for i in 0..n {
        dst.add(i).write_volatile(value as u8);
    }
    dst
}
#[no_mangle]
unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let x = a.add(i).read_volatile();
        let y = b.add(i).read_volatile();
        if x != y {
            return x as i32 - y as i32;
        }
    }
    0
}
#[no_mangle]
unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while s.add(n).read_volatile() != 0 {
        n += 1;
    }
    n
}
// Precompiled alloc contains an exception table. The runtime does not unwind.
#[no_mangle]
extern "C" fn __CxxFrameHandler3() -> ! {
    unsafe { ExitProcess(2) }
}
#[cfg(target_arch = "x86")]
fn unsigned_divide(n: u64, d: u64) -> u64 {
    if d == 0 {
        unsafe { ExitProcess(2) }
    }
    let mut q = 0u64;
    let mut r = 0u64;
    for i in (0..64).rev() {
        let carry = r >> 63;
        r = (r << 1) | ((n >> i) & 1);
        if carry != 0 || r >= d {
            r = r.wrapping_sub(d);
            q |= 1u64 << i;
        }
    }
    q
}
#[cfg(target_arch = "x86")]
#[no_mangle]
extern "C" fn hangar_udiv(a: u64, b: u64) -> u64 {
    unsigned_divide(a, b)
}
#[cfg(target_arch = "x86")]
#[no_mangle]
extern "C" fn hangar_sdiv(a: i64, b: i64) -> i64 {
    let q = unsigned_divide(a.unsigned_abs(), b.unsigned_abs());
    if (a < 0) != (b < 0) {
        q.wrapping_neg() as i64
    } else {
        q as i64
    }
}
#[cfg(target_arch = "x86")]
#[no_mangle]
extern "C" fn hangar_urem(a: u64, b: u64) -> u64 {
    a.wrapping_sub(unsigned_divide(a, b).wrapping_mul(b))
}
#[cfg(target_arch = "x86")]
#[no_mangle]
extern "C" fn hangar_srem(a: i64, b: i64) -> i64 {
    let r = hangar_urem(a.unsigned_abs(), b.unsigned_abs()) as i64;
    if a < 0 {
        r.wrapping_neg()
    } else {
        r
    }
}
// MSVC's x86 64-bit divide helpers pop their arguments, unlike a C function.
#[cfg(target_arch = "x86")]
core::arch::global_asm!(
    ".global __allrem",
    "__allrem:",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "call _hangar_srem",
    "add esp, 16",
    "ret 16",
    ".global __aullrem",
    "__aullrem:",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "call _hangar_urem",
    "add esp, 16",
    "ret 16",
    ".global __alldiv",
    "__alldiv:",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "call _hangar_sdiv",
    "add esp, 16",
    "ret 16",
    ".global __aulldiv",
    "__aulldiv:",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "push dword ptr [esp + 16]",
    "call _hangar_udiv",
    "add esp, 16",
    "ret 16",
);

#[repr(C)]
struct FindData {
    attributes: u32,
    times: [u32; 6],
    size_high: u32,
    size_low: u32,
    reserved: [u32; 2],
    name: [u8; 260],
    alternate: [u8; 14],
}
#[cfg_attr(not(target_arch = "x86"), link(name = "kernel32", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "kernel32",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
unsafe extern "system" {
    fn GetCurrentDirectoryA(len: u32, buffer: *mut u8) -> u32;
    fn GetLogicalDriveStringsA(len: u32, buffer: *mut u8) -> u32;
    fn FindFirstFileA(pattern: *const c_char, data: *mut FindData) -> Handle;
    fn FindNextFileA(handle: Handle, data: *mut FindData) -> i32;
    fn FindClose(handle: Handle) -> i32;
    fn GetFileAttributesA(name: *const c_char) -> u32;
    fn GetModuleFileNameA(module: Handle, name: *mut u8, len: u32) -> u32;
}
#[cfg_attr(not(target_arch = "x86"), link(name = "winmm", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(name = "winmm", kind = "raw-dylib", import_name_type = "undecorated")
)]
unsafe extern "system" {
    fn PlaySoundA(sound: *const u8, module: Handle, flags: u32) -> i32;
}
pub fn current_dir() -> String {
    let mut b = [0u8; 260];
    let n = unsafe { GetCurrentDirectoryA(260, b.as_mut_ptr()) } as usize;
    if n == 0 || n >= 260 {
        "C:\\".into()
    } else {
        String::from_utf8_lossy(&b[..n]).into_owned()
    }
}
pub fn roots() -> Vec<String> {
    let mut b = [0u8; 128];
    let n = unsafe { GetLogicalDriveStringsA(128, b.as_mut_ptr()) } as usize;
    if n >= 128 {
        return vec![];
    }
    b[..n]
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}
pub fn is_dir(name: &str) -> bool {
    let Ok(name) = path(name) else {
        return false;
    };
    let attr = unsafe { GetFileAttributesA(name.as_ptr()) };
    attr != u32::MAX && attr & 16 != 0
}
pub fn list_dir(folder: &str) -> Result<Vec<crate::ui::FileItem>> {
    unsafe {
        let pattern = path(&format!("{}\\*", folder.trim_end_matches(['/', '\\'])))?;
        let mut data: FindData = core::mem::zeroed();
        let handle = FindFirstFileA(pattern.as_ptr(), &mut data);
        if handle as isize == -1 {
            if GetLastError() == 2 && is_dir(folder) {
                return Ok(Vec::new());
            }
            return Err(error("Cannot browse folder"));
        }
        let mut files = Vec::new();
        loop {
            let raw = data.name.split(|c| *c == 0).next().unwrap_or(&[]);
            if let Ok(name) = core::str::from_utf8(raw) {
                if name != "." && name != ".." && name.is_ascii() {
                    files.push(crate::ui::FileItem {
                        name: name.into(),
                        path: format!("{}\\{}", folder.trim_end_matches(['/', '\\']), name),
                        directory: data.attributes & 16 != 0,
                    });
                }
            }
            if files.len() > 8192 {
                FindClose(handle);
                return Err("Directory exceeds 8192 items; choose a smaller folder".into());
            }
            if FindNextFileA(handle, &mut data) == 0 {
                let e = GetLastError();
                FindClose(handle);
                if e != 18 {
                    return Err(format!("Folder enumeration failed: {e}"));
                }
                break;
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
}
fn sidecar_path(name: &str) -> String {
    let mut b = [0u8; 260];
    let n = unsafe { GetModuleFileNameA(ptr::null_mut(), b.as_mut_ptr(), 260) } as usize;
    if n == 0 || n >= 260 {
        return String::new();
    }
    let s = String::from_utf8_lossy(&b[..n]);
    let dir = s.rsplit_once('\\').map_or(".", |(d, _)| d);
    format!("{dir}\\{name}")
}
fn load_paths(name: &str, count: usize) -> Vec<String> {
    let p = sidecar_path(name);
    if p.is_empty() {
        return vec![];
    }
    read(&p)
        .ok()
        .filter(|b| b.len() <= 16384)
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.lines().take(count).map(String::from).collect())
        .unwrap_or_default()
}
fn save_paths(name: &str, paths: &[String]) -> Result<()> {
    let p = sidecar_path(name);
    let name = path(&p)?;
    let data = paths.join("\r\n");
    unsafe {
        let f = CreateFileA(
            name.as_ptr(),
            0x40000000,
            0,
            ptr::null_mut(),
            2,
            0x80,
            ptr::null_mut(),
        );
        if f as isize == -1 {
            return Err(error("Recent-file list is read-only"));
        }
        let mut n = 0;
        let ok = WriteFile(
            f,
            data.as_ptr().cast(),
            data.len() as u32,
            &mut n,
            ptr::null_mut(),
        );
        CloseHandle(f);
        if ok == 0 || n as usize != data.len() {
            return Err(error("Cannot save recent files"));
        }
    }
    Ok(())
}
pub fn load_recent() -> Vec<String> {
    load_paths("tore-hangar-recent.txt", 8)
}
pub fn save_recent(paths: &[String]) -> Result<()> {
    save_paths("tore-hangar-recent.txt", paths)
}
pub fn load_decals() -> Vec<String> {
    load_paths("tore-hangar-decals.txt", 16)
}
pub fn save_decals(paths: &[String]) -> Result<()> {
    save_paths("tore-hangar-decals.txt", paths)
}

static mut AUDIO_DATA: *mut Vec<u8> = ptr::null_mut();
pub fn stop_audio() {
    unsafe {
        PlaySoundA(ptr::null(), ptr::null_mut(), 0);
        if !AUDIO_DATA.is_null() {
            drop(Box::from_raw(AUDIO_DATA));
            AUDIO_DATA = ptr::null_mut();
        }
    }
}
pub fn play_audio(wav: Vec<u8>) -> Result<()> {
    stop_audio();
    unsafe {
        AUDIO_DATA = Box::into_raw(Box::new(wav));
        if PlaySoundA(
            (*AUDIO_DATA).as_ptr(),
            ptr::null_mut(),
            0x0001 | 0x0004 | 0x0002,
        ) == 0
        {
            stop_audio();
            return Err("Windows could not play this PCM clip".into());
        }
    }
    Ok(())
}

#[cfg_attr(not(target_arch = "x86"), link(name = "kernel32", kind = "raw-dylib"))]
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "kernel32",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
unsafe extern "system" {
    fn SetFilePointer(file: Handle, offset: i32, high: *mut i32, origin: u32) -> u32;
}
pub fn file_size(name: &str) -> Result<usize> {
    let name = path(name)?;
    unsafe {
        let file = CreateFileA(
            name.as_ptr(),
            0x80000000,
            1,
            ptr::null_mut(),
            3,
            0x80,
            ptr::null_mut(),
        );
        if file as isize == -1 {
            return Err(error("Cannot open source LIB"));
        }
        let mut high = 0;
        let n = GetFileSize(file, &mut high);
        CloseHandle(file);
        if high != 0 || n > i32::MAX as u32 {
            return Err("Source LIB exceeds 2 GiB or cannot be measured".into());
        }
        Ok(n as usize)
    }
}
pub fn read_range(name: &str, at: usize, size: usize) -> Result<Vec<u8>> {
    if size > hangar_core::archive::RESOURCE_LIMIT * 2 + 4 || at > i32::MAX as usize {
        return Err("Source range exceeds limit".into());
    }
    let name = path(name)?;
    unsafe {
        let file = CreateFileA(
            name.as_ptr(),
            0x80000000,
            1,
            ptr::null_mut(),
            3,
            0x80,
            ptr::null_mut(),
        );
        if file as isize == -1 {
            return Err(error("Cannot open source LIB"));
        }
        if SetFilePointer(file, at as i32, ptr::null_mut(), 0) == u32::MAX {
            let e = error("Cannot seek source LIB");
            CloseHandle(file);
            return Err(e);
        }
        let mut out = vec![0; size];
        let mut cursor = 0;
        while cursor < size {
            let mut n = 0;
            if ReadFile(
                file,
                out[cursor..].as_mut_ptr().cast(),
                (size - cursor) as u32,
                &mut n,
                ptr::null_mut(),
            ) == 0
                || n == 0
            {
                let e = error("Cannot read source resource");
                CloseHandle(file);
                return Err(e);
            }
            cursor += n as usize;
        }
        CloseHandle(file);
        Ok(out)
    }
}

pub fn remove_file(name: &str) -> Result<()> {
    let name = path(name)?;
    if unsafe { DeleteFileA(name.as_ptr()) } == 0 {
        Err(error("Cannot remove test file"))
    } else {
        Ok(())
    }
}
