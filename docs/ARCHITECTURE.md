# Architecture and format references

Implementation milestone: a native LIB/BRF editor with an SH preview and a
restricted static writer. Agent decisions below were chosen for the requested
lightweight portable application. The user's accepted CPU minimum is SSE2.

## Layers

- `hangar-core/archive.rs`: bounded EALIB directory, shared source storage,
  lossless unedited archive serialization, raw DCL decoding and stored writes.
- `clone_aircraft.rs`: selected-PT dependency graph, private names and reference
  rewriting. `ui_clone.rs` indexes sibling/additional source LIBs, reads only
  requested payloads, and presents the export review.
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
- `document.rs`: entry-granular reversible operations. Original source buffers
  are reference counted. Saving tracks a revision; undo after save shows dirty.
- `picture.rs` / `audio.rs`: bounded indexed PIC and PCM readers adapted from
  Fighters, lossless raster-byte patches, PNG encoding and WAV wrapping.
- `ui.rs`: shared events, layout and draw commands from the supplied theme.
  `ui_browser.rs` lists platform-provided files/roots and recent LIBs.
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

The loose-SH workflow in authoring.rs remains separate. It retains its original
shared-dependency contract and is not the default New aircraft action.

SH writing only accepts the understood straight-line subset of vertex buffers,
faces, texture/fog selection and source strings. A spatial header, control-flow,
vertex-normal record or any other skipped record marks the pose read-only.
The writer preserves record sizes, rejects coordinate/byte-center overflow,
and updates face centers/normals where present. General aircraft do not qualify.
The synthetic demo is an editor fixture, not a proven game-loadable aircraft.

A complete SH writer must preserve every branch, LOD, state switch, local
transform, contact box, visibility plane, normal, relocation, and reference.
The next milestone is an editable intermediate representation with original
record provenance and byte-identical no-op round trips, before opening up
writes to real aircraft. Test general writes against the independent OpenFA
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
shown explicitly. Package validation checks the archive directory, not full
semantic or animation dependency closure.

The animation timeline and geometric graft controls remain unimplemented and
are not shown as working controls. Graft supports the existing single-field
donor operation. Typed path dialogs, fixed panel splits and a wireframe viewport
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
