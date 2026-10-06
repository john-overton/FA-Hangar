# First Windows test

Use `tore-hangar-win64.zip` on a modern 64-bit Windows box. Use
`tore-hangar-win98-me-pentium4.zip` on Windows 98/ME with an SSE2-capable CPU.
Extract to a writable folder with an ASCII path and launch `tore-hangar.exe`.
The EXE is portable; the other files are documentation and licensing.

## Quick application check

1. Choose **File > Load synthetic demo** (or click the Hangar logo). Confirm a wireframe and nine synthetic entries appear.
2. Orbit with middle mouse, pan with Shift+middle mouse, zoom with the wheel.
   Try 1/3/7 views and Home to frame.
3. With DEMO.SH selected, press G, X, type 10, Enter. Confirm Modified.
   Ctrl+Z should restore Saved. Try R/Z/30 and S/X/120, then undo.
4. Select DEMO.PT, click Flight, edit a numeric field. Ctrl+A clears the
   value before typing. Verify amber highlighting, saved values and undo/redo.
5. Ctrl+S, enter a **new full output path**, and press Enter. Reopen the LIB
   with Ctrl+O and confirm the changes. Reusing an existing output path should
   display an error without changing that file.
6. Close with unsaved edits. Esc should return; typing DISCARD should close.

The synthetic demo is only an editor test; do not install it in the game.

## Real LIB and new-aircraft workflow

Use a copy of your own game installation for the game check.

1. Open FA_1.LIB first and confirm its 2,001 entries load without the old
   ASCII-name error. Then open FA_2.LIB and search for a donor PT, for example F18.PT. Search for
   F18.SH separately to inspect the main model, then export it to a new SH file.
2. Select F18.PT and click **New aircraft**. Supply that exported SH, ID
   `TEST18`, and display name `Hangar F18 Test`; type CREATE at the review.
3. Confirm TEST18.PT, TEST18.SH, TEST18_A/B/C/D/S.SH and the observed texture
   are present. The PT fields should retain the donor values while names and
   main/shadow references use TEST18.
4. Package to TEST18.LIB, reopen it, and inspect TEST18.SH and TEST18.PT.
5. For the actual game check, place only this new LIB beside FA.EXE in the
   copied installation, with that folder as the working directory. Launch FA,
   find the new aircraft, and check selection, appearance, controls, stores,
   damage and shadow. Remove TEST18.LIB to remove the test variant.

This first test intentionally reuses the donor's main model so that identity
and packaging can be tested before introducing a different model. A later
import can substitute another compatible SH. Hardpoints, flight properties,
damaged shapes and shadow remain donor-derived until deliberately changed.
Part grafting is not available yet. General retail shapes are read-only in
the geometry editor, but can be imported intact into a new aircraft package.

## File browser, audio and painting

1. Ctrl+O: navigate a folder, try Up and Drives, select a LIB and Open.
   Restart and verify it appears under Recent LIBs. A missing recent file
   should show an error and retain the current document.
2. Select DEMO.5K or a retail 5K/11K sound. Test Play, Stop and switching entries.
   Export WAV and play it in a separate audio player.
3. Select DEMO.PIC or a retail texture. Export PNG, inspect its dimensions/colors,
   then paint a stroke with the indexed brush. Confirm one undo restores it.
4. Select a textured SH, click a visible panel, then UV / paint its PIC. Confirm
   the amber UV outline and live lower model preview. Zoom and pan the atlas.
5. Paint on the atlas and watch the model; return to Model and enable Paint
   selected panel on model. Test brush strokes, cancel with Esc, and undo/redo.
6. Clone a texture to a new name and verify the original PIC stays unchanged.
   One undo should restore the shape references and remove the clone together.
7. Package to a new LIB, reopen and check the painted pixels. Test the livery
   in the original game, including other LODs, damage and shared textures.

Painting requires an embedded full palette or a loaded PALETTE.PAL. The synthetic
PIC includes its palette; retail textures often rely on the LIB's base palette.
Clone/paint scope is the decoded static pose, so check the remaining model states
separately. No retail data is included in the test package.

## Please report

Windows version and architecture, CPU/SSE2 or VM setup, whether the window
opened, which checklist step failed, and the exact status/error text. A screenshot
of a drawing/layout problem will help. Original-game results should be recorded
separately from successful editor packaging.
