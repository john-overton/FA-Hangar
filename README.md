<p align="center"><img src="docs/FAHANGAR.png" alt="F.A. Hangar: a jet in Union Jack livery, half rendered as a wireframe, over sunset clouds" width="100%"></p>

<h1><p align="center">
  <a href="https://github.com/john-overton/FA-Hangar/actions/workflows/windows.yml"><img src="https://img.shields.io/github/actions/workflow/status/john-overton/FA-Hangar/windows.yml?event=release&label=release%20build" alt="Release build status"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-GPL--3.0--only-blue" alt="License: GPL-3.0-only"></a>
  <a href="docs/CHANGELOG.md"><img src="https://img.shields.io/badge/version-0.9.2-orange" alt="Version 0.9.2"></a>
  <a href="rust-toolchain.toml"><img src="https://img.shields.io/badge/rust-1.91.1-b7410e?logo=rust" alt="Rust 1.91.1"></a>
  <a href="docs/COMPATIBILITY.md"><img src="https://img.shields.io/badge/platforms-Windows%2098%2FME%20%C2%B7%20Win64%20%C2%B7%20Linux%20X11-555" alt="Platforms: Windows 98/ME, Win64, Linux X11"></a>
  <a href="docs/ARCHITECTURE.md"><img src="https://img.shields.io/badge/core-no__std-6c757d" alt="no_std core"></a>
</p>
</h1>

A small, standalone Rust workshop for Fighters Anthology LIB files, built toward
a Blender-light workflow for the game's models and their related resources. Native
Win32/GDI on Windows, X11/Xwayland for local Linux development. The supplied
[design system](fa-hangar-design/README.md) supplies the Gunmetal colors and
Blender-inspired workspace.

**Early working editor, not a complete SH authoring tool.** LIB and textual
BRF editing work. Aircraft geometry is edited per region where Hangar can
prove the change safe, and moving parts change only through a catalog of
same-size settings, with forms not seen in retail labelled until tested in
the game. No game data ships.

## Highlights

- Open multiple EALIB archives at once; search, inspect, add, remove, replace,
  export, copy and move resources between LIBs with reviewed dependencies.
- Edit PT/NT/JT/OT/SEE/ECM textual BRF operands in source storage units, with
  saved-vs-current values, structured groups and an envelope table.
- Graft characteristic groups from a donor definition as one undo step.
- Export an aircraft, weapon or other object with its resources into a new,
  privately named LIB. Names no LIB provides are kept as in the source, or a
  texture is retargeted, after an explicit review.
- Edit an object's short and long names in place, rename an aircraft's
  reference ID with its private files, or duplicate it in the same LIB with
  a Copy/Share review per resource.
- View SH static poses and place hardpoints. Edit Mesh selects vertices or
  faces (click, box, part) and moves, scales, deletes, flips, duplicates,
  extrudes and adds geometry on retail aircraft, region by region, with a
  Blender-style move/rotate/scale gizmo and magnetic vertex snapping.
- List moving parts (gear, flaps, rudder, hook, brakes, bays, afterburner,
  swing wings, canards), preview their states, and change their gate values,
  gear swing range and direction, rotation axis and pivot.
- Paint PIC textures on the atlas or directly on the model, replace a color
  with a brush or across a texture or panel, erase back to or
  restore the original artwork, edit palettes and UVs, and bake PNG,
  tail-number and national decals. Give selected panels their own PIC
  (cloned, or any PIC with kept, scaled or projected UVs) while the rest of
  the aircraft keeps its atlas, or remap stretched panels from the view onto
  a new texture with square texels, baked from their current look.
- Hide, move or reassign the markings the game fills in at run time (national
  roundels, tail and nose art), or make them a texture of your own.
- Preview PCM audio and PIC images; export WAV, PNG and geometry-only OBJ.
- Lossless by default: unedited archives round-trip byte-for-byte, retail LIB
  names are protected and custom saves keep numbered backups.

The full feature list and how-to guides are in the [manual](docs/MANUAL.md).

## Download and build

### Download

Each [GitHub release](https://github.com/john-overton/FA-Hangar/releases)
carries two ZIPs with the version in their names:
`fa-hangar-<version>-win98-me-pentium2.zip` and
`fa-hangar-<version>-win64.zip`, each holding `fa-hangar-<version>.exe`,
`SHA256.txt` and the documentation. The same packages are workflow artifacts of
[GitHub Actions](.github/workflows/windows.yml), which runs only on Windows and
only on demand (Actions, Run workflow) or when a release is published, so the
build badge shows the last release build and reads "no status" until the first
one. **Help > About** in the app shows the version. To cut a release, see
[docs/RELEASING.md](docs/RELEASING.md).
The legacy executable's smoke test runs on the modern Windows runner, not on
Windows 98. Source and notices are included in the repository; packages carry
the license and notices alongside the executable.

The 32-bit build targets Windows 98/ME on **Pentium II or newer** (no SSE
required; Pentium III, Pentium 4 and Athlon also work). The 64-bit
build targets modern Windows. Each is a portable executable of about 2 MiB,
including the built-in `--smoke-test` self-check so a release build can be
checked on the target machine. Copy it to a writable location and run it. No installer or runtime
DLL is required. Windows file paths are ASCII in this first version.

**Windows 98/ME runtime compatibility remains unverified.** The executable
header and imports are audited, but an actual OS/hardware test is still
required. See [compatibility and acceptance](docs/COMPATIBILITY.md).

### Run locally on Linux

Rust 1.91.1 is pinned. Install X11 development libraries (Arch: `libx11`,
Debian/Ubuntu: `libx11-dev`) and use an X11 session or Xwayland.

```sh
cargo run --locked -- --demo
cargo run --locked -- /path/to/FA_2.LIB
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked -- --smoke-test
```

PNG decoding uses the small no_std Rust crates miniz_oxide and adler2,
compiled into the executable. No Python,
Blender, OpenFA, web browser, GPU runtime or network access is needed by the
Windows editor. Python is used only for development checks.

The Linux executable also supplies a CLI; see
[Command line](docs/MANUAL.md#command-line).

### Windows builds

Both targets build on Linux without a Windows SDK, using Rust's bundled
linker and generated import libraries:

```sh
rustup target add x86_64-pc-windows-msvc
RUSTC_BOOTSTRAP=1 cargo build --release --locked -Zbuild-std=core,alloc \
  --target targets/i686-win98-windows-msvc.json
cargo build --release --locked --target x86_64-pc-windows-msvc
python3 tools/check_pe.py --legacy target/i686-win98-windows-msvc/release/fa-hangar.exe
python3 tools/check_pe.py target/x86_64-pc-windows-msvc/release/fa-hangar.exe
```

The 32-bit build uses the custom target
[`targets/i686-win98-windows-msvc.json`](targets/i686-win98-windows-msvc.json):
`i686-pc-windows-msvc` with CPU `pentium2` and the x87 float ABI, so the
compiler emits no SSE or SSE2. Rust ships no prebuilt `core`/`alloc` for it,
so `-Zbuild-std` rebuilds them from the `rust-src` component (listed in
`rust-toolchain.toml`); `RUSTC_BOOTSTRAP=1` allows that flag on the pinned
stable toolchain. Clippy for this target takes the same flags.

The audit also checks the embedded app icon and version resources;
`--extract-icon OUT.ico` writes the icon back out.

## Documentation

| Document | Contents |
| --- | --- |
| [Manual](docs/MANUAL.md) | Opening LIBs, workspaces, saving, CLI, export, painting, hardpoints, controls, limits |
| [Changelog](docs/CHANGELOG.md) | User-visible changes by version |
| [Architecture](docs/ARCHITECTURE.md) | Layers, format evidence, writing policy and remaining writer work |
| [Validation](docs/VALIDATION.md) | What each test build was verified against |
| [Compatibility](docs/COMPATIBILITY.md) | Platform limits and legacy acceptance still required |
| [Releasing](docs/RELEASING.md) | Version bump, tag and the release workflow |
| [Windows test](docs/WINDOWS-TEST.md) | Manual Windows and game acceptance checklist |
| [Design system](fa-hangar-design/README.md) | Gunmetal tokens, components and [brand voice](fa-hangar-design/BRAND.md) |

## Scope still ahead

A complete animated SH writer, new animated parts, BSP-aware placement of new faces,
topology-aware UV unwrapping, animation playback, complete runtime dependency
closure, geometric grafting, verified gameplay-unit conversions,
resizable editor splits and bitmap fonts.
There are no inactive timeline controls pretending these features work.
Current limits of the shipped tools, and the reason for each, are listed in
the [manual](docs/MANUAL.md#limits-and-why).

## Structure and provenance

`hangar-core` contains portable `no_std` + `alloc` formats and editing history.
`fa-hangar` contains shared UI logic plus small native platform backends.
[Architecture and format references](docs/ARCHITECTURE.md) records the decisions
and remaining writer work.

## License

[Third-party notices](THIRD_PARTY_NOTICES.md) covers the adapted GPL source and
DCL decoder. Code is [GPL-3.0-only](LICENSE); user-owned game content is not
included or relicensed.
