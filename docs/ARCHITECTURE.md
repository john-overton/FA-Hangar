# Architecture and format references

Implementation milestone: a native LIB/BRF editor with an SH preview and a
restricted static writer. Agent decisions below were chosen for the requested
lightweight portable application. The user's accepted CPU minimum is SSE2.

## Layers

- `hangar-core/archive.rs`: bounded EALIB directory, shared source storage,
  lossless unedited archive serialization, raw DCL decoding and stored writes.
- `authoring.rs`: isolated donor PT + imported main SH workflow. Rewrites five
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
- `ui.rs`: shared events, layout and draw commands from the supplied theme.
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

Aircraft creation follows the shadow-derived damage-family contract documented
in the sibling `docs/spec/fa-xx-export.md`. It requires an explicit `_S.SH`
reference and all five companion resources. It does not guess generic object
families. Imported main SH bytes and donor companion bytes remain unchanged.
Only PT identity and main/shadow reference strings change. Textures are found
through the bounded static-pose reader; full dependency closure is not claimed.

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
remain; vertex Edit mode, native file pickers and bitmap fonts are future work.
