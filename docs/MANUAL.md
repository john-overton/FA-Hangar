# T.O.R.E Hangar manual

How to use T.O.R.E Hangar to inspect and edit Fighters Anthology LIB files.
For downloads and builds see the [README](../README.md); for format and writer
decisions see [ARCHITECTURE.md](ARCHITECTURE.md).

**Early working editor, not a complete SH authoring tool.** LIB and textual
BRF editing work. Aircraft geometry is edited per region, wherever Hangar can
prove the change is safe, and moving parts change only through a catalog of
same-size settings, with forms not seen in retail labelled until tested in
the game. No game data ships.

## Contents

- [Getting started](#getting-started)
  - [Opening LIBs and the file browser](#opening-libs-and-the-file-browser)
  - [Workspaces](#workspaces)
- [What works](#what-works)
- [Protected LIBs and saving](#protected-libs-and-saving)
- [Game limits](#game-limits)
  - [The game folder](#the-game-folder)
  - [SH textures](#sh-textures)
- [Command line](#command-line)
- [Export an object and its resources](#export-an-object-and-its-resources)
  - [Unresolved in source](#unresolved-in-source)
- [Names, reference ID and duplicates](#names-reference-id-and-duplicates)
  - [Identity panel](#identity-panel)
  - [Rename reference ID](#rename-reference-id)
  - [Duplicate aircraft](#duplicate-aircraft)
- [Work across LIBs](#work-across-libs)
- [Flight envelope table](#flight-envelope-table)
  - [Negative-G engine cut-out](#negative-g-engine-cut-out)
- [Paint a livery](#paint-a-livery)
  - [Display palette](#display-palette)
  - [Select panels](#select-panels)
  - [Per-panel textures](#per-panel-textures)
  - [Remap panels from the view](#remap-panels-from-the-view)
    - [Fixing a stretched panel](#fixing-a-stretched-panel)
  - [Replace a color](#replace-a-color)
  - [Erase and restore textures](#erase-and-restore-textures)
- [Hardpoints, materials and decals](#hardpoints-materials-and-decals)
- [Ship, ground and animation tools](#ship-ground-and-animation-tools)
- [Edit shapes](#edit-shapes)
- [Moving parts](#moving-parts)
- [Controls](#controls)
- [Limits and why](#limits-and-why)
  - [Why does Hangar refuse this?](#why-does-hangar-refuse-this)
  - [Textures Hangar creates](#textures-hangar-creates)
  - [Palette](#palette)
  - [Palette companions](#palette-companions)
  - [Compression and LIB size](#compression-and-lib-size)
  - [Names](#names)
  - [FA loader limits](#fa-loader-limits)
  - [Retail LIB names](#retail-lib-names)
  - [Archive and resource sizes](#archive-and-resource-sizes)
  - [Shape geometry](#shape-geometry)
  - [Moving part settings](#moving-part-settings)
  - [Damage family](#damage-family)
  - [Stored originals](#stored-originals)
  - [Source values](#source-values)
  - [References and package checks](#references-and-package-checks)
  - [Missions and other LIBs](#missions-and-other-libs)
  - [Runtime markings](#runtime-markings)
  - [Verification status](#verification-status)

## Getting started

On Windows, copy the portable executable to a writable location and run it. On
Linux, run it from the source tree; see [Download and build](../README.md#download-and-build).

```sh
cargo run --locked -- --demo
cargo run --locked -- /path/to/FA_2.LIB
```

`--demo` opens a synthetic demo that needs no game data; it exercises
transforms without retail data.

### Opening LIBs and the file browser

**Ctrl+O** or the File menu opens a LIB through the in-app file browser. The
browser has directory rows, drive roots, recent LIBs and an editable path. It
does not create missing directories. Recent paths are kept in
`tore-hangar-recent.txt` beside the executable; if that location is read-only,
history still works for the current session.

Open multiple LIBs together without a fixed count cap, switching through
outliner roots without discarding edits. Each LIB retains selection, camera,
definition group and undo history. **File > New empty LIB** provides a
destination for assembling resources.

### Workspaces

The UI follows the Gunmetal design system in `tore-hangar-design`: one menu
bar with flat workspace tabs, a categorized outliner with type icons, linked
aircraft/shape selection, grouped source-value properties, and
Browse/Model/Flight/Graft/Package/Paint workspaces. Raw fields show saved values
beside current values, with amber edits and reset controls. Windows uses Tahoma
for interface labels and Lucida Console for resource data, at the design's
sizes and weights.

- **Menu bar.** File, Edit, Lib, Entry, View, Tools and Help menus list their
  shortcuts as keycaps. The active workspace tab is filled and joins the editor
  below. The active LIB name sits at the right with an amber dot while it has
  unsaved edits and a lock when it is a protected retail name. While a menu is
  open, clicking outside it only closes it.
- **Status bar.** Key hints for the current workspace and mode (or the last
  message), then `ENTRY · LIB` and the save state: the dirty dot with the
  number of unsaved edits, **Saved**, **Validated** after a passing package
  check, or the validation error count.
- **Viewport header.** The mode select switches between **Object Mode**,
  **Edit Mesh**, **Hardpoints**, **Parts** and **Texture Paint**. **View**
  holds Frame all, Front, Side, Top and Toggle projection. At the right, the
  hardpoint marker toggle and the shading control: **Wireframe**, **Solid**
  (face colors lit by their angle to the view, no textures) and **Textured**.
  In a narrow viewport the right group folds into one menu button.
- **Viewport.** The tool strip (Select, Move, Rotate, Scale, Frame, Texture
  Paint) floats at the top left. The navigation gizmo at the top right follows
  the camera; click an axis cap to view along it. The grid is aligned to the
  origin with every fifth line brighter, the X and Y axes run the full grid and
  the origin is an amber dot.
- **Outliner.** The filter field matches entry names as you type (Ctrl+F;
  Esc or Enter leaves it). The three type buttons beside it show only
  aircraft (PT), shapes (SH) or images (PIC) in every open LIB and in the
  Browse table; click the lit button again to show everything. Groups show
  their entry count and type badge and collapse from their row; stored
  originals (`.ORG`) list right below Images. The entry the editors show is
  the active row (bright amber text); the entries it works with, such as an
  aircraft's linked shape or the model a texture was opened from, are
  highlighted as selected.
- **Property panels.** The right-hand editor stacks collapsible panels.
  Click a panel header to collapse or expand it; Ctrl+click keeps only that
  panel open. Each panel remembers its state. The wheel scrolls the panels; a
  thumb at the right edge and a chevron at the bottom show that more follows.
- **Number fields.** Every integer source value is a number field: drag
  left or right to scrub (Shift for fine steps, Ctrl to snap to ten steps),
  click without dragging to type an exact value, use the hover arrows to step
  once, and press Backspace over the field to restore the operand saved in
  the file on disk. Esc during a drag cancels it. A drag, a step, a typed
  value and a reset are each one undo step. A value that differs from the file
  turns amber. Values the game stores with BRF's `^` marker read **scaled**
  after the number; they stay in raw source units. `$hex` operands, strings
  and pointers keep their notation and open the type prompt when clicked.

| Workspace | Use |
| --- | --- |
| Browse | Select, filter, export and remove entries; inspect entry bytes |
| Model | Static pose view, transforms, vertex edit mode, hardpoints and **Parts** |
| Flight | Recognized definition fields and the [envelope table](#flight-envelope-table) |
| Graft | Choose a donor, select aspects, review changes, **Apply graft** |
| Package | Review added, modified and removed entries; advisory checks; reports |
| Paint | Texture painting with **Paint**, **Materials** and **Decals** tabs |

## What works

- Browse folders/drives and reopen recent LIBs. Open multiple LIBs together
  without a fixed count cap, switching through outliner roots without
  discarding edits. Each LIB retains selection, camera, definition group and
  undo history. File > New empty LIB provides a destination for assembling
  resources.
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
- Save a new or existing custom LIB with the required EOF sentinel. Untouched
  entry payloads keep their original compression and bytes. An unedited archive
  round-trips byte-for-byte, including padding. Modified entries are stored
  uncompressed.
- Edit PT/NT/JT/OT/SEE/ECM textual BRF operands. Recognized schemas get named
  fields; other blocks retain their indexed labels. Comments, whitespace,
  line endings, labels, scaling markers and untouched values survive edits.
- Edit an object's short and long names in place, rename an aircraft's
  reference ID with its private files, or duplicate it in the same LIB
  ([Names, reference ID and duplicates](#names-reference-id-and-duplicates)).
- Export the selected object into a separate LIB with a new ID, short name
  and long name.
  Aircraft, weapons, equipment, shapes and individual resources use the same
  reviewed dependency graph and filename map. Aircraft retain the reviewed
  damage/shadow family. Unknown binary objects copy unchanged and report that
  their dependencies cannot be discovered.
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
- Move, rotate and scale the supported static SH subset in Object mode.
  **Tab** opens [Edit Mesh](#edit-shapes) on any shape, retail aircraft
  included: vertex and face selection, box and part select, move, rotate,
  scale, delete, flip, duplicate, extrude, make face and add vertex, each
  checked region by region. In vertex select, a press on a vertex becomes a
  drag after the pointer moves 4 pixels, keeps its offset from the cursor and
  moves the whole selection.
- List an aircraft's [moving parts](#moving-parts) by role, preview gear,
  flaps, hook, brakes, bays and afterburner in any state, and change the
  settings their native code already has (gate values, swing range,
  direction, rotation axis, pivot) as one undo step each.
- Entry-level undo/redo, dirty state and explicit discard on close. Retail LIB
  names are protected; custom LIBs can be replaced with numbered backups.

## Protected LIBs and saving

Retail names are reserved case-insensitively in **every folder**, in both the
GUI and CLI. You can open, inspect, extract, export objects and edit in memory;
save those changes to a different LIB name. Ctrl+S suggests `HANGAR.LIB` for a
retail source and the current filename for a custom LIB. The object export
wizard continues to suggest its new object ID. The browser lets you choose
another name or destination on every save.

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
`MYMOD.BAK`, then `MYMOD.B01`, `MYMOD.B02`, and so on up to `MYMOD.B99`,
without replacing earlier backups; the new file is staged as `MYMOD.TMP` (or
`.T01`..`.T99`). These names never contain `.LIB`: Fighters Anthology loads
every file in its folder whose name contains `.LIB` as another LIB, so older
Hangar backups named `MYMOD.LIB.bak` were loaded as duplicate LIBs (see Game
limits). Move such old backups out of the game folder. The new file is
written and flushed before the old file is moved.
If installation of the new file fails, Hangar tries to restore the old name;
if restoration fails, the error identifies the backup and staged file. A power
loss between moves can require restoring the backup manually. Backups are kept
beside the LIB until you remove them. Resource/PNG/WAV/OBJ exports remain
create-new. Retail name protection does not affect reading from game discs.

A packaging error leaves edits in memory and displays its cause. The app writes
only when explicitly asked.

## Game limits

Fighters Anthology crashes, rather than reporting an error, when its folder or
a texture breaks one of the limits below. They come from FA.EXE itself;
[ARCHITECTURE.md](ARCHITECTURE.md#fa-loader-limits-and-the-sh-texture-layout)
gives the addresses.

### The game folder

At startup FA reads **every file in its folder** (subfolders are skipped). It
upper-cases each name; a name that contains `.LIB` and whose file starts with
`EALIB` is opened as a LIB, and every other file is one resource. So
`swpatch.lib`, `MYMOD.LIB.bak` or `OLD.LIB.copy` all load as LIBs.

| Limit | Value | What breaks |
| --- | --- | --- |
| LIB files | 20 | FA keeps 20 LIB handles; a 21st overruns them |
| Resources (every LIB entry plus every other file) | 9,950 | FA's resource table holds 9,950; the installed retail LIBs use about 7,520 |
| LIB file name | 13 characters | the name is copied into a 14-byte slot; `TOPGUN.LIB.BAK` (14) overflows it |

When you save a LIB into a folder that holds `FA.EXE` or a retail LIB name,
Hangar counts what FA would load after the save (the saved LIB, its new
`.BAK`, every other LIB and file) and the status ends with, for example,
"Game folder: 6 of 20 LIBs, 7,614 of 9,950 resources". Old Hangar backups
named `X.LIB.bak`, `X.LIB.bak.1` or `X.LIB.tmp` get a warning: "FA loads
TopGun.LIB.bak as a LIB; move it out of the game folder". If the save would
break a limit, a dialog lists each one with the numbers; **Cancel** writes
nothing and **Save anyway** saves. The CLI refuses such a save and prints the
numbers. Every backup (`MYMOD.BAK`, `MYMOD.B01`, …) in the game folder is a
resource too, so keep few of them there.

### SH textures

Every texture a retail shape draws on its textured faces (all 1,070 across
the retail LIBs) has one layout, and FA's texture mapper depends on it:

- kind 0, a raw raster (not span-coded);
- exactly 256 pixels wide, 1 to 1,280 rows;
- a row-offset table after the raster (`64 + row × 256`);
- no embedded palette: pixels are indices into the game palette.

A texture without the row table crashes FA as soon as a face using it is
drawn (the external view of an aircraft, for example). Hangar writes this
layout for every SH texture it creates: generated panel sheets, **Remap
selected panels from view…** and **Repair textures for FA**. Clones and
exports copy retail bytes. **Assign texture…** can point faces at any PIC in
the LIB; when that PIC is not an FA texture the status says so.

**Package checks** report each PIC that a textured face draws and that is
not in this layout as an **ERROR**, "Would crash FA's texture mapper", with
the reasons (for example "64 pixels wide, not 256, no row-offset table,
embedded 768-byte palette") and the shapes that draw it. Retail LIBs report
none: `MOON.SH`'s 41-wide `_MOON.PIC` is drawn as a sprite, not by a
textured face. While such errors are listed, **Repair textures for FA**
appears in the Package output column. It rewrites each of those PICs, and its
stored original `X.ORG` when that has the old layout too, so Restore texture
and the Eraser keep working:

- the PIC is widened to 256 columns with every existing pixel at the same
  (u, v), so the SH UVs stay valid and no SH changes; the new columns repeat
  each row's last pixel (a generated sheet's panel color);
- the row table is added and the embedded palette dropped. Indices are kept
  when the embedded palette equals the loaded base PAL; where it differs,
  the used colors are mapped to the nearest base PAL color and the status
  says so. Without a base PAL the indices are kept and the status says they
  are unverified.

The repair is one undo step. A PIC wider than 256 pixels cannot keep its UVs
and is reported, not changed. The CLI form writes a new LIB and prints each
texture's header before and after:

```sh
cargo run --locked -- repair-textures MYMOD.LIB MYMOD2.LIB FA_2.LIB
```

The last argument (a PAL, or a LIB with `PALETTE.PAL`) is the game palette
when the input has no `PALETTE.PAL`. `--texture-repair-check MYMOD.LIB
F5EV.SH FA_2.LIB` runs the Package repair through the app and checks that
every SH byte and the textured viewport stay the same, with undo and redo.

## Command line

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

Manual checks against your own LIBs (never part of CI) are also in
`--help`: `--geometry-check NEW_DIR FA_2.LIB F18.SH` runs every region edit
and part setting, including each reachable gear direction and range form, on
in-memory copies and writes the results create-new to `NEW_DIR`;
`--edit-check FA_2.LIB F18.SH` drives Edit Mesh and Parts through the app,
undoing each step; `--identity-check FA_2.LIB F14.PT F14Z F18.PT F18Z
NEW.LIB` renames one aircraft and duplicates another through their reviews,
checks that undo restores the exact bytes and saves the result create-new;
`--face-texture-check FA_2.LIB NEW.LIB F18.SH F16.SH A10.SH` clones the PIC
for four tail faces of each shape through **Clone texture for selected
faces**, paints the copy, checks that nothing else changed, runs **Use shape
texture** and its undo, and saves the painted result create-new.
`--replace-check FA_2.LIB F18.SH NEW_DIR` runs **Replace color…** on the
shape's main PIC for the whole texture and for its tail panels, then a 3D
Replace stroke and the eraser, checking that only matching raster bytes
change, that `X.ORG` matches the saved texture, undo, redo and Restore
texture; it writes before/after PNGs and two LIBs create-new to `NEW_DIR`.
`--remap-check FA_2.LIB NEW_DIR F18.SH F16.SH A10.SH` lists each shape's
most stretched textured faces (texels per unit along each face's vertical
and horizontal axes), remaps the most stretched side faces from the side
they face with **Bake current look** through the panel selection and the
Remap dialog, checks the new PIC's layout, CODE coverage, bindings and every
other face, renders the side view before and after and the new PIC as PNG,
runs **Use shape texture** and its undo, and writes `REMAP.LIB` create-new.
After an SH name, `--faces 3821,2E84,2E9F` picks the faces instead (file
offsets, as refusals name them), `--clone HEX,...` the faces to clone first
(default: the same), and `--view 270,0` the camera yaw and pitch; name the
SH again for its other side. With explicit faces the check clones them
through **Clone texture for selected faces** and undoes it, remaps them,
requires square texels spanning both directions, and paints one texel (in
memory) before and after to report how many screen pixels it reaches.
`--palette PALETTE.PAL|LIB` supplies the game palette for a LIB without one.
`--proof-census FA_2.LIB NEW_OUT.txt [OLD.txt]` lists the texture and vertex
proofs of every face each shape's neutral model draws, and compares them
with an earlier list.
`--palette-check TOPGUN.LIB NEW.LIB F5EV.PT F14.PT F14.SH` prints the
palette each entry resolves and where it comes from, then copies `F5EV.PT`
with its linked files into a new LIB through **Copy to** and lists the PALs
that arrived; the new LIB is written create-new.
The `replace`, `replace-model` and `replace-dialog` snapshot workspaces show
the tool and the dialog. `--snapshot
OUT.svg NEW.LIB F18.SH paint-side 1280x800` renders a textured side view;
the `assign-texture` workspace shows the Assign texture dialog.

Use `inspect` to confirm field indices and values for your own file first.
The example changes the recognized object weight operand, in its source units.

Retail name protection applies to the CLI as well. Object export and panel
repair have CLI forms, described in
[Export an object](#export-an-object-and-its-resources) and
[Ship, ground and animation tools](#ship-ground-and-animation-tools).

## Export an object and its resources

1. Open the source LIB and select an object, such as `A10.PT` or `AIM9M.JT`.
2. Click **Export object**. Enter a new ID, for example `A10V1` or `MYAIM9`,
   then its short name (as in lists, for example `A-10V`) and its long name.
   **Back** returns to the previous step. The selected entry supplies the
   source object.
3. Review the filename map. Hangar copies the resolved resource graph and
   assigns private names that do not collide with any scanned source entry.
   Use Back to change names, or Add source LIB when dependencies are elsewhere.
   Names no searched LIB provides are listed under
   [Unresolved in source](#unresolved-in-source).
4. Click **Export new LIB**, choose a destination, and save the suggested
   `A10V1.LIB` or another new filename. The exported object opens for editing.
5. Reopen the LIB, inspect its model/fields, and test it in Fighters Anthology.

The source document and original files are preserved. Before export, the review
is a draft; cancelling it keeps the source open. Cancelling the final output
picker leaves the new unsaved LIB open for later packaging. Current in-memory
source edits are included, while the original document retains its own history.

The current document has first priority, followed by explicit source choices
and unambiguous open documents (including their in-memory edits). Other LIBs
in its folder are indexed for dependencies and naming collisions; only needed
payloads are read. Conflicting external copies require an explicit source
choice. New entries are written in name order, and the donor stays open.

The clone includes explicit BRF resource references, catalog-resolved filename
literals in module data/code sections, an aircraft's shadow-derived A/B/C/D/S
family, known cockpit picture families, available store-icon companions and the
display palette as `<ID>.PAL`: the object's own `<ID>.PAL`, else `PALETTE.PAL`
from this LIB, else the palette Hangar shows it with (another open LIB or a
LIB in the folder, see [Display palette](#display-palette)). The review's
row reads **Palette: PALETTE.PAL from FA_2.LIB** → `A10V1.PAL`; with no
palette at all, the first note says the new LIB will show in grayscale.
It inspects whole module sections, so it is not limited to the currently
displayed shape pose. It preserves imported game symbols,
compiled addresses and section sizes; short aliases fit small filename fields.
Geometry, numeric characteristics and leaf image/audio bytes remain unchanged.

This is a private **resource** package, not proof of complete original-game
runtime behavior. Game procedures and dynamically generated names remain
outside the file graph. Unresolved HUD name candidates are shown in the review
and preserved. SH texture names count only when they are E2 texture records;
bytes inside face or vertex data that happen to look like a name (retail
`F14_C.SH` has one that reads as `B.PIC`) are not references. The tool
supports the reviewed `_S.SH` aircraft damage-family convention. Other
recognized objects follow their stored references and known companion-file
conventions. Opaque binary resources can be copied, but their unknown
references are not renamed or invented. Display names change only in
recognized identity records.

### Unresolved in source

A referenced name that is in neither the source LIB nor any searched LIB is
*unresolved in source*. Some retail resources ship that way: `MIG31.PT` names
`Y141.HUD`, and `~BGUN.PT`'s `EJECT_S.SH` shadow implies `EJECT_A.SH` to
`EJECT_D.SH`, none of which exist in any retail LIB. The game runs without
them, so the export can too, but only once you say so.

First check whether a source LIB is missing: names from another disc LIB
resolve as soon as that LIB is added with **Add source LIB**. Anything still
absent is listed in the review's **Unresolved in source** section with the
resource that references it and how it is stored (BRF string, texture record,
HUD texture name, module filename or damage-family convention). A texture
record that no SH traversal reaches is marked *not drawn by any pose*.

- **Keep as in source** (the default) copies the referencing resource with the
  name unchanged, byte for byte. The name is not renamed, copied or counted as
  a private resource, and no private name may take it, so the export behaves
  exactly like the source. A missing damage-family member stays absent.
- **Use texture…** retargets one texture reference to a PIC instead: one already
  in the package (the referencing shape's own skin is listed first), or **Other
  PIC in a source LIB…** by name. The substitute joins the package under a
  private name, written into the copied shape's fixed 13-character name slot.
  Only the copied resource changes; the source LIB and other aircraft keep the
  stored name. Non-texture names can only be kept.

While any name is kept, **Export new LIB** stays disabled until **Export with
N unresolved references, as in the source LIB** is ticked. Changing a row back
to Keep asks again. Validation lists the kept names as warnings.

For an externally authored shape, use **Lib > From loose SH file**. That older
workflow requires a real SH file and retains shared stock dependencies. It is
separate from **Export object**, which clones the selected object and assets.

The CLI equivalent accepts additional source LIBs explicitly:

```sh
cargo run --locked -- export-object FA_2.LIB A10.PT A10V1 "My A-10" A10V1.LIB FA_1.LIB
cargo run --locked -- export-object FA_2.LIB AIM9M.JT MYAIM9 "My missile" MYAIM9.LIB FA_1.LIB
cargo run --locked -- export-object FA_2.LIB MIG31.PT MIG31X "My MiG-31" MIG31X.LIB FA_1.LIB --keep-unresolved
cargo run --locked -- export-object FA_2.LIB F14.PT F14X "My F-14" F14X.LIB FA_1.LIB --substitute OLD.PIC=_F14.PIC
cargo run --locked -- export-object FA_2.LIB F14.PT F14X "F-14X Tomcat" F14X.LIB FA_1.LIB --short F-14X
```

TITLE is written as both the short and the long name; `--short NAME` and
`--long NAME` replace one of them. Without a flag, an unresolved name refuses
the export and lists each name with its resource. `--keep-unresolved` keeps every unresolved name as in the source.
`--substitute OLD.PIC=NEW.PIC` (repeatable) retargets every reference to
`OLD.PIC` in the copied resources to `NEW.PIC`, which must be in a searched
LIB; a substitute for a name that is not unresolved is an error. Source LIBs
are read like the GUI reads them, directory first and then only the payloads
the export needs, so the 140–186 MiB disc LIBs (`FA_7.LIB`, `FA_10.LIB`,
`FA_11.LIB` and their `B` copies) work as sources.

See [the Windows test checklist](WINDOWS-TEST.md).

## Names, reference ID and duplicates

An object definition's names block holds three strings. In `F14.PT`:
`"F-14"` is the short name, `"F- 14D Tomcat"` the long name (the space is in
the retail file) and `"F14.PT"` the object's reference to itself. The game
lists aircraft by these names; missions and other files find the aircraft
by its file name, the reference ID `F14` plus `.PT`.

### Identity panel

Select a PT, NT, JT or OT whose names block Hangar recognizes. The first
panel of the inspector is **Identity**:

- **Short name** and **Long name**: click either field and type the new
  name. Each change is one undo step and changes only that string in the
  definition. A changed name shows in amber with its **Saved** value below and
  a reset button. A weapon's second names block (`si_names`) follows when it
  held the same text.
- Names are 1 to 40 plain ASCII characters without quotes, semicolons or
  control characters. 40 is Hangar's limit, not a known game limit; retail
  names reach 11 (short) and 28 (long) characters, so check longer names in
  the game.
- **Reference ID** shows the file stem, such as `F14`. On an aircraft,
  **Rename…** beside it and **Duplicate aircraft…** below it open the
  reviews described next. Other objects show the ID only.

### Rename reference ID

**Rename…** in the Identity panel, **Entry > Rename reference ID…** or
**Rename reference ID…** in a PT's context menu asks for the new ID (1 to 6
letters, digits or underscore) and opens a review. Nothing changes until
**Rename**.

The review lists every resource renamed, old → new, the resources whose
stored references are rewritten in place, private files kept because their
names do not start with the old ID, and the shared resources left alone with
the reason. Renamed are the aircraft and the files private to it: its main
and shadow shapes with the A–D damage family, skins and cockpit art, its
HUD, sensors or stores no other object uses (with their `$` icons) and the
stored originals (`.ORG`) of renamed textures. The old ID in each name is
replaced, so `F14_C.SH` becomes `F14Z_C.SH` and `_F14_A.PIC` becomes
`_F14Z_A.PIC`. The aircraft's own palette follows: `F14.PAL` becomes
`F14Z.PAL`. Weapons, sounds, the game palette and anything another object
in the LIB uses keep their names.

Every recognized reference in the LIB is rewritten: BRF strings (including
the aircraft's own `F14.PT` string), module filenames and texture names, each
within its stored slot. The review refuses, listing why, when a new name is
already in the LIB, does not fit an 8.3 name, or does not fit the compiled
slot that stores it, or when the HUD the game finds by the aircraft's name is
shared with another aircraft, or when a definition Hangar cannot read names
a renamed file. Choose a shorter or different ID.

The review warns: **Missions and other LIBs that refer to F14.PT by name
will not find the renamed aircraft.** It also counts the missions and other
unparsed text resources in this LIB that name it; they are not rewritten.
Same-stem files the aircraft holds no stored reference to, such as `F14.PTS`
and `F14.HUD` in `FA_2.LIB`, keep their names and are listed as notes. One
undo step restores the exact bytes. Retail LIB names stay protected on save;
save the result as a custom LIB.

### Duplicate aircraft

**Duplicate aircraft…** in the Identity panel, **Entry > Duplicate
aircraft…** or a PT's context menu (where it replaces the plain Duplicate)
runs the export steps — new ID, short name, long name — and then reviews a
copy inside the same LIB.

Each row is a resource the new aircraft uses, with **Copy** or **Share**:

- **Copy** gives the new aircraft its own file under a new private name.
  By default the aircraft, its shapes and damage family, skins and HUD are
  copied when nothing else in the LIB uses them. A HUD the game finds by the
  aircraft's name is always copied.
- **Share** keeps the existing name: the copy refers to the same file. By
  default weapons, sounds, sensors and stores, the game palette and every
  resource already shared in the LIB are shared. Sharing a shape also shares
  what only it uses, such as its skins.
- A damage family, a texture with its suffix family (`_F18`, `_F18_A`, …) and
  a store with its icon switch together.

The aircraft's own `<ID>.PAL` is a row like any other. When the LIB has
neither it nor `PALETTE.PAL`, a **Palette** row offers the palette Hangar
shows the aircraft with as `<new ID>.PAL`, with **Copy** (the default) or
**Skip**; see [Palette companions](#palette-companions) for why it is never
`PALETTE.PAL`.

Names are checked against the whole LIB, and copied textures take their
stored originals along. **Duplicate aircraft** applies it as one undo step
and selects the new PT. In `FA_2.LIB`, `F18.PT` shares its shapes and skins
with `F18C.PT`, so by default only the PT and its HUD are copied; choose
**Copy** on `F18.SH` and the damage family for a separate model.

## Work across LIBs

1. **Ctrl+O** opens another LIB. Click an inactive root in the outliner to
   switch; the active root expands into its categorized entries. The **+**
   opens a LIB.
2. Select a resource and press **Ctrl+C**, switch to a destination LIB, then
   **Ctrl+V**. Alternatively, drag an entry onto another LIB root, or
   right-click it and choose **Copy to** or **Move to**. The paste review
   includes discovered dependencies by default, using snapshots of open source
   LIBs. Turn the option off for an intentional single-resource copy.
3. Resolve different same-name resources with **Keep target** or **Take
   source**. Review the notes, then **Apply resources**. One undo in the target
   reverses the entire copy; source LIBs are unchanged. Ambiguous external
   source copies require choosing the intended owner or closing a conflicting
   source.
4. **Entry > Rename resource** previews changes to decoded stored references in
   the active LIB. Compiled filename capacity is checked before applying. Known
   implicit-family bindings and users in other open LIBs can block a rename;
   use [Rename reference ID](#rename-reference-id) for an aircraft and its
   private files. **Ctrl+D** duplicates one resource under a new name,
   retaining its shared dependencies; [Duplicate
   aircraft](#duplicate-aircraft) copies an aircraft with its private files.
5. Pin a definition with **Entry > Use as graft donor**, switch LIBs and select
   a target. Graft's selectable groups work across library boundaries. Dragging
   onto a same-type definition in the active outliner also prepares a graft.
6. **Ctrl+S** saves the active LIB with the existing protection/backup policy.
   **Ctrl+W** closes the active LIB. Closing the application checks every open
   document for unsaved edits. Save As cannot overwrite another open LIB path.

Open documents also provide dependency names for package checks. Unique model,
texture and palette resources can be previewed from other open LIBs. References
link to a unique open provider and list observed users in other open LIBs; the
editor does not guess among conflicting providers or infer game load order.
The number of open LIBs is limited by available memory rather than an editor
count cap. Each archive retains its format/size checks. Undo and copy snapshots
share immutable payload buffers but also consume memory. No automatic writes
occur when switching libraries or preparing copies.

An object (PT, JT, OT or NT) copied or moved to another LIB brings its
palette, so Hangar shows its colors there: a review row **`F14.PAL` · Palette
for Hangar's colors, from …** with **Copy** (the default) or **Skip**. It is
the source's own `F14.PAL`, or else the palette Hangar showed the object
with, saved as `F14.PAL`. There is no row when the target already has
`PALETTE.PAL` or an identical `F14.PAL`; the notes say so. A different
`F14.PAL` in the target is a collision to resolve like any other. A move
leaves the palette in the source, where the rest still uses it. **Ctrl+D**
on an object offers the same row for the new name.

Each LIB has a collapse arrow in the scrollable outliner. Expanded inactive
LIBs show their categories and resources too. Drag an entry onto another LIB
root or one of its rows to review a transfer. The review opens with **Copy**
selected and **Include linked files** off (this item only); tick the box for the
object and its linked files, or choose **Move**, then resolve any name
collisions before applying.

Once a pressed entry moves a few pixels it becomes a drag. A chip beside the
pointer shows the entry's type icon and name and what releasing would do:
**Copy to MYMOD.LIB** over a row of another LIB (that row and its LIB root are
filled amber-deep with an amber outline), **Graft with F18.PT** over a
same-type definition in the same LIB (outlined in steel), or a short reason
where a release does nothing, such as **Already in this LIB**. The status bar
repeats the release action. **Esc** or the right mouse button cancels the drag.

Right-click an entry for its context menu. It selects the entry first
(switching LIB if needed) and lists **Copy to** and **Move to** submenus with
every other open LIB, then **Copy**, **Paste**, **Duplicate**, **Rename…**
(on a PT, **Duplicate aircraft…** and **Rename reference ID…**),
**Export entry…**, **Export object…** for definitions, and **Delete**, with
their shortcuts. **Copy to** and **Move to** open the same transfer review
with Copy or Move already chosen. Right-click a LIB root for **Paste**,
**Collapse** or **Expand**, **Package LIB** and **Close LIB**. Items that cannot
run (Paste with nothing copied, Copy to with one LIB open) are dimmed. Click
outside the menu, press Esc or right-click elsewhere to close it. Move removes the selected source item and its unshared linked
entries. Known shared dependencies remain in the source, and files supplied by
other open LIBs are copied. Both documents update together in memory; each has
its own undo step. Files change only when saved. An emptied source remains an
empty editor document; the LIB writer requires at least one entry to save it.

## Flight envelope table

Flight opens recognized envelopes as a table. The chevrons beside
"Envelope 1 of 3" choose the envelope; its G, points, stall lift and maximum
speed sit above the Speed and Altitude cells. Every value is a
[number field](#workspaces): drag to scrub, click to type, Backspace for the
saved value. Rows past the point count are unused and show their number in
muted ink; changed cells are amber. Wheel scrolls the table. The **Field
groups** panel picks the group (with field counts); **All fields** and the Raw
fields dock keep the underlying BRF view with on-disk values and reset
buttons.

### Negative-G engine cut-out

**Neg-G cut-out** in the Model inspector's Propulsion panel is the PT field
`plane.negGLimit`, which is also in Flight's Propulsion field group. Both
show it in its stored unit, 1/256 s. FA counts how long the aircraft holds negative G
without a break. When the count reaches this value, FA cuts the throttle to
0 and the engine spools down at the aircraft's throttle-down rate. As soon
as G is back at 0 or above, the count restarts and the engine spools back
up to the throttle you have set. You do not need to restart it.

| Value | Effect |
| --- | --- |
| 0 | No cut-out. Most retail fighters, including the F-5E. |
| seconds × 256 | Cut-out after that much continuous negative G: 2,560 is 10 s (retail F/A-18), 7,680 is 30 s (retail A-1 and SF.260). |
| Negative | Engine held at 0% throttle at all times. Do not use. |

Limits and why:

- **There is no chance roll.** The cut-out always happens after the same
  time. Any moment at 0 G or above resets the count, so pick a time longer
  than your longest dive.
- **The G level does not matter.** FA only checks whether G is below zero,
  so -0.1 G counts as much as -4 G.
- **Keep it at 30,720 (120 s) or less.** The count is a signed 16-bit
  number. Near the 32,767 maximum it can wrap round and the cut-out stops
  working reliably.
- **How far below zero the aircraft can pull is set by the envelope**, not
  by this field. FA uses the most negative envelope row that contains the
  current speed and altitude, less a reduction for external stores. On the
  retail F-5E, the -4 G row covers only about 570 to 760 ft/s below
  3,000–4,000 ft. To pull -4 G anywhere else, widen that row in the
  [envelope table](#flight-envelope-table).
- **Afterburner during a cut-out** gives no extra thrust, but FA keeps
  burning fuel at the afterburner rate until you deselect it.

The evidence, with FA.EXE addresses, is in
[ARCHITECTURE.md](ARCHITECTURE.md#engine-fields-negative-g-cut-out-and-throttle-rates).
The throttle-up and throttle-down fields (`throttleAcc`, `throttleDacc`) show
their unit, %/s, in the field table.

## Paint a livery

**Base color** is in the Model inspector and Paint > Materials. Its palette
picker replaces the most common flat color in the decoded pose, including all
faces using that index. **Panel color** changes only the selected untextured
face. Both are one undo step. Textured surfaces get their color from their PIC;
the UI points to Materials when no flat-color faces are present.

For a supported panel without a texture, click the viewport **brush icon** or
enable **Paint model / auto-create texture**. Hangar stages a private PIC
filled with the panel's original color and maps the polygon onto it. Release
commits the sheet, SH mapping and first stroke together; Esc cancels, and
Ctrl+Z removes the entire change. The base palette must be loaded. **Create
paintable panel texture** also performs this step explicitly, before adding a
decal.

Generated panel sheets follow the panel. The polygon is projected onto its
own plane, its longer side along the sheet's width, and the sheet takes the
panel's proportions at the shape's own texel density (the texels per unit its
textured faces already use; 3 per unit when it has none), so pixels are square
on the model and a 4:1 panel gets a 4:1 panel area. Each side is kept between
8 and 256 pixels, scaling both sides together. The PIC itself is an FA texture
(see [SH textures](#sh-textures)): 256 pixels wide and as tall as the panel
area, which sits at its left edge; the rest is filled with the panel's color.
It carries no palette, so the base PAL must be loaded to paint it. With
**Panel lock** off, flat panels
of the same color that share an edge and lie in one plane get one sheet sized
to all of them, so a stroke crosses them without a seam; with Panel lock on,
only the panel under the brush is converted. The status names the sheet and
its size, for example "Created F1800.PIC, a 256 × 8 FA texture with a 42 × 8
panel area, mapped to 2 coplanar panels". Sheets made by earlier versions
(64 × 64 or sized to the panel, with a palette and no row table) crash FA;
Package checks flag them and **Repair textures for FA** converts them.

Automatic mapping supports ordinary opaque polygons with a known material
state. Special shading, unresolved state, insufficient CODE space or jump
reach produce a diagnostic without changing the document. Other decoded faces
retain their material and UVs. This is planar panel mapping, not full UV unwrap.

1. Select a PT/SH and choose **Textured** (or View > Textured / wireframe).
2. Click a visible panel. The inspector identifies its face and named PIC.
3. **Export object** now creates private copies of the discovered textures,
   including the damage family. Paint those copies for a separate livery.
   To give only some panels their own PIC, use **Clone texture for selected
   faces** (see [Per-panel textures](#per-panel-textures)). **Clone texture
   for whole shape** retargets every reference to the PIC in the decoded
   pose, so the whole aircraft moves to the copy.
4. Click **UV / paint …**. Amber outlines show that face's footprint on the
   atlas. Choose a palette swatch and **Brush**. The lower **3D preview** updates
   during the stroke. Wheel zooms the atlas; middle-drag pans it. Middle-drag
   and wheel over the model preview orbit and zoom the model.
5. Alternatively, click the **brush icon** below Frame in the viewport toolbar.
   No panel selection is required. **Panel lock: off** lets a stroke cross
   visible faces and PICs; enable it to constrain a stroke. UV interpolation
   restarts at panel boundaries so distant atlas islands are not joined by
   paint streaks. Release commits all touched PICs and generated mappings as
   one undo step; Esc cancels the whole stroke and keeps the brush active. A
   stroke can touch up to 64 texture entries; the status reports how many flat
   panels it converted to new textures.
6. Use **Export PNG** for an external image, or **Package LIB** to save the
   edited PIC in game format. Reopen the new LIB, inspect and test it in FA.

The brush radius is in texture pixels. Shared/mirrored UVs and shared PIC names
mean other panels or models can change too. The preview is an unlit,
orthographic static pose with named textures; runtime-selected decals,
unvisited LODs/animations and game lighting are not reproduced. A "panel" here
is one decoded polygon, not a semantic group of aircraft parts. Surface recolor
acts on matching untextured face color indices in the decoded pose; it does not
rewrite Gouraud vertex colors or textured materials.

A full embedded PIC palette or a resolved base palette (see [Display
palette](#display-palette)) is needed for painting. In grayscale, painting
is blocked until a complete palette is available. Span holes are preserved; this brush does not create new opaque
pixels outside existing spans. PNG decal import and bounded per-face UV
transforms are also available. Arbitrary audio-format conversion and
topology-aware UV unwrapping remain outside this version.

### Display palette

Retail textures hold palette indices, so the colors on screen depend on the
PAL Hangar draws with. For the selected entry it uses the first of:

1. **Load palette…** (a 768-byte 6-bit RGB PAL, or a LIB containing
   `PALETTE.PAL`), for this LIB until it is closed;
2. `PALETTE.PAL` in this LIB;
3. `<ID>.PAL` of the object that owns the entry: a PT's own (`F14.PAL` for
   `F14.PT`); for a shape or texture, the nearest aircraft or object that
   reaches it through its stored references and the damage-family, HUD and
   icon conventions. This works with any number of aircraft in one LIB;
4. the only other valid PAL in this LIB;
5. `PALETTE.PAL` in another open LIB (retail LIBs first);
6. `PALETTE.PAL` in a LIB in the same folder, `FA_2.LIB` and `FA_1.LIB`
   first, read from their directories without loading them;
7. the last game palette Hangar found, remembered in
   `tore-hangar-palette.txt` beside the executable (for the session only
   when that folder is read-only);
8. otherwise a grayscale ramp.

The **Palette** panel in Paint names the source in its first row, for
example **Source F14X.PAL** or **Source PALETTE.PAL from FA_2.LIB**; a
remembered palette reads **…, remembered**. **Details** in the dock shows the
same line. In grayscale the panel shows **No game palette found; colors are
approximate.** above **Load palette…**, and the Model overlay repeats the
warning. A `PALETTE.PAL` found in or loaded from a retail LIB, or a loaded
file named `PALETTE.PAL`, becomes the remembered palette. The answer is kept until the selection's owner, the
open LIBs or their PAL entries change, so selecting entries stays fast.

### Select panels

Outside Edit Mesh, panels are selected in the Model viewport and in the
Paint workspace's model preview:

| Action | Control |
| --- | --- |
| Select one panel | Click it (no paint tool on) |
| Add or remove a panel | Shift+click |
| Select none | Esc, or click empty viewport space |

With **Brush**, **Eraser** or **Replace** on, a plain click paints, as
before, and Shift+click still selects without painting, so a selection
survives painting (as in Blender's paint modes, painting never changes the
selection). Selected panels show amber edges; with no paint tool on they
also fill amber-deep, and while one is on only the edges show so strokes
stay visible. The viewport overlay ("3 panels selected") and the **Face
textures** panel give the count.

It is one selection with Edit Mesh's faces: **Tab** enters Edit Mesh in face
select with the same faces, and leaving Edit Mesh keeps them. The selected
panels feed **Clone texture for selected faces**, **Assign texture…**, **Use
shape texture**, **Replace color…** (which opens on **Selected panels**) and
**Remap selected panels from view…**.

### Per-panel textures

An aircraft normally draws every textured face from one atlas PIC: the shape
selects the texture once and every face after it uses it. To paint some
panels without changing the rest, give them their own PIC.

Select the faces in **Edit Mesh** (face select, **3**), or click and
Shift+click panels in the Model workspace or the Paint workspace's model
preview (see [Select panels](#select-panels)). The **Face textures** panel in
the inspector (and the **Mesh** menu in Edit Mesh) lists each selected
face's texture and offers:

- **Clone texture for selected faces** (the primary action) copies the
  faces' PIC to a new private 8.3 name, suggested as the first six letters of
  the source and `T1` (`_F18.PIC` becomes `_F18T1.PIC`), and draws only the
  selected faces from the copy with their UVs unchanged. Every other face
  keeps the original PIC. A stored original `X.ORG` is copied with it, as for
  other clones. The SH, the new PIC and its original are one undo step. The
  name must be unused in this LIB, the other open LIBs and the source
  catalogs.
- **Assign texture…** opens a list of the PICs in this LIB. Type to filter
  it, click a row or use Up and Down, and choose the UV mapping:
  - **Keep** leaves the stored UVs as they are. It needs a PIC of the same
    size as the faces' current one; otherwise it is off and the dialog says
    why.
  - **Scale** scales the UVs from the current PIC's size to the new one,
    rounding to whole pixels. Faces whose UVs no longer fit a byte are
    widened to word UVs.
  - **Project** maps the faces flat onto the PIC with square pixels, fitted
    to its size: **Auto** uses the faces' own plane (longest edge along the
    width), **Top**, **Side** and **Front** look along an axis. Untextured
    faces can only be projected; they become textured faces.
- **Remap selected panels from view…** makes a new PIC laid out as the view
  shows the panels; see [Remap panels from the view](#remap-panels-from-the-view).
- **Use shape texture** returns faces to the shape's own texture: their
  original records go back exactly, keeping later moves or flips of the face.
  It applies to faces Hangar assigned; on any other face the status reads "No
  Hangar texture assignment to remove".

Without a selection the per-face actions are off and the panel reads "Select
faces in Edit Mesh or pick a face to paint". **Clone texture for whole shape**
stays beside them for the whole-aircraft copy.

Once assigned, the faces paint into their own PIC: the brush, eraser and
decals on those faces change only that PIC. Turn on **Panel lock** to keep a
stroke on the selected panel; without it, a stroke that crosses onto a
neighbouring face paints that face's PIC.

Assigned faces keep their part, so moving parts still move them, and their
draw order. Selected faces that follow each other in the shape share one
detour; scattered faces get one each. Hangar must prove which texture each
face draws with: it follows every path through the shape, through loops,
part calls and moving-part code, and needs the same texture on all of them.
A face that a part stub resumes drawing at, a face with a pointer or
relocation inside it, and a face reached under more than one texture (the
reason names them, for example "different textures reach it on different
paths: _F18.PIC (E2 at CODE+71) and the E0 record at CODE+28A8") are refused.
The three dialogs check this when they open: a refused selection shows the
reason in full and the primary button stays off, so pick other faces or
Cancel. Damage shapes
(`_A` to `_D`) are separate geometry: their faces never match the main shape
by position and bytes in retail data, so Hangar does not offer to apply an
assignment to them; assign their faces separately.

### Remap panels from the view

Some panels draw a thin strip of the atlas stretched over a much larger
area, so their texels are tall or wide on the model and paint smears along
one direction. **Remap selected panels from view…** (in the **Face
textures** panel, the Paint panel and the **Mesh** menu) gives the selected
panels a new PIC laid out exactly as the viewport shows them, with square
texels.

1. Select the panels (click and Shift+click, or Edit Mesh face select).
2. Orbit with the middle mouse button, or press **1**, **3** or **7** (front,
   side, top), until the panels face you. The layout is the orthographic
   view: the new texture holds the panels as you see them, so a panel seen
   at an angle is foreshortened in it, and the texture reads correctly from
   the panel's front, never mirrored.
3. Choose the action. The dialog shows the new name (a private 8.3 name,
   suggested as for a clone, for example `_F18T1.PIC`; type another), the
   sheet size with the panels' extent inside it, the texel density, and
   **Fill**:
   - **Bake current look** (the default): every texel the panels cover takes
     what the panel shows there now, sampled from its current texture at the
     nearest pixel; flat panels bake their colour. Where panels overlap in
     the view, the nearer one wins, as on screen.
   - **Blank**: the panels' most common colour, ready to paint from scratch.
   Around the panels, texels repeat the nearest panel edge for 3 pixels and
   the rest of the sheet takes the panels' most common colour.
4. **Remap** draws the panels from the new PIC as one undo step. They show
   it at once in the textured viewport and on the atlas, and the brush,
   eraser and Replace paint only the new PIC on them. **Use shape texture**
   returns them to the shape's texture with their original records.

The density is the shape's own texels per unit, as for generated panel
sheets, so the remapped panels match their neighbours. The new PIC has the
layout of every texture retail shapes draw from: 256 pixels wide, as tall as
the panels need up to 1,280 rows (at least 8), a row table and no palette of
its own, so its pixels are game-palette indices like retail skins. The
panels sit at the left with a 2-pixel margin; the rest of each row is
padding. Panels wider than 252 texels or taller than 1,276 shrink, both
directions together. The game palette (`PALETTE.PAL` in the LIB, or one
loaded) is required.

Refused with the reason, changing nothing: a panel nearly edge-on to the
view (it must face you within about 75°; "Turn the view to face the
panel"), a panel seen from behind, runtime markings without a named
texture, and every face Assign texture refuses (texture state not proved, a
part stub resuming at it, pointers inside it). The new PIC keeps no `.ORG`
until it is first painted; `X.ORG` then holds the remapped texture, which
the eraser and **Restore texture** bring back.

#### Fixing a stretched panel

A tail fin that smears every brush dab into a line along the fin maps a
strip of the atlas one pixel wide: all its corners share one U. To give it
a proper layout:

1. Turn the view so the fin's side faces you: **View · Side** (**3**; in
   Edit Mesh, numpad 3) shows one side square on. For the other side, orbit
   half a turn with the middle mouse button.
2. Click the fin's panels on this side, Shift+click to add the rest. The
   **Face textures** panel lists their texture.
3. **Remap selected panels from view…**, keep **Bake current look**, and
   **Remap**. The fin now draws from its own PIC with square texels, looking
   as before; one painted pixel is one texel on the fin.
4. The panels on the far side face away and are refused ("faces away from
   the view"). Orbit to the other side and remap them the same way; each
   side gets its own PIC, so neither reads mirrored.

**Use shape texture** puts the fin back on the atlas; **Ctrl+Z** undoes each
remap.

### Replace a color

A PIC stores a palette index per pixel. **Replace** changes the pixels of one
index, or of similar colors, to the paint color and leaves every other pixel
alone.

**Replace brush.** The Paint tool control reads **Brush**, **Eraser**,
**Replace**, **Pick**; the Model inspector's 3D brush row has **Brush**,
**Eraser**, **Replace**. Choose **Replace**, then hold **Alt** and click the
color to replace on the atlas or on the model, or click **Pick** and then the
color. The Paint panel shows **Replace** [A] with [B]: A is the source index,
B the paint color chosen in the palette. Strokes use the brush size as usual
but change only opaque pixels whose index matches A; transparent pixels and
span holes are never touched. Release commits the stroke as one undo step;
Esc discards it.

**Tolerance** (0 to 64 steps) widens the match to indices whose color lies
within that distance of A: the straight-line RGB distance in 6-bit palette
steps, the units a PAL stores (one step is about 4 on a 0–255 scale). At 0
only index A matches, even when another index holds the same color. The
palette is the PIC's own, or the base palette, as for painting.

On the model, **Lock strokes to the panel** keeps a Replace stroke on the
locked panel and inside that panel's UV footprint. Faces with their own PIC
(see [Per-panel textures](#per-panel-textures)) replace in that PIC only. A
flat panel converts to a generated sheet only when its color matches A,
otherwise the status reads, for example, "Panel color 12 is not index 34;
nothing to replace".

**Replace color…** sits in the Paint panel, and in the **Mesh** menu and the
**Face textures** panel when faces are selected. It opens a dialog with:

- **From** and **To** swatches. Click one, then a palette cell, or click
  **Pick from image** and then the texture or the model (Esc returns to the
  dialog). From is the Replace source and To the paint color, shared with
  the brush.
- **Tolerance**, as for the brush.
- **Scope**: **Whole texture** (the selected PIC, or the picked panel's),
  **Selected panels** (only pixels inside the UV footprint of the selected
  or picked faces, in each PIC they draw from) or **Faces' textures** (every
  PIC the selected faces draw from, whole; offered when there is more than
  one). Edit Mesh opens on Selected panels.
- A live count, for example "2,164 pixels will change in _F18.PIC."

**Replace pixels** applies every affected PIC as one undo step. The status
says what changed, for example "Replaced 1,204 pixels of index 34 with 112 in
_F18.PIC; original kept as _F18.ORG. One undo step."

As with the brush, the first replace of an existing PIC keeps `X.ORG`, panel
sheets generated in this session keep none, and the **Eraser** and
**Restore texture** bring replaced pixels back. Shared UVs and shared PICs
mean other panels change too. A UV footprint covers the pixels whose centres
lie inside or on the face's UV polygon.

### Erase and restore textures

The first time a stroke (Brush or Replace), decal, **Replace color…**, PIC
palette edit or **Replace entry** changes an existing `X.PIC`, Hangar keeps the entry as it was as `X.ORG` in the same
LIB, in the same undo step. The name keeps any prefix (`_F18.ORG`, `~F18H.ORG`,
`$AIM9.ORG`); the stored bytes and compression flag are copied exactly. Later
edits never touch it, so `X.ORG` stays the artwork from before the first edit.
Undoing that first edit removes both.

- **Eraser** sits beside **Brush** in Paint, and in the Model inspector for 3D
  painting. It paints the original back under the same brush circle, one undo
  step per stroke, and changes raster bytes only. Erasing every painted pixel
  returns the texture to its exact saved bytes; an `X.ORG` kept during this
  session is then removed too, so the LIB reads as unchanged.
- **Restore texture** replaces `X.PIC` with `X.ORG` byte for byte and removes
  `X.ORG`, as one undo step. The status reads, for example, "Restored _F18.PIC
  from _F18.ORG". If the original came from the opened file, the texture shows
  as unchanged again.
- The Paint panel shows the stored original, for example **Original kept**
  `_F18.ORG` with its stored size, 14,476 B.
- Panel sheets generated in this session keep no `.ORG`. Their original is the
  panel's solid face color, which the eraser and Restore texture paint back.
  Sheets that were already in the opened LIB are backed up like any texture.
- Without an `X.ORG`, the eraser and Restore texture use the entry as it was at
  the last open or save, for this session only. Otherwise the status reads "No
  stored original for X.PIC".
- `.ORG` entries appear under **Original textures** in the outliner, collapsed
  by default. Selecting one previews it read-only, with **Open X.PIC** and
  **Restore texture**. Painting, decals and palette edits apply to the PIC.
- Rename, Delete, Ctrl+D, copy and move between LIBs, **Clone texture for
  whole shape**, **Clone texture for selected faces**, family texture clones
  and **Export object** carry the stored original with its PIC. CLI `replace` also keeps it.
- **Package > Remove stored originals** removes every `.ORG` as one undo step
  for a distribution build. A later edit keeps a new original, taken from the
  entry as it was at the last open or save, so remove them just before
  packaging.

This is not retroactive: textures painted with earlier versions have no stored
original. An existing `X.ORG` that is not a PIC is never used, overwritten or
removed, and that texture gets no stored original. FA looks resources up by
name and should ignore `.ORG` entries; this is still to be confirmed in the
original game (see [WINDOWS-TEST.md](WINDOWS-TEST.md)).

## Hardpoints, materials and decals

Select an aircraft PT and click **Hardpoints** above the model. Steel diamonds
mark its stations; the selected station turns amber. Step between stations
with the chevrons beside "HP1 of 2". Drag a diamond in an orthographic view,
scrub or type the X, Y and Z number fields under **Location**, or press G,
X/Y/Z, an offset and Enter. H places a new station at the cursor's view-plane position. Add,
duplicate, remove, move and store assignments each form one undo step. The
inspector also exposes weight class, item count, location code and flags;
**All station fields** opens the structured table for slew settings and other
source operands.

Coordinates and classifications remain stored FA values. A shared SH requires
selecting its owning PT explicitly. Adding supports up to 64 stations; removing
the last station is refused, so clear its store instead. Changing a store makes
a private BRF reference block without changing another station's shared block.
A valid resource name does not guarantee in-game weapon/station compatibility.

The livery inspector has **Paint**, **Materials** and **Decals** tabs. Materials
can edit saved PAL/embedded-PIC colors as 6-bit RGB components (0..63),
transform the selected face's stored UVs, and clone a texture across the
selected aircraft's reachable SH family. The latter scans complete stored
module sections, including hidden texture names, rather than only the displayed
pose. Palette edits affect all palette users; shared SH records and UVs remain
shared. The cloned aircraft-ID PAL is an editor preview palette, not an
override of FA's global PALETTE.PAL. Materials identifies that case explicitly.
Use the intended game palette when judging a livery, and Export object when you
need private assets.

To add a marking:

1. Select a textured model panel or open its PIC, then choose **Decals**.
2. Import a transparent PNG, enter tail-number text, or choose a national
   preset. Built-in presets are US stars-and-bars, UK and French roundels, and
   the Japanese roundel. These are compact editor artwork; import PNG for exact
   national variants or authentic squadron artwork.
3. Imported PNG paths are remembered in the **Squadron library**. The list
   holds 16 files and is saved beside the executable in tore-hangar-decals.txt.
   Artwork stays in its original file; removing a list item does not delete it.
   A read-only executable folder retains new selections for the session only.
4. Click or drag on the texture atlas or a visible model panel. Center,
   width (px), rotation (°) and opacity (%) are number fields under
   **Placement**; **Mirror horizontally** is a checkbox. Tail text uses built-in block
   lettering, up to 24 ASCII letters/digits, spaces, dashes, slashes or periods,
   with a selectable palette ink color.
5. Inspect the palette-mapped texture and live model preview. **Apply decal**
   bakes it into the PIC as one undo operation; Cancel or Esc discards the
   preview. Save the LIB explicitly to write it to disk.

PNG import accepts non-interlaced static grayscale, RGB, indexed and alpha PNGs
at standard sample depths, up to 2048x2048 and 16 MiB. Alpha is blended into the
existing texture and mapped to the nearest palette color. PIC transparency
holes, headers and span tables are preserved; decals cannot create pixels
outside existing opaque spans. A complete usable palette is required. The final
LIB contains indexed pixels, not editable decal layers or a new game decal
system. Mirrored/shared UVs can put the same marking on other panels; the
preview shows those effects. PNG parsing follows the
[PNG format specification](https://www.w3.org/TR/png-3/).

## Ship, ground and animation tools

NT definitions now expose the reviewed Object + NPC fields and linked stations.
Select a ship, tank, AAA or launcher, then use **Hardpoints** for source
positions and stores. **Station data** switches between **Loadout** and
**Slew** (heading, pitch and slew limits). These are stored values, not invented degree or
distance conversions. The Properties groups also expose movement acceleration
and engagement/firing parameters. Shape animation and weapon launch behavior
are separate contracts.

**Parts** in the viewport mode select lists moving parts, previews poses and
changes their settings; see [Moving parts](#moving-parts). Turret tracking,
smooth animation playback and new animated parts are not available.

The local developer command below creates baseline/edited weapon, building,
ship, tank, AAA, SAM and mobile-launcher packages, plus F-18/A-10 texture and
part-placement cases. Supply your own game LIBs and a new output directory:

```sh
cargo run --locked -p hangar-core --example acceptance -- FA_2.LIB FA_1.LIB NEW_TEST_FOLDER
```

The folder contains READ-ME.txt and a pending results sheet. Test one candidate
folder at a time. Version 0.8.2 places generated panel records before the SH end
marker, relocates the import tail and writes native module packing. Tested A-10
and F-18 outputs pass exact independent SH round trips; original-game acceptance
still requires a game test. Part-placement candidates retain the original layout.

For LIBs painted with earlier builds, select the affected SH (or its linked
aircraft) and choose **Entry > Repair generated panel mappings**, then save the
custom LIB. Repair is one undo step and keeps every PIC unchanged. The CLI can
instead write a separate new file:

```sh
cargo run --locked -- repair-panels INPUT.LIB MODEL.SH FIXED.LIB
```

Do not load the original and repaired copies together: they retain the same
resource names. Use 0.8.2 or newer for further automatic panel texture creation.

## Edit shapes

**Tab** (or **Edit Mesh** in the viewport mode select) edits the selected SH,
or the shape of the selected aircraft, in place. Retail aircraft are edited
per region: before anything changes, Hangar proves which stored vertex every
face shows and that no pointer, relocation or native code touches the bytes
it rewrites. Whatever cannot be proved is refused with the reason, and the
rest of the shape stays editable. Every operation below is one undo step.

### Select

| Action | Control |
| --- | --- |
| Vertex select / face select | **1** / **3**, the header's vertex and face buttons, or **Select** menu |
| Select one | Click a vertex marker, or click a face in face select |
| Add or remove from the selection | Shift+click |
| Select everything / nothing | **A** |
| Box select | Drag on empty space, or **B** then drag anywhere; Shift extends, Ctrl removes |
| Select the part under the pointer | **L** |
| Select all faces of the selected faces' parts | **Select > Select linked part** |
| Invert | **Select > Invert** |

Faces are picked from what the viewport draws: in **Solid** and **Textured**
shading the face under the pointer, in **Wireframe** the nearest face whose
outline contains the pointer. Face select shows a dot at each face centre.
In Solid and Textured shading selected faces fill amber-deep with amber
edges; in Wireframe their edges turn amber. The overlay and inspector show
the count. Switching modes carries the selection over: faces
select their corners, and in vertex select an operation uses every face whose
corners are all selected.

The **Selection** panel names the part that draws the first selected face or
vertex (for example **Gear left**, or **Body (root frame)**) and its source
offset.

### Operations

| Operation | Key | Menu |
| --- | --- | --- |
| Move, rotate, scale | **G**, **R**, **S**, then X/Y/Z and a value | **Mesh > Move** |
| Delete faces | **X** or **Delete** | **Mesh > Delete faces** |
| Flip normals | **Alt+N** | **Mesh > Flip normals** |
| Duplicate, then move | **Shift+D**, then a move as for G | **Mesh > Duplicate** |
| Extrude, then move | **E**, then a move as for G | **Mesh > Extrude** |
| Make a face from the selected vertices | **F** | **Mesh > Make face** |
| Add a vertex at the median | | **Mesh > Add vertex at median** |
| Give the selected faces a copy of their PIC | | **Mesh > Clone texture for selected faces** |
| Draw the selected faces from another PIC | | **Mesh > Assign texture…** |
| Return faces to the shape's texture | | **Mesh > Use shape texture** |

- **Pivot.** R and S turn or scale about the selection's median. With
  **Pivot: Individual** (inspector or Mesh menu) in face select, each
  connected group of selected faces scales or turns about its own centre,
  like Blender's Individual Origins.
- **Delete** replaces each face record with a same-size jump, so nothing
  else in the shape moves. **Flip** reverses a face's corner order and normal.
- **Duplicate** and **Extrude** preview while you type the move. Enter
  applies both steps as one undo step; Esc cancels both. Extrude keeps the
  moved cap selected and joins it to the edge of the selection with side
  faces; the original faces are removed, as in Blender.
- **Make face** orders the corners around their centre as seen in the view
  and faces the new polygon towards you.
- **New faces** copy the colour and shading of a face next to them, or take
  the flat colour picked in **Flat colour** (the base colour dialog). A
  textured neighbour gives its colour only, because its UVs do not fit the
  new corners.
- **Face textures** are described under
  [Per-panel textures](#per-panel-textures).
- New faces and vertices are drawn where an existing face of the same part
  is drawn: Hangar appends them before the shape's end marker and routes that
  face through them. They take free vertex slots below 640, the largest slot
  count of any retail shape.

### Refusals and limits

An operation that cannot be proved safe changes nothing. The status bar and
an amber notice in the inspector give the reason, for example:

- a face with a pointer or relocation inside it, or one native code
  addresses;
- a vertex whose slot shows a different vertex on another path;
- a face in a part the preview pose rotates (reset the pose in **Parts**,
  or use gear down, where the legs are at rest);
- per-vertex shaded new faces (new slots have no F6 vertex records);
- no CODE or relocation room for appended geometry, or a host face farther
  than a 16-bit jump from the end of CODE.

Hangar proves which vertex each corner shows, and which texture each face
draws with, by following every path through the shape at once: branches,
loops, part calls and every outcome of the moving-part code. A proof needs
one answer on all of them, so a refusal names what reaches the face instead
(another vertex for the slot, two textures, or a path with no texture
selected since the shape start). Texture refusals are explained under
[Per-panel textures](#per-panel-textures).

Hangar never recomputes BSP order, visibility planes, bounds or collision
records. New faces draw in the order of the face they are attached to. There
is no 3D cursor. Edited shapes have not been loaded in the original game yet;
see [WINDOWS-TEST.md](WINDOWS-TEST.md).

## Moving parts

**Parts** in the viewport mode select (or **Entry > Parts**) lists the
shape's moving parts: everything the shape's native stubs draw under a game
variable or turn with a transform. Parts are grouped by role with an icon:
**Gear** (legs, nose gear, gear meshes), **Control surfaces** (flaps, rudder,
canards, slats), **Wings**, **Brakes and hook**, **Bays**, **Engines**
(afterburner, thrust vectoring) and **Other**. Names come from the variables
the stub reads and the side of the part's pivot, for example **Gear left**,
**Nose gear**, **Flap left (state -1)** or **Hook (state 1)**. Toggled
meshes carry the game state they are drawn in, because the meaning of each
retail state is not verified. Gear doors driven by the same variables as the
legs are numbered (**Nose gear 2**). C4 parts no stub drives are listed as
**Static parts**.

Click a part, or click its geometry in the viewport, to select it. Its faces
are selected for Edit Mesh (Tab), highlighted, and its pivot is marked.

### Preview a pose

**Pose preview** sets the game variables the parts read. It changes only the
picture: it is never saved, never part of undo, and it stays set while you
edit, switch between Parts and Edit Mesh, or undo, because it is stored by
variable name (`_PLgearDown`), not by address. Selecting another entry
clears it. The viewport overlay lists the active pose in amber.

- **Gear down**, **Gear up**, **Flaps down** and **Afterburner** set the
  usual combinations.
- Toggles (gear down, flaps, rudder, brake, hook, afterburner, bay) are
  buttons with the values the shape and the retail census compare against.
- **gearPos** is shown in percent down: 100% is gearPos 0, 0% is -8192
  (OpenFA's range). Other transforms (swing wing, canards, bay doors) take
  raw source values. Backspace over a field returns it to unset.
- **Reset pose** clears everything.

### Change settings

The settings panel of a selected part shows the parameters its stub already
has. Each change rewrites one field in place, at the same size, then re-reads
the shape and checks that the stub still says exactly what was asked. Each is
one undo step.

| Setting | Control | Values |
| --- | --- | --- |
| Gate value | buttons | the values the retail census shows for that variable |
| Gate test | Equal / Not equal | je or jne after the compare |
| Swing range (gear) / Shift | Select | sar 1, 2 or 3 where the stub's bytes allow; swing-wing terms 1 to 7 |
| Direction (gear) | Select | NEG or No NEG (opposite fold directions), where the stub's bytes allow |
| Rotation axis | Select | yaw (r0), pitch (r1), roll (r2) |
| Pivot | X, Y, Z number fields | the part's C4 translation, in source units |

Gear swing range is shown in degrees assuming gearPos runs from 0 to -8192:
sar 1 swings 90°, sar 2 45°, sar 3 22°.

**Not seen in retail.** Gear direction and range changes may use encodings
that no retail shape contains: `mov ax,ax` in place of NEG, or a 4-byte
`sar ax, n` with a 2-byte no-op. Hangar's stub evaluator proves the new law,
but the game has not run them yet. Such options carry a **Not seen in
retail** badge in the Select, the setting shows the badge while it is in
effect, and the part is marked **NOT RETAIL** in the list. Returning to the
census form restores the original bytes exactly.

What cannot change, and why:

- **Locked** parts: their stubs use code outside the reviewed subset (radar
  and turret stubs, insect wings, ejection logic), or variables that are not
  reviewed aircraft parts (`_PLstate`, `_PLdead`, `_currentTicks`). The
  reason is shown.
- A gear direction or range that does not fit the stub's bytes: a stub with
  only a 3-byte `sar ax, 1` has no room for a NEG or a larger shift; NEG with
  shift 2 or 3 needs 7 bytes. The Select offers only what fits, and the reason
  is shown under fixed settings.
- Direction of other transforms (swing wing, canards, bay doors): only gear
  laws are reviewed for same-size changes.
- A pivot that native code writes at run time.
- New parts, new stubs, new imports or a different law: out of scope.

The preview applies the same law the stub evaluator reads, so a reversed leg
folds the other way in the viewport. Whether the game agrees is listed in
[WINDOWS-TEST.md](WINDOWS-TEST.md).

## Controls

| Action | Control |
| --- | --- |
| Open LIB / package new LIB | Ctrl+O / Ctrl+S, or File menu |
| Add entry / export selected entry | Ctrl+I / Ctrl+E |
| Copy / paste resources | Ctrl+C / Ctrl+V, or drag to another LIB root |
| Entry or LIB context menu | Right-click it in the outliner |
| Cancel an entry drag | Esc or right mouse button |
| Duplicate resource / close active LIB | Ctrl+D / Ctrl+W |
| Replace / export OBJ | Inspector buttons |
| Search entry names | Ctrl+F or search field; Esc leaves search |
| Select entry | Click or Up/Down; wheel scrolls the outliner |
| Edit a definition operand | Drag its number field to scrub, click to type (Ctrl+A clears), Backspace restores the saved value |
| Collapse a panel / keep only one | Click its header / Ctrl+click its header |
| Show only aircraft, shapes or images | Type buttons beside the outliner filter |
| Scroll fields | Wheel over panels, Flight workspace or Raw fields |
| Graft characteristic groups | Entry > Use as graft donor, select target, open Graft |
| Copy one donor field | Select a field, Tools > Copy one donor field |
| Remove selected entry | Delete; Ctrl+Z restores it |
| Undo / redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Orbit / pan | Middle-drag / Shift+middle-drag |
| Zoom / frame | Wheel over viewport / Home or period |
| Shading | Viewport header: Wireframe / Solid / Textured, or View menu |
| View along an axis | Click an axis cap on the navigation gizmo |
| Front / side / top / projection | 1 / 3 / 7 / 5, including numpad (in Edit Mesh, the top-row 1 and 3 switch select modes) |
| Edit Mesh | Tab; see [Edit shapes](#edit-shapes) for its keys |
| Vertex / face select | 1 / 3 in Edit Mesh (numpad 1 and 3 still change the view) |
| Select in Edit Mesh | Click, Shift+click, A, B or drag for a box (Ctrl removes), L for the part under the pointer |
| Delete / flip / duplicate / extrude / make face | X or Delete / Alt+N / Shift+D / E / F |
| Preview a moving part's state | Parts: Pose preview buttons and fields (never saved) |
| Transform supported static shape | G / R / S, X/Y/Z toggles axis lock, numeric value, Enter |
| Hardpoint placement / movement | H at cursor; drag diamond or G then X/Y/Z |
| Decal placement | Click/drag on atlas or model; Apply decal / Esc cancel |
| Erase paint / restore a texture | Paint panel: Brush / Eraser control, Restore texture |
| Replace a color | Paint panel: Replace, then Alt+click the color (or Pick); Replace color… for a whole texture or panels |
| Select panels (Model viewport, Paint model preview) | Click; Shift+click adds or removes (also with the brush on); Esc or empty space clears |
| Remap stretched panels | Select them, orbit until they face you, Remap selected panels from view… |
| Cancel transform or dialog | Esc or right mouse button |
| Close with unsaved edits | Click Discard changes, or Cancel/Esc to return |

G/R/S start with no axis lock; X, Y or Z locks that axis and pressing it again
removes the lock. Translation is in integer source coordinates: with no lock
one value moves along X and three values (`X Y Z`) move freely. Rotation is in
degrees about the locked axis, or about the principal axis nearest the view
direction. Scale is in percent on the locked axis, or uniform with no lock.
In Object mode transforms pivot on the shape origin; in Edit Mesh they act
only on the selection and pivot on its median point, or with **Pivot:
Individual** on each group of selected faces.

## Limits and why

Every constraint Hangar imposes, why it exists and what to do instead. The
evidence behind each reason is in
[ARCHITECTURE.md](ARCHITECTURE.md) and [VALIDATION.md](VALIDATION.md);
where no reason is recorded, the entry says **reason unverified**. Features
not yet available are listed under
[Scope still ahead](../README.md#scope-still-ahead).

An operation Hangar cannot prove safe changes nothing and says why. Hangar
cannot see inside the game, so its rules come from three sources: what
FA.EXE is known to do (from its disassembly), what every retail file has in
common (censuses of the retail LIBs), and what Hangar can prove about the
bytes it rewrites.

### Why does Hangar refuse this?

Messages are quoted as Hangar prints them; `{…}` stands for a name or
number.

| Message | Entry |
| --- | --- |
| "{name} is a protected retail LIB. Open/extract is allowed; save with a different LIB name." | [Retail LIB names](#retail-lib-names) |
| "Choose a LIB name with .LIB only at its end: FA loads every file whose name contains .LIB" | [FA loader limits](#fa-loader-limits) |
| "{n} LIB files would load from this folder; FA keeps at most 20." | [FA loader limits](#fa-loader-limits) |
| "{n} resources ({n} in LIBs + {n} loose files); FA's resource table holds 9,950." | [FA loader limits](#fa-loader-limits) |
| "{name} is {n} characters; FA's LIB name slot holds 13." | [FA loader limits](#fa-loader-limits) |
| "FA loads {name} as a LIB; move it out of the game folder." | [FA loader limits](#fa-loader-limits) |
| "Would crash FA's texture mapper" (Package checks) | [Textures Hangar creates](#textures-hangar-creates) |
| "{name} is not an FA texture ({why}); FA would crash drawing it." | [Textures Hangar creates](#textures-hangar-creates) |
| "Load the base PAL before generating a panel texture" | [Palette](#palette) |
| "Load the base .PAL before painting this partial-palette PIC" | [Palette](#palette) |
| "Use 1..6 letters, digits or underscore; suffixes reserve two characters" | [Names](#names) |
| "{name}: {new} exceeds a compiled filename slot ({n} bytes)" | [Names](#names) |
| "{old} has a short compiled name slot; use a shorter object ID" | [Names](#names) |
| "{name} -> {new} is longer than an 8.3 name" | [Names](#names) |
| "Missions and other LIBs that refer to {name} by name will not find the renamed aircraft." | [Missions and other LIBs](#missions-and-other-libs) |
| "Archive exceeds 128 MiB limit", "File exceeds 128 MiB limit" | [Archive and resource sizes](#archive-and-resource-sizes) |
| "{label} contains a pointer or relocation field at CODE+{offset}" | [Shape geometry](#shape-geometry) |
| "native code addresses {label} at CODE+{offset}" | [Shape geometry](#shape-geometry) |
| "Vertex at {offset} is not current at the host face: slot {n} shows another vertex" | [Shape geometry](#shape-geometry) |
| "Face at {offset}: Hangar cannot prove which texture draws it: {why}" | [Shape geometry](#shape-geometry) |
| "Face at {offset}: a part stub resumes drawing at this face, so it cannot be moved" | [Shape geometry](#shape-geometry) |
| "A selected vertex is in a rotated part; reset its pose or edit it in local coordinates" | [Shape geometry](#shape-geometry) |
| "Per-vertex shaded faces need F6 vertex records; choose a flat or textured style" | [Shape geometry](#shape-geometry) |
| "CODE has no virtual-address room for this continuation; relocation support is required" | [Shape geometry](#shape-geometry) |
| "Relocation table has no room for the moved import tail" | [Shape geometry](#shape-geometry) |
| "Panel continuation exceeds the 16-bit SH jump reach" | [Shape geometry](#shape-geometry) |
| "No {n} free vertex slots below the retail ceiling of 640" | [Shape geometry](#shape-geometry) |
| "Appending geometry needs the native end marker and import tail" | [Shape geometry](#shape-geometry) |
| "A face centre exceeds its byte width; the record would have to grow" | [Shape geometry](#shape-geometry) |
| "Face at {offset} is nearly edge-on to the view; turn the view to face the panel" | [Textures Hangar creates](#textures-hangar-creates) |
| "Face at {offset} draws runtime markings (no named texture); it cannot be remapped" | [Runtime markings](#runtime-markings) |
| "The stub's code is outside the reviewed subset at CODE+{offset} ({why}); it is never edited" | [Moving part settings](#moving-part-settings) |
| "NEG with shift {n} needs 7 bytes and this law slot has 6; set shift 1 first" | [Moving part settings](#moving-part-settings) |
| **Not seen in retail** (badge) | [Moving part settings](#moving-part-settings) |
| "No stored original for {name}" | [Stored originals](#stored-originals) |

### Textures Hangar creates

Generated panel sheets, **Remap selected panels from view…** and **Repair
textures for FA** all write one layout. Clones, exports and **Assign
texture…** do not create pixels: they copy or point at existing PICs.

| Limit | Why | Instead |
| --- | --- | --- |
| Every texture Hangar creates is kind 0 (raw raster), exactly 256 pixels wide, has a row-offset table and no embedded palette | All 1,070 textures retail shapes draw on textured faces have this layout. FA.EXE reads the row table while setting up a textured polygon (`mov ecx,[ecx+ebp*4]` at 0x4CAF0D); a 64 × 64 generated sheet without one crashed FA in the external view. Other widths are unverified in the game. | Nothing to choose. To give a panel a different shape, see the next rows. |
| At most 1,280 rows | FA bounds the texture row by 0x500 (1,280) at the same address. | Remap fewer panels at once; larger extents shrink to fit. |
| A generated panel sheet holds its panel in a sub-rectangle at the left edge, as tall as the panel area; the rest repeats the panel color | The panel takes its own proportions at square texels, but the sheet must still be 256 wide (row above). The panel's UVs address that sub-rectangle. | Paint inside the panel area; the padding is never drawn. |
| Panel area size comes from the aircraft's texel density, each side 8 to 256 pixels, both scaled together | Density is the texels per unit the shape's textured faces already use (2.2 on the A-10, 3.1 on the F-18, 9.4 on the F-14; 3 when it has none), so new texels match their neighbours. 256 is the sheet width; the 8-pixel minimum is **reason unverified**. | None; the density follows the shape. |
| Remap from view fits the panels into 252 × 1,276 with a 2-pixel margin, one scale for both axes | One scale keeps texels square; 252 and 1,276 leave the margin inside 256 × 1,280. The margin repeats the nearest panel texel; the 2-pixel width is **reason unverified**. | Remap smaller groups for more texels each. |
| Remap refuses panels more than about 75° from the view, or seen from behind | Under a quarter of its true area in the view, the face would get the stretched texels Remap exists to fix; a back face is one the renderer culls. | Orbit until the panels face you, or remap them in groups by direction. |
| **Assign texture…** can point faces at any PIC; it warns when FA cannot map it | Assign creates no PIC, so it cannot fix one; Package checks list it as an error. | Run **Repair textures for FA**. |
| Sheets made by earlier Hangar versions (64 × 64 or panel-sized, with a palette, no row table) crash FA | Same row-table read as above; this is the TopGun.LIB crash. | Package checks flag them; **Repair textures for FA** widens each to 256 with every pixel at the same UV, adds the row table, drops the palette, and changes no SH. See [SH textures](#sh-textures). |
| Repair does not change a PIC wider than 256 | Its UVs cannot all be kept in 256 columns. | Assign the faces a PIC at most 256 wide (Scale or Project), or remap them. |

### Palette

| Limit | Why | Instead |
| --- | --- | --- |
| PIC pixels are indices into the game palette; Hangar writes no embedded palette in textures | Retail SH textures carry none (palette size 0 in all 1,070), so the layout above has none. Whether FA would use an embedded palette on an SH texture is unverified. | Pick colors from the game palette. |
| Painting a generated sheet, or any PIC without a full palette of its own, needs a base palette | The PIC holds only indices; without a base palette Hangar cannot show or match colors, so it shows grayscale and blocks painting. Hangar finds one as in [Display palette](#display-palette). | Keep FA_2.LIB in the same folder or open it, or use **Load palette…** with a 768-byte PAL or a LIB containing `PALETTE.PAL`. |
| A cloned aircraft's `<ID>.PAL` is an editor preview palette | FA's palette lookup is global; Hangar does not override it. | Judge a livery against the game palette. |
| Repair maps colors to the nearest base color only where the old embedded palette differs | Indices are kept when the embedded palette equals the base PAL (as in TopGun.LIB); otherwise the status says colors were mapped. Without a base PAL, indices are kept and reported as unverified. | Load the base PAL before repairing. |

### Palette companions

Which palette Hangar shows is in [Display palette](#display-palette).

| Limit | Why | Instead |
| --- | --- | --- |
| Copies, moves, duplicates and exports carry an object's palette as `<ID>.PAL`, never as `PALETTE.PAL`; a target that has `PALETTE.PAL` gets none | FA loads every LIB in its folder and keeps the newest file's copy of a duplicate name, so a `PALETTE.PAL` in a mod LIB would recolor every aircraft in the game. FA does not read `<ID>.PAL`; only Hangar uses it, to show the object's colors. | Nothing to choose; **Skip** leaves it out. |
| Package checks warn about a custom `PALETTE.PAL` that differs from the game's | Same reason: it would replace the game palette for everything. | Remove it unless recoloring the whole game is intended. |
| A shape's or texture's owner comes from stored references and the reviewed naming conventions in the current LIB; a shared texture takes the nearest owner's palette, aircraft first | Names built at run time leave nothing in the bytes, and other LIBs' objects are not scanned. Two owners can have different palettes; one is shown. | Select the owner, or use **Load palette…**. |
| The folder search reads the directories of up to 32 LIBs beside the current one, once per folder while the same LIBs are open | Selecting entries must stay fast; FA_2.LIB is read first. 32 is a bound, not a game limit. A LIB added to the folder later is found after a LIB is opened or closed. | Open the LIB that has the palette. |
| Only `PALETTE.PAL` from a retail LIB, or a loaded `PALETTE.PAL` file, is remembered between sessions | A mod's palette must not become the default for every LIB opened later. | **Load palette…** |

### Compression and LIB size

| Limit | Why | Instead |
| --- | --- | --- |
| A painted, decal-baked, palette-edited, replaced, repaired or newly created entry is stored uncompressed (flag 0); **Export object** writes its copies uncompressed | Hangar has a DCL decoder but no compressor (not implemented). | Expect the LIB to grow. Untouched entries, `.ORG` originals, texture clones within a LIB and an exact Restore texture keep their stored bytes and compression flag. Remove stored originals before distributing. |

### Names

| Limit | Why | Instead |
| --- | --- | --- |
| LIB entry names are ASCII 8.3 (DOS punctuation such as `$`, `#` and `^` is allowed) | The LIB directory stores each name in a 13-byte field. | Choose names of at most 8 characters plus extension. |
| Reference ID is 1 to 6 letters, digits or underscore | Private names add up to two characters to the ID (`F14_C.SH`, `_F14_A.PIC`, a clone's `T1`) and must still be 8.3. | Use a shorter ID. |
| Prefixes `~ _ $ & # ^` are kept; rename replaces the ID after them | Retail names use them and Hangar keeps naming conventions intact: `$AIM9.PIC` is the store icon of `AIM9`, `_F18.PIC` the F-18's texture, and the damage family is `F18_A.SH` to `F18_D.SH`. What `~`, `&`, `#` and `^` mean to the game is **reason unverified**; Hangar only preserves them. | Keep the prefix when naming by hand. |
| A rename is refused when the new name does not fit a compiled slot | Compiled modules store names in fixed slots: 14 bytes in an SH texture record (E2), 13 bytes in a HUD picture field, and the original length elsewhere. Hangar never moves bytes in a module or enlarges a section. | Choose a name no longer than the one it replaces. |
| **Rename resource** is refused for names the game finds by convention (damage family, default HUD, store icon, palette) and for users in other open LIBs | Renaming one member would break the family the game derives from the name. | Use [Rename reference ID](#rename-reference-id) for an aircraft and its private files. |
| Generated names: a panel sheet is the SH stem's first six characters and two hex digits (`F1800.PIC`); a clone is the first six letters and `T1` | Both stay 8.3. | Type another name in the dialog where offered. |

### FA loader limits

FA reads every file in its folder at startup and crashes, rather than
reporting an error, when the folder breaks one of these. Addresses are in
[ARCHITECTURE.md](ARCHITECTURE.md#fa-loader-limits-and-the-sh-texture-layout);
the save-time check is described under [The game folder](#the-game-folder).

| Limit | Why | Instead |
| --- | --- | --- |
| At most 20 LIB files | FA stores 20 LIB handles (at 0x54A648) before the LIB count; a 21st overruns them. | Merge mods or move unused LIBs out. |
| At most 9,950 resources: every LIB entry plus every other file in the folder | The resource table is allocated for 9,950 35-byte records (0x5505A bytes); FA crashed writing it (0x478F69) with Hangar backups present. The installed retail LIBs use about 7,520. | Keep backups, `.ORG` originals and loose files few. |
| LIB filenames at most 13 characters | The name is copied into a 14-byte slot without a length check; `TOPGUN.LIB.BAK` (14) overflows it. | Short LIB names. |
| Any file whose upper-cased name contains `.LIB` anywhere, with an `EALIB` header, loads as a LIB | FA tests the name with `strstr` for `.LIB`, case-insensitively after upper-casing. | Hangar's backups are `<STEM>.BAK`, then `.B01` to `.B99`, and the stage `<STEM>.TMP`, never `X.LIB.bak`; destinations with `.LIB` before the end are refused. Move old `X.LIB.bak` files out of the game folder. |

Saving into a folder with `FA.EXE` or a retail LIB counts what FA would load
after the save and shows it in the status; a save past a limit asks for
**Save anyway** in the GUI and is refused by the CLI.

### Retail LIB names

| Limit | Why | Instead |
| --- | --- | --- |
| Hangar never writes a file named like a retail LIB (`FA_1.LIB` … `_SETUP.LIB`, the list in [Protected LIBs and saving](#protected-libs-and-saving)), in any folder or letter case; there is no unlock switch | Overwriting an installed game LIB would destroy the user's game files. The list is the exact set from the installer and discs, not a `FA_*.LIB` pattern, so `SWPATCH.LIB` (a mod archive) stays writable. | Open, edit and save under another name; Ctrl+S suggests `HANGAR.LIB`. |

### Archive and resource sizes

These are Hangar's own bounds, not FA limits. They keep every read and scan
bounded so a huge or malformed file fails with a message; the exact figures
are **reason unverified**.

| Limit | Value | Instead |
| --- | --- | --- |
| LIB opened as a document (loaded whole into memory) | 128 MiB; FA_7, FA_10, FA_10B, FA_11 and FA_11B exceed it | Add them as source catalogs, or copy what you need into a smaller mod LIB. |
| Source LIB catalogs (**Add source LIB catalog**, object export) | up to 2 GiB each, 64 LIBs, 131,072 names; directory and range reads only | |
| Entries in one LIB | 1 to 65,535 | |
| Decoded resource / BRF text / decal PNG | 16 MiB / 1 MiB / 16 MiB and 2048 × 2048 | |
| Copy, export and rename graphs | 4,096 resources, 128 MiB decoded | Split the work. |
| Undo history | 64 operations, payload history trimmed above 32 MiB | Save between large batches. |

### Shape geometry

Hangar edits an SH by rewriting only bytes it can prove nothing else depends
on. It is not a complete SH writer: it never recomputes BSP order,
visibility planes, bounds or collision records, because it has no writer for
them yet. See [Refusals and limits](#refusals-and-limits) for how refusals
appear and how Hangar proves vertices and textures, and [Fixing a stretched
panel](#fixing-a-stretched-panel) for panels whose texture smears.

| Limit | Why | Instead |
| --- | --- | --- |
| A face with a pointer or relocation inside it, a pointer target inside it, or native code addressing it cannot be rewritten, deleted, flipped or used as a host | Other code jumps into or reads those bytes; changing them would break it. | Edit a neighbouring face, or move its vertices if they are writable. |
| A vertex is refused when its slot shows a different vertex on another path | SH faces name vertex slots, not vertices. Hangar proves which vertex a slot holds at each face by following every path that reaches it, through loops and calls; a slot rewritten on another path has no single answer. | Select the vertex where it is drawn alone. |
| A face whose texture state cannot be proved is refused for texture and layout operations | One E2 record selects the texture for every later face; Hangar must prove which selector reaches the face over every path. The refusal names what reaches it: two textures, or a path with no E2/E0 since the shape start. | For new geometry, name a texture; otherwise select faces drawn under one texture ([Per-panel textures](#per-panel-textures)). |
| A face a part stub resumes drawing at cannot be moved | The stub's native code jumps to that record; moving it would change where the part draws. | Leave it in place; edit the faces after it. |
| Model-space moves of a part the preview pose rotates are refused | Hangar converts model-space moves to stored coordinates only through unrotated part frames. | **Reset pose** in **Parts** (or gear down, where legs rest), or edit in local coordinates. |
| Faces never drawn are read-only | No path reaches them, so nothing can be proved about them. | |
| New faces cannot be per-vertex shaded | New slots carry no F6 vertex records. | Flat (0x63) or textured faces. |
| Appended geometry needs CODE virtual-address room and relocation-table room | New records go before the shape's end marker; the import tail moves and its relocations move with it. | Edit in place (move, flip, delete), which needs no room. |
| The host face must be within ±32 KiB of the continuation | The SH `48` jump has a 16-bit reach. | Edit in place. |
| New vertices take slots below 640 | 640 is the largest slot count of any retail FA_2.LIB shape (CITY2.SH); more is untested in the game. | Reuse existing vertices. |
| A shape without the native end marker and import tail cannot take appended geometry | A trailing end object would absorb the continuation and no whole-CODE reader would see it. | |
| Appended faces draw in the order of their host face | BSP placement of new faces is not recomputed. | Choose a host drawn where the new faces should appear. |
| Records never grow in place; a byte face centre that would overflow is refused | Growing a record would move every later byte. | Smaller moves, or delete and add the face. |
| Same-size in-place edits (move, flip, delete as a same-size stub, scale of a writable face) | Have none of the appending limits above. | |

The model view is a static-pose projection, not the game's full drawing
interpreter. Resource code is never executed. OBJ exports drop materials and
animation and cannot be imported back as a lossless SH edit.

### Moving part settings

| Limit | Why | Instead |
| --- | --- | --- |
| Only the settings a part's stub already has can change, each in place at the same size | After each change Hangar re-reads the stub and requires every instruction outside the changed field at the same address with the same kind and the binding to report the new value; a same-size change keeps that provable. | Use the settings table under [Change settings](#change-settings). |
| A gear direction or range that does not fit the stub's bytes is not offered | NEG with shift 2 or 3 needs 7 bytes; a 3-byte `sar ax, 1` has room for nothing else. | Reverse the direction first or set shift 1, as the message says. |
| **Not seen in retail** forms are allowed but badged | No retail shape contains them; the evaluator proves the law but the game has not run them. | Test in FA; returning to the census form restores the original bytes. |
| Locked parts (radars, turrets, insect wings, ejection logic; `_PLstate`, `_PLdead`, `_currentTicks`) | Their code is outside the reviewed subset or their variables are not reviewed aircraft parts. | None yet. |
| No new parts, stubs, imports or different laws | A new part needs new x86 stub code, a new import for the game variable it reads, and relocation entries; Hangar authors none of these. | Edit existing parts' settings. |
| Ailerons, elevators and the canopy cannot be animated | The FA_2.LIB part census found stubs for gear, flaps, rudder, brakes, hook, bays, afterburner, canards and swing wings, and none for ailerons, elevators or a canopy: they are static geometry in the retail shapes read. | |

### Damage family

| Limit | Why | Instead |
| --- | --- | --- |
| Per-face operations (Clone texture for selected faces, Assign texture, Remap) do not carry over to `_A` to `_D` | In FA_2.LIB no face of the F-18, F-16, A-10, F-22, F-14, MiG-29 or Su-27 main shape has a record with the same bytes at the same offset in any damage shape, so faces cannot be matched safely. | Repeat the operation on each damage shape, or use **Clone texture for whole shape** / the family texture clone in Materials, which follows stored texture references. |

### Stored originals

| Limit | Why | Instead |
| --- | --- | --- |
| `X.ORG` is kept only from the first edit made with this version | Earlier versions kept none, and Hangar cannot recover what was overwritten. | Restore from the source LIB or a backup. |
| Each `X.ORG` is a full copy of the texture | It keeps the stored bytes exactly; it is also one more LIB entry, which counts toward FA's 9,950 resources. | **Package > Remove stored originals** just before a distribution build. |
| An existing `X.ORG` that is not a PIC is never used, overwritten or removed | Hangar never adopts data it did not write. | Rename that entry. |
| FA should ignore `.ORG` entries | FA looks resources up by name; not yet confirmed in the game. | See [Verification status](#verification-status). |

### Source values

| Limit | Why | Instead |
| --- | --- | --- |
| Values are shown and edited in source storage units, not knots, pounds or Mach | No gameplay unit conversion has evidence behind it. A `^` marker is retained, not reinterpreted. Changing a field does not guarantee how the game uses it. | Compare against retail values. The gear swing is shown in degrees and gearPos in percent assuming OpenFA's 0 to -8192 range. Fields whose FA.EXE use is reviewed show their stored unit beside the value (`negGLimit` 1/256 s, throttle rates %/s). |
| Opaque binary definitions can be exported or replaced but are not edited | Unknown data is preserved, not guessed. | |

### References and package checks

The References dock indexes observed BRF strings and bounded module filename
literals, including names outside the displayed pose. It also identifies the
reviewed PT damage family, same-name default HUD and available store icons.
Each link names its evidence: BRF string, texture record (an SH E2 record
confirmed against the shape's record inventory), HUD texture name, module
filename or a naming convention. Use **Package > Add source LIB catalog** to
locate dependencies in other LIBs; these catalogs are directory-only and do
not load their payloads. Multiple external providers are reported as
ambiguous. **Export report** writes the change list and all retained check
results to a new text file.

Self-name literals are excluded from navigation. Counts cover observed users
in the current LIB, not every possible runtime lookup. External catalog matches
identify possible providers, not game load order or verified external payloads.
The aircraft-cloning wizard retains its broader source and aliasing rules.

Package checks also report, as information, aircraft whose LIB has no
palette Hangar can use on its own ("Colors in Hangar need a palette; add one
with Load palette or copy from FA_2.LIB"), and warn when a custom LIB's
`PALETTE.PAL` differs from the game's ("This would recolor the whole game").

Package checks are advisory. Supported payload checks apply to changed entries;
unknown encodings and incomplete scans remain unverified. Opening a saved LIB
establishes a new baseline, so CLI `validate` checks its archive and references
without treating every existing payload as newly edited.

### Missions and other LIBs

| Limit | Why | Instead |
| --- | --- | --- |
| **Rename reference ID** does not update missions | Missions find an aircraft by its file name (`F14.PT`); Hangar does not parse mission text, and other LIBs are not searched. The review counts the missions in this LIB that name the aircraft. | Keep the ID of an aircraft missions use, or **Duplicate aircraft** to add a new ID beside it. |
| Same-stem files with no stored link (`F14.PTS`, `F14.HUD`) keep their names | Nothing in the aircraft refers to them, so Hangar has no evidence the game pairs them. | Rename them by hand if needed. |

### Runtime markings

Some faces draw no named texture: the game fills them with markings at run
time. Hangar shows them blank in the preview and refuses to remap them.

- The shape decides whether a face carries a marking, where it sits, and
  which slot it uses: an `E0` record selects a runtime texture slot for the
  faces that follow, until an `E2` restores the skin. The community SH guide
  (cited in the T.O.R.E Fighters format notes, `objects-and-shapes.md`) names
  slots 0/1 left/right tail art, 2 nose art and 3/4 left/right wing markings;
  aircraft need not place them where the names suggest. Retail F5EV.SH, for
  example, draws one face under slot 4 and one under slot 3 (its roundels).
- The game picks the image for each slot at run time. That this follows the
  aircraft's nation or unit is **not yet verified**; where FA.EXE loads slot
  images has not been traced.

Tools for runtime markings are planned. Until then, bake markings into a
texture with [Decals](#hardpoints-materials-and-decals).

### Verification status

Hangar is tested on Linux (core tests, the shared-UI smoke test, CLI checks
against the user's retail LIBs) and in Windows CI (headless smoke test on
current 64-bit Windows, for both executables). See
[COMPATIBILITY.md](COMPATIBILITY.md) and [VALIDATION.md](VALIDATION.md).

| Area | Status |
| --- | --- |
| Windows 98/ME | Not yet run. The 32-bit build's imports are audited against an allow-list of Windows 98 APIs; that cannot prove it runs there. |
| Windows file paths | ASCII only: Hangar uses the ANSI Win32 API with no Unicode layer, so it runs on Windows 98. |
| Hangar output in the original game | Not yet confirmed. Three user reports from the game drove fixes: generated panels that vanished (A10_V2.LIB), the TopGun.LIB texture crash and the `.LIB.bak` startup crash. The fixes have not been retested in FA. |
| Edited shapes, part settings and **Not seen in retail** forms, `.ORG` entries, repaired textures, new sheet sizes, Replace, rename and duplicate | Pass Hangar's checks; not yet loaded in FA. |

Acceptance steps for each are in [WINDOWS-TEST.md](WINDOWS-TEST.md). Record
game results separately from a successful save: a valid LIB is not proof the
game accepts it.
