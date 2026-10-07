# Changelog

User-visible changes by version, newest first. The workspace version lives in
the root [`Cargo.toml`](../Cargo.toml). What was verified for each version is
recorded in [VALIDATION.md](VALIDATION.md); manual acceptance steps are in
[WINDOWS-TEST.md](WINDOWS-TEST.md).

## Unreleased

- **App icon.** A gold outline on a gunmetal plate with TORE over HANGAR
  ([design](../tore-hangar-design/icons/app/README.md)). Windows builds embed
  it at 16, 24, 32 and 48 px in 8-bit and 32-bit color plus a 256 px PNG, so
  Explorer, the title bar and the taskbar show it on Windows 98/ME and current
  Windows. The EXE also carries version information (product name, version,
  GPL notice) for its Properties dialog. On Linux the X11 window sets
  `_NET_WM_ICON`.

## 0.9.0

Design-system UI pass, region-scoped SH editing on retail aircraft, a
moving-parts catalog with pose preview, and stored texture originals with an
eraser and Restore texture. Nothing in this release has been verified in the
original game yet; see [WINDOWS-TEST.md](WINDOWS-TEST.md).


- **Edit Mesh** works on retail aircraft, region by region. Vertex and face
  select modes (1 / 3 and header buttons), click, Shift+click, A, box select
  (B or a drag; Ctrl removes), L for the part under the pointer and **Select
  linked part**. Face picking follows the shaded raster, or the nearest
  outline in Wireframe. Selected faces are filled amber-deep with amber edges.
- Edit Mesh operations, each one undo step: G/R/S through the region writer
  (with **Pivot: Individual** for faces), X/Delete to delete faces, Alt+N to
  flip normals, F to make a face, Shift+D and E to duplicate or extrude and
  then move, and **Add vertex at median**. New faces copy a neighbour or use a
  flat colour from the colour dialog. A refused operation names its reason in
  the inspector. **Select** and **Mesh** header menus list everything with
  keycaps.
- **Parts** lists moving parts by role with icons (Gear left, Nose gear,
  Flap left (state -1), Rudder, Speed brake, Hook, Bay doors, Afterburner,
  Swing wing, Canards). Selecting one selects its geometry for Edit Mesh and
  marks its pivot; clicking a part in the viewport selects it.
- **Pose preview** with Gear down, Gear up, Flaps down and Afterburner
  presets, toggles and gear position in percent. The pose is stored by
  variable name, survives edits and undo, and is never saved; the old
  address-keyed state list is gone.
- Part settings as one undo step each: gate value, gate test, swing range,
  direction, rotation axis and pivot, with reasons on locked controls. Gear
  direction and swing range may now use same-size encodings that no retail
  shape contains; they show a **Not seen in retail** badge until verified in
  the game, and returning to the retail form restores the original bytes.
- Years, IDs, flags, types, classes and sizes show without thousands
  separators (`1997`, not `1,997`).
- `--geometry-check` also walks every reachable gear direction and range
  form; `--edit-check` drives Edit Mesh and Parts on real shapes.

- The editor bodies follow the design system. The outliner has aircraft,
  shape and image type filters, 20px zebra rows with hover, group counts and
  type badges, an active row distinct from its linked entries, and ink filter
  text with a focus border. Browse uses 20px rows under a 28px column header.
- The right-hand editors are collapsible property panels with group icons,
  right-aligned labels and 20px rows (Ctrl+click a header keeps only that
  panel open). They scroll with the wheel and show a scroll thumb instead of
  cutting rows off.
- Integer source values everywhere (Model properties, Raw fields, the
  Flight table and envelope, hardpoint stations, decal placement) are number
  fields: drag to scrub, click to type, Backspace restores the saved operand.
  "^" scaled operands read **scaled**; hardpoint locations are an X/Y/Z
  vector with axis colours.
- The Shape row is a Select of the shapes the entry uses; the link button
  opens its references. Graft uses checkbox rows with diff summaries and a
  primary **Apply graft**; Package leads with a summary notice, result badges
  and a primary **Package LIB**. Brush and Eraser are one segmented control.
- Dialogs share one frame with a title and no shadow; Cancel is a ghost
  button and the dialog's action is primary. Copy across prompts, hints and
  status messages no longer joins phrases with slashes.
- The window chrome follows the Gunmetal design system: flat workspace tabs
  with the active one joined to its editor, menus with keycap shortcuts and no
  drop shadow, the active LIB with a round dirty dot and a lock for protected
  names, and a status bar with key hints, `ENTRY · LIB`, the unsaved edit count
  or saved/validated state.
- Viewport header: a mode select (Object Mode, Edit Mesh, Hardpoints, Parts,
  Texture Paint), a View menu, the hardpoint marker toggle and a Wireframe /
  Solid / Textured control. **Solid** is new: flat face colors lit by their
  angle to the view. Controls fold into a menu instead of disappearing at
  800 x 600.
- The viewport has a floating tool strip, a navigation gizmo that follows the
  camera (click a cap to view along that axis), an origin-aligned grid with
  brighter major lines, full-length axis lines and an origin dot.
- Text uses the design's styles (sizes and weights for labels, titles, hints,
  values and badges) on Windows, X11 and SVG snapshots; long labels truncate
  by measured width with an ellipsis. Icons come from the design icon set.
- Dock tabs are one segmented control in a 28px header; sounds badge as 11K.
- Clicking outside an open menu closes it without acting on the control below.
- The first paint, decal, PIC palette edit or Replace entry on an existing
  `X.PIC` keeps its previous entry as `X.ORG` in the same LIB and undo step,
  with exact bytes and compression flag. Not retroactive. See
  [Erase and restore textures](MANUAL.md#erase-and-restore-textures).
- **Eraser** beside **Brush** (atlas and 3D model) paints the original back;
  **Restore texture** returns `X.PIC` to `X.ORG` byte for byte as one undo step.
  Generated panel sheets return to their face color.
- `.ORG` entries are listed under **Original textures**, preview read-only and
  follow their PIC through rename, delete, copy/move, texture clones and object
  export. **Package > Remove stored originals** drops them for distribution.
- Package checks parse `.ORG` as PIC, warn about originals without a PIC and
  note layout mismatches.
- Fixes: brush errors stay visible; faces whose texture is not loaded no longer
  retry panel generation; Pick color ignores transparent pixels; the atlas UV
  outline follows a draft panel; Esc keeps the paint tool active; the status
  counts flat panels converted by a stroke.
- SH geometry writes now update the stored normal and centre only of faces
  whose vertices moved; every other face keeps its bytes. Recomputed normals
  use the winding found in retail shapes (previously they were written
  inverted), skip collinear leading vertices, and keep the stored normal when
  a face has no non-degenerate vertex triple.
- Transform and vertex-drag previews refresh face normals the same way the
  write does, so the textured preview hides the same rear faces the saved
  shape will.
- Edit mode G/R/S act on the selected vertices about their median point;
  previously R and S transformed the whole shape. G/R/S start unconstrained:
  S scales uniformly, R uses the view axis and G accepts `X Y Z` offsets.
  X/Y/Z toggles the axis lock. A toggles select all/none.
- Vertex presses need 4 pixels of movement before they drag, so a click only
  selects; drags keep the grab offset instead of snapping the vertex to the
  cursor and move the whole selection. Shift+click adds or removes vertices,
  X/Y/Z locks a drag axis, and the selection survives undo/redo.
- Edit mode on a shape owned by another open LIB, or in perspective view,
  now says why vertices cannot be dragged instead of ignoring the click or
  showing the station message.
- The textured viewport frames on the committed shape like the wireframe,
  vertex markers and station cursor, so G previews visibly move and overlays
  stay aligned.
- Parts preview states are cleared when an undo, redo or replace moves the
  shape's import addresses (for example a panel texture added or removed), so
  a state can no longer apply to the wrong guard. The state input list
  scrolls with the wheel instead of hiding rows beyond the panel.
- The viewport Select tool is a real tool that leaves paint mode (it was
  wired to Frame). The shading toggle reads Textured and is highlighted when
  textured shading is on. Controls under an open menu or dialog no longer
  show hover, and clicks on menu padding no longer reach controls beneath.
- Singular counts read "1 reference", "1 direct user", "1 aircraft user".
  SVG snapshots preserve repeated spaces.
- Generated-panel layout sets SizeOfImage to cover every section, including a
  relocation table padded across a page when `.reloc` is the last section.
  Retail FA_2 shapes always place `$$DOSX` after `.reloc` and were not
  affected.
- SH reading covers the whole CODE section: every record, embedded x86 stub,
  import and trampoline is accounted for, and moving parts (gear, flaps,
  rudder, speed brake, hook, bay doors, afterburner, swing wing, canards) are
  identified by the game variable that drives them. Stored part angles now
  show in the preview. The Linux CLI gains `--shape-inventory`,
  `--shape-pose` and stub census checks for manual analysis.
- Retail aircraft shapes are no longer read-only as a whole. Each vertex and
  face reports whether it can be edited and why not. Vertex moves inside gear
  and other parts write the part's local coordinates. The core can delete,
  flip, add, duplicate, extrude and scale faces and add vertices, and can
  change part settings in place (gear shift and rotation axis, pivots, gate
  values and je/jne sense of toggled parts). The editing UI for these follows
  in a later build; the Linux CLI gains `--geometry-check` for manual
  real-data checks.

## 0.8.2

Corrects generated-panel SH layout and provides repair for older painted LIBs
while preserving their PIC pixels. Generated panel records are placed before
the SH end marker, the import tail is relocated and native module packing is
written. See [Ship, ground and animation tools](MANUAL.md#ship-ground-and-animation-tools)
for the repair command.

## 0.8.1

Adds collapsible multi-LIB trees, reviewed copy/move drops, no fixed open-LIB
count cap, and distinct workspace tabs.

## 0.8

Adds ship/ground-vehicle NT fields and station tools, SH state-switch preview
and part placement, a viewport brush that crosses panels, corrected camera
handedness and skin composition, and clickable discard controls.

## 0.7

Adds an editable envelope table, automatic textures for supported flat-color
panels, visible base-color controls, export of weapons and other objects with
their resources, and initial SH vertex edit mode.

## 0.6

Adds viewport hardpoint tools, palette/UV editing, family texture cloning, and
previewed PNG/text/national/squadron decals baked into PIC textures.

## 0.5

Adds structured characteristic grafts, source-aware package reports, and
multiple open LIBs with independent edits, view state and undo history.

## 0.4.1

Protects retail LIB filenames and saves custom LIBs with backups.

## 0.4

Duplicates a selected aircraft directly into its own privately named LIB. The
editor also provides an in-app file browser and recent LIBs, audio playback/WAV
export, PIC preview/PNG export, and linked model/texture painting.
