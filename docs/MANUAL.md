# T.O.R.E Hangar manual

How to use T.O.R.E Hangar to inspect and edit Fighters Anthology LIB files.
For downloads and builds see the [README](../README.md); for format and writer
decisions see [ARCHITECTURE.md](ARCHITECTURE.md).

**Early working editor, not a complete SH authoring tool.** LIB and textual
BRF editing work; general animated aircraft geometry remains read-only until
its spatial and control records can be rewritten safely. No game data ships.

## Contents

- [Getting started](#getting-started)
  - [Opening LIBs and the file browser](#opening-libs-and-the-file-browser)
  - [Workspaces](#workspaces)
- [What works](#what-works)
- [Protected LIBs and saving](#protected-libs-and-saving)
- [Command line](#command-line)
- [Export an object and its resources](#export-an-object-and-its-resources)
- [Work across LIBs](#work-across-libs)
- [Flight envelope table](#flight-envelope-table)
- [Paint a livery](#paint-a-livery)
- [Hardpoints, materials and decals](#hardpoints-materials-and-decals)
- [Ship, ground and animation tools](#ship-ground-and-animation-tools)
- [Controls](#controls)
- [Limits](#limits)
  - [Source values](#source-values)
  - [References and package checks](#references-and-package-checks)
  - [Models and loader](#models-and-loader)

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

The UI follows the supplied concept: one menu/workspace bar, categorized
outliner with type icons, linked aircraft/shape selection, grouped source-value
properties, and Browse/Model/Flight/Graft/Package/Paint workspaces. Raw fields
show saved values beside current values, with amber edits and reset controls.
Windows uses Tahoma for interface labels and Lucida Console for resource data.

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
- Export the selected object into a separate LIB with a new ID/display name.
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
- Move, rotate and scale the supported static SH subset. Tab opens vertex edit
  mode; click a vertex, drag in an orthographic view, or use G for numeric
  offsets. A selects all vertices. Shapes with unhandled spatial records,
  bounds, visibility logic or animation remain read-only. The synthetic demo
  exercises transforms without retail data.
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
`MYMOD.LIB.bak`, then `.bak.1`, `.bak.2`, and so on without replacing earlier
backups. The new file is written and flushed before the old file is moved.
If installation of the new file fails, Hangar tries to restore the old name;
if restoration fails, the error identifies the backup and staged file. A power
loss between moves can require restoring the backup manually. Backups are kept
beside the LIB until you remove them. Resource/PNG/WAV/OBJ exports remain
create-new. Retail name protection does not affect reading from game discs.

A packaging error leaves edits in memory and displays its cause. The app writes
only when explicitly asked.

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

Use `inspect` to confirm field indices and values for your own file first.
The example changes the recognized object weight operand, in its source units.

Retail name protection applies to the CLI as well. Object export and panel
repair have CLI forms, described in
[Export an object](#export-an-object-and-its-resources) and
[Ship, ground and animation tools](#ship-ground-and-animation-tools).

## Export an object and its resources

1. Open the source LIB and select an object, such as `A10.PT` or `AIM9M.JT`.
2. Click **Export object**. Enter a new ID, for example `A10V1` or `MYAIM9`,
   then its display name. The selected entry supplies the source object.
3. Review the filename map. Hangar copies the resolved resource graph and
   assigns private names that do not collide with any scanned source entry.
   Use Back to change names, or Add source LIB when dependencies are elsewhere.
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
family, known cockpit picture families, available store-icon companions and an
editor palette copy. It inspects whole module sections, so it is not limited to
the currently displayed shape pose. It preserves imported game symbols,
compiled addresses and section sizes; short aliases fit small filename fields.
Geometry, numeric characteristics and leaf image/audio bytes remain unchanged.

This is a private **resource** package, not proof of complete original-game
runtime behavior. Game procedures and dynamically generated names remain
outside the file graph. Unresolved HUD name candidates are shown in the review
and preserved; required missing resource filenames block export. The tool
supports the reviewed `_S.SH` aircraft damage-family convention. Other
recognized objects follow their stored references and known companion-file
conventions. Opaque binary resources can be copied, but their unknown
references are not renamed or invented. Display names change only in
recognized identity records.

For an externally authored shape, use **Lib > From loose SH file**. That older
workflow requires a real SH file and retains shared stock dependencies. It is
separate from **Export object**, which clones the selected object and assets.

The CLI equivalent accepts additional source LIBs explicitly:

```sh
cargo run --locked -- export-object FA_2.LIB A10.PT A10V1 "My A-10" A10V1.LIB FA_1.LIB
cargo run --locked -- export-object FA_2.LIB AIM9M.JT MYAIM9 "My missile" MYAIM9.LIB FA_1.LIB
```

See [the Windows test checklist](WINDOWS-TEST.md).

## Work across LIBs

1. **Ctrl+O** opens another LIB. Click an inactive root in the outliner to
   switch; the active root expands into its categorized entries. The **+**
   opens a LIB.
2. Select a resource and press **Ctrl+C**, switch to a destination LIB, then
   **Ctrl+V**. Alternatively, drag an entry onto another LIB root. The review
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
   use **Export object** for private aircraft families. **Ctrl+D** duplicates
   one resource under a new name, retaining its shared dependencies.
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

Each LIB has a collapse arrow in the scrollable outliner. Expanded inactive
LIBs show their categories and resources too. Drag an entry onto another LIB
root or one of its rows to review a transfer. Choose **Item only** or **Object
and linked files**, then **Copy** or **Move**; resolve any name collisions
before applying. Move removes the selected source item and its unshared linked
entries. Known shared dependencies remain in the source, and files supplied by
other open LIBs are copied. Both documents update together in memory; each has
its own undo step. Files change only when saved. An emptied source remains an
empty editor document; the LIB writer requires at least one entry to save it.

## Flight envelope table

Flight opens recognized envelopes as a table. Use the G-row arrows to choose
the envelope, then edit its point count, stall lift, maximum speed, and the
Speed/Altitude cells. Unused point slots are dimmed; changed cells are amber.
Wheel scrolls the table. The All group and Raw fields dock retain the underlying
BRF view and saved-value comparisons.

## Paint a livery

**Base color** is in the Model inspector and Paint > Materials. Its palette
picker replaces the most common flat color in the decoded pose, including all
faces using that index. **Panel color** changes only the selected untextured
face. Both are one undo step. Textured surfaces get their color from their PIC;
the UI points to Materials when no flat-color faces are present.

For a supported panel without a texture, click the viewport **brush icon** or
enable **Paint model / auto-create texture**. Hangar stages a private 64x64 PIC
filled with the panel's original color and maps the polygon onto it. Release
commits the sheet, SH mapping and first stroke together; Esc cancels, and
Ctrl+Z removes the entire change. The base palette must be loaded. **Create
paintable panel texture** also performs this step explicitly, before adding a
decal.

Automatic mapping supports ordinary opaque polygons with a known material
state. Special shading, unresolved state, insufficient CODE space or jump
reach produce a diagnostic without changing the document. Other decoded faces
retain their material and UVs. This is planar panel mapping, not full UV unwrap.

1. Select a PT/SH and choose **Textured** (or View > Textured / wireframe).
2. Click a visible panel. The inspector identifies its face and named PIC.
3. **Export object** now creates private copies of the discovered textures,
   including the damage family. Paint those copies for a separate livery.
   **Clone texture for this shape** remains available for individual changes;
   that narrower command only retargets references in the decoded pose.
4. Click **UV / paint …**. Amber outlines show that face's footprint on the
   atlas. Choose a palette swatch and **Brush**. The lower **3D preview** updates
   during the stroke. Wheel zooms the atlas; middle-drag pans it. Middle-drag
   and wheel over the model preview orbit and zoom the model.
5. Alternatively, click the **brush icon** below Frame in the viewport toolbar.
   No panel selection is required. **Panel lock: off** lets a stroke cross
   visible faces and PICs; enable it to constrain a stroke. UV interpolation
   restarts at panel boundaries so distant atlas islands are not joined by
   paint streaks. Release commits all touched PICs and generated mappings as
   one undo step; Esc cancels the whole stroke. A stroke can touch up to 64
   texture entries.
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
PAL (or another LIB containing PALETTE.PAL) for preview. Missing colors are
shown in grayscale and painting is blocked until a complete palette is
available. Span holes are preserved; this brush does not create new opaque
pixels outside existing spans. PNG decal import and bounded per-face UV
transforms are also available. Arbitrary audio-format conversion and
topology-aware UV unwrapping remain outside this version.

## Hardpoints, materials and decals

Select an aircraft PT and click **Hardpoints** above the model. Steel diamonds
mark its stations; the selected station turns amber. Drag a diamond in an
orthographic view, edit X/Y/Z numerically, or press G, X/Y/Z, an offset and
Enter. H places a new station at the cursor's view-plane position. Add,
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
3. Imported PNG paths are remembered in **Squadron / PNG library**. The list
   holds 16 files and is saved beside the executable in tore-hangar-decals.txt.
   Artwork stays in its original file; removing a list item does not delete it.
   A read-only executable folder retains new selections for the session only.
4. Click or drag on the texture atlas or a visible model panel. Set width,
   rotation, opacity and horizontal mirroring. Tail text uses built-in block
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
positions and stores. The **Loadout / station flags** button switches to
heading, pitch and slew limits. These are stored values, not invented degree or
distance conversions. The Properties groups also expose movement acceleration
and engagement/firing parameters. Shape animation and weapon launch behavior
are separate contracts.

**Parts** in the Model toolbar opens animation inspection. Named imported state
inputs select reviewed branches without changing the file. Set a state such as
gear-down to reveal its C4 parts; edit a part's X/Y/Z placement with one-step
undo. Existing rotations, code addresses and other bytes remain intact. Stored
angles are shown for inspection. Native angle arithmetic, smooth animation,
turret tracking and arbitrary animated geometry are not yet editable.

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

## Controls

| Action | Control |
| --- | --- |
| Open LIB / package new LIB | Ctrl+O / Ctrl+S, or File menu |
| Add entry / export selected entry | Ctrl+I / Ctrl+E |
| Copy / paste resources | Ctrl+C / Ctrl+V, or drag to another LIB root |
| Duplicate resource / close active LIB | Ctrl+D / Ctrl+W |
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
| Vertex edit mode | Tab; click vertex, A selects all, G numeric offsets, orthographic drag |
| Transform supported static shape | G / R / S, X/Y/Z, numeric value, Enter |
| Hardpoint placement / movement | H at cursor; drag diamond or G then X/Y/Z |
| Decal placement | Click/drag on atlas or model; Apply decal / Esc cancel |
| Cancel transform or dialog | Esc or right mouse button |
| Close with unsaved edits | Click Discard changes, or Cancel/Esc to return |

Scale currently acts on the selected axis in percent; rotation is in degrees,
translation in integer source coordinates.

## Limits

Features not yet available are listed under
[Scope still ahead](../README.md#scope-still-ahead). Windows file paths are
ASCII in this first version.

### Source values

This version uses numeric **source storage values**, not invented conversions
to knots, pounds or Mach. A `^` marker is retained, not silently reinterpreted.
Changing a field is not a guarantee of how the original game consumes it.
Opaque binary definitions can be exported or replaced but are not guessed.

### References and package checks

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

### Models and loader

The loader never executes code from a resource. Unsupported records produce
an explicit diagnostic. The current model reader is a static-pose projection,
not the original game's full drawing interpreter. OBJ exports discard
materials and animation and cannot be imported back as a lossless SH edit.
