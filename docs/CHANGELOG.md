# Changelog

User-visible changes by version, newest first. The workspace version lives in
the root [`Cargo.toml`](../Cargo.toml). What was verified for each version is
recorded in [VALIDATION.md](VALIDATION.md); manual acceptance steps are in
[WINDOWS-TEST.md](WINDOWS-TEST.md).

## Unreleased

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
