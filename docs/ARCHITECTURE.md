# Architecture and format references

Implementation milestone: a native LIB/BRF editor with an SH preview and a
restricted static writer. Agent decisions below were chosen for the requested
lightweight portable application. The user's accepted CPU minimum is SSE2.

## Layers

- `hangar-core/archive.rs`: bounded EALIB directory, shared source storage,
  lossless unedited archive serialization, raw DCL decoding and stored writes.
- `clone_aircraft.rs`: selected-object dependency graph, private names and reference
  rewriting. `ui_clone.rs` indexes sibling/additional source LIBs, reads only
  requested payloads, and presents the export review.
- `dependencies.rs`: shared inert BRF/module reference scanner and incremental
  current-LIB index. Shared source handles identify unchanged entries; a changed
  catalog invalidates extensionless-name scans. Reverse links support direct
  users and cycle-safe transitive aircraft users. It keeps no decoded payloads.
- `validation.rs`: advisory archive/payload/reference report. It checks output
  offsets and untouched compressed bytes, validates supported changed payloads,
  and distinguishes unavailable scans from references outside the current LIB.
- `palette.rs`: display palette rules (owners through the dependency index,
  the LIB's own PAL, sibling ranking), the `<ID>.PAL` companion of copies and
  the palette package checks. See [Display palettes](#display-palettes-and-palette-companions).
- `authoring.rs`: legacy donor PT + imported main SH workflow. Rewrites five
  identity/reference strings, aliases the reviewed A/B/C/D/S family, copies
  observed pose textures, and reports missing textures/shared stock references.
- `brf.rs` and `schema.rs`: source-range operands, schema annotation, width and
  pointer validation. Edits change only one operand. All values stay in source
  units. Structural fields are exposed for expert use; semantic gameplay
  validation and dependency checks are future work.
- `model.rs`: PL/PE CODE lookup and bounded SH data traversal. Imported modules
  are never loaded as libraries or executed. Integer source coordinates and
  integer camera math keep the legacy runtime free of floating-point helpers.
- `resource_ops.rs`: bounded transfer graphs, explicit collision decisions,
  filename-slot-aware renaming and duplicate plans. `Document::transaction`
  validates a shared-buffer draft before applying a single undo batch.
- `document.rs`: entry-granular reversible operations. Original source buffers
  are reference counted. Saving tracks a revision; undo after save shows dirty.
- `picture.rs` / `audio.rs`: bounded indexed PIC and PCM readers adapted from
  Fighters, lossless raster-byte patches, PNG encoding and WAV wrapping.
- `ui.rs`: shared events, layout and draw commands from the supplied theme.
  `ui_browser.rs` lists platform-provided files/roots and recent LIBs.
  `ui_dependencies.rs` places resource navigation beside the viewport and
  renders scrollable package checks and the added/modified/removed build list.
  `ui_libraries.rs` parks complete document/history objects and view state,
  manages source snapshots and resource reviews, and protects inactive dirty
  documents. The active `doc` remains the target for existing editor operations.
  Palettes are heap-backed to keep native Win32 stack frames small.
  `ui_media.rs` handles stroke transactions and a CPU triangle rasterizer with
  per-pixel face/UV hit buffers. Those buffers drive model painting; live strokes
  overlay the texture cache until mouse release commits one document operation.
- `windows.rs`: Win32 ANSI events, GDI back buffer, filesystem and allocation.
  `linux.rs`: Xlib events/drawing plus standard Linux filesystem support.
- `build.rs` (windows-msvc only): writes the committed app icon
  (`tore-hangar-design/icons/app/tore-hangar.ico`) as RT_ICON/RT_GROUP_ICON 1
  plus a VERSIONINFO into a `.res` that the linker converts itself, so no
  resource compiler or Python is needed. `windows.rs` loads it with
  `LoadIconA`; `linux.rs` reads the same file's 32-bit entries for
  `_NET_WM_ICON`.
- `cli.rs`: local Linux inspection and automation using the same core.

No vendored retail fixtures, absolute dependency on a sibling repository,
external compiler process, or runtime download is required.

## Evidence consulted

The local `/home/john/Development/T.O.R.E-Fighters` checkout supplied:

- `tools/fa_lib.py`, `tools/test_fa_lib.py`, and `crates/tore-formats/src/lib.rs`:
  7-byte EALIB header; 18-byte entries; 13-byte names, compression byte, 32-bit
  absolute offsets; mandatory additional directory entry with offset = EOF.
- `crates/tore-formats/src/dcl.rs`: bounded raw-literal DCL decoder. Hangar
  removes its lazy synchronized table cache; tables are small and local.
- `aircraft.rs` and `aircraft_schema.rs`: textual BRF grammar and field order.
  The copied schema contains field names/types, not retail values.
- `module.rs`, `shape.rs`, `tools/export_faxx.py`, and
  `docs/formats/objects-and-shapes.md`: module section bounds, SH vertex slots,
  relative control links, static pose traversal, face widths and center/normal
  storage. Source vertices use their stored axes; the viewport displays source
  Z vertically. Normal/center record mapping is separate.
- `docs/spec/fa-xx-export.md` and `tools/check_shape_roundtrip.py`: why a
  geometry-only OBJ conversion cannot be treated as a general safe SH writer.

The borrowed DCL implementation and schema are GPL-3.0. See notices and license.
The original sibling repository is read-only reference material for this work.

## Writing policy

Unchanged archive bytes are preserved exactly. Once edited, directory offsets
are recalculated and untouched payloads, flags and directory name bytes are
retained. New/replaced entries use flag 0. Unknown compression is carried
through unchanged; decoding it produces an explicit error.

BRF annotation is conditional on complete root schema length and kind matching.
Unrecognized fields remain indexed operands. Numeric edits validate storage
width; pointer edits must resolve. No unit conversions or runtime behavior are
inferred from mockup numbers. This does not validate all field relationships.

The selected-aircraft clone follows the shadow-derived damage-family contract
in the sibling `docs/spec/fa-xx-export.md`. It reads the main/shadow references
from the PT and includes all A/B/C/D/S companions. BRF string operands and
catalog-resolved module filename literals form the recursive resource graph.
Available `$<store>.PIC` icons are included using the ordnance-menu contract.
A same-name HUD is included when a PT leaves the explicit HUD pointer null.

Compiled modules are inspected as inert PL/PE sections. Only CODE, DATA, .data,
.rdata and .text contribute filename literals. Imports, exports, relocation
tables and DOS stubs remain untouched. Replacements never move bytes or enlarge
sections. Reviewed E2 texture-name fields have 14-byte storage and reviewed HUD
picture fields have 13-byte storage; other strings are constrained to their
original capacity. Texture/cockpit suffix families and store/icon stem pairs
remain consistent. All output names are checked against the complete scanned
catalog, then resolved output references are checked again against the package.

In SH modules an `E2 00` pair counts as a texture operand only when
`shape_code::Inventory` places an E2 record exactly there; the record's static
reachability travels with the reference. The same bytes inside any other
decoded record (face indices and texture coordinates, vertex words) are data,
not names. Opaque spans, and modules the inventory cannot parse, keep the
bounded byte heuristic. Retail `FA_2.LIB` shapes held 90 such phantom
one-to-four character names (for example `B.PIC` inside a reached FC face
record of `F14_C.SH`) that the byte heuristic had reported.

Extensionless module strings with no catalog match are not invented as files.
Unresolved names in reviewed HUD fields, such as ~F104_W in the A-10's donor
HUD, are reported and preserved. A referenced name absent from the document
and every searched catalog, including a missing damage-family member or a
stored main/shadow/HUD root, is *unresolved in source*. The build takes an
explicit `clone_aircraft::Policy` and refuses by default, listing each name
and its referencing resource. `Keep` leaves the stored bytes untouched: the
name is not renamed, copied or mapped, and it is added to the reserved name
set so no generated private name can take it (a kept name equal to a fixed
private name is a collision error). `Substitute(PIC)` applies only to PIC
references: the substitute must be in a searched catalog, joins the package
and is written into the copied resource's existing slot by the same bounded
rewrite (BRF string, 14-byte E2 field, 13-byte HUD field) under that slot's
capacity budget. A substitute for a name that is not unresolved is an error.
The output audit accepts exactly the kept names as unresolved. This verifies a
stored resource graph, not every dynamically generated lookup in the original
executable. A second aircraft definition in the graph is rejected rather than
exporting an incomplete second damage family.

The source catalog uses directory-only reads and bounded range reads, in the
GUI wizard and in the `export-object`/`clone-aircraft` CLI alike. Current
in-memory entries win, then explicitly added source LIBs; conflicting sibling
copies require an explicit choice. Limits are 64 extra LIBs, 131072 catalog
names, 4096 copied resources, 4096 unresolved references and 128 MiB decoded
output. The source files can be up to 2 GiB without being loaded wholesale;
opening a LIB as the edited document still loads it whole and is limited to
128 MiB. The editor's private palette is named `<new ID>.PAL`; game-global
palette lookup is not overridden.

The References dock indexes the current document with optional directory-only
source catalogs (64 LIBs / 131072 catalog entries). It shares the clone scanner
and adds reviewed damage-family, default-HUD and store-icon conventions.
Other open documents contribute provider names and cached reverse-user links;
other runtime-derived families remain unverified. It excludes self-name literals
from user navigation and follows reverse edges with cycle protection to find observed aircraft users. Scans are
limited to 4096 reference operands per resource, 65536 indexed links and 128 MiB
of decoded non-leaf input. Budget failures and opaque resources are explicit;
an incomplete scan must not be presented as proof that a resource has no users.
Palette, image and sample bytes are not searched for filenames.

Package results keep at most 2048 detailed checks while retaining total error,
warning and omitted-result counts. Supported changed SH checks use the existing
bounded static reader, not a full animation verifier. Missing local filenames
are warnings because other game LIBs may supply them. The report is advisory
and does not add a new save gate. Existing archive/destination checks still run
on every save.

The loose-SH workflow in authoring.rs remains separate. It retains its original
shared-dependency contract and is not the default Export object action.

SH vertex writing accepts the understood static subset of vertex buffers,
faces, texture/fog selection, relative jumps and source strings. A spatial header, other control-flow,
vertex-normal record or any other skipped record marks the pose read-only.
The writer preserves record sizes, rejects coordinate/byte-center overflow,
and updates centers/normals only of faces whose vertices moved. Centers are the
truncated vertex average; normals are unit vectors (32765) from the first
non-degenerate vertex triple with retail winding (c-a)x(b-a) in right/forward/up
order, stored right/up/forward. A face with no such triple keeps its stored
normal. General aircraft do not qualify; their vertices are written per region
by `shape_geometry` instead (see Region geometry editing).
The synthetic demo is an editor fixture, not a proven game-loadable aircraft.

A complete SH writer must preserve every branch, LOD, state switch, local
transform, contact box, visibility plane, normal, relocation, and reference.
Decoded records now retain source spans; vertex writes check provenance and
topology and preserve bytes for a no-op. A complete editable intermediate
representation covering every branch remains future work. Test general writes against independent OpenFA
round-trip tools and then the original game. Unsupported records must never
be silently flattened or discarded.

## UI differences from the design reference

The UI follows `tore-hangar-design` (tokens, component READMEs, screens).
`ui_view.rs` and the feature slices emit drawing commands and hit regions from
the same layout. The shared smoke test lays out every editor and dialog at
1280 x 800 and 800 x 600 and checks that each hit region stays in the window
and inside the fills its control draws (hovered when it only fills on hover),
that no two hit regions overlap (viewport markers and the menu surface
excepted), and that chrome labels are not truncated at 1280 x 800.

- **Tokens.** `tokens/tokens.json` is the source of truth;
  `tools/gen/theme.py` generates `theme.rs` (integer only: the label column is
  `PROP_LABEL_PCT`, letter spacing is `tracking` in 1/100 em) and `tokens.css`.
- **Text.** `Draw::Text` carries a `Style` from `theme::text`. Windows caches
  one `CreateFontA` per style (Tahoma for UI, Lucida Console for data) instead
  of pre-rasterized Barlow/JetBrains bitmap fonts, so glyph shapes differ from
  the mockups and Win98 maps weight 500 to regular. Linux loads the closest X
  core font per style and falls back to `fixed` (double-struck for bold); the
  fallback can run up to about 10% wider than the estimate. Section
  letter-spacing is not drawn (GDI has no cheap tracking in the allow-list).
  Truncation uses per-character advances generated from Liberation Sans
  (Arial metrics, close to Tahoma).
- **Icons.** `tools/gen/glyphs.py` rasterizes `icons/*.svg` once into 16px and
  12px two-level coverage masks (`ui_glyphs.rs`); edge pixels are the icon
  color mixed into the known ground, so every pixel is still a solid fill.
- **Shape.** Corners use a 1px notch for `radius-sm`/`radius-md`/`radius-xs`;
  there is no anti-aliased rounding. The lip is a 1px `color::LIP` line.
- **Components** (`ui_widgets.rs`): buttons, icon buttons, segmented controls,
  checkbox and checkbox rows, select and dropdown menu, NumberField with
  scrub/step/type/reset, type badges, notices, panels with property rows,
  keycaps, dirty dot and focus ring. Chrome, the editor bodies and the Edit
  Mesh and Parts inspectors use them; the old boxed button is gone.
- **Panel stacks.** The right-hand editors lay out a `Stack`: rows draw and
  take hit regions only when entirely visible, panel frames are cut at the
  edges and spliced beneath their rows once the panel's height is known, and
  the wheel scrolls in pixels (the range comes from the last layout). There is
  no clip region in the draw list. Collapsed state is one bit per panel id
  (`widgets::pane`); a Ctrl+click keeps one panel open.
- **NumberField targets.** `NumberTarget::Field` (BRF fields, envelope cells
  included), `Station` (hardpoint columns; positions are signed, `$hex`
  words sign-extended) and `Decal` (draft placement, no saved value). A click
  without a drag types a value. Backspace writes the saved operand text back,
  so `$hex` notation and bytes match the file; `$hex`, string and pointer
  operands are sunken text fields that open the type prompt.
- **Units.** BRF values have no evidenced units, so none are shown; "^"
  scaled operands read "scaled" in raw source units. Units appear where the
  editor defines them: decal px, ° and %, byte sizes, brush px.
- **Edit Mesh header.** The Select and Mesh menus and the vertex/face
  segmented control appear in Edit Mesh only. At 800 x 600 the segmented
  control is left out (1 and 3 and the Select menu remain) before the menus
  are.
- **Not implemented from the mockups:** LOD select, the Hardpoints header
  menu, the vertical property tab strip (field groups are a panel list instead), the
  animation timeline, editor-type switching per header, draggable splitters,
  graft geometry aspects and viewport ghost preview, panel header tools, and
  collapsing side editors to tab strips at 800 x 600 (both side editors keep
  210 and 270 px there). These are not shown as inert controls.
- **Solid shading** draws the CPU raster without textures: palette face
  colors (textured panels in `ink-muted`) scaled by the camera-space normal.

Saved entry snapshots share source buffers. Changed field values and resources
are marked amber; the raw table can restore a saved operand. Source units are
shown explicitly. Package validation checks the archive directory, untouched
payload preservation, supported changed payloads and observed stored names.
It does not establish full semantic or animation dependency closure.

The animation timeline and geometric graft controls remain unimplemented and
are not shown as working controls. `definition.rs` annotates linked station and
envelope blocks only after kind/count validation. `ui_graft.rs` exposes semantic
field groups, donor snapshots, explicit conflicts and target-to-donor previews.
Grouped numeric grafts match labels, kinds and scaling; they preserve identity,
resource pointers and all unselected operands, applying in one document undo
transaction. Record-size changes and geometric grafting are separate work.
Typed path dialogs and fixed panel splits remain. File selection uses an
in-app directory/drive browser with recent LIBs.

## Material editing boundary

FC colors and E2 texture-name operands have fixed, bounded source offsets.
Color remapping changes only the low palette byte of matching untextured FC
records. Texture cloning replaces the 14-byte bounded name operands reached by
the reader and adds a copied PIC in one undo transaction. Unvisited references
remain untouched; this is not complete LOD/animation material closure.

PIC painting records a source offset for each opaque pixel. Aliased spans or
metadata overlaps disable painting. Raw-kind PICs can carry an unused nonzero
span capacity with a null span pointer; that inactive field is preserved.
No-op bytes are never re-encoded. Painting mutates only raster bytes and keeps
palette, row tables, spans, glyph metadata and transparency structure intact.

The static preview uses bounded CPU triangle rasterization, depth and nearest
indexed texture lookup. UV V is flipped against the source PIC height, as in
the reviewed Fighters exporter/renderer. Rendering and picking use the same
mapping. It is unlit and orthographic, with fan triangulation, not game renderer
parity. Brush size is measured in texture pixels, not world-space distance.

## Retail protection and recoverable saves

`hangar-core/src/save.rs` owns the explicit installer/disc filename list and
storage-independent save protocol. Matching ignores case, recognizes both path
separators and Windows drive prefixes, and catches trailing-dot/space and stream
aliases. LIB destinations reject ambiguous Win32/device syntax. No hash or
folder-based exemption is used. The low-level create-new writer also refuses
reserved names, covering resource export destinations.

GUI and CLI archive writers share `saving.rs`: validate the destination and
serialized EALIB, create/flush a fresh same-directory stage, move an existing
custom LIB to an unused backup name, then install the staged file. All moves
refuse an existing destination; rollback also refuses to clobber a file created
by another process. Windows uses `MoveFileA`; Linux uses same-directory hard
link/unlink. Failures preserve the old bytes; failed rollback reports both
recovery paths. This is recoverable replacement, not a crash-atomic transaction.
No background backup pruning or retail unlock switch is provided.

Companion names come from `save::companion`: the stage is `<STEM>.TMP` (then
`.T01`..`.T99`) and the backup `<STEM>.BAK` (then `.B01`..`.B99`), where
`<STEM>` is the path without its final `.LIB`. They never contain `.LIB`,
because FA loads any such file as a LIB (below); a destination with `.LIB`
before its end is refused for the same reason. Before a GUI save,
`saving::game_folder` lists the destination folder with the existing
`list_dir` (FindFirstFileA/FindNextFileA) and reads seven header bytes of each
`.LIB`-named file with `read_range`; no Win32 import was added.
`save::game_folder` then models the folder after the save.

## Multi-library transaction boundaries

Opening a LIB adds a document; path normalization reselects an already-open
file. Library IDs remain stable when the active document is swapped with a
parked one. Each document owns its saved baseline and undo/redo history, camera,
selection, field group and palette override. Save As rejects a path owned by
another open file. Closing the application considers every document's dirty
state. There is no fixed document-count or aggregate stored-byte cap; available
memory and per-archive format limits apply. Shared clipboard/history buffers can
outlive a closed document.

A resource copy snapshots the source and other open LIBs. The source owns its
local names; other providers must agree on stored bytes or the review fails
with an ambiguity diagnostic. The graph has a 4096-resource and 128 MiB decoded
scan limit. Target collisions require explicit keep/take decisions. The review
records original target entries so stale plans fail rather than overwrite later
edits. Missing game-supplied resources remain review notes and package warnings.

Rename plans rewrite recognized BRF and module literals only, preserving fixed
compiled slots and unknown bytes. They refuse known damage/HUD/store/palette
conventions that need family-wide identity changes, and known external users
without a local resource in another open LIB. Unknown runtime names remain
unverified. Aircraft-family renames and copies go through `identity.rs` (see
Identity, rename and duplicate ownership); the plain rename keeps refusing them.


## Object export and initial SH authoring (0.7)

The existing clone API now accepts any resource root. Recognized BRF objects
follow the same recursive scanner, including weapon identity blocks and
store/icon stem pairs. PT roots retain the reviewed damage-family rules.
Opaque bytes stay unchanged with a dependency-discovery note. Leaf media is
copied without inventing references; only recognized identity text is renamed.

The original 0.7 shape_edit.rs added a bounded panel continuation writer
(the tail placement below is superseded by 0.8.2). It replaces a reviewed
opaque FC polygon site with a relative jump to an appended E2/FC sequence,
restores its known E0/E2 material selector, and returns to the old successor.
All original instruction RVAs remain stable. New byte UVs use dominant-axis
planar projection. The private PIC started as a 64x64 raster with the loaded
palette embedded; that layout crashes FA, and sheets are now retail textures
(see [FA loader limits and the SH texture layout](#fa-loader-limits-and-the-sh-texture-layout)),
filled with the old polygon color. Special shading and unresolved inherited
material state are refused, as are overlapping relocations, exhausted virtual
space and out-of-range relative jumps.

Zero raw CODE padding is reused when possible; otherwise an aligned CODE copy
is appended and only its raw-file pointer/size and relevant module size headers
change. Other section payloads and RVAs are retained. The writer reparses and
compares geometry plus other faces' texture/UV state before returning. This
supports selected panels on real aircraft without enabling their unsupported
geometry writers. It is not a complete branch/animation/layout writer, and
original-game loading remains a separate acceptance check.

The UI stages the new SH and PIC during the first brush stroke. Release commits
both resources in one document transaction; cancellation keeps source bytes
unchanged. Edit mode exposes vertex provenance and numeric/view-plane movement
only on the supported static subset, updating source face normals/centers.
Base color remaps the most frequent flat-color index in the decoded pose;
selected-panel color patches one reviewed palette word.

The envelope table is a view of recognized 44-operand records. Cells call the
existing field editor and history system; source scaling and raw fields remain
available. No gameplay-unit conversions are inferred.

## Hardpoint and material tools

The hardpoint editor annotates only reviewed PT station records. New fields keep
original source ranges; station transactions update the count and complete row
together, validate signed coordinate limits, and refuse table resizing when other
pointers share the block. Store reassignment creates a private string block.
Unreferenced old blocks are retained, so conservative stored-name scans can still
report their filenames. Source-coordinate markers use the same camera axes as
the model; mouse movement previews a view-plane edit and release commits once.
The saved station view supplies amber changed-value feedback.

Material edits remain fixed-size patches: PAL/embedded-PIC RGB bytes, source UV
coordinates with original byte/word widths, and bounded stored texture-name
slots. Family texture cloning traverses reviewed aircraft dependencies and scans
whole SH sections. It does not reconstruct the shape VM or rewrite topology.
Context-model caches reload after undo so UV/material previews follow restored
bytes rather than stale geometry snapshots.

Decal input is bounded PNG decoded through no_std miniz_oxide, or programmatic
block text/national artwork. PNG chunk CRCs, scanline filters, sample depths,
palette indices and decoded lengths are checked; interlace and APNG are refused.
Opaque target pixels receive alpha-composited colors quantized to the existing
palette. Original span holes and metadata survive. Placement previews always
rebuild from an unchanged entry snapshot and use the same picture cache path as
the live 3D renderer. Apply checks that snapshot and commits one document change.

The imported-art library stores only paths in an executable-adjacent sidecar.
No image data, font runtime, GPU renderer, new platform APIs or runtime DLLs are
introduced. Pending decal/station state is heap-backed where necessary to keep
the custom Windows runtime's stack frames bounded.

## NPC, animation inspection and rendering (0.8)

NT recognition requires the exact Object + NPC root layout. The existing
hardpoint annotator and structural station writer then support PT and NT through
the same count/kind checks, reference preservation and undo. Ground/ship
definitions enter the Ground objects category and link to their main SH.
Heading, pitch and slew limits have a direct inspector view. These values do
not imply that visual turret tracking or native launch-axis parity is solved.

Model::with_state follows only the previously reviewed word-comparison and
reentry patterns. State addresses are labelled using bounded import tables and
exact CODE trampoline aliases; no resource code is executed. Reached C4 records
retain source placement, raw rotation words and target provenance.
animation::place_part patches only one signed placement word after validating
the reached record. Source programs, rotations, relative targets and module
layout remain intact. The CHAP/SA2 loaded-launcher envelope selects its reviewed
loaded static branch, without running HARDNumLoaded.

The camera now uses a right-handed basis: screen X is negative body X in front
view; top view faces the upper surface. Wireframe, rasterizer, picking and
station/vertex tools share forward/inverse transforms. Raster vertices retain
1/16-pixel XY and fractional source depth. Stored normals exclude rear-facing
artwork; coplanar textured faces take priority over a flat base. The software
raster size is capped at 512 pixels wide.

Native SH composition research in USNF-ATF Docs/formats/sh.md (2026-09-11)
establishes keyed index-255 copying over a palette/Gouraud base, F6 slot colors,
and G_TextureFlip at 0x48d020. Hangar retains native V inversion, captures F6
colors, preserves the underlying fill for keyed ED/EE faces, and omits empty
runtime-indexed texture-only overlays. Lighting and indexed mission artwork
remain outside this preview's fidelity claim. No asset UVs or raster bytes are
flipped to compensate for camera behavior.

A free brush stroke stages up to 64 PICs and a cumulative generated-panel SH.
Face transitions reset atlas interpolation; returning to a PIC reuses its staged
pixels. Release checks source snapshots and commits a single document batch.
Esc drops all staged resources. The viewport brush icon and optional panel lock
share this path. The close confirmation has explicit Discard changes and Cancel
buttons; its destructive action is scoped to the pending close dialog.

### Remaining full-animation work

1. Extend source-record provenance into a complete graph covering every LOD,
   state branch, callback boundary and relocation with exact no-op serialization.
2. Recover native angle laws, channel ranges, axes and pivots, including ground
   turret/launcher consumers. Current state switches are not continuous animation.
3. Add local-space geometry edits with all affected normals, visibility planes,
   bounds and contact records updated; retain unsupported records explicitly.
4. Extend module layout support beyond the bounded tail relocation added in
   0.8.2. CODE currently grows only within its original virtual-address gap;
   arbitrary section/RVA movement and other module directories remain unsupported.
5. Reconcile the user's Windows results for each baseline/edited candidate,
   including launch points, slew arcs, skin orientation, LOD and damage states.


## Multi-LIB outliner and moves (0.8.1)

One scrollable row sequence contains every LIB root, category and visible entry.
Stable library IDs keep ordering independent of active-document swapping. Root
collapse state belongs to each document; wheel scrolling covers the whole tree.
Opening another LIB retains the current dirty document and its history.
The workspace tabs have a separate background tray, borders and selected accent.

Dropping opens a review with explicit scope and copy/move choices. A move
validates source snapshots and target collision decisions, retains the known
shared dependency closure in its source, and copies dependencies from additional
providers. Both document drafts must validate before either is assigned.
The histories remain per document; source and target each have one undo step.
Moving the final entries may empty the source in memory, while the existing
nonempty LIB save contract remains in force.


## Native generated-panel layout and repair (0.8.2)

The user supplied A10_V2.LIB after three generated engine panels disappeared
in FA while remaining visible in Hangar. The old writer placed their E2/FC
routines after the EndShape marker and import stubs, and sometimes placed a
new raw CODE copy at the end of the file. The bounded projection followed
these jumps, hiding the module-layout errors from the editor checks.

shape_layout.rs now inserts complete continuations before the marker. Body
instruction RVAs remain fixed. The marker/stub tail moves by a 16-byte-aligned
amount; HIGHLOW target values and relocation sites move together. Relocation
blocks are regrouped by page. CODE is packed first at the native 0x200/0x400
offset, relocation padding ends at a 4 KiB file boundary, and code/initialized
data sizes are updated. Import section data and other section payloads survive.
Native code that references fields inside the converted face is rejected.

Legacy repair recognizes only Hangar's E2/FC/restore/jump pattern, checks every
source stub and return, and handles both padded and old unpadded stubs. It
moves those routines before the terminator, then verifies identical decoded
geometry and materials. Existing PICs are not rewritten. Repair is explicit
in the Entry menu, or included when another panel is generated on a legacy SH.
Unrecognized tails, relocation kinds/directories, and exhausted address/relocation
space fail without applying an edit. This remains bounded SH authoring, not an
arbitrary PE or full animated geometry writer.


## Whole-CODE inventory and part bindings (0.9 groundwork)

`shape_code::Inventory` never writes; it is the analysis behind
`shape_geometry` and `shape_parts` (below). It splits all
of CODE into contiguous records with exact spans, in order: SH records, 1E
bytes (one record each, since retail pointers land inside 1E runs), the end
object, embedded x86, x86 data, the end marker with its zero padding, the
`FF 25` import trampolines, and opaque spans. Offsets are CODE-relative.

The record grammar is OpenFA's instruction table with these evidence-backed
deviations, all measured on FA_2.LIB:

- 06 is an 18-byte head plus a separate 38 or 50 record; its count at +14 (5
  or 8) covers a rel16 at +16 and that trailer. The rel16, measured from +18,
  lands on a record start for all 48,821 retail 06s (22 of them inside 1E runs
  that OpenFA merges into one pad). 0C/0E/10 follow the same shape (count at +10; 14 or
  12 bytes) and all 15,155 of their rel16s hit record starts. 6C is 10 bytes
  followed by its own 38/48/50 record, so the 48 jump inside OpenFA's 6C is
  exposed. Its word at +8, measured from the end of the head, is a branch:
  it lands on a record start for all 10,557 retail 6Cs. Hangar's older reached-path table (06 = 14, 0C = 10, 05 = 4, ...)
  resynchronises on these count fields by accident.
- The 38 operand is not a code pointer: measured from the record end it lands
  on a record start for 46 of 64,981 retail 38s, and no tested base (record
  start, field, previous record, preceding 06 target, absolute) beats chance
  (best 17%). It is kept as unknown data, so Model's 38 scope reading is
  unverified; the geometry walks use that reading only where it adds paths.
- 40 frame offsets are relative to each field's own position.
- Face normals are present when content flag 0x40 is set (no retail face sets
  0x20 alone).
- An end object is 18 bytes when bytes 16-17 are zero and bytes 1-5 are not
  (1138 retail cases), otherwise it runs to the marker as in OpenFA (138 cases,
  2 to 794 bytes, some read by x86 self-offsets). A record that would straddle the F2 end-object
  target leaves the bytes before it opaque (SOLDIER.SH's two dead bytes).

F0 blocks are not decoded linearly. From each `F0 00` the inventory follows x86
with OpenFA's opcode subset: jcc/jmp targets are queued, `push A; push B; ret`
to an import other than do_start_interp/_ErrorExit is a call returning to A,
and `push SH; push do_start_interp; ret` resumes SH at SH. Bytes between decoded
blocks are parsed as SH records (for example the C4 between a gear stub's jne
and its target) or become x86 data; spans addressed by call/pop/add
self-offsets are x86 data tables (CATGUY). Anything else is opaque to the next
known boundary. Limits: 256 blocks, 8192 instructions per flow, 64 paths and
256 steps per evaluated stub path.

Records carry pointers with field offset, base and target: Rel16/Rel32/Off16
SH displacements, HIGHLOW sites from `.reloc` (Abs32, CODE or import slot
targets), x86 rel8/rel32 branches and `add r32, K` self-offsets after
`call $+5; pop`. All are inputs for a later relocating writer. Reachability is
the union over every branch: calls return to the next record, 06/0C/0E/10/6C/A6/C8/AC take
both paths, 48/40 jump, 00 and 1E end a path, and an F0 reaches every SH resume
in its x86 flow. `slots` is every vertex slot any 82 record writes, for a
free-slot allocator.

Real-data coverage (manual `--shape-inventory`, 2026-10-07): FA_2.LIB 1274 of
1275 SH fully covered with no opaque span (SOLDIER.SH keeps 2 opaque bytes, as
OpenFA does); swpatch.lib 7 of 7. FA_1.LIB and FA_4B.LIB contain no SH. No SH
pointer target misses a record start. No parse failed or panicked.

The stub evaluator walks each F0 symbolically over a whitelisted subset: word
or dword compares of an imported variable (or a register loaded from one)
against an immediate, jcc/jmp, `call $+5; pop`, constant add/sub, loads of
imported words, sar/shl/shr/neg/add/sub/imul and register moves, and 16-bit
stores through a register holding a CODE address. Each path yields its
conditions, its stores as reverse-Polish laws over variable names, and its SH
resume. Bindings group these by resume target: Toggle (a 12/6E/C4/C6 call or
direct geometry reached under condition sets) or Xform (law stores into the
target C4/C6 words, with the C4 translation as pivot). Calls to other imports
(@HardpointAngle@4, _InsectWingAngle@0, @HARDNumLoaded@8), pushad blocks, byte
compares and flag writes are reported as unrecognised, never approximated. In
FA_2.LIB 101 stubs are unrecognised; none belong to aircraft gear, flaps,
brakes, rudders, hooks, bays, afterburners, canards or swing wings.

Model now tags each vertex (`vertex_tags`, aligned with `vertices`) and face
with its innermost C4/C6 part and 12/6E/C4/C6 call group, keeps the stored 82
coordinate as the pivot-local point, follows 6E/C6 calls, and composes Q14
fixed-point frames. Stored C4 angles always apply; a law applies only when all
its variables are supplied, so the neutral parse keeps the stored words.
Rotation follows OpenFA's xform reader: in C4 space (right, up, forward) the
matrix is Rz(-r2) Ry(-r0) Rx(r1) with 8192 units per half turn, so r0 turns
about up, r1 about right and r2 about forward. Sine and cosine use the same
Bhaskara approximation as `sin_cos`, at FA angle resolution. With OpenFA's
gearPos range (0 down, -8192 up) the A-10's main and nose legs both retract
forward and the F/A-18's nose leg forward, matching those aircraft; this is
preview plausibility, not in-game verification. `Model::with_pose` takes
import-name keys, which survive tail relocation; `with_state` keeps
address keys for existing callers, and `state_names`,
`pose_from_state`/`state_from_pose` map between them.

`--stub-census OUTPUT LIB...` groups address-masked stub signatures by
variable, lists binding parameters per variable, and records which animated
shapes lack each animation import and how much `.reloc` space remains.

## Region geometry editing (0.9)

`shape_geometry` puts the inventory to work. It decodes every FC face and 82
vertex buffer in CODE, whether or not a pose reaches it. Each record gets the
frame that draws it: the root, an innermost C4/C6 part (12/6E calls keep the
caller's frame), Mixed when several frames reach it, or Unreached. A shape is
no longer writable or read-only as a whole: each vertex and face says whether
it can be edited, and why not.

Slot liveness is proved, never assumed: the vertex a face's slot shows is
the one 82 buffer that may have written the slot last on every path that
draws the face, found by the dataflow analysis described in
[Dataflow proofs over SH control flow](#dataflow-proofs-over-sh-control-flow).
The frame walks follow `Model`: a 1E covered by a preceding 38 scope of the
same walk is padding, and walks repeat until that set is stable. Without
this, 30 to 50% of retail BSP faces looked unreachable even though the
preview draws them.

A face may be rewritten, deleted, flipped or used as a host only when no
pointer or relocation field lies inside it, no pointer target lies strictly
inside it, and no self-offset or x86 store addresses it. A vertex is writable
when its buffer passes the same test, every drawn face that uses its slot
resolves, and each of the faces it drives can take a new normal.

All operations are pure functions from SH bytes to SH bytes. Each one is
re-parsed before it returns. The inventory must still be contiguous with the
same opaque byte count, the stub count and every binding must be unchanged,
and a shape the model reader accepted must still parse. Untouched faces must
decode identically, and the new records must resolve to the expected buffers.

- `delete_faces` replaces each FC with a same-size stub: `48` jumping to the
  record end, `1E` fill, and a closing `48` when the record has 8 bytes or
  more, as `texture_panel` does. Pointers to the record start still land on a
  valid record.
- `flip_faces` reverses corner order and UVs and negates the stored normal in
  place. The centre is kept. Flipping twice restores the bytes.
- `write_vertices` writes stored (local) coordinates. The normals of exactly
  the faces whose slots resolve to the moved vertex are recomputed retail-style:
  (c−a)×(b−a) in right/forward/up order, length 32765, stored right/up/forward.
  A face whose stored normal opposed that winding keeps its orientation.
  Centres move by the change in the corner average rather than being
  recomputed. About 4% of retail faces, mostly gear, store centres that are not
  the local average, and this keeps whatever offset the original tool stored.
  A byte centre that would overflow is refused. `scale_faces` scales corners
  about their local centroid. `shape_edit::move_vertices` falls back to this
  writer through `move_model_vertices` when every part frame above the
  selection is unrotated in the shown model.
- `append_geometry` (with `add_face`, `add_vertices`, `duplicate_faces` and
  `extrude_faces`) detours a host face of the same frame. The continuation is
  `[host copy] [82 new vertices] [faces] [E2 switch and restore] [48 back]`,
  placed before the end marker through `shape_layout`. Every existing corner
  must resolve at the host to the buffer it named, in the host's frame. With
  no host given, one is picked from that frame's faces, preferring those that
  share the most slots. Index width (u16 above slot 255), byte or word centre
  and UV width are chosen automatically. New faces are lit flat 0x63 by default.
  Per-vertex shaded content (0x80) is refused or mapped to 0x63/0x6C, because
  new slots carry no F6 records. A textured face drawn under another texture
  gets an E2 switch, then the host's own selector record restores the state.
  An unknown host material is refused. Extrude keeps, flips or removes the
  originals, winds side quads outward against the face normal, and skips edges
  parallel to the offset.
- New vertices take the first run of slots that no 82 writes and no face
  references, below `SLOT_CEILING` = 640, the highest slot count of any retail
  FA_2.LIB shape (CITY2.SH). Slots are contiguous from 0 in retail aircraft, so
  in practice this means the slots after the last one in use.

Limits fail with explicit messages. These include CODE virtual-address room,
relocation-table room, the ±32 KiB `48` reach from the host to the
continuation, and the slot ceiling. A module without the native end
marker/import tail is also refused, because its trailing end object would
absorb the continuation and no whole-CODE reader would see it. In-place
edits have none of these limits.

Read-only: F6 vertex records (gouraud vertex normals are not recomputed),
faces never drawn, Mixed-frame hosts, rotated part frames for model-space
moves (callers pass local coordinates instead), any face with internal
pointers, and record growth in place. Draw order of appended faces follows
the host; BSP placement of new faces is not recomputed.

## Part settings catalog (0.9)

`shape_parts::parts` lists every stub-driven part. Toggles are 12/6E/C4/C6
calls drawn under conditions; transforms are laws stored into a C4/C6. Each
part has a semantic name, its conditions, pivot, block and controls.
`apply_part_setting` changes one field in place at the same size. It then
re-parses the inventory and requires the stub to be recognised still, with
the same instruction boundaries and kinds (a jcc keeps its class), and the
binding to report the new value. Bindings of other stubs must not change.
Stubs the evaluator does not fully understand are locked, as are variables
outside the reviewed list (`_PLdead`, `_PLstate`, `_currentTicks` and others).

| Control | Field | Allowed | Evidence |
|---|---|---|---|
| Gate compare | imm8 of `66 83 3D <var> imm8` | gearDown 0/1/4, flaps −2..1, rudder −1/0/1, brake, hook, afterBurner and bayOpen 0/1 | values observed per variable in the FA_2.LIB census; vtOn and slats show one value and stay fixed |
| Branch sense | `74`/`75` after that compare | je or jne | same 2-byte form; both senses occur in retail. Ranged branches (`7C`, `7D`, `7F`) stay fixed |
| Shift | imm8 of `66 C1 F8/F9 n` | 1–3; swing-wing terms 1–7 | C1 forms with 2 and 3 occur for gear |
| Gear law slot | `sar ax` (D1 or C1) and an optional NEG before a C4 store | the forms of the slot table below | see "Same-size gear encodings" |
| Shift (other D1) | `66 D1 F8` outside a gear slot | read-only (1) | 2 or 3 would need the 4-byte C1 form |
| Direction (other laws) | `66 F7 D8` present or absent | read-only | only gear laws are reviewed for same-size changes |
| Rotation axis | disp8 of `66 89 43 d` | +6 r0 yaw, +8 r1 pitch, +0A r2 roll | gear uses +8 and +0A, swing wings +6; two stores of one stub may not share an axis |
| Pivot | C4/C6 translation words | any i16 | refused when any stub stores into them or an unrecognised stub addresses them |

Naming heuristic. The role comes from the law variable (gearPos, swingWing,
canardPos, bayDoorPos, vtAngle) or else the gate variable (left/right flap,
rudder, brake, hook, bayOpen, gearDown mesh, afterburner, vtOn, slats). The
side comes from the variable for flaps, else from the pivot's right coordinate
(below −1 left, above 1 right), else from the centroid of a toggled block's
first vertex buffer. The gear transforms farthest from the centreline are the
main legs. Gear pivots more than halfway from them towards the most forward
gear pivot are named nose gear; this also names the A-10's offset nose leg.
Gear doors and legs driven by the same variables are not told apart and are
numbered. Toggle names carry the gate value ("Flap left (state -1)"), because
the meaning of each retail state is not verified.

Bindings of one stub that resume at one target are one part. A ranged gear
stub stores 0 on one path and a computed law on the other; both paths now
share one part instead of a second "Gear mesh" entry with the same id.

Out of scope: authoring new stubs or adding template stubs to shapes without
them, new imports, relocating stubs, and laws other than the gear slot forms.

### Same-size gear encodings

The user allowed same-size x86 encodings that are not in the retail census
for gear fold direction and swing range, provided the evaluator proves the
result. A gear law slot is the `sar ax` right before a `mov [ebx+d], ax`
store into a C4 rotation word of a `_PLgearPos` law, plus the instruction
after the shift when it is `neg ax` (`66 F7 D8`), `mov ax, ax`
(`66 89 C0`) or `xchg ax, ax` (`66 90`). The slot's size never changes;
inside it Hangar writes one canonical form per (shift, NEG) state, and shift
1 keeps the 3-byte D1 form so a census slot returns to its exact bytes:

| Slot | Census form | Reachable states | Not in the census |
|---|---|---|---|
| 3 bytes | `D1 F8` (sar 1) | sar 1 | none; direction and range are fixed |
| 4 bytes | `C1 F8 0n` (n = 2, 3) | sar 1–3 | sar 1 as `C1 F8 01` |
| 6 bytes | `D1 F8`, `F7 D8` (sar 1, NEG) | sar 1 ± NEG; sar 2–3 without NEG | `D1 F8 89 C0`; `C1 F8 0n 90` |
| 7 bytes | `C1 F8 0n`, `F7 D8` (n = 2, 3, NEG) | sar 1–3 ± NEG | `C1 F8 01 …`; `… 89 C0` |

(All forms carry the `66` operand-size prefix.) NEG with shift 2 or 3 needs
7 bytes, so a 6-byte slot refuses it with that reason, and so on. A slot
with a branch target inside it is not offered. The decoder and evaluator
accept `66 90` as a no-op; adding it changed nothing in the retail
inventory (identical `--shape-inventory` output over FA_2.LIB).

Each change re-parses the inventory and requires: the same coverage and stub
count, the stub still recognised, every instruction outside the slot at the
same boundary with the same kind, the slot re-parsing as the requested
(shift, NEG), and every path's law for that word equal to the old law with
its trailing NEG and last shift replaced. Other parts' bindings must not
change. `Control::retail` and `Control::unseen` mark forms outside the
census (gear slot forms, C1 shift amounts and rotation words not seen for
that variable); the UI badges them **Not seen in retail**.

## Parts and Edit Mesh UI boundary

The Parts panel only reads `shape_parts::parts` and writes through
`apply_part_setting` (or, for C4 parts no stub drives, the existing
`animation::place_part`). It never assembles stub bytes. Option lists come
from each control's `Allowed`; a fixed control shows its reason. The preview
pose is a map from import variable name to value passed to
`Model::with_pose`. It is app state only (never in the document, undo or a
save) and, keyed by name, survives edits, undo and tail relocation, so the
old address-keyed state list and its revalidation are gone. The shown model
is posed in every Model workspace mode, so Edit Mesh works on the posed
geometry; `shape_geometry::write_model_points` converts model-space moves to
stored coordinates and refuses vertices whose part frames the pose rotates.

Edit Mesh selections are model vertex indices (re-picked by stored offset
after every reload) and face file offsets. Every operation reads the entry's
bytes, calls one `shape_geometry` function, and replaces the entry once:
one undo step. Duplicate and extrude preview by running the core function on
each keystroke and apply the final offset in one call.

## Texture originals boundary

`hangar-core/src/originals.rs` owns stored originals. The companion of `X.PIC`
is `X.ORG`, keeping the full stem and any `_`, `~`, `$` prefix, so every
companion is a valid 8.3 LIB name. A companion is created only by
`with_originals`, inside the same `Document::transaction` as the edit that
replaces an existing, decodable PIC, and only when no entry named `X.ORG`
exists. The backup is the pre-edit `Entry` renamed, or the saved `Entry` when
this session already changed the PIC without a backup (for example after its
original was removed). It shares storage and keeps the compression flag, so
nothing is decoded or re-encoded to keep it, and undo of the first edit removes
both entries. A `.ORG` counts as a stored original
only when its payload parses as a PIC; any other `X.ORG` is never adopted,
overwritten, restored from or removed.

Restore is one transaction: `X.PIC` takes the original's exact stored bytes and
flag and `X.ORG` is removed. When the original shares the saved entry's payload,
the saved `Entry` itself is used, so `same_storage` reports the texture as
unchanged. The eraser uses `Pic::paint_from`, the same bounded circle as the
brush, writing original indices only through the per-pixel source offsets. It
requires `Pic::same_layout` (dimensions and offset map), so headers, palettes,
span tables and glyph data stay untouched. A stroke that ends byte-identical to
the saved entry or the stored original reuses that entry's storage; erasing all
the way back to the saved entry also drops an `X.ORG` added in the session.

Generated panel sheets are recognized by Hangar's raw sheet header (8 to 256
pixels a side; square before per-panel sizing), a name made of an SH stem
(first six characters) and two hex digits, and the SH face records that name
them. Their original is the face's color byte in the
SH, so a sheet generated in this session keeps no `.ORG`. A sheet already in
the saved LIB is backed up like any PIC, so a mistaken match never loses
artwork. The session's saved entries are only a fallback.

Dependencies treat `ORG` as a leaf. Rename, delete, transfer and move keep the
pair together: a companion follows its PIC's keep/take choice and stays in the
source when its PIC does. Object export copies valid originals under the new
texture names, reserved against the source catalog, and reports a taken name
rather than overwriting it; texture clones copy the companion when present.
Package checks parse changed `.ORG` payloads as PIC, warn on originals without
their PIC and note raster-layout mismatches. FA is expected to ignore entries it
never looks up by name; original-game acceptance is listed in WINDOWS-TEST.md.

## Color replacement

`Pic::replace` (the brush circle), `Pic::replace_all` and
`Pic::replace_count` act on palette indices. A pixel changes only when it has
a source offset (span holes have none), its index is in the `IndexSet` and it
lies inside the optional region mask; its raster byte is the only byte
written, so header, palette, row and span tables, glyph data and transparency
stay intact (index 255 of a glyph strip is transparent: never matched, never
written). `matching` builds the set with integers only: tolerance T takes
every index whose color, converted back to the 6-bit values a PAL stores (the
exact inverse of the 8-bit expansion), lies within `dr² + dg² + db² ≤ T²` of
the source; T = 0 is the source index alone. Colors are the PIC's palette over
the active base palette, as for painting. `footprint` rasterises face UVs
(V flipped as the renderer samples it) with a bounded even-odd test of pixel
centres, boundary included so thin faces cover their edge pixels; at most
4,096 polygons of 64 corners within ±65,536. No match, or a source equal to
the target, changes no byte. The app reuses the brush's commit path:
`exact_entry` reuses saved or original storage on an exact return,
`with_originals` keeps `X.ORG` on a first edit (session panel sheets
excepted), and a dialog apply puts every changed PIC into one
`Document::transaction`.

## Texture state proof and per-face texture assignment

SH faces carry no texture name. An E2 record selects a texture (E0 selects an
untextured state) and every later textured FC draws with it, so one E2 near
the start of a retail aircraft serves the whole atlas. `Geometry::material`
proves which selector holds at a face with the same dataflow analysis as
slot liveness (next section): every E2/E0 record that may be the last one
before the face, on any path, must have the same bytes. `Model` clears its
selector after an F0 stub because native code runs there; the proof sees
through the stub's resumes. Over the neutral F-18, F-16 and A-10 models it
proves every drawn face (FA_2.LIB, 2026-10-07).

## Dataflow proofs over SH control flow

The first proofs walked back from a face to the nearest writer and recursed
into every pointer entering that span. A cycle in that search ended the
proof ("its control flow loops"): a backward branch, or a called block whose
nested call led back into the caller. The F-5 (`F5EV.SH`, retail and
edited alike) has the second form: `12` calls the block right after a `38`
scope, so the block runs once called and once entered under the scope, and
a nested `12` in it calls a block that falls into later code. The walk also
counted the returns of nested calls as returns of the outer block. In
FA_2.LIB, 17,988 of 137,959 drawn faces were refused this way.

`shape_flow` replaces it with a forward "may" analysis over the whole CODE
graph, built from the inventory's records and pointers:

- **Nodes** are records paired with the 38 scope end in force, as `Model`
  tracks it per call frame: a 1E is padding while a scope covers it and a
  return otherwise; 00 returns; 38 extends the scope.
- **Edges**: SH branches (06, 0C/0E/10 with a pointer, 6C, A6, AC, C8) go
  both ways, 48 and 40 to their targets, other records fall through. x86
  records go to every CODE target their decoded instructions name (rel8 and
  rel32 branches, HIGHLOW pushes of SH resumes); import trampolines and x86
  data tables are not control flow. All of a stub's resumes are taken, so
  every pose is covered. A stub the evaluator does not recognise also
  resumes at its self-offset targets. Code is never executed.
- **Calls** (12, 6E, C4, C6) are procedures. Each called block is analysed
  once from a symbolic entry state; its summary is the set its returns
  leave, with "entry" standing for whatever the caller had. A call applies
  the summary to the caller's set and continues at the next record with the
  caller's scope. For a last-writer problem this is exact over valid
  call/return paths, so unrelated call sites never merge. Procedure entry
  sets are then joined from every reachable call site, and a face's set is
  the join over its (procedure, scope) nodes.
- **Values** are the last writer: an E2/E0 record (compared by bytes) for
  the texture state, an 82 buffer for a slot, "nothing since the shape
  start", or "unknown" after unexplained bytes (opaque spans, x86 data, the
  end marker) and calls whose target is not a record start. A set holds up
  to four values and then becomes top, which is never a proof; that cannot
  turn an unproved face into a proved one, since only a single value is a
  proof and a writer replaces the whole set.
- **A proof** is a set with exactly one writer. The refusal names what
  reaches the face instead: two textures by name and offset, a path with
  no E2/E0 (or no 82 for the slot) since the shape start, undecodable bytes
  at an offset, or "no path from the shape start draws it".

Everything is bounded and iterative, with no recursion: the product graph
has at most 2^19 nodes (else "too many paths to follow"), the worklist pops
at most `(CAP + 3) × 2 × nodes + procedures + 64` times (a node's set grows at
most `CAP + 1` times and a summary change requeues each caller as often),
and the call-site rounds stop within `procedures × (CAP + 2) + 2`. The
graph is built once per `Geometry`; the per-face results of the 64 most
recently proved states (the texture state and slots) are kept. The old
recursive proof's 128-level cap and its stack measurements no longer apply.

Over FA_2.LIB (2026-10-07, `--proof-census`), the texture state is proved
for 134,546 of 137,959 drawn faces (was 116,558) with no disagreement where
the old proof succeeded, and every remaining refusal is a path with no
selector since the shape start. The vertex report and face refusals of all
1,275 shapes, and every drawn face's slot proof, are identical to the old
proof's.

Not modelled: x86 code changes neither state (recognised stubs store only
into C4/C6 words; an unrecognised stub's stores are unknown and assumed not
to touch E2/E0 or 82 bytes), and a `Model` C4/C6 call restores the caller's
selector while this analysis keeps what the block leaves, as the old proof
did. Where `Model` also knows the selector (114,494 drawn faces in FA_2.LIB)
the proof names the same bytes in every case.

`shape_texture::assign_texture` gives faces their own texture. Selected
faces are grouped into runs: contiguous records with the same texture and
restore selector and no pointer target at an inner record start. Each run
gets one continuation, placed in space freed by a rebuilt assignment or
appended before the end marker through `shape_layout`:

`E2 NEW.PIC (16)` · copies · restore selector (E2 16 or E0 4, the proved
record's exact bytes) · `48` back to the end of the run · stored originals

The first site becomes `48` to the continuation, `1E` fill and a closing
`48` to its own end; later sites of the run jump to their own end (never
reached). The originals are never reached either; they are why the format is
self-describing. A continuation is recognised only structurally: an E2, one
or more FC copies, a 16-byte E2 or 4-byte E0, a `48` whose target ends a run
of sites whose lengths are the originals' lengths, each site holding exactly
the stub bytes, and each copy drawing the same slots as its original. Retail
code, legacy `texture_panel` continuations and other appended geometry do
not match.

UV modes: Keep copies the UVs; Scale computes `(uv * to + from / 2) / from`
per axis, keeping word UVs and widening byte UVs that exceed 255; Project
fits a planar projection (the faces' own plane with the longest edge along
U, or a fixed Top/Side/Front plane) into the PIC with one scale for both
axes. Untextured faces are only projected; their content becomes
`(content & 0x60) | 4` as for generated panels, refusing other subtypes and
special colour words. Faces already drawn from a Hangar assignment are taken
from their continuation with its originals and restore, so assignments never
nest; the rest of that continuation is rebuilt and its old bytes are filled
with `1E`. `restore_texture_assignment` writes each stored original back to
its site, taking the copy's current normal, centre and corner order (a later
vertex move or flip), and rebuilds the remaining faces. Faces that are not
drawn from an assignment are refused: "No Hangar texture assignment to
remove".

Refused: faces a part stub resumes drawing at (the binding's target record
would change), faces `face_refusal` rejects (pointer or relocation fields,
inner pointer targets, native addressing, never drawn), faces whose texture
state cannot be proved, rebuilding a continuation that another record jumps
into or that carries foreign pointers, and the usual layout limits
(virtual-address and relocation room, ±32 KiB jump reach, native end
marker). Each result is re-parsed: CODE coverage and opaque bytes, stubs and
bindings unchanged; every new continuation recognised with its sites; and,
in the neutral pose and each pose that draws a moved face, the same number of
drawn faces, each moved face at its new offset with the expected texture,
same corners and same part, every other face identical. A no-op (Keep onto
the texture faces already use) returns the input bytes.

Automatic hosts for new geometry (`pick_host`) prefer faces outside
assignments. An operation that detours an assigned copy itself (duplicate or
extrude of that face, delete) leaves its continuation unrecognised; the copy
then keeps drawing from its texture, but Use shape texture refuses it.

The app's Clone texture for selected faces runs Keep onto a renamed copy of
the faces' PIC (stored bytes and flag shared, `.ORG` copied by
`originals::cloned`) in one transaction. Painting follows the faces' texture
name, so the brush writes only the new PIC on them. Applying an assignment
to the damage family is not offered: in FA_2.LIB no face of the F-18,
F-16, A-10, F-22, F-14, MiG-29 or Su-27 main shape has a record with the same
bytes at the same CODE offset in any `_A`..`_D` shape, so faces cannot be
matched safely.

Generated panel sheets use the same planar projection. The sheet size comes
from the shape's texel density (`atlas_density`: summed UV edge length over
summed model edge length of named textured faces; 2.2 texels per unit on
the A-10, 3.1 on the F-18, 9.4 on the F-14, median 2.8 over 1,073 FA_2.LIB
shapes; 3 is the fallback), rounded per side,
scaled together so the longer side is at most 256 and the shorter at least
8. Sheets are raw kind-0 PICs; every texture a retail shape uses is 256 wide
with any height, so other widths are unverified in the game (as the earlier
64 × 64 sheets were). `coplanar_panels` groups connected flat faces of one
colour, subtype and part whose normals agree within about 2.5° and whose
corners lie within one unit of the first face's plane; they keep one legacy
continuation each but share the sheet and one projection. When `Model` has
no selector for the panel, its continuation restores the proved one.

## Remap from view and the retail texture layout

`shape_remap::remap_from_view` projects the selected faces' corners, in the
shown pose, through `model::view_point(yaw, pitch, p)` (the same proper
rotation the viewport's `camera_point` now calls) with points scaled by 256
as the renderer's fixed point, and drops depth: an orthographic layout in
which screen right is U and screen up is stored V, so the PIC is the screen
image (row 0 at the top) and reads unmirrored from each face's front. A
face whose projected area is under a quarter of its true area (Newell, both
in integers; about 75° from the view) or whose stored normal the renderer
would cull is refused. The layout's extent times the shape's
`atlas_density` gives the panels' texels; both sides scale together into
252 × 1,276, and one rational scale for both axes keeps texels square. The
bake runs per texel centre `(2i + 1, 2j + 1)` against each face's fan
triangles in the new integer UVs (doubled), so it matches what the renderer
will sample; the edge-function weights interpolate the face's old UVs (or
take its flat colour), the old PIC is read at the nearest pixel exactly as
`render_model` reads it, and the nearer face wins where faces overlap.
Indices of a PIC with its own palette are mapped to the nearest base colour
(identity when they agree). Blank fills covered texels with their dominant
index; the margin repeats the nearest covered texel for `MARGIN + 1` passes
and the rest of the sheet takes the dominant index. The faces then go
through `assign_texture_uvs`, `assign_texture` with the UVs given (byte UVs
widen to words when needed, untextured faces become textured as for
Project), so every proof, refusal, continuation rule and the re-parse
verification of per-face assignment apply, and Use shape texture reverses
it; the result is re-parsed once more for the new name and UVs.

The new PIC is written by `picture::retail_texture` in the layout of every
texture retail SH shapes use (1,070 across the retail LIBs): kind 0, 256
wide, raster at 64, no embedded palette, the unused span capacity
`10 × (rows + 1)` with a null span pointer as retail carries it, and a row
table of `64 + row × 256`, at most 1,280 rows. FA.EXE indexes that table
while setting up textured polygons (`mov ecx,[ecx+ebp*4]` at 0x4CAF0D with a
0x500 row bound); a generated 64 × 64 sheet without it crashed the game in
the external view. `is_retail_texture` checks the layout on every remap.
Generated panel sheets use the same writer (see below).

App side, the panel selection outside Edit Mesh is `EditState::mesh_faces`
(face file offsets), the same list Edit Mesh's face select uses, pruned on
refresh to the offsets the shown model draws; `selected_face` stays the
active panel for Panel lock and the Paint panel.

## FA loader limits and the SH texture layout

Two crashes in FA.EXE with Hangar output set hard rules; the evidence is
FA.EXE disassembly and the retail LIBs.

**Texture layout.** Polygon texture setup bounds the V row by 0x500 and then
reads a per-row pointer at 0x4CAF0D (`mov ecx,[ecx+ebp*4]`, the table from the
PIC header's row offset at 34). A PIC with no row table (row offset and size
0) faults there. A census of the textures retail SH textured faces draw (by
the model reader's textured faces: 1,063 in FA_2.LIB plus 7 in SWPATCH.LIB,
1,070 in all) found one layout: kind 0, width 256, `row_size = 4 × height`,
`row[r] = 64 + 256 r`, no palette (size at 22 is 0), span pointer 0 with the
unused capacity `10 × (height + 1)`, length `64 + 260 × height`. E2 records
also name four PICs no textured face draws (`_MOON.PIC` 41 × 41, `CATB.PIC`
and `CATF.PIC` 640 × 480, `SOLDIER.PIC` 320 × 200, each with a row table at
its own stride and no palette), used by sprite records.

`picture::retail_texture` writes that layout and `is_retail_texture` checks
it; `retail_texture_check` adds the reasons, and `to_retail_texture` converts
a PIC of at most 256 columns: every pixel keeps its (u, v) (the height is
unchanged, so V-flipped UVs still address the same row), extra columns repeat
the row's last pixel, span holes take a neighbouring opaque pixel, the
palette is dropped, and indices are mapped to the nearest game colour only
where the embedded palette differs from the game palette (`PaletteCheck`).
Generated panel sheets (`shape_edit::panels`) are `retail_texture` sheets 256
wide and as tall as the planar panel size, with the panel's UVs unchanged in
the sub-rectangle at the left edge; the face's colour byte was always a
game-palette index, so dropping the old embedded copy of the base PAL
changes no index. `originals::panel_sheet` recognizes both the legacy and the
retail sheets so Restore texture and the Eraser keep the panel colour.

`validation::texture_layout_problems` takes each SH's E2 texture links from
the dependency index (operands confirmed against the record inventory), and
reports a local PIC that fails `retail_texture_check` when a textured face of
that SH draws it; when the model reader cannot decode the shape, any textured
FC record in its inventory counts, and when neither reader can, the E2
record alone does. Retail LIBs report none. `repair_textures` converts those
PICs and their stored originals (when not already retail) in one transaction
and never touches an SH.

**Loader limits.** `_LibStartUp` enumerates `*.*` in the game folder and
skips directories (the find wrapper at 0x479E10 tests attribute 0x10). It
upper-cases each `name.ext` and skips names it already loaded (14-byte slots
at 0x54A530). A name that contains `.LIB` (`strstr` against 0x4F7FD4) is
opened and kept as a LIB when its header is `EALIB`, otherwise closed; its
name is copied into the slot without a length check, its handle stored at
0x54A648 and the count at 0x54A698 incremented: 20 handles fit before the
count. Each directory entry becomes a 35-byte record in the global resource
table (the write at 0x478F69); a file without `.LIB` becomes one record. The
table is allocated as 0x5505A bytes, 9,950 records. `save::MAX_LIBS`,
`MAX_RESOURCES` and `MAX_LIB_NAME` hold these limits and `save::game_folder`
counts a folder the same way. The installed retail LIBs hold 7,520 entries
(FA_1 2,001, FA_2 5,405, FA_4B 77, FA_4D 22, SWPATCH 15).

## Identity, rename and duplicate ownership

`hangar-core/src/identity.rs` owns names blocks and in-place aircraft
identity changes. A names block is any `ot_names`/`si_names` pointer whose
block holds exactly three strings: short name, long name, self reference
(`F14.PT`). Name edits replace only the quoted operand through the lossless
BRF editor; a weapon's `si_names` follows when it held the same text. Names
are ASCII without quotes, semicolons or control characters, 1..40 long. No
game-side limit was found in the sibling format notes; retail names reach 11
(short) and 28 (long) characters, so 40 is the existing export limit, kept.
The export takes separate short and long names (`clone_aircraft::Names`);
a single title still fills both.

Ownership starts from the export graph (`clone_aircraft::graph`, the same
traversal as the export: PT, main/shadow and A–D damage family, explicit or
name-derived HUD, private or game palette, BRF and module references, store
icons). Users come from a full scan of the LIB: every BRF/PL/PE entry's
stored references and the reviewed conventions (damage family, default HUD,
store icon), bounded at 128 MiB decoded; unparsed entries are only searched
for the aircraft's file name. A graph resource is shared when it is shared by
convention (weapons `JT`, sounds, fonts, other objects, the game palette) or
when any user lies outside the private set; this repeats until stable.
Groups move as one: the damage family, a texture with its suffix family, a
store with its `$` icon. Sensors, ECM and fuel tanks are private when no other
object uses them.

Rename substitutes the old ID after any `~ _ $ & # ^` prefix in each private
name; a private name without the ID keeps its name. `.ORG` companions follow
their PICs. Every BRF/module entry in the LIB is rewritten through the same
bounded slot rules as resource rename, so the self reference and other
objects' references follow. Refusals: a new name already present (and not
leaving), longer than 8.3, two names colliding, a slot too small, a
name-derived HUD that is shared, or a BRF/module whose references cannot be
read but whose bytes name a renamed file. The plan applies as one
transaction, with stale-entry checks; undo restores the exact bytes and a same-ID rename is
empty. Missions and unparsed text are not rewritten; the review counts the
ones that name the aircraft and warns that other LIBs are not searched.
Same-stem files with no stored link (`F14.PTS`, `F14.HUD`) keep their names.

Duplicate builds the export package against the current document with
`clone_aircraft::build_sharing`: names in the share set are not traversed or
copied, and copied resources keep their stored bytes for them. Defaults copy
the private set except sensors and stores, and share the rest; the aircraft
and a name-derived HUD are always copied, and groups switch together. Private
names are generated against the whole LIB, `.ORG` companions follow copied
PICs, and the package is inserted as one transaction after a collision check.

## Engine fields: negative-G cut-out and throttle rates

Hangar labels three PT fields with a stored unit (`definition::unit`) and
groups `negGLimit` with Propulsion: **Neg-G cut-out** in the Model
inspector, plus Flight's Propulsion field group and Graft aspect. The evidence is FA.EXE disassembly; values stay in their
source units and the raw field names stay in the field table.

**Runtime offsets.** FA copies the loaded PT into the `_cpt` buffer at
0x50D268, packed in BRF field order. That puts `vtLimitDown` at 0x50D3C5,
which the sibling flight notes already tie to its reader at 0x47ADD0, and
`engines` at 0x50D3B4, `negGLimit` at 0x50D3B5 (a signed word),
`thrust`/`aftThrust` at 0x50D3B7/0x50D3BB, `throttleAcc`/`throttleDacc` at
0x50D3BF/0x50D3C1 and `fuelConsumption` at 0x50D3C9. Only two routines read
0x50D3B5, and no code reads `+0x14D` from a PT pointer. The `[ebx+0x14D]`
accesses at 0x46F519 work on a network instance record, not a PT.

**Timer, 0x451E80 (engine update).** `cp+0x19B` (0x50D01B) is the aircraft's
controlled G in fixed8: 256 is 1 G and the stick drives it within the
envelope limits. If G >= 0, the word counter at `cp+0x20F` (0x50D08F) is
cleared. If G < 0 and `negGLimit` is nonzero and greater than the counter,
the counter gains `_serviceTicks` (word 0x546BA0), the game time since this
aircraft was last serviced. The game clock 0x552928 counts 1/256 s: TIMEUpdate
derives whole seconds from it (`>> 8`) for the time of day modulo 86,400. So
`negGLimit` is a duration in 1/256 s of uninterrupted negative G. The G
magnitude does not matter: -0.1 G and -4 G count the same. The counter is
also cleared when the aircraft is set up (0x4519CD).

**Cut-out, 0x451A60 (throttle limit).** The routine starts from a 100%
ceiling (the player's is lowered by the byte at 0x52254A) and drops it to 0
with no fuel. If `negGLimit` is nonzero and not greater than the counter, the
ceiling is 0. The commanded throttle (`cp+0x1F2`, 0x50D072) is clamped to it,
and afterburner bit 0x20 of `cp+0x16F` is cleared unless the throttle is
100. 0x451E80 then slews the actual throttle (0x50D06E, fixed8 percent)
toward the command at `throttleDacc` or `throttleAcc` (0x451EC2..0x451EF0
through 0x4119A0, step `rate × 256 × ticks >> 8`). Both rates are therefore
percent per second. Thrust (0x47A8C0) scales `thrust` or `aftThrust` by the
actual throttle (times a percentage at 0x54B6F4, less a speed term), and 0x47A860 gives
none at all once that throttle reaches 0.
The cut-out is a forced spool-down, not the damage flameout: it has no RNG
call, message or restart step. When G returns to 0 or above, the counter
resets and the next frame copies the throttle lever back into the command
(0x451B00, from 0x5451F4), so the engine spools up at `throttleAcc`. The same
update runs for AI aircraft through their throttle routine 0x452050.
Selecting afterburner during a cut-out sets the afterburner bit again after
the clamp (0x451B18..0x451B33). Thrust still follows the falling throttle, but
fuel use reads that bit (0x451F3A) and stays at the afterburner rate.

**Values.** 0 disables the cut-out: neither routine acts. FA_2.LIB stores 0
for most fighters (F-5E, F-14, F-16, MiG-29, Su-27), 2,560 (10 s) for the
F/A-18s, airliners and most helicopters, 4,608 (18 s) for the Strikemaster and
7,680 (30 s) for the A-1 and SF.260. A negative value always caps the throttle
at 0, because the signed comparison in 0x451A60 is true whatever the G. The
counter is a signed word that stops gaining time only after reaching the
limit, so a limit within one service step of 32,767 can wrap negative and
never trip.

**Envelope interplay.** Once per game second, 0x452140 sets the G limits
(0x50D0D7 minimum, 0x50D0D9 maximum). It scans envelope rows `envMin` to
`envMax` and keeps the most negative and most positive rows whose polygon
contains the current speed and altitude (0x49D230 class 0). It interpolates
by speed toward the next row out, then applies the stores-load reduction
scaled by `loadedElevator`. AI with the byte at `cp+0xE2` at or below 1 lose
1 G each way. A global option bit (0x4EB6F8 & 0x20, unidentified) gives the
player 1 G more each way within `envMin`/`envMax`. The minimum is held at or
below 0 and the maximum at or above 2 G. 0x477ED0 scales the player's limits
by `100 − byte 0x522547` percent and passes them to the pitch StickInput at
0x47C106. How much negative G an aircraft can pull is therefore set by its
negative envelope rows, not by `negGLimit`.

Weak-structure damage (fault 30, byte 0x5224EC) is separate. At G >= 6 or
G < -3, it rolls the RNG and can break up the aircraft (0x410D86).

## Display palettes and palette companions

`hangar-core/src/palette.rs` holds the portable rules; `ui_palette.rs` is the
one resolver that sets `base_palette` (every view, paint, decal, repair and
export path reads that field). Order for the selected entry:

1. `palette_override` (Load palette; per open LIB, with its source label);
2. `PALETTE.PAL` in the current LIB;
3. the owner's `<ID>.PAL`: `palette::owners` walks the dependency index's
   reverse links (stored references plus the damage-family, HUD and icon
   conventions the export and identity graphs follow) breadth first from
   the entry, PTs first at each distance, and takes the first owner whose
   `<ID>.PAL` the LIB holds. A PT is its own owner. Bounded at 4,096 names;
4. the only other valid PAL in the current LIB (`palette::local`);
5. `PALETTE.PAL` in another open LIB, ranked by `palette::sibling_rank`
   (FA_2.LIB, FA_1.LIB, other retail names, then by name);
6. `PALETTE.PAL` in a LIB in the current LIB's folder: the directory-only
   index and one range read per LIB (`ui_clone::index`, `read_range`),
   same ranking, at most 32 LIBs, skipping open ones;
7. the remembered game palette, `tore-hangar-palette.txt` beside the
   executable (source label, then 1,536 hex digits) through the same
   sidecar helpers as the recent-file list; a failed write keeps it for the
   session. Only a retail `PALETTE.PAL` (found, or loaded from a retail LIB
   or a `PALETTE.PAL` file) is written. The smoke test sets `NO_MEMORY`;
8. a grayscale ramp (`palette_loaded` false).

Caching: `refresh_data` calls the resolver after the dependency index
update. The key is the active LIB id and path, the open LIBs' ids and paths,
the owner's `<ID>.PAL` name, and every `.PAL` entry in every open document
compared by storage identity; an unchanged key reuses the stored 768 bytes.
The folder scan is cached per folder until the path or the open LIB set
changes. A loaded palette bypasses the cache and is applied exactly.

Companions (`palette::companion`): an object (PT, JT, OT, NT) copied to
another LIB, duplicated, or exported gets `<new ID>.PAL`, from the source's
own `<ID>.PAL` (stored bytes kept) or else the resolved palette's bytes
(`palette::encode` turns an expanded palette back into exact 6-bit bytes).
Skipped when the target has `PALETTE.PAL` or identical bytes under the name;
different bytes are a normal collision. `Plan::add_palette` adds it as a
reviewed item (TakeSource is Copy, KeepTarget is Skip) and marks it so
`move_between` never removes it from the source. `Duplicate::add_palette`
only fills the gap when the LIB has neither the donor's `<ID>.PAL` (already
a graph resource with Copy/Share) nor `PALETTE.PAL`. Export inserts
`PALETTE.PAL` into the catalog and serves the resolved bytes for it when the
current LIB has neither palette, so `clone_aircraft` maps it to `<ID>.PAL`
and conflicting sibling copies never stop an export. Nothing writes
`PALETTE.PAL` into a custom LIB: FA keeps the newest of duplicate names
across its LIBs, so it would recolor every aircraft. `palette::check` adds
the package checks (aircraft with no local palette; a custom `PALETTE.PAL`
that differs from the retail one).
