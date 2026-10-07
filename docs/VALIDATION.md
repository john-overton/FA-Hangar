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
General SH part grafting and complete animation/LOD texture dependency closure
remain pending. The 0.7 pass below adds generic object export.


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

## Version 0.7 envelope, object export and SH tools

54 core tests pass. New coverage includes weapon identities and implicit icon
pairs, unchanged numeric operands, leaf/opaque exports, generated panel UVs and
palette pixels, byte-identical SH no-op writes, follow-up vertex movement,
coordinate limits, exhausted virtual space/jump reach and face-relocation
refusals. A panel with an existing named texture is never silently replaced
when its PIC is unavailable.

Shared UI smoke edits an envelope table cell and undoes it, changes base color,
stages/cancels/commits an automatically textured stroke, reopens the resulting
archive, and restores the original SH while removing the generated PIC in one
undo step. It tests numeric vertex movement, drag preview/commit/cancellation,
mode-switch painting guards and control bounds at 800x600 and 1280x800. Native
Linux envelope, palette, mesh and generated-texture layouts were inspected.

Local read-only retail checks converted one supported flat panel in F18.SH
(357 decoded vertices / 287 faces) and A10.SH (373 / 299). The independent
Fighters tore-formats Shape::with_export_state reader confirmed that every
decoded face position and normal stayed equal and exactly one face's texture
and UVs changed. A separate section comparison confirmed unchanged RVAs and
byte-identical non-CODE payloads. These checks cover the selected static pose,
not every original-game branch, camera or damage state.

AIM9M.JT exported into a seven-resource private weapon package with its shape,
icon, texture, two sounds and editor palette. Every non-string BRF operand
matched the source. The generic GUI wizard also completed ID/name entry,
automatic source resolution, review, export and reopen for this weapon.
Temporary game-derived outputs stayed outside the repository.

Formatting, strict Linux/Windows Clippy, Linux smoke, both Windows cross-builds
and PE audits pass with 59 reviewed imports and no new runtime dependencies.
Core milestone 9c7280a passed both Windows CI jobs; the final UI is checked by
its pushed workflow. Actual Windows 98/ME operation and original-game loading
of generated panel textures remain manual acceptance checks. Full animated SH
geometry and topology editing are not enabled by this milestone.

## Version 0.8 object workflows, states and viewport fixes

59 portable core tests pass. Additional cases cover NT station movement, slew
edits, add/remove/store changes; byte-local C4 placement and no-op preservation;
state-guard alternatives; bounded loaded-launcher selection; and F6 color/normal
provenance. Shared UI smoke checks NT tools, part edit/undo, cancel/discard buttons
for one LIB and the whole app, camera handedness/top depth, keyed versus opaque
index 255, base-fill composition, backface exclusion, multi-PIC stroke commit/
cancel, and two generated panel sheets restored with their SH in one undo.

All seven real-data GUI export workflows passed ID/name entry, source resolution,
review, export and reopen: AIM9M.JT, BLDG1.OT, TICON.NT, M1.NT, ZSU23.NT,
SA2A.NT and CHAP.NT. The local acceptance example separately verifies unchanged
numeric clone operands, controlled field edits, payload recovery and undo.
All seven main shapes match the independent scenery reader's face counts,
including the newly supported loaded SAM/Chaparral branches.

The independent tore-formats reader checked every binary combination of the
observed guards: 128 F-18 states and 8 A-10 states. Generated-panel geometry and
normals match their baseline in every combination, with exactly one texture/UV
change per pose. C4 placement differences are visible in 64 and 4 states,
respectively. The patched static OpenFA tool round-trips both part-placement
SHs byte-for-byte. It rejects the appended-tail generated-texture layout (the
earlier unpadded stub also broke whole-module instruction boundaries). This
external-tool limitation is recorded in the handoff rather than treated as a
successful full-module validation.

The user's Windows screenshots show opposite-wing, horizontally mirrored paint
and interfering skin layers in 0.7. The camera transform had negative determinant,
the top preset faced below the model, and the renderer lacked native keyed/base
composition and front-normal selection. Fixes change editor projection and
rendering only; the SH UVs and PIC pixels are retained. Stock A-10 preview was
inspected locally after correction. Exact parity for the user's edited A10_V2
package still requires their Windows retest; that file was not provided.

Native Linux layouts were inspected at compact and normal sizes. Strict Clippy,
Linux UI smoke, Windows cross-builds and PE audits are required for the final
commit; the pushed workflow also runs the shared smoke on both Windows targets.
Local handoff files contain user-owned game data and remain outside Git/CI.
Original-game and Windows 98/ME acceptance remain pending user tests. Native
continuous animation, visual turret editing, full topology and module relocation
are not claimed by this milestone.


## Version 0.8.1 workspace pass

61 core tests pass. Move tests cover shared transitive dependencies, source and
target undo, empty-source moves, stale source/target snapshots, and a skipped
conflicting root with no partial mutation. UI smoke verifies copy/move and
scope switches, permits opening while dirty, opens 26 synthetic documents,
collapses inactive roots, scrolls to the last root, and checks 800x600 hit bounds.
Native compact outliner and drop-review layouts were inspected. Original-game
acceptance assets are unchanged from the 0.8 handoff; the included editor is
updated to 0.8.1.


## Version 0.8.2 generated-panel repair

The user-provided A10_V2.LIB has 47 entries and three generated PICs.
The repaired package changes only A10_V2.SH. All 46 other stored payloads and
compression flags, including every painted texture and PT operand, stay exact.
The input file remains unchanged (SHA-256
4de3682ad5c574d1f75f5bd6f45a883e3ad1e230a8b7c73b0a55158b372f1ca7).

The independent reader compares all eight combinations of the observed A-10
state guards, matching symbols across relocated aliases. Face positions, normals,
colors, UVs, material names and counts are identical before/after repair. The
shared UI repair command preserves rendered pixels, one undo restores the exact
archive and redo restores the repaired archive.

The patched static OpenFA decoder/compiler now round-trips the repaired SH
byte-for-byte: 25,088 bytes, SHA-256
0ed99e88320068203849a4916bbde88ac730667fc6b61f281dcdd49c25e58457.
It also passes exact round trips for another panel added to that repaired SH
and new generated-panel outputs from the stock A-10 and F-18. These replace
the failing whole-module checks recorded for earlier appended-tail layouts.

64 core tests include import-tail movement across relocation pages, repeated
panel generation, padded/unpadded legacy repair, no-op idempotence, native
record references, malformed/truncated modules and unsupported metadata.
Original-game visual confirmation remains pending the user's retest; parser
and compiler success alone is not recorded as native-game acceptance.

Formatting, strict Clippy, shared UI smoke, both Windows cross-builds and PE
audits pass. The binaries are 672,256 bytes (32-bit) and 786,944 bytes (64-bit),
with the same 59 reviewed imports and no runtime DLL additions.

## 0.9.0 bug-fix sweep

A read-only probe over the user's retail FA_2.LIB (1,275 SH entries, 1,249
decodable by Hangar's reader) checked the face-normal grammar before changing
the SH writer. No retail face sets content flag 0x20 without 0x40: every
0x2x face also carries 0x40, so the 0x40 (OpenFA) and 0x60 (previous Hangar)
has-normal rules decode identical faces in all 1,249 shapes. 137,908 faces
store a normal; all have unit length (32765). Recomputing them from their
vertices, the stored normal matches (c-a)x(b-a) in right/forward/up order:
the new writer's normal agrees (cosine > 0.9) for 128,632 faces, is opposite
for 89, and 3,196 are degenerate. Previously written normals were inverted.
Stored centres equal the truncated vertex average (including C4 translation)
within one unit for 134,608 of 134,707 non-degenerate faces. No FA_2 shape is
writable by the static-subset writer, so the corrected writer is exercised by
synthetic core tests (byte/word centres, collinear leading vertices, fully
degenerate faces, byte-centre overflow, untouched faces byte-identical).

The same probe generated up to 12 successive panel textures on each of the
757 FA_2 shapes that accept one (6,566 outputs). Every output's SizeOfImage
covered all sections; retail SH modules always order sections CODE[, .idata],
.reloc, $$DOSX by RVA. A synthetic module with .reloc last and a 1 KiB
relocation table reproduced an understated SizeOfImage (0x7000 for a section
ending at 0x7200), now fixed. Tail detection refused no FA_2 shape; first-panel
failures were CODE room (11), degenerate panels (17) and special colors (4).

Strict Clippy, 68 core tests, the shared UI smoke test (including edit-mode
G/R/S, vertex drag threshold/grab offset/Shift selection, textured framing,
Parts state revalidation and scroll, toolbar/menu fixes) and both Windows
cross-builds with PE audits pass: 689,664 bytes (32-bit) and 805,376 bytes
(64-bit), the same 59 reviewed imports and no runtime DLLs. Windows
interaction and original-game checks remain pending.


## 0.9.0 SH inventory and part census

A read-only `--shape-inventory` run over the user's retail LIBs: 1,274 of the
1,275 SH entries in FA_2.LIB and all 7 in swpatch.lib decode with no opaque
spans, and every recorded SH pointer lands on a record start (for example all
48,821 `06` and 15,155 `0C/0E/10` relative pointers). FA_1.LIB and FA_4B.LIB
contain no SH entries. The `38` operand does not behave as a pointer under any
base tried (46 of 64,981 hit a record start) and remains unverified.

151 shapes carry part bindings. Every gear, flap, brake, rudder, hook, bay,
afterburner, canard and swing-wing stub matches a recognised toggle or
transform pattern; the 101 unrecognised stubs are ship/radar turrets, insect
wings, launcher counts, effects and ejection logic. Flaps, rudders, brakes,
hooks and afterburners select alternate pre-modelled meshes; gear, swing
wings, canards and bay doors write C4 rotation words. Gear transforms vary
only in shift (1: 401 stubs, 2: 16, 3: 15), negation and the rotation word;
pivots are per-aircraft. Posed A-10 and F/A-18 gear retract in plausible
directions in the preview. This is preview plausibility, not original-game
verification.

## 0.9.0 geometry editing and part settings

108 core tests pass. 17 of them are synthetic geometry fixtures covering
frames and slot proofs. They also cover refusals for an inner pointer, an
in-face relocation, a slot rewritten by a called block, mixed frames, CODE
room, jump reach, the slot ceiling, collinear and per-vertex-shaded faces,
and a missing native tail. Further cases cover no-op byte identity, pure-
function determinism, one undo step through `Document`, 38-scope padding,
texture switch and restore, word indices above slot 255, and truncated and
corrupted input. 3 part-settings tests cover naming, every editable control
with the binding re-checked, read-only D1 shift and direction, and locked
unrecognised stubs. A shared synthetic SH assembler (`shape_testkit`) builds
the fixtures; no retail data is used.

Manual `--geometry-check` against the user's retail FA_2.LIB, 2026-10-07:

- Analysis of all 1,275 SH: every shape analyses, and 313,097 of 313,097
  stored vertices are writable. 249,364 of 249,943 faces are editable; the
  other 579 are never drawn. 1,779 stub-driven parts are listed, with 21
  locked (`_currentTicks`, `_PLstate`, `_PLdead`: radars, towers and the
  ejection seat). swpatch.lib: 7 SH, 5,580 of 5,580 vertices, 3,892 of 3,892
  faces and 130 parts, none locked.
- Edits on in-memory copies of F18, F14, A10, F16, AV8 and B2, written
  create-new under /tmp/claude-1000/wave3a/: delete, flip, add face, duplicate,
  extrude (flipped base), part-vertex move and 110% scale of a drawn root face.
  All 42 edits pass. Inventory coverage stays at 100% with no opaque bytes,
  bindings are unchanged, every drawn face slot still resolves, and file sizes
  are unchanged (the tail moved within existing CODE slack). The model reader
  shows −1/0/+1/+1/+4..5 faces as expected. B2's neutral model already
  failed on an unsupported pose guard before any edit, so B2 relies on the
  inventory and geometry checks alone.
- 83 part-setting edits across the six aircraft (gate compare, je/jne,
  rotation axis, pivot and C1 shift on F18/F14/F16 gear) each change one byte
  or one word. All pass the re-check that the binding reports the new value.
  Nose gear naming matches the aircraft, including the A-10's offset nose leg.
- Evidence found on the way: the 6C word at +8 is a branch for all 10,557
  retail 6Cs. Without the 38-scope padding reading, 30–50% of the faces the
  preview draws looked unreachable. 148 vertices sit behind jump chains
  deeper than 32 regions, so proofs follow up to 128.

None of these edited shapes has been loaded in the original game yet.

## 0.9.0 texture originals

75 core tests pass, including synthetic `.ORG` coverage: one backup on the
first edit and none on the second, undo removing both, a removed original
recreated from the saved entry rather than the painted state, a pre-existing non-PIC
`X.ORG` never adopted or overwritten, byte-exact restore that returns the entry
to its saved storage, a literal-only DCL (flag 4) original keeping its flag,
eraser circles restoring exact raster bytes, rename/duplicate/delete/copy/move
carrying the companion (including onto an identical target PIC), family clones and object export copying it, and
validation of orphan and mismatched originals. The UI smoke clicks Brush,
Eraser, Restore texture and Remove stored originals, erases back to the saved
bytes (the session's `.ORG` is dropped and nothing reads as changed), keeps the
`.ORG` after a partial erase, checks a same-color palette entry is a no-op,
undo/redo, Esc, the read-only `.ORG`
preview, Delete, decal and palette backups, the 3D eraser and a generated
panel's return to its face color, and repacks/reopens with `.ORG` intact.

`--restore-check` on a copy of the user's FA_2.LIB (F18.SH, F16.SH, A10.SH)
kept `_F18.ORG`, `_F16.ORG` and `_A10.ORG` once with flag 4 and their stored
sizes, erased back to the exact saved bytes, restored byte for byte with no
remaining changes, and passed undo/redo, validation and repack. The existing
`--paint-check` still passes. Formatting, strict Clippy and both Windows
release builds pass; the PE audit reports 714,240 bytes (32-bit) and 836,608
bytes (64-bit) with the same 59 reviewed imports. Loading a LIB that contains
`.ORG` entries in the original game has not been tested yet.

## 0.9.0 Edit Mesh, Parts and same-size gear encodings

111 core tests pass. New ones walk every reachable state of each census
gear slot (D1, D1+NEG, C1 2/3, C1 2/3+NEG): exactly the states that fit are
reachable, each state has one encoding, the census form returns to the
original bytes from every state, no-op settings are byte-identical, and the
preview turns each leg by -(gearPos >> n), negated with the NEG. Refusals
cover a NEG in a 3- or 4-byte slot, shift 2 or 3 with NEG in 6 bytes, NEG with
shift 2 in 6 bytes, swing-wing direction and truncated input. The synthetic
parts aircraft (`shape_testkit::demo_parts`) lists 11 named parts, all
vertices writable and all faces editable.

The shared UI smoke test (headless, Linux; it runs on both Windows targets in
CI) drives Edit Mesh and Parts on that aircraft through hit regions: the mode
Select, 1/3 and the header buttons, click, Shift+click and empty-space clicks,
A, a drag box and B, L, the Select and Mesh menus at both sizes, delete (menu
and X), Alt+N, E then Z 4, Shift+D then X 3 (and Esc cancelling it), S 50 with
individual origins and median, F with a neighbour's colour and with a flat
colour from the colour dialog, Add vertex, and raster face picking. Every edit
is one undo step back to the exact bytes. Refusals show core's reason: two
vertices for F, and E or G on a gear leg the pose rotates. Parts: the role
list, presets, toggles, the gearPos field (never dirtying the document), part
pick from the list and the viewport, the Direction Select with its **Not seen
in retail** option applied and undone, pivot scrub and Backspace, a gate value,
and locked reasons. Hit-region geometry passes for Edit Mesh (faces, with a
refusal notice, and vertices), Parts, the scrolled settings panel and a
non-retail setting at 1280 x 800 and 800 x 600.

Manual checks against a copy of the user's FA_2.LIB, 2026-10-07:

- `--geometry-check` analysis of all 1,275 SH is unchanged by the new `nop`
  decoding: 313,097 of 313,097 vertices writable, 249,364 of 249,943 faces
  editable, and `--shape-inventory` output byte-identical. Parts now number
  1,767 (21 locked): the 12 ranged-gear second paths (A4, F4J, M17, M21F) are
  merged into their legs instead of listed as extra "Gear mesh" parts.
- F18, F14, A10, F16 and AV8: all 35 region edits and 78 part settings pass
  (inventory 100%, bindings unchanged except the edited one). Their 30 gear
  legs have 3-byte slots (18, fixed), 4-byte C1 slots (3, sar 1-3) and 6-byte
  D1+NEG slots (9, four states each). Every reachable state re-parses with the
  expected law and previews, and the census bytes return.
- F111, MIG29, SU25 and SU35 add the 7-byte C1+NEG slots: 7 legs reach all
  six states. A4, F4J and M17 add the ranged gear (two paths through one
  slot); both paths' laws are checked. On A4 and F4J the check's own choice of
  face was refused for add, duplicate and extrude (a degenerate face; a
  textured face with no active texture at its host), as designed; F8 has no
  face that heuristic accepts.
- `--edit-check` on the same five aircraft runs delete, flip, extrude,
  duplicate, G, S with individual origins, make face and add vertex on a body
  face, and every flippable gear direction, through the app: 49 of 49 pass
  and each undo restores the exact entry.
- Stack: the deepest slot-liveness proof is 48 levels and 21 KiB on x86_64
  release (cap 128, about 57 KiB), so it stays recursive.

Formatting, strict Clippy and both Windows release builds pass; the PE audit
reports 1,127,424 bytes (32-bit, was 939,008) and 1,288,704 bytes (64-bit,
was 1,080,832) with the same 59 reviewed imports and no runtime DLLs. About a
third of the growth is the region operations from core that the app now
links (append, extrude, duplicate, delete, flip); the rest is the Edit Mesh
and Parts UI and their smoke tests.

None of the edited shapes, part settings or non-retail gear forms has been
loaded in the original game yet; steps are in WINDOWS-TEST.md.

## Unreleased app icon

The `.res` that `build.rs` writes is byte-identical to `llvm-rc` output for
the equivalent `.rc` (ICON plus VERSIONINFO). `check_pe.py` finds icon group 1
with 32-bit and 8-bit DIBs at 16, 24, 32 and 48 px plus the 256 px PNG, and
version 0.9.0, in both release builds; `--extract-icon` writes back a file
byte-identical to the committed `.ico`, and every entry decodes (ImageMagick)
to the committed PNG pixels. On Linux/Xwayland, `xprop` reads `_NET_WM_ICON`
from the demo window as 48, 32 and 16 px images matching the PNGs. The PE
audit reports 1,158,656 bytes (32-bit, was 1,127,936) and 1,320,960 bytes
(64-bit, was 1,290,240) with 60 reviewed imports (`LoadIconA` added).

The icon has not yet been seen in Explorer, a title bar or a Properties
dialog on Windows 98/ME or current Windows; steps are in WINDOWS-TEST.md.

## Unreleased unresolved-in-source exports

Reproduced the report: `export-object FA_2.LIB F14.PT ...` with FA_1, FA_4B,
FA_4D, swpatch, FA_4C and FA_3 as sources stopped at "F14_C.SH references
missing B.PIC". `B.PIC` is not a stored reference. The scanner took the bytes
`E2 00 42 00` at CODE offset 0x1ED5 for an E2 texture operand; they lie inside
the 39-byte FC face record at 0x1EC6, which the static traversal reaches. The
shape's only texture record is the E2 at 0x0070 (`_f14_c.PIC`). Across
FA_2.LIB and swpatch.lib, the record inventory rejected 90 such E2-like
patterns inside FC, F6, 38, 78, 82 and C8 records (tokens such as `B`, `l`,
`!`, `9T9A`); 1,602 genuine E2 records were kept, 130 of them unreached.

Census: every FA_2.LIB `.PT` (145) exported with FA_1, FA_4B, FA_4D,
swpatch, FA_4C, FA_3, FA_7, FA_10, FA_10B, FA_11 and FA_11B as sources.

- With the byte heuristic, 33 refused; 32 of them only or also because of
  phantom names (A310, AF1, AH1, ALPH, ATL, AURORA, B707, C5, DRAK, E8, F104,
  F111, F14, F31, F31F, F4, F5E, F5EE, IL76, M2000, MF1, MI24, MIG21F, MIG29,
  MIG31, SEAHAR, SFR, SFRV, SU25, TU160, TU95, UH60).
- With the inventory check, 143 export without flags. Two have names that are
  in no retail LIB under the game folder, including FA_7/FA_10/FA_11 and the
  LHX LIBs: `MIG31.PT` -> `Y141.HUD` (BRF string, hudName) and `~BGUN.PT` ->
  `EJECT_A.SH` to `EJECT_D.SH` (damage-family convention from its
  `EJECT_S.SH` shadow). Root names used to be read unconditionally, so even
  with the inventory check both failed with "Missing referenced resource".
  With `--keep-unresolved` all 145 export.
- F14X (56 resources, 1,734,006 bytes) exports with no flags; `validate`
  passes with 0 errors (one existing HUD note, `~F14_W`), and `references`
  on `F14X_C.SH` shows only `_F14X0_C.PIC` (texture record, local).
- MIG31X (`--keep-unresolved`, 36 resources) reopens and validates with 0
  errors; `Y141.HUD` is a warning, and `references MIG31X.PT` shows it as a
  BRF string not in the LIB. BGUNX (28 resources) validates with the four
  family members as warnings. `--substitute EJECT_A.SH=_F14.PIC` is refused
  (not a texture); `--substitute B.PIC=_F14_C.PIC` on F14 is refused (not
  unresolved).
- The five disc LIBs of 140 to 186 MiB are read as sources through the
  directory index and range reads; before, the CLI failed with "File exceeds
  128 MiB limit". The GUI wizard already indexed its sources this way and was
  not affected; opening such a LIB as the edited document is still limited to
  128 MiB.
- `--clone-check` through the GUI wizard (automatic sibling sources) exports
  and reopens F14 without acknowledgement and MIG31 after ticking the
  checkbox.

Core tests cover refusal by default, Keep preserving bytes, Substitute
changing only the 14-byte name slot of the copy, a second aircraft and the
source LIB keeping the stored name, kept names reserved from private names
and fixed-name collisions, missing family members, substitute validation and
E2 reachability. The smoke test drives a synthetic dangling texture through
the review's hit regions: blocked, substituted from the picker, kept again,
a typed missing PIC refused, acknowledged, exported and reopened, then the
substitute path exported and reopened. The review was rendered at 800x600
and 1280x800 with the picker open.

Formatting, strict Clippy, all tests and the smoke test pass; the PE audit
reports 1,219,584 bytes (32-bit, was 1,180,672) and 1,391,104 bytes (64-bit,
was 1,346,048) with the same 60 reviewed imports. No exported aircraft has
been flown in the original game yet; steps are in WINDOWS-TEST.md.

## Unreleased identity panel, rename and duplicate

Real data: a copy of `FA_2.LIB` (5,405 entries) under a scratch folder,
`--identity-check FA_2.LIB F14.PT F14Z F18.PT F18Z IDENTITY.LIB` (release
build, 3.8 s for both reviews, the undo/redo check and the save).

- Rename F14 -> F14Z: 14 resources renamed (`F14.PT`, `F14.SH`, `F14_A.SH`
  to `F14_S.SH`, `_F14.PIC` and `_F14_A.PIC` to `_F14_D.PIC`, `F14.ECM`,
  `F14R.SEE` -> `F14ZR.SEE`), 0 other entries rewritten, 24 shared resources
  left alone: 8 sounds, 4 weapons (by convention), `PALETTE.PAL`, and by use
  `F14CC.HUD` (also ALPH, BUC, CKUO and Q5), `F.BI`, `F250.GAS`,
  `VIS340.SEE` and the weapons' shapes and skins. No refusals, no private file
  kept. 69 missions in the LIB name `F14.PT` and are not rewritten; `F14.HUD`
  and `F14.PTS` keep their names (no stored link from `F14.PT`). Undo restored
  the exact LIB bytes and redo the rename.
- Duplicate F18 -> F18Z with the default choices: 2 copied (`F18.PT`, and
  `F18.HUD` -> `F18Z.HUD`, which the game finds by the aircraft's name) and
  20 names shared. `F18.SH`, the damage family and the `_F18` skins are also
  used by `F18C.PT`, so they stay shared; `F18Z.PT` refers to them and to
  `F18_S.SH` unchanged. The new PT was selected.
- `validate`: the source reports 0 errors / 1,729 warnings; the result
  0 errors / 1,743 warnings. The 14 new warnings are `F18Z.HUD`'s copies of
  `F18.HUD`'s unresolved cockpit names (`~F18`, `~F18H`, ...; the cockpit art
  is in another disc LIB). Entries 5,405 -> 5,407; observed references that
  resolve inside the LIB 4,859 -> 4,879. The saved file is 31,627,535 bytes
  (source 31,546,692): changed entries are stored uncompressed.
- `references`: `F14Z.PT` links only local names (`F14Z.SH`, `F14Z_S.SH`,
  `F14ZR.SEE`, `F14Z.ECM`, `F14Z_A` to `F14Z_D` by the damage-family
  convention, the shared weapons, sounds and `F14CC.HUD`); `F14Z.SH` draws
  `_F14Z.PIC`; `F14CC.HUD` lists ALPH, BUC, CKUO, F14Z and Q5 as users.
  `F18Z.PT` links `F18Z.HUD` (default HUD convention) and the shared F18
  family. No `F14*` name but `F14.HUD`, `F14.PTS` and the shared `F14CC.HUD`
  remains.
- `export-object FA_2.LIB F14.PT F14X "My F-14" ... --keep-unresolved
  --short F-14X` writes `"F-14X"`, `"My F-14"`, `"F14X.PT"` (38 resources
  with FA_2 alone).

Core tests cover names-block edits (bytes outside the operand unchanged,
`si_names` following matching text, validation), ownership (private versus
shared by convention and by use, damage and skin families moving together),
a rename rewriting the self reference, another object's reference, E2 and
HUD fixed slots with nothing else changed and undo restoring the bytes,
collision, 8.3, slot and shared-HUD refusals, the same-ID rename being empty,
and duplicates with the default and toggled Copy/Share choices referencing
shared names, `.ORG` companions following copied PICs and the LIB validating.
The smoke test drives, at 800x600 and 1280x800, the Identity fields (a
refused name, amber with the saved value, reset, undo), Rename review, Back,
apply, undo and the empty same-ID review, the duplicate wizard and review
with Copy/Share toggles (weapon and icon, main shape and its skin), apply
with the new PT selected and validating, undo, and the context menu on a PT
and on a shape. Hit geometry covers the rename and duplicate reviews, the
wizard's short-name step and the changed Identity panel at both sizes.

Formatting, strict Clippy, all tests and the smoke test pass. `App` was
3,856 bytes after adding the boxed drafts, enough to put `mainCRTStartup`
over the 4 KiB frame the x86_64 build cannot probe; the export review's
unresolved-name state moved into a box (3,808 bytes) and the build links.
Collecting name sets into a `BTreeSet` pulled in the stable sort; the names
are inserted one by one. The PE audit reports 1,343,488 bytes (32-bit) and
1,537,536 bytes (64-bit) with the same 60 reviewed imports. Neither aircraft
has been listed or flown in the original game yet; steps are in
WINDOWS-TEST.md.

## Unreleased per-panel textures and sized panel sheets

Read-only probes on a copy of the user's retail `FA_2.LIB` (2026-10-07; the
original file was not modified, and no game data is in the repository):

- **Texture state proof.** `Geometry::material` proves the selector for
  116,558 of the 137,959 faces the neutral models of 1,250 decodable shapes
  draw, and for every face in 966 of them. Where `Model` also knows the
  selector (107,599 faces) the proof names the same bytes in every case. The
  unproved faces are mostly in shapes with no E2/E0 before them. The neutral
  F-18, F-16 and A-10 are proved completely, including the 64, 48 and 126
  textured faces for which `Model` has no selector (it clears it at F0
  stubs).
- **Every face assigns and restores.** On `F18.SH` (287 drawn faces),
  `F16.SH` (361) and `A10.SH` (299), each face alone was assigned to a new
  PIC (Keep for textured faces, Project for flat ones) and then restored:
  947 of 947 succeeded, each verified by the core's own re-parse.
- **App-driven check.** `--face-texture-check FA_2.LIB FA2_TEX.LIB F18.SH
  F16.SH A10.SH` picked the four highest side-facing textured faces in the
  rear 30% of each aircraft (the vertical tails; on the A-10 also an engine
  nacelle side) and ran Clone texture for selected faces through the Mesh
  menu action and its name prompt:

  | Shape | Copy | Continuations | SH bytes | Bindings | Faces |
  | --- | --- | --- | --- | --- | --- |
  | `F18.SH` | `_F18T1.PIC` | 2 | 33,280 unchanged | 18 unchanged | 287 = 287 |
  | `F16.SH` | `_F16T1.PIC` | 2 | 37,376 unchanged | 19 unchanged | 361 = 361 |
  | `A10.SH` | `_A10T1.PIC` | 4 | 25,088 unchanged | 12 unchanged | 299 = 299 |

  The continuations fit the existing CODE padding, so no SH grew. Inventory
  coverage stayed complete (0 opaque bytes), all four faces draw from the
  copy, and every other face keeps its offset, texture and UVs. A 15 px brush
  at each face's UV centre under Panel lock changed 149, 298 and 447 pixels
  of the copies; `_F18.PIC`, `_F16.PIC` and `_A10.PIC` stayed byte-identical.
  Use shape texture put every original record back byte for byte at its site
  with every drawn face (offset, texture, UVs, flags) identical to the
  untouched shape, and its undo restored the assigned bytes. The written LIB
  (5,411 entries) reopened and every edited SH re-parsed.
- **Renders.** `--snapshot OUT.svg LIB SHAPE paint-side 1280x800` before
  (`FA_2.LIB`) and after (`FA2_TEX.LIB`) show the magenta marks only on the
  F-18 and F-16 tail fins and the A-10 fin and nacelle; the rest of each
  livery is unchanged. Renders stayed in `/tmp/claude-1000/face-tex/`
  (not committed).
- **Generated panel sheets.** At each shape's measured density (3.1, 2.6 and
  2.2 texels per unit on the F-18, F-16 and A-10) single flat panels give
  sheets in their own proportions, for example 16 × 11, 19 × 8, 25 × 10,
  38 × 8 and 31 × 25 on the F-18 (41, 28 and 26 distinct sizes). Before the proved-selector fallback 44 of 104,
  55 of 102 and 87 of 143 flat faces were refused for an unresolved material
  state; now all 349 convert. 27, 20 and 52 flat faces have at least one
  coplanar neighbour of the same colour to share a sheet with.
- **Damage family.** No face of the F-18, F-16, A-10, F-22, F-14, MiG-29 or
  Su-27 main shape has a record with the same bytes at the same CODE offset
  in any `_A`..`_D` shape (50 F-18 faces recur in `F18_C.SH` at other
  offsets), so applying an assignment to the family is not offered.
- **Texture widths.** Every one of the 457 distinct PICs that FA_2.LIB shapes
  name is 256 pixels wide; heights run from 11 to 1,037. Generated sheets of
  other widths, like the earlier 64 × 64 sheets, are unverified in the game.

Core tests cover a single face, a contiguous run sharing one detour,
scattered faces, a flat face projected and restored to its exact record,
byte and word UVs with Keep, Scale (widening to words, keeping word UVs) and
Project (fixed planes, square texels), refusals (inner pointer, part-stub
resume target, stored originals, bad names, nothing to remove), the no-op
identity, the reverse round trip, a face in a C4 part under two gear poses,
a second assignment moving faces without nesting and restoring in steps, a
flip of the copy kept by the restore, every face of the synthetic textured
kit, truncated and corrupted shapes, and sheet sizing (4:1, upright, rotated
30°, skewed, a triangle, clamping both ways, a coplanar pair, degenerate
refusal, density). The smoke test drives, through rendered controls, Edit
Mesh face select, the disabled per-face actions and their reason, Clone
texture for selected faces from the Mesh menu (the SH and the new PIC change,
one undo step), a Panel-lock stroke on the assigned face that changes only
the copy, undo to the exact archive bytes, the clone from a face picked in
the Model workspace beside Clone texture for whole shape and its hint, Assign
texture with Keep refused for another size, Scale, Project on the Top plane
and Keep onto a same-size PIC, Use shape texture and both undos, the "No
Hangar texture assignment to remove" refusal and an untextured clone refusal,
and the Assign texture dialog's controls inside the window and apart at
800x600 and 1280x800. The panel smoke checks that a generated sheet's size
and UVs follow the panel instead of 64 × 64.

Formatting, strict Clippy, all tests and the smoke test pass. New state
lives in the boxed Edit Mesh state, so `App` keeps its size; sets in the new
code are filled by insertion, since collecting one pulled in the stable
sort's 4 KiB stack buffer. The PE audit reports 1,456,640 bytes (32-bit) and
1,664,512 bytes (64-bit) with the same 60 reviewed imports. Per-face
textures and the new sheet sizes have not been loaded in the original game
yet; steps are in WINDOWS-TEST.md.

## Unreleased color replacement

`--replace-check FA_2.LIB F18.SH NEW_DIR` on a copy of the user's retail
`FA_2.LIB` (2026-10-07; the original file's SHA-256 was unchanged before and
after, and no game data is in the repository). `_F18.PIC` is a raw 256 × 644
PIC drawn by 178 of the 287 faces of `F18.SH`; colors come from
`PALETTE.PAL`.

- **Whole texture.** Replace color from index 255 (white in `PALETTE.PAL`,
  the most used index, covering the skin) to 160 (black) at tolerance 0:
  126,217 pixels, the same number
  the dialog counted and exactly the pixels of that index; every changed
  byte is one of those raster bytes, header and transparency unchanged.
  Status: "Replaced 126,217 pixels of index 255 with 160 in _F18.PIC;
  original kept as _F18.ORG. One undo step." `_F18.ORG` reads byte for byte
  as the saved `_F18.PIC` and keeps its flag 4 and 14,476 stored bytes.
  Undo restored the exact bytes and removed `_F18.ORG`; redo matched the
  replaced LIB byte for byte; **Restore texture** returned the saved entry's
  storage with no changes left, and its undo matched again.
- **Tail panels.** In Edit Mesh, the six highest side-facing textured faces
  of the rear 30% (the twin fins) with **Selected panels** and tolerance 4
  (three indices match index 255): 2,164 pixels of a 2,966-pixel footprint
  changed, as counted; the other 124,323 matching pixels outside the
  footprint stayed. `_F18.ORG` was byte-identical, undo and redo exact, and
  the written LIB reopened with `_F18.ORG` intact.
- **3D brush and eraser.** A 15 px Replace stroke at a fin face's UV centre
  changed 26 pixels of index 152, only that index; the eraser over the same
  dab returned the saved bytes and dropped the session's `_F18.ORG`.
- **Renders.** `before.png`, `whole.png` and `fin.png` of `_F18.PIC` and
  `--snapshot ... F18.SH paint-side 1280x800` of the original, `WHOLE.LIB`
  and `FIN.LIB` show the whole light skin turned black in the first and
  only the fins in the second. They stayed in `/tmp/claude-1000/replace/` (not
  committed). The existing `--paint-check` and `--restore-check` still pass
  on the same copy.

Core tests cover exact and tolerance matching (tolerance 0 excludes an index
with the same color; the 6-bit inverse is exact for every step), the brush
changing only matching pixels in its circle, whole-image and footprint
regions for `replace_all` and the brush, footprints of a triangle, a
degenerate face, clipping and refused input, span-coded PICs (holes that
read as index 0 untouched, span tables unchanged), glyph strips (index 255
kept transparent and refused as a target), the no-op identity, and bounds and
truncation. The smoke test drives, through rendered controls: Replace in the
Paint tool control and the Model brush row (one segmented control with Brush,
Eraser and Pick, inside the inspector at 800x600 and 1280x800), a stroke
without a source, Alt+click on the atlas and on the model, Pick in Replace,
strokes over mixed pixels changing only matching ones, tolerance through the
NumberField widening the match, `DEMO.ORG` kept and undone, the eraser and
Restore texture after a replace, Esc discarding a stroke, Panel lock clipping
a 3D stroke to the face's footprint, a flat panel refused for another color
and converted for its own, and Replace color in Whole texture, Selected
panels and Faces' textures scopes with the count shown, the typed tolerance
returning to the dialog, Pick from image (also in Edit Mesh, keeping the
selection) and Esc, Cancel, apply and undo to the exact archive bytes, and
the dialog's controls inside the window and apart at both sizes.

Formatting, strict Clippy, all tests and the smoke test pass. The tool and
dialog state is one boxed struct, so `App` grew by its pointer to 2,680
bytes. Alt is read with `GetKeyState`, already imported; the PE audit
reports 1,501,696 bytes (32-bit) and 1,716,224 bytes (64-bit) with the same
60 reviewed imports. Replace has not been tried in the original game or on
Windows 98 yet; steps are in WINDOWS-TEST.md.

## Unreleased panel selection and Remap from view

`--remap-check FA_2.LIB NEW_DIR F18.SH F16.SH A10.SH` on a copy of the
user's retail `FA_2.LIB` (2026-10-07; the original's SHA-256 was the same
before and after, and no game data is in the repository). Colors come from
`PALETTE.PAL`.

- **Stretch census.** Texels per source unit along each textured face's
  vertical axis (up projected onto the face; forward for flat faces) and
  across it, from its largest fan triangle; stretch is their ratio. F-18:
  178 faces measured, 51 stretched 1.5× or more; the worst are lower side
  panels at 0.3 and 0.7 texels per unit vertically against 3.2 across (9.7×
  and 4.8×, CODE 2EB4 and 2E57), a canted rear panel at 8.7×, the nose
  underside at 3.5× and the intake sides (2274, 1EEC) at 7.0 against 3.0
  (2.3×); the fins reach 1.5× (48FB). F-16: 237 faces (12 more map a line or
  point of the PIC), 112 at 1.5× or more, the worst near-horizontal panels
  at 13.9× and 10.0× (2E12, 2C1E: 0.2 and 0.3 texels per unit across), the
  forward side 1EE7 at 3.3× and the fin at 1.6×. A-10: 152 faces, 47 at
  1.5× or more, the worst the top of the rear fuselage at 10.6× (17A9, 0.2
  vertically against 1.6), the nacelle undersides at 4.5× and 4.0×, the
  forward side 3B83 at 2.4× and the fins at 1.2×.
- **Remap from the side, Bake.** Through the panel selection and the Remap
  dialog, from the side the faces face (yaw 270 or 90, pitch 0): F-18 four
  left forward-side faces (2274, 21C3, 2190, 1992, 1.8× to 2.3×) to
  `_F18T1.PIC`, 256 × 33 at 3.1 texels per unit, afterwards 1.0× to 1.3×;
  F-16 three faces (1EE7, 3856, 1F86, up to 3.3×) to `_F16T1.PIC`,
  256 × 53 at 2.6, afterwards 1.0× to 1.2× (a fourth, 213C, was refused as
  edge-on to the side view, as designed); A-10 the forward side 3B83 (2.4×)
  to `_A10T1.PIC`, 256 × 15 at 2.2, afterwards 1.0×.
- **Checks.** Each new PIC passes `is_retail_texture` (kind 0, 256 wide, row
  table, no palette, 3,964 to 13,844 bytes). CODE stayed completely
  explained (0 opaque bytes), the 18, 19 and 12 part bindings and the stubs
  are unchanged, 287, 361 and 299 faces are drawn before and after, the
  remapped faces draw from the new PIC, every other face keeps its CODE
  offset, texture and UVs, and the shapes re-parse. **Use shape texture**
  returned every drawn face to its original texture, UVs and content, and
  its undo restored the remapped bytes. `REMAP.LIB` (5,408 entries) was
  written create-new, reopened, and every SH re-parses.
- **Renders.** Side views at 640 × 400 before and after differ in 38 (F-18),
  240 (F-16) and 12 (A-10) pixels; 38, 216 and 8 of them lie on the
  remapped panels (where the bake resamples the old texture at texel
  centres) and the rest within one pixel of them. `*-panels.png` shows the
  remapped faces selected and
  `*-_F18T1.png` etc. the new PICs. They stayed in
  `/tmp/claude-1000/remap/out1/` (not committed).

Core tests cover a stretched fin baked to square texels from the side (a
vertical gradient of 7 old rows spread over 40 rows, U increasing with
forward), the bake agreeing with the old look at sample points in four
rotated views, a plate seen from the left reading unmirrored (and the right
face refused there as seen from behind), edge-on refusals from the front, the
top and near-grazing angles, two neighbouring faces sharing one continuous
layout and one continuation, flat faces baking their colour and Blank
filling the dominant index, clamping to 256 × 1,280 and refused sizes, a
missing source PIC, Use shape texture after a remap and after remapping
again, the retail layout of every PIC written, and no floating point. The
smoke test drives, through the rendered viewport and controls: click,
Shift+click add and remove, Esc and empty space clearing, the amber-deep
fill with no paint tool and edges only with the brush on, Shift+click with
the brush on selecting without painting while a plain click paints and keeps
the selection, Replace color opening on Selected panels and changing only
their footprint, Remap with Bake (256 wide, faces drawing from it, the look
kept, undo to the exact bytes), painting the remapped panel changing only
the new PIC (`KITT1.ORG` made by that first stroke), Blank, the edge-on
refusal from the front, Edit Mesh taking the selection and its Mesh menu,
the Paint workspace model preview selecting panels, and the dialog's
controls inside the window and apart at 800x600 and 1280x800.

Formatting, strict Clippy (also for both Windows targets), all tests and the
smoke test pass. The selection and dialog live in the boxed Edit Mesh
state, so `App` stays 2,680 bytes. No Win32 API was added; the PE audit
reports 1,561,600 bytes (32-bit) and 1,767,424 bytes (64-bit) with the same
60 reviewed imports. Remapped textures have not been loaded in the original
game yet; steps are in WINDOWS-TEST.md.

## Unreleased FA crash fixes: texture layout and loader limits

2026-10-07. The user's `TopGun.LIB` crashed FA in the F-5's external view
(access violation at 0x4CAF0D, the per-row pointer lookup in polygon texture
setup) and FA crashed at startup writing the resource table (0x478F69) with
Hangar backups in the folder. Evidence is FA.EXE disassembly and the retail
LIBs (read-only); nothing from them is committed.

- **Texture census.** The textures retail shapes draw on textured faces
  (model reader): 1,063 in FA_2.LIB and 7 in SWPATCH.LIB, 1,070 in all; every
  one is kind 0, 256 wide, row table `64 + 256 r`, no palette, span pointer 0
  with capacity `10 × (rows + 1)`, length `64 + 260 × rows`. FA_2.LIB's 1,170
  SH texture-record links name 1,160 PICs; 1,156 have that layout and four
  are drawn only by sprite records (`_MOON.PIC` 41 × 41 for MOON.SH,
  `CATB.PIC`/`CATF.PIC` 640 × 480, `SOLDIER.PIC` 320 × 200). 25 FA_2 shapes do
  not decode in the model reader; the check falls back to their record
  inventory. `validate` reports no texture-layout error on FA_1, FA_2, FA_3,
  FA_4B, FA_4C, FA_4D or SWPATCH (FA_7, FA_10, FA_10B, FA_11 and FA_11B
  exceed the 128 MiB archive limit and were not opened).
- **TopGun.LIB repair** (a copy in `/tmp/claude-1000/crashfix/`; the file in
  Downloads was not written and its SHA-256 is unchanged). `validate`: 14
  errors, `F5EV00.PIC` to `F5EV0D.PIC`, each "64 pixels wide, not 256, no
  row-offset table, embedded 768-byte palette. Drawn by F5EV.SH". Header
  before: kind 0, 64 × 64, row table none, palette 768 B, span size 0. Their
  embedded palette equals FA_2.LIB's `PALETTE.PAL` at all 256 indices (used
  indices 58 and 147). `repair-textures TopGun.LIB TopGun-repaired.LIB
  FA_2.LIB` rewrote all 14: kind 0, 256 × 64, row table at 16,448 (256 B),
  palette 0 B, span size 650, "embedded palette equals the game palette;
  indices kept". All 18 SH entries are byte-identical (F5EV.SH compared
  extracted), `validate` on the result reports 0 errors (the 15 HUD-name
  warnings were there before), and the result is
  `/tmp/claude-1000/crashfix/TopGun-repaired.LIB` (1,087,729 bytes; repaired
  PICs are stored uncompressed). `--texture-repair-check TopGun.LIB F5EV.SH
  FA_2.LIB`: 14 errors before, the Package repair left every SH byte and the
  textured orthographic viewport pixels unchanged, no texture-layout error
  after, one undo restored the archive and redo the repair.
- **Game folder.** A folder of links to the installed retail LIBs and
  FA.EXE: saving `TopGun.LIB` there reports "Game folder: 6 of 20 LIBs, 7,614
  of 9,950 resources" (7,520 retail entries, 93 TopGun entries, FA.EXE).
  With a copy of `TopGun.LIB.bak` beside it the CLI refused the save:
  "TOPGUN.LIB.BAK is 14 characters; FA's LIB name slot holds 13", 7 LIBs and
  7,707 resources.

Core tests cover the reasons `retail_texture_check` gives, conversion
keeping every texel at its UV (and the rendered colors) with the palette
equal, different (nearest color, flagged) or missing (flagged), span holes,
refusals of wider and glyph PICs, every generated panel sheet in the retail
layout, the Package error and repair (SH unchanged, `.ORG` converted, sprite
texture ignored, one undo), backup and stage names without `.LIB`, the
99-backup limit, and the game-folder counts (20 LIBs, 9,950 resources, 13
characters, legacy backups, an existing target and its `.BAK`). The smoke
test, through rendered controls at 800x600 and 1280x800, runs Package checks
on the demo's legacy DEMO.PIC, clicks **Repair textures for FA** (textured
viewport and DEMO.SH unchanged, the button gone, undo exact), checks the
Assign texture warning, and saves into a synthetic game folder (a temp folder
on Linux, scratch names in the current folder on Windows) with FA.EXE and a
legacy `HGFOLD.LIB.bak`: the loader-limit dialog, Enter not confirming,
Cancel writing nothing, Save anyway saving with the warning, and a later save
without the legacy file creating `HGFOLD.BAK` with no dialog.

Formatting, strict Clippy (also for both Windows targets), 165 core tests
and the smoke test pass. `App` is 2,688 bytes. No Win32 API was added; the
PE audit reports 1,592,832 bytes (32-bit) and 1,807,360 bytes (64-bit) with
the same 60 reviewed imports. The repaired LIB has not been loaded in the
original game yet; steps are in WINDOWS-TEST.md.

## Unreleased negative-G cut-out field

2026-10-07. Static evidence only. The FA.EXE routines named in ARCHITECTURE.md
(0x451A60, 0x451E80, 0x452140) were read in disassembly and none was
executed. The cut-out time and the effect of a nonzero `negGLimit` have not
been flown in the original game. The census of retail values used FA_2.LIB.
The user's TOPGUNFX.LIB and TopGun.LIB F5EV.PT both store 0, the same as
retail, so neither has a cut-out.

A core test checks that `negGLimit` and the throttle rates group with
Propulsion and carry their units. The smoke test, at 1280x800 and 800x600,
finds the **Neg-G cut-out** number field in the Model inspector and the
`1/256 s` unit drawn beside it. Formatting, strict Clippy, 166 core tests,
the smoke test and the x86_64 Windows release build with its PE audit (60
reviewed imports) pass.
