#!/usr/bin/env python3
"""Reject incompatible headers/imports in the no-CRT Windows executables."""
import argparse
import struct
from pathlib import Path

# Reviewed ANSI APIs available in the Windows 98/ME SDK family. This is an
# import gate, not a substitute for executing the app on the original OS.
ALLOWED = {
    'winmm.dll': {'PlaySoundA'},
    'kernel32.dll': set('CloseHandle CreateFileA DeleteFileA ExitProcess FlushFileBuffers GetCommandLineA GetFileSize GetLastError GetModuleHandleA GetProcessHeap HeapAlloc HeapFree ReadFile WriteFile GetCurrentDirectoryA GetLogicalDriveStringsA FindFirstFileA FindNextFileA FindClose GetFileAttributesA GetModuleFileNameA'.split()),
    'user32.dll': set('BeginPaint CreateWindowExA DefWindowProcA DestroyWindow DispatchMessageA EndPaint FillRect GetClientRect GetKeyState GetMessageA InvalidateRect LoadCursorA MessageBoxA PostQuitMessage RegisterClassA ReleaseCapture SetCapture ShowWindow TranslateMessage UpdateWindow'.split()),
    'gdi32.dll': set('BitBlt CreateCompatibleBitmap CreateCompatibleDC CreateFontA CreatePen CreateSolidBrush DeleteDC DeleteObject LineTo MoveToEx SelectObject SetBkMode SetTextColor TextOutA StretchDIBits'.split()),
}

def audit(path, legacy):
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
    print(f'PASS {Path(path).name}: PE{bits}, {len(data):,} bytes, {sum(map(len,found.values()))} reviewed imports, no runtime DLLs')
    return found

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--legacy', action='store_true')
    args = parser.parse_args()
    audit(args.executable, args.legacy)
