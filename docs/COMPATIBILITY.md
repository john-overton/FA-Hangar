# Compatibility and acceptance

The requested minimum CPU is Pentium 4/SSE2 (John, 2026-10-06). Older Pentium,
Pentium II/III, K6 and non-SSE2 Athlon machines are outside this target.

| Build | Intended OS | Evidence |
| --- | --- | --- |
| i686-pc-windows-msvc, custom runtime | Windows 98, 98 SE, ME; also modern x64 Windows through WOW64 | Cross-link and PE/import audit; CI smoke on modern Windows; original OS test pending |
| x86_64-pc-windows-msvc, custom runtime | Modern 64-bit Windows | Cross-link and PE/import audit; Windows CI smoke |
| x86_64-unknown-linux-gnu | Local Linux with X11/Xwayland | Core tests, shared UI smoke, CLI and snapshots |

This uses **Rust 1.91.1**, not an old Rust release with a modern standard
library hidden underneath. Official Windows Rust targets have newer OS
baselines ([Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)).
Hangar deliberately bypasses their Windows `std` runtime:

- `no_std` application and core, `alloc`, aborting panics, single UI thread.
- Own process entry point, aligned allocator backed by `GetProcessHeap`, and
  small memory/integer compiler helpers. No C runtime or Rust `std` imports.
- ANSI Win32, GDI bitmap drawing and WinMM PCM playback. No DirectX, OpenGL, DWM, COM, Unicode shim or TLS.
- Legacy PE32 OS/subsystem version 4.0, without ASLR/NX flags. These are legacy
  loader settings, not a claim that all modern exploit mitigations apply.
- Stable Rust `raw-dylib` declarations create import libraries, and bundled
  `rust-lld` links them. No proprietary historical SDK is needed to build.
  [Rust linking reference](https://doc.rust-lang.org/reference/items/external-blocks.html#the-link-attribute).

The alternative Rust9x standard-library port was considered. Its documented
MSLU/runtime requirements are unnecessary for this small ANSI application
([Rust9x build requirements](https://github.com/rust9x/docs/blob/main/building-with-r9x.md)).
The custom runtime is an agent implementation decision, not a user requirement.

`tools/check_pe.py` verifies machine type, GUI subsystem, legacy version and
flags, lack of static TLS/delay imports, and an explicit API allowlist. New
platform APIs must be reviewed before adding them. This cannot prove that all
compiler-generated instructions or runtime behavior work on Windows 98.

## Limits and portable behavior

- 128 MiB per archive/input file, 16 MiB per decoded resource, 1 MiB BRF text.
- History retains up to 64 operations, trimming changed payload history above
  32 MiB. Original archive storage is shared. Large archives still require RAM
  for input, edited resources, and packaging output; on small machines, edit a
  smaller mod LIB rather than a whole retail library.
- Output is create-new, flushed before success. A failed write removes the
  partial new output where the operating system permits it. Existing files
  are not replaced. Abrupt power failure is not a transactional save guarantee.
- No registry entries, installer or network calls. A recent-file sidecar is
  stored beside the EXE; it can be deleted to clear history. System fonts
  are used. ASCII paths on Windows; LIB entry names retain ASCII 8.3 syntax,
  including DOS punctuation such as `$` picture prefixes and `#`/`^` sounds.
- Shared UI/format logic is tested on Linux. Windows smoke exercises the
  custom allocator, integer math, drawing command generation, model transform,
  undo, directory enumeration, UV hit mapping, live texture painting and archive
  serialization without displaying a window. Audible playback needs a manual
  device check; CI does not assume an audio device. Linux uses ALSA `aplay`.

The additional Windows APIs are `FindFirstFileA`/`FindNextFileA`/`FindClose`,
path/drive queries, `StretchDIBits` and `PlaySoundA`, all from the existing Win32
API family. The audio buffer stays owned until playback is stopped before
release ([PlaySound memory lifetime](https://learn.microsoft.com/en-us/previous-versions/dd743680(v=vs.85))).
No WinMM DLL is bundled; Windows supplies it.

## Manual legacy acceptance still required

On Windows 98/98 SE and ME, using an SSE2-capable VM/CPU:

1. Run the 32-bit EXE alone in a fresh folder without compatibility shims.
2. Load Demo, orbit/pan/zoom, transform, undo, and package a synthetic LIB.
3. Reopen it; edit a field, package under another name, and compare extraction.
4. Open a user-owned retail LIB and inspect a known SH/PT pair. Export and
   replace an entry; test a full packaging operation on a writable disk.
5. Test existing output files, read-only media, malformed LIBs, low memory and
   closing with unsaved edits. Verify diagnostics and source preservation.
6. Load a deliberately authored mod in Fighters Anthology and validate the
   intended field behavior. A structurally valid archive is not game acceptance.

Do not label original-game loading or Windows 98/ME execution as passed until
these checks have actually been run and recorded.
