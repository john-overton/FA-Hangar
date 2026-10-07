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

Extensionless module strings with no catalog match are not invented as files.
Unresolved names in reviewed HUD fields, such as ~F104_W in the A-10's donor
HUD, are reported and preserved. Required explicit filenames must resolve.
This verifies a stored resource graph, not every dynamically generated lookup
in the original executable. A second aircraft definition in the graph is
rejected rather than exporting an incomplete second damage family.

The source catalog uses directory-only reads and bounded range reads. Current
in-memory entries win, then explicitly added source LIBs; conflicting sibling
copies require an explicit choice. Limits are 64 extra LIBs, 131072 catalog
names, 4096 copied resources and 128 MiB decoded output. The source files can
be up to 2 GiB without being loaded wholesale. The editor's private palette is
named `<new ID>.PAL`; game-global palette lookup is not overridden.

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
and updates face centers/normals where present. General aircraft do not qualify.
The synthetic demo is an editor fixture, not a proven game-loadable aircraft.

A complete SH writer must preserve every branch, LOD, state switch, local
transform, contact box, visibility plane, normal, relocation, and reference.
Decoded records now retain source spans; vertex writes check provenance and
topology and preserve bytes for a no-op. A complete editable intermediate
representation covering every branch remains future work. Test general writes against independent OpenFA
round-trip tools and then the original game. Unsupported records must never
be silently flattened or discarded.

## UI differences from the design reference

The 0.2 pass follows the supplied four-page concept with fixed
Browse/Model/Flight/Graft/Package workspaces, a compact menu bar, grouped
outliner, type icons, linked PT/SH selection, categorized properties, and a
Raw fields/Hex/Details dock. `ui_view.rs` emits drawing commands and hit regions
from the same layout. Minimum-size hit regions are checked in the shared smoke.
Windows uses Tahoma labels and Lucida Console data; Linux keeps its available
X11 fixed font. Both backends remain native, with no added dependencies.

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
transaction. Record-size changes and geometric grafting are separate work. Typed path dialogs, fixed panel splits and a wireframe viewport
remain; vertex Edit mode and bitmap fonts are future work. File selection now uses an
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
unverified. Aircraft-family creation stays in the dedicated Export object wizard.


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
planar projection. The private 64x64 PIC embeds the loaded palette and starts
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
