# Current test build validation

2026-10-06. Implementation work, not a claim of original-game or Windows 98
acceptance. Tests used Rust 1.91.1. No game payloads, extracted models or retail
screenshots are committed or uploaded in build artifacts.

- Twenty-three core tests pass: archive boundaries, duplicate/unsafe names, DCL
  malformed streams, untouched compressed bytes, BRF lossless edits, history
  branches, shape bounds/transforms, DOS-punctuation names, saved-value
  comparisons and donor-family creation/rejection.
- Linux shared-UI smoke passes: selection, transform/invalid-input handling,
  undo, BRF edit, donor wizard, package save and reopening. The 0.2 smoke also
  clicks controls through rendered hit regions, checks linked PT/SH selection,
  menus, highlights, and control bounds at 800x600 across all five workspaces.
- Linux native Xlib rendering was exercised on an unmapped private window and
  captured to a local image. The real F18-derived variant was visually checked
  for layout and correct source-axis orientation. All five redesigned workspaces
  were rendered natively; 1280x800 and 800x600 layouts were visually checked. No user desktop window was
  opened for this check.
- Local Rust formatting, strict Clippy, Linux build, and both Windows release
  cross-builds pass. Both PE import/header audits pass; the executables import
  59 functions across Kernel32/User32/GDI32/WinMM, with no CRT dependency.
- Windows CI passed for both architectures on Windows Server 2022, including
  running each custom-runtime executable's headless smoke path. These checks
  exercise the allocator, integer math, drawing commands, transform and undo.
  They do not exercise native GDI mouse/window interactions or a Win98 kernel.
- FA_1.LIB's reported failure was reproduced: 99 picture names begin with `$`.
  The name validator now accepts ASCII DOS 8.3 punctuation while rejecting
  separators and other invalid path characters. FA_1.LIB (2,001 entries),
  FA_2.LIB, FA_4B.LIB and FA_4D.LIB all pass byte-identical no-op repacks;
  source hashes stay unchanged.
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

The 0.3 media pass additionally validates PIC span holes, byte-local painting,
UV/texture decoding, FC color patches, cloned texture/reference batch undo,
and WAV round trips. Shared UI smoke checks that both atlas painting and 3D
brush hits update the model render before mouse release, commit/reopen correctly,
and undo back to the original bytes. It enumerates directories and opens the
in-app browser without creating a window.

A local real-F18 check mapped a model hit to _F18.PIC, changed exactly one raster
byte, observed the live render change, round-tripped the archive and undid the
change. Source media was not written. Exported 256x644 PNG CRC/zlib/image data
were decoded independently with Python; an 18,061-sample 11025 Hz mono WAV was
read with Python's wave module. Playback on an actual Windows audio device is
still a manual acceptance check.

The 0.4 selected-aircraft export pass adds recursive graph/cycle tests, source
collision checks, missing-source rejection, hidden/non-displayed texture names,
store-icon and cockpit-family alias relationships, and directory-only reads.
Tests verify that image/audio/palette bytes stay exact, compiled sizes stay
exact, only recognized name slots change, import sections remain unchanged,
and a private variant can itself be cloned with its palette preserved.

The real selected-PT GUI event sequence was exercised for A10.PT and F18.PT:
new ID, display name, automatic sibling sources, rename review, output picker,
write and reopen. It produced 44 and 58 privately named resources respectively.
The A-10 model still decodes to the same 373 vertices and 299 faces. Both the
normal and 800x600 review layouts were rendered and checked. The Windows smoke
also writes a synthetic scratch LIB, indexes it, reads a selected payload by
file offset, checks its bytes, and removes that newly created fixture.

Remaining acceptance: the [manual Windows checklist](WINDOWS-TEST.md), actual
Windows 98/ME operation, and loading/flying the new variant in original FA.
General SH part grafting, generic object creation and complete animation/LOD
texture dependency closure are not implemented.


The 0.4.1 save-policy pass adds five portable tests: all 17 reserved filenames
and Win32 aliases, repeated saves with retained backups, staged-write/rename/
rollback failures, concurrent destination collisions, and first-time saves.
The 28 core tests, Linux smoke and strict Clippy checks pass. Shared executable
smoke now also writes/replaces/reopens synthetic custom LIBs, checks two backup
generations, rejects a retail output name, verifies dirty state survives
rejection, and checks that Ctrl+S suggests the current custom path.

A disposable FA_2.LIB copy was used for CLI QA: protected writes were rejected,
A10.PT extraction succeeded, a custom copy was edited/replaced and reopened, and
both numbered backup hashes matched the expected previous versions. Read-only
and symlink destinations were rejected; replacing a hard-linked custom copy
left its retail-named sibling intact. The actual retail source SHA-256 stayed
unchanged. Package layouts were rendered locally at 1280x800 and 800x600.
Both Windows cross-builds and import audits pass (59 reviewed imports, no CRT).
Windows 98/ME execution and interrupted-save recovery on those OSes remain
manual acceptance checks.


The resource-navigation/package-check pass adds six core tests (34 total):
cycle-safe reverse aircraft users, cache invalidation on edit/undo/catalog
changes, explicit scan limits, removed-resource diagnostics, changed payload
errors versus unknown encodings, and bounded reports with retained counts.
Existing clone tests continue to cover hidden module references and exclusion
of imports/sample bytes after extraction of the shared scanner.

Shared UI smoke clicks a shape's texture link and returns through its aircraft
user, retains the live model context, verifies no dirty state from navigation,
checks remove/undo/report invalidation, and scrolls both references and results.
It checks dock hit regions at 800x600 and 1280x800, including the live-preview
tab. Native Linux model/References and Package renders were visually inspected
at those sizes; this pass adds no platform APIs or runtime dependencies.

Core tests, strict Clippy, the shared UI smoke, both Windows release builds and
PE audits pass. Audits still report 59 reviewed imports and no runtime DLLs;
executables are 360,448 bytes (32-bit) and 430,080 bytes (64-bit). Synthetic CLI
`references` and `validate` checks also pass. This pass has not been run on
Windows 98/ME or tested in original Fighters Anthology. The current-LIB index
and advisory package checks do not establish runtime dependency closure.


## Version 0.5 follow-up

Structured definition groups, donor reviews and multi-library workspaces passed
42 core tests, shared UI smoke, Clippy, both release cross-builds and PE audits.
Windows CI run 37522788653 passed both target jobs for d71967f. Tests cover
independent document history/camera state, inactive dirty-document close guards,
reviewed copies and collision decisions, stale-plan rejection, reference-aware
rename/undo, and dependency resolution across open libraries. No original-game
or Win98/ME acceptance was claimed.

## Version 0.6 hardpoint, material and decal pass

49 portable core tests pass. New cases cover no-op and byte-local station moves,
count updates, add/duplicate/remove, shared-block guards and private store
references; palette and UV-slot patches; hidden family texture literals; PNG
CRCs, all five filters, indexed and 16-bit samples; alpha, opacity, rotation,
mirroring and preserved PIC span holes.

Shared UI smoke clicks and drags station markers, checks no dirty state before
release, edits stores, and undoes UV/palette changes. It previews/applies/cancels
national/text/PNG decals, verifies exact archive restoration with one undo, and
checks all tools and the 16-entry PNG library at 800x600 and 1280x800. Native
Linux hardpoint, material, national-decal and tail-text layouts were inspected.
Context models reload after undo, so UV previews return to the restored data.

An independently generated PNG using Python zlib and all five filters was loaded
through the application. A separate Python alpha/palette compositor matched
every exported pixel. Package reopen, metadata/mask preservation and one-step
undo also passed. No retail artwork or squadron logos are committed.

Strict Clippy, Linux shared smoke, both Windows release cross-builds and PE
audits pass. The completed binaries are 543,232 bytes (32-bit) and 638,464 bytes
(64-bit), with the same 59 reviewed imports and no runtime DLLs. PNG decoding
adds statically linked no_std miniz_oxide/adler2 code. Format milestone 010fba4
passed Windows CI; the final UI is checked by its pushed workflow.

Original-game behavior and Windows 98/ME runtime operation remain manual
acceptance checks. National presets are compact editor artwork; exact variants
and authentic squadron logos are supplied as PNG. The private aircraft-ID PAL
is an editor preview palette and does not override FA's game-global palette.
