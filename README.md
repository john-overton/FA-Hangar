# T.O.R.E Hangar

A small, standalone Rust workshop for Fighters Anthology LIB files, built toward
a Blender-light workflow for the game's models and their related resources. Native
Win32/GDI on Windows, X11/Xwayland for local Linux development. The supplied
[design system](tore-hangar-design/README.md) supplies the Gunmetal colors and
Blender-inspired workspace.

**Early working editor, not a complete SH authoring tool.** LIB and textual
BRF editing work; general animated aircraft geometry remains read-only until
its spatial and control records can be rewritten safely. No game data ships.

The UI follows the supplied concept: one menu/workspace bar, categorized
outliner with type icons, linked aircraft/shape selection, grouped source-value
properties, and Browse/Model/Flight/Graft/Package/Paint workspaces. Raw fields show
saved values beside current values, with amber edits and reset controls.
Windows uses Tahoma for interface labels and Lucida Console for resource data.

Version 0.4.1 protects retail LIB filenames and saves custom LIBs with backups.
Version 0.4 duplicates a selected aircraft directly into its own privately named
LIB. The editor also provides an in-app file browser and recent LIBs, audio playback/WAV
export, PIC preview/PNG export, and linked model/texture painting. Recent paths
are kept in `tore-hangar-recent.txt` beside the executable; if that location is
read-only, history still works for the current session.

## What works

- Browse folders/drives and reopen recent LIBs. Click directories to enter,
  select a file, then Open; the editable path field also accepts a directory.
- Preview raw `.5K` / `.11K` PCM8 audio and PCM8 mono WAVs; Play/Stop and WAV
  export. Windows uses its built-in WinMM service. Linux playback uses `aplay`.
- Preview and export PIC images as PNG with transparency. Paint existing
  opaque raster pixels with original palette indices, including span-coded
  PICs. Headers, palettes, row/span tables and unknown bytes are preserved.
- Pick a model face, inspect its UV footprint on the named texture, and paint
  the atlas with a live model preview, or brush directly on the model. Each
  stroke is one undo operation. Zoom/pan the atlas for individual pixels.
- Clone a PIC and retarget the current shape's decoded named references in
  one undo step. Remap palette indices on decoded untextured FC faces without
  changing geometry, relative links or record sizes.
- Open EALIB archives, search entries, inspect bytes, add, remove, replace and
  export resources. Stored and raw-literal DCL-compressed entries are readable.
- Inspect stored references, direct users and transitive aircraft users in the
  References dock (Links at compact sizes). Click a resource to select it;
  opening a texture from a model retains its live model preview. Shared shapes
  show their observed direct-user count in the viewport.
- Review added, modified and removed entries in Package. Run advisory checks
  for archive integrity, untouched payload preservation, supported changed
  payloads and references outside the current LIB. Scroll results and click an
  entry to inspect it. These checks do not establish game-runtime compatibility.
- Save a new or existing custom LIB with the required EOF sentinel. Untouched entry payloads
  keep their original compression and bytes. An unedited archive round-trips
  byte-for-byte, including padding. Modified entries are stored uncompressed.
- Edit PT/JT/OT/SEE/ECM textual BRF operands. Recognized schemas get named
  fields; other blocks retain their indexed labels. Comments, whitespace,
  line endings, labels, scaling markers and untouched values survive edits.
- Duplicate the selected PT into a separate LIB with a new ID/display name.
  Resolve its resource graph recursively, copy and rename the damage/shadow
  family, textures, cockpit/HUD, equipment, weapons, sounds and private palette,
  then rewrite references. Review every old-to-new filename before exporting.
- Keep loose-SH authoring available separately under Lib > From loose SH file.
- Filter recognized definition values by envelope, propulsion, handling,
  weights, damage, hardpoints, systems, seeker, motor or warhead. Linked station
  and envelope records receive semantic labels only when their layouts match.
- Pin any compatible definition as a donor, or choose a differently named
  entry from a donor LIB. Select aspects, review target-to-donor differences,
  and Apply graft as one undo step. Conflicting layouts/types/scaling require
  deselecting that aspect or correcting the records. Hardpoint grafts transfer
  numeric station values; store references remain with the target.
- Keep single-field donor copying under Tools > Copy one donor field.
- View a bounded static pose from SH data, orbit/pan/zoom, use front/side/top
  views, and export geometry-only OBJ files.
- Move, rotate and scale the supported straight-line SH subset. Shapes with
  unhandled spatial records, bounds, visibility logic or animation remain
  read-only. The synthetic demo exercises transforms without retail data.
- Entry-level undo/redo, dirty state and explicit discard on close. Retail LIB
  names are protected; custom LIBs can be replaced with numbered backups.

This version uses numeric **source storage values**, not invented conversions
to knots, pounds or Mach. A `^` marker is retained, not silently reinterpreted.
Changing a field is not a guarantee of how the original game consumes it.
Opaque binary definitions can be exported or replaced but are not guessed.

## Protected LIBs and saving

Retail names are reserved case-insensitively in **every folder**, in both the
GUI and CLI. You can open, inspect, extract, clone aircraft and edit in memory;
save those changes to a different LIB name. Ctrl+S suggests `HANGAR.LIB` for a
retail source and the current filename for a custom LIB. The aircraft wizard
continues to suggest its new aircraft ID. The browser lets you choose another
name or destination on every save.

The exact list comes from the supplied installer and discs:

| Source | Protected filenames |
| --- | --- |
| Disc 1 `SETUP.ESA` installer | `FA_1.LIB`, `FA_2.LIB`, `FA_4B.LIB`, `FA_4D.LIB` |
| Disc 1 loose archives | `FA_4C.LIB`, `FA_7.LIB` |
| Disc 2 loose archives | `FA_3.LIB`, `FA_10.LIB`, `FA_10B.LIB`, `FA_11.LIB`, `FA_11B.LIB` |
| Disc 1 bundled demo/installer | `LHX0.LIB`–`LHX4.LIB`, `_SETUP.LIB` |

`SWPATCH.LIB` is a toolkit/mod archive in the supplied installation, so it stays
writable. This is an explicit filename list, not a blanket `FA_*.LIB` rule or
content fingerprint. Renaming a retail copy to a custom name makes that copy
writable. The reserved names cannot be used for new output files either.

Saving an existing custom `MYMOD.LIB` keeps the previous file as
`MYMOD.LIB.bak`, then `.bak.1`, `.bak.2`, and so on without replacing earlier
backups. The new file is written and flushed before the old file is moved.
If installation of the new file fails, Hangar tries to restore the old name;
if restoration fails, the error identifies the backup and staged file. A power
loss between moves can require restoring the backup manually. Backups are kept
beside the LIB until you remove them. Resource/PNG/WAV/OBJ exports remain
create-new. Retail name protection does not affect reading from game discs.

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
cargo run --locked -- references /path/to/FA_2.LIB F18.SH
cargo run --locked -- validate /path/to/MYMOD.LIB
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
350–420 KiB. Copy it to a writable location and run it. No installer or runtime
DLL is required. Windows file paths are ASCII in this first version.

**Windows 98/ME runtime compatibility remains unverified.** The executable
header and imports are audited, but an actual OS/hardware test is still
required. See [compatibility and acceptance](docs/COMPATIBILITY.md).

[GitHub Actions](.github/workflows/windows.yml) runs only on Windows and emits
separate `tore-hangar-win98-me-pentium4` and `tore-hangar-win64` ZIP artifacts.
The legacy executable's smoke test runs on the modern Windows runner, not on
Windows 98. Source and notices are included in the repository; packages carry
the license and notices alongside the executable.

## Duplicate an aircraft into its own LIB

1. Open the source LIB and select an aircraft PT, such as `A10.PT`.
2. Click **New aircraft**. Enter a new ID, for example `A10V1`, then its display
   name. This workflow uses the selected aircraft; it does **not** ask for a
   loose SH path.
3. Review the filename map. Hangar copies the resolved resource graph and
   assigns private names that do not collide with any scanned source entry.
   Use Back to change names, or Add source LIB when dependencies are elsewhere.
4. Click **Export new LIB**, choose a destination, and save the suggested
   `A10V1.LIB` or another new filename. The exported aircraft opens for editing.
5. Reopen the LIB, inspect its model/fields, and test it in Fighters Anthology.

The source document and original files are preserved. Before export, the review
is a draft; cancelling it keeps the source open. Cancelling the final output
picker leaves the new unsaved LIB open for later packaging. Save source edits
before beginning the workflow.

The current document has first priority. Other LIBs in its folder are indexed
for dependencies and naming collisions; only needed payloads are read. Added
source LIBs resolve missing/ambiguous external resources. Conflicting external
copies require an explicit source choice. New entries are written in name order.

The clone includes explicit BRF resource references, catalog-resolved filename
literals in module data/code sections, the shadow-derived A/B/C/D/S family,
known cockpit picture families, available store-icon companions and an editor
palette copy. It inspects whole module sections, so it is not limited to the
currently displayed shape pose. It preserves imported game symbols, compiled
addresses and section sizes; short aliases fit small filename fields. Geometry,
numeric characteristics and leaf image/audio bytes remain unchanged.

This is a private **resource** package, not proof of complete original-game
runtime behavior. Game procedures and dynamically generated names remain
outside the file graph. Unresolved HUD name candidates are shown in the review
and preserved; required missing resource filenames block export. The tool
supports one aircraft and the reviewed `_S.SH` damage-family convention.

For an externally authored shape, use **Lib > From loose SH file**. That older
workflow requires a real SH file and retains shared stock dependencies. It is
separate from **New aircraft**, which clones the selected aircraft and assets.

The CLI equivalent accepts additional source LIBs explicitly:

```sh
cargo run --locked -- clone-aircraft FA_2.LIB A10.PT A10V1 "My A-10" A10V1.LIB FA_1.LIB
```

See [the Windows test checklist](docs/WINDOWS-TEST.md).

## Paint a livery

1. Select a PT/SH and choose **Textured** (or View > Textured / wireframe).
2. Click a visible panel. The inspector identifies its face and named PIC.
3. **New aircraft** now creates private copies of the discovered textures,
   including the damage family. Paint those copies for a separate livery.
   **Clone texture for this shape** remains available for individual changes;
   that narrower command only retargets references in the decoded pose.
4. Click **UV / paint …**. Amber outlines show that face's footprint on the
   atlas. Choose a palette swatch and **Brush**. The lower **3D preview** updates
   during the stroke. Wheel zooms the atlas; middle-drag pans it. Middle-drag
   and wheel over the model preview orbit and zoom the model.
5. Alternatively, stay in Model and enable **Paint selected panel on model**.
   Mouse hits map back to texture coordinates. Dragging remains on the selected
   face; release commits the stroke. Ctrl+Z undoes it; Esc cancels a live stroke.
6. Use **Export PNG** for an external image, or **Package LIB** to save the
   edited PIC in game format. Reopen the new LIB, inspect and test it in FA.

The brush radius is in texture pixels. Shared/mirrored UVs and shared PIC names
mean other panels or models can change too. The preview is an unlit,
orthographic static pose with named textures; runtime-selected decals,
unvisited LODs/animations and game lighting are not reproduced. A "panel" here
is one decoded polygon, not a semantic group of aircraft parts. Surface recolor
acts on matching untextured face color indices in the decoded pose; it does not
rewrite Gouraud vertex colors or textured materials.

A full embedded PIC palette or a base palette is needed for painting. Hangar
loads `PALETTE.PAL` from the current LIB, or you can load a 768-byte 6-bit RGB
PAL (or another LIB containing PALETTE.PAL) for preview. Missing colors are shown in grayscale and painting is blocked
until a complete palette is available. Span holes are preserved; this brush
does not create new opaque pixels outside existing spans. PNG/WAV export does
not imply PNG import, arbitrary audio-format conversion or re-UV tools.

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
| Graft characteristic groups | Entry > Use as graft donor, select target, open Graft |
| Copy one donor field | Select a field, Tools > Copy one donor field |
| Remove selected entry | Delete; Ctrl+Z restores it |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Orbit / pan | Middle-drag / Shift+middle-drag |
| Zoom / frame | Wheel over viewport / Home or period |
| Front / side / top / projection | 1 / 3 / 7 / 5, including numpad |
| Transform supported static shape | G / R / S, X/Y/Z, numeric value, Enter |
| Cancel transform or dialog | Esc or right mouse button |
| Close with unsaved edits | Close window; type DISCARD or Esc to return |

Scale currently acts on the selected axis in percent; rotation is in degrees,
translation in integer source coordinates. The file browser has directory
rows, drive roots, recent LIBs and an editable path. It does not create missing directories. A packaging error leaves edits
in memory and displays its cause. The app writes only when explicitly asked.

## Scope still ahead

A complete animated SH writer, geometric vertex/face editing, topology changes,
UV layout editing, palette RGB editing, hardpoint tools, animation playback,
complete dependency closure, richer grafting, unit-aware gameplay controls,
resizable editor splits and bitmap fonts.
There are no inactive timeline controls pretending these features work.

The References dock indexes observed BRF strings and bounded module filename
literals, including names outside the displayed pose. It also identifies the
reviewed PT damage family, same-name default HUD and available store icons.
Use **Package > Add source LIB catalog** to locate dependencies in other LIBs;
these catalogs are directory-only and do not load their payloads. Multiple
external providers are reported as ambiguous. **Export report** writes the
change list and all retained check results to a new text file.

Self-name literals are excluded from navigation. Counts cover observed users
in the current LIB, not every possible runtime lookup. External catalog matches
identify possible providers, not game load order or verified external payloads.
The aircraft-cloning wizard retains its broader source and aliasing rules.

Package checks are advisory. Supported payload checks apply to changed entries;
unknown encodings and incomplete scans remain unverified. Opening a saved LIB
establishes a new baseline, so CLI `validate` checks its archive and references
without treating every existing payload as newly edited.

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
