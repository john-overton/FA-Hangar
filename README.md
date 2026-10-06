# T.O.R.E Hangar

A small, standalone Rust workshop for Fighters Anthology LIB files. Native
Win32/GDI on Windows, X11/Xwayland for local Linux development. The supplied
[design system](tore-hangar-design/README.md) supplies the Gunmetal colors and
Blender-inspired workspace.

**Early working editor, not a complete SH authoring tool.** LIB and textual
BRF editing work; general animated aircraft geometry remains read-only until
its spatial and control records can be rewritten safely. No game data ships.

The 0.2 UI follows the supplied concept: one menu/workspace bar, categorized
outliner with type icons, linked aircraft/shape selection, grouped source-value
properties, and Browse/Model/Flight/Graft/Package workspaces. Raw fields show
saved values beside current values, with amber edits and reset controls.
Windows uses Tahoma for interface labels and Lucida Console for resource data.

## What works

- Open EALIB archives, search entries, inspect bytes, add, remove, replace and
  export resources. Stored and raw-literal DCL-compressed entries are readable.
- Package a new LIB with the required EOF sentinel. Untouched entry payloads
  keep their original compression and bytes. An unedited archive round-trips
  byte-for-byte, including padding. Modified entries are stored uncompressed.
- Edit PT/JT/OT/SEE/ECM textual BRF operands. Recognized schemas get named
  fields; other blocks retain their indexed labels. Comments, whitespace,
  line endings, labels, scaling markers and untouched values survive edits.
- Create a new aircraft from an imported SH and a compatible PT donor. The
  wizard rewrites its identity and main/shadow references, clones the donor's
  damage/shadow family, and copies textures observed in the static pose.
- Copy a selected field from the same named entry in another LIB, with an
  editable value preview before applying it. Entry replacement can transfer
  complete objects through export/import.
- View a bounded static pose from SH data, orbit/pan/zoom, use front/side/top
  views, and export geometry-only OBJ files.
- Move, rotate and scale the supported straight-line SH subset. Shapes with
  unhandled spatial records, bounds, visibility logic or animation remain
  read-only. The synthetic demo exercises transforms without retail data.
- Entry-level undo/redo, dirty state, explicit discard on close, and packaging
  to a **new** file. Existing files are never overwritten.

This version uses numeric **source storage values**, not invented conversions
to knots, pounds or Mach. A `^` marker is retained, not silently reinterpreted.
Changing a field is not a guarantee of how the original game consumes it.
Opaque binary definitions can be exported or replaced but are not guessed.

## Run locally on Linux

Rust 1.91.1 is pinned. Install X11 development libraries (Arch: `libx11`,
Debian/Ubuntu: `libx11-dev`) and use an X11 session or Xwayland.

```sh
cargo run --locked -- --demo
cargo run --locked -- /path/to/FA_2.LIB
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --locked -- --smoke-test
```

No Rust dependencies beyond the two local workspace crates. No Python,
Blender, OpenFA, web browser, GPU runtime or network access is needed by the
Windows editor. Python is used only for development checks.

The Linux executable also supplies a CLI. `--help` lists commands for archive
inspection, extraction, repacking, replacement and field editing. For example:

```sh
cargo run --locked -- list /path/to/FA_2.LIB
cargo run --locked -- inspect /path/to/FA_2.LIB F18.PT
cargo run --locked -- set /path/to/FA_2.LIB F18.PT 35 23051 /new/path/MYMOD.LIB
cargo run --locked -- --snapshot /new/path/workspace.svg
```

Use `inspect` to confirm field indices and values for your own file first.
The example changes the recognized object weight operand, in its source units.

## Windows builds

Both targets build on Linux without a Windows SDK, using Rust's bundled
linker and generated import libraries:

```sh
rustup target add i686-pc-windows-msvc x86_64-pc-windows-msvc
cargo build --release --locked --target i686-pc-windows-msvc
cargo build --release --locked --target x86_64-pc-windows-msvc
python3 tools/check_pe.py --legacy target/i686-pc-windows-msvc/release/tore-hangar.exe
python3 tools/check_pe.py target/x86_64-pc-windows-msvc/release/tore-hangar.exe
```

The 32-bit build targets Windows 98/ME on **Pentium 4/SSE2 or newer**. The 64-bit
build targets modern Windows. Each is a portable executable, currently about
160–200 KB. Copy it to a writable location and run it. No installer or runtime
DLL is required. Windows file paths are ASCII in this first version.

**Windows 98/ME runtime compatibility remains unverified.** The executable
header and imports are audited, but an actual OS/hardware test is still
required. See [compatibility and acceptance](docs/COMPATIBILITY.md).

[GitHub Actions](.github/workflows/windows.yml) runs only on Windows and emits
separate `tore-hangar-win98-me-pentium4` and `tore-hangar-win64` ZIP artifacts.
The legacy executable's smoke test runs on the modern Windows runner, not on
Windows 98. Source and notices are included in the repository; packages carry
the license and notices alongside the executable.

## Create an aircraft from an SH

1. Open the donor LIB and select its PT entry. Save any edits first.
2. Click **New aircraft**, enter the imported SH path, a unique ID of up to
   six characters, and its display name.
3. Review the draft and type `CREATE`. This starts a separate in-memory LIB.
   The donor archive stays untouched.
4. Edit the new PT's fields. Add any missing textures with **Add entry**.
5. **Package LIB** to a new path, then reopen it to inspect the result.

The imported main SH is preserved byte-for-byte. The A/B/C/D damaged/fragment
shapes and S shadow are unchanged donor geometry under new names. The donor
must provide the complete reviewed `_S.SH` family; other conventions are
rejected. Flight, hardpoints, cockpit/equipment and availability remain donor
values until edited. Stock dependencies remain shared with the original game.
Known missing main-pose textures block packaging until supplied. Dependencies
in unvisited animations/LODs are not yet exhaustively checked.

This creates a **mod candidate**, not a guarantee that any arbitrary SH will
fly correctly. Generic OT object creation and geometric part grafting are not
implemented. See [the Windows test checklist](docs/WINDOWS-TEST.md).

## Controls

| Action | Control |
| --- | --- |
| Open LIB / package new LIB | Ctrl+O / Ctrl+S, or File menu |
| Add entry / export selected entry | Ctrl+I / Ctrl+E |
| Replace / export OBJ | Inspector buttons |
| Search entry names | Ctrl+F or search field; Esc leaves search |
| Select entry | Click or Up/Down; wheel scrolls the outliner |
| Edit a definition operand | Click its field; Ctrl+A clears the input |
| Scroll fields | Wheel over inspector, Flight workspace or Raw fields |
| Copy a donor field | Select a field, open Graft, then Choose donor |
| Remove selected entry | Delete; Ctrl+Z restores it |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Orbit / pan | Middle-drag / Shift+middle-drag |
| Zoom / frame | Wheel over viewport / Home or period |
| Front / side / top / projection | 1 / 3 / 7 / 5, including numpad |
| Transform supported static shape | G / R / S, X/Y/Z, numeric value, Enter |
| Cancel transform or dialog | Esc or right mouse button |
| Close with unsaved edits | Close window; type DISCARD or Esc to return |

Scale currently acts on the selected axis in percent; rotation is in degrees,
translation in integer source coordinates. File dialogs currently use a typed
path. They do not create missing directories. A packaging error leaves edits
in memory and displays its cause. The app writes only when explicitly asked.

## Scope still ahead

A complete animated SH writer, vertex/face selection, topology changes,
texture and palette editing, hardpoint tools, animation playback, dependency
closure, richer grafting, unit-aware gameplay controls, native file pickers,
resizable editor splits and bitmap fonts.
There are no inactive timeline controls pretending these features work.

The loader never executes code from a resource. Unsupported records produce
an explicit diagnostic. The current model reader is a static-pose projection,
not the original game's full drawing interpreter. OBJ exports discard
materials and animation and cannot be imported back as a lossless SH edit.

## Structure and provenance

`hangar-core` contains portable `no_std` + `alloc` formats and editing history.
`tore-hangar` contains shared UI logic plus small native platform backends.
[Architecture and format references](docs/ARCHITECTURE.md) records the decisions
and remaining writer work. [Third-party notices](THIRD_PARTY_NOTICES.md) covers
the adapted GPL source and DCL decoder. Code is GPL-3.0-only; user-owned game
content is not included or relicensed.
