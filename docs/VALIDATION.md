# First test build validation

2026-10-06. Implementation work, not a claim of original-game or Windows 98
acceptance. Tests used Rust 1.91.1. No game payloads, extracted models or retail
screenshots are committed or uploaded in build artifacts.

- Eleven core tests pass: archive boundaries, duplicate/unsafe names, DCL
  malformed streams, untouched compressed bytes, BRF lossless edits, history
  branches, shape bounds/transforms and donor-family creation/rejection.
- Linux shared-UI smoke passes: selection, transform/invalid-input handling,
  undo, BRF edit, donor wizard, package save and reopening.
- Linux native Xlib rendering was exercised on an unmapped private window and
  captured to a local image. The real F18-derived variant was visually checked
  for layout and correct source-axis orientation. No user desktop window was
  opened for this check.
- Local Rust formatting, strict Clippy, Linux build, and both Windows release
  cross-builds pass. Both PE import/header audits pass; the executables import
  48 functions across Kernel32/User32/GDI32 only, with no CRT dependency.
- Windows CI passed for both architectures on Windows Server 2022, including
  running each custom-runtime executable's headless smoke path. These checks
  exercise the allocator, integer math, drawing commands, transform and undo.
  They do not exercise native GDI mouse/window interactions or a Win98 kernel.
- A user-owned retail FA_2.LIB containing 5,405 entries was opened locally.
  A no-op repack was byte-identical. Editing F18.PT object weight changed only
  its payload; all 5,404 other payloads and compression flags were identical.
  The source hash remained unchanged.
- F18.SH decoded into 357 vertex records and 287 faces in the selected static
  pose. It was correctly marked read-only for geometry writes.
- A TEST18 aircraft candidate was built from F18.PT and an imported F18.SH:
  seven renamed definition/shape-family entries plus _F18.PIC. It reopened.
  Exactly five PT identity/reference operands differed; the other definition
  text remained identical. Gameplay values were not changed. The source donor
  was not modified.

Remaining acceptance: the [manual Windows checklist](WINDOWS-TEST.md), actual
Windows 98/ME operation, and loading/flying the new variant in original FA.
General SH part grafting, generic object creation and complete animation/LOD
texture dependency closure are not implemented.
