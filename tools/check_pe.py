#!/usr/bin/env python3
"""Reject incompatible headers/imports in the no-CRT Windows executables, and
check the app icon and version resources that crates/hangar-app/build.rs links in."""
import argparse
import struct
from pathlib import Path

# Reviewed ANSI APIs available in the Windows 98/ME SDK family. This is an
# import gate, not a substitute for executing the app on the original OS.
ALLOWED = {
    'winmm.dll': {'PlaySoundA'},
    'kernel32.dll': set('CloseHandle CreateFileA DeleteFileA MoveFileA ExitProcess FlushFileBuffers GetCommandLineA GetFileSize GetLastError GetModuleHandleA GetProcessHeap HeapAlloc HeapFree ReadFile WriteFile GetCurrentDirectoryA GetLogicalDriveStringsA FindFirstFileA FindNextFileA FindClose GetFileAttributesA GetModuleFileNameA SetFilePointer'.split()),
    'user32.dll': set('BeginPaint CreateWindowExA DefWindowProcA DestroyWindow DispatchMessageA EndPaint FillRect GetClientRect GetKeyState GetMessageA InvalidateRect LoadCursorA MessageBoxA PostQuitMessage RegisterClassA ReleaseCapture SetCapture ShowWindow TranslateMessage UpdateWindow'.split()),
    'gdi32.dll': set('BitBlt CreateCompatibleBitmap CreateCompatibleDC CreateFontA CreatePen CreateSolidBrush DeleteDC DeleteObject LineTo MoveToEx SelectObject SetBkMode SetTextColor TextOutA StretchDIBits'.split()),
}

def audit(path, legacy, extract_icon=None):
    data = Path(path).read_bytes()
    def u16(at): return struct.unpack_from('<H', data, at)[0]
    def u32(at): return struct.unpack_from('<I', data, at)[0]
    if data[:2] != b'MZ': raise ValueError('Missing MZ header')
    pe = u32(60)
    if data[pe:pe+4] != b'PE\0\0': raise ValueError('Missing PE signature')
    optional = pe + 24
    bits = {0x10b: 32, 0x20b: 64}.get(u16(optional))
    if bits != (32 if legacy else 64): raise ValueError('Unexpected architecture')
    if u16(pe+4) != (0x14c if legacy else 0x8664): raise ValueError('Unexpected machine')
    if u16(optional+68) != 2: raise ValueError('Expected Windows GUI subsystem')
    if legacy:
        if (u16(optional+40), u16(optional+42)) != (4, 0): raise ValueError('Legacy OS version must be 4.0')
        if (u16(optional+48), u16(optional+50)) != (4, 0): raise ValueError('Legacy subsystem must be 4.0')
        if u16(optional+70) & (0x40 | 0x100): raise ValueError('Legacy ASLR/NX flags must be clear')
    sections = []
    at = optional + u16(pe+20)
    for i in range(u16(pe+6)):
        s = at + i*40
        sections.append((u32(s+12), max(u32(s+8), u32(s+16)), u32(s+20)))
    def offset(rva):
        for va, length, start in sections:
            if va <= rva < va+length: return start+rva-va
        raise ValueError(f'RVA outside sections: {rva:x}')
    def string(at):
        end = data.index(0, at)
        return data[at:end].decode('ascii')
    directories = optional + (96 if bits == 32 else 112)
    if u32(directories+9*8): raise ValueError('Static TLS is not supported by the legacy runtime')
    if u32(directories+13*8): raise ValueError('Unexpected delay imports')
    imports = offset(u32(directories+8))
    found = {}
    for i in range(64):
        at = imports+i*20
        lookup, timestamp, chain, name, first = struct.unpack_from('<IIIII', data, at)
        if not any((lookup,timestamp,chain,name,first)): break
        dll = string(offset(name)).lower()
        if dll not in ALLOWED: raise ValueError(f'Unexpected runtime dependency: {dll}')
        thunk = offset(lookup or first)
        names = []
        for j in range(2048):
            value = struct.unpack_from('<I' if bits == 32 else '<Q', data, thunk+j*(bits//8))[0]
            if not value: break
            if value >> (bits-1): raise ValueError('Ordinal import cannot be audited')
            symbol = string(offset(value)+2)
            if symbol not in ALLOWED[dll]: raise ValueError(f'Unreviewed import: {dll}!{symbol}')
            names.append(symbol)
        else: raise ValueError('Unterminated import list')
        found[dll] = names
    else: raise ValueError('Unterminated DLL list')
    if set(found) != set(ALLOWED): raise ValueError(f'Unexpected DLL set: {found.keys()}')
    # Resources: type -> id -> language, ordinal ids only.
    res, root = {}, u32(directories+2*8)
    def walk(rva, path):
        at = offset(root+rva)
        named, ids = u16(at+12), u16(at+14)
        for k in range(named+ids):
            ident, target = struct.unpack_from('<II', data, at+16+k*8)
            key = path + (None if ident >> 31 else ident,)
            if target >> 31:
                if len(key) < 3: walk(target & 0x7fffffff, key)
            else:
                rva, size = struct.unpack_from('<II', data, offset(root+target))
                res.setdefault(key[:2], data[offset(rva):offset(rva)+size])
    if root: walk(0, ())
    # LoadIconA(hInstance, 1) needs icon group 1; Windows 98/ME needs an 8-bit
    # DIB at every DIB size (it cannot use the 32-bit alpha or PNG entries).
    group = res.get((14, 1))
    if not group: raise ValueError('Missing app icon resource (RT_GROUP_ICON 1)')
    images, kinds = [], set()
    for k in range(struct.unpack_from('<H', group, 4)[0]):
        entry = group[6+k*14:6+k*14+14]
        w, _, _, _, _, bpp, size, ident = struct.unpack('<BBBBHHIH', entry)
        image = res.get((3, ident))
        if image is None or len(image) != size: raise ValueError(f'App icon image {ident} missing or truncated')
        png = image[:8] == b'\x89PNG\r\n\x1a\n'
        if not png and (len(image) < 40 or struct.unpack_from('<IiiHH', image)[0::4] != (40, bpp)):
            raise ValueError(f'App icon image {ident} is not a {bpp}-bit DIB')
        images.append((entry[:12], image))
        kinds.add((w or 256, 'png' if png else bpp))
    dib_sizes = {w for w, kind in kinds if kind != 'png'}
    if not dib_sizes or any((w, 8) not in kinds for w in dib_sizes): raise ValueError('App icon lacks 8-bit DIBs for Windows 98/ME')
    sizes = ' '.join(f'{w}:' + '/'.join(sorted(str(k) for v, k in kinds if v == w)) for w in sorted({w for w, _ in kinds}))
    version = res.get((16, 1), b'')
    fixed = version.find(struct.pack('<I', 0xfeef04bd))
    if fixed < 0: raise ValueError('Missing VERSIONINFO resource')
    ms, ls = struct.unpack_from('<II', version, fixed+8)
    if extract_icon:
        out = struct.pack('<HHH', 0, 1, len(images))
        body = b''
        for head, image in images:
            out += head + struct.pack('<I', 6+16*len(images)+len(body))
            body += image
        Path(extract_icon).write_bytes(out + body)
    print(f'PASS {Path(path).name}: PE{bits}, {len(data):,} bytes, {sum(map(len,found.values()))} reviewed imports, no runtime DLLs, '
          f'icon {sizes}, version {ms>>16}.{ms&0xffff}.{ls>>16}')
    return found

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--legacy', action='store_true')
    parser.add_argument('--extract-icon', type=Path, metavar='ICO', help='write the embedded app icon back out as an .ico')
    args = parser.parse_args()
    audit(args.executable, args.legacy, args.extract_icon)
