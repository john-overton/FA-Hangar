# Compatibility and acceptance

The requested minimum CPU is Pentium II (John, 2026-10-09; it was Pentium
4/SSE2 until 0.9.0). The 32-bit build uses no SSE or SSE2, so Pentium II/III,
Celeron, Athlon and later CPUs are in range; Pentium, Pentium MMX and K6
(no `cmov`) are not.

| Build | Intended OS | Evidence |
| --- | --- | --- |
| i686-win98-windows-msvc (custom target, Pentium II), custom runtime | Windows 98, 98 SE, ME; also modern x64 Windows through WOW64 | Cross-link and PE/import audit; CI smoke on modern Windows; original OS test pending |
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

The 32-bit build uses the custom target `targets/i686-win98-windows-msvc.json`,
a copy of `i686-pc-windows-msvc` with CPU `pentium2` and without its
`x86-sse2` float ABI. The stock i686 target assumes a Pentium 4, and its
prebuilt `core`/`alloc` contain SSE2 even when the application is built with
an older `-C target-cpu`, so they are rebuilt for the custom target with
`-Zbuild-std` from `rust-src`. Because the target has no prebuilt library, a
build without `-Zbuild-std` fails rather than mixing in SSE2 code. Hangar uses
no floating point, so the x87 float ABI is unused in practice. `RUSTC_BOOTSTRAP=1`
enables `-Zbuild-std` on the pinned stable compiler; a toolchain bump must
recheck that the custom target and `-Zbuild-std` still build.

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
- Retail LIB filenames are reserved across directories and letter case. Custom
  LIBs are staged (`<STEM>.TMP`) and flushed, then the old file is moved to an
  unused numbered `<STEM>.BAK`/`.B01`..`.B99` name before installing the
  replacement. No backup or stage name contains `.LIB`, which FA would load
  as another LIB. Failed installation attempts
  restore the backup; a failed restore reports both recovery paths. Abrupt power
  failure between moves can require manually restoring the backup. Raw resource
  exports remain create-new. Read-only files and symlink/reparse outputs are
  rejected; no existing file is truncated.
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
path/drive queries, `MoveFileA` for same-directory non-replacing moves, `SetFilePointer` for bounded source reads, `StretchDIBits` and `PlaySoundA`, all from the existing Win32
API family. The audio buffer stays owned until playback is stopped before
release ([PlaySound memory lifetime](https://learn.microsoft.com/en-us/previous-versions/dd743680(v=vs.85))).
No WinMM DLL is bundled; Windows supplies it.

## Manual legacy acceptance still required

On Windows 98/98 SE and ME, on a Pentium II-class or newer CPU or VM:

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
