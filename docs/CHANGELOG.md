# Changelog

User-visible changes by version, newest first. The workspace version lives in
the root [`Cargo.toml`](../Cargo.toml). What was verified for each version is
recorded in [VALIDATION.md](VALIDATION.md); manual acceptance steps are in
[WINDOWS-TEST.md](WINDOWS-TEST.md).

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
