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
5. Ctrl+S, save as `MYMOD.LIB`, then edit and save again to the same path.
   Confirm the new edits reopen and `MYMOD.LIB.bak` contains the previous
   version. A third save should keep `.bak` and add `.bak.1`.
6. Close with unsaved edits. Esc should return; typing DISCARD should close.

The synthetic demo is only an editor test; do not install it in the game.

## Protected sources

1. Open a disposable copy named `FA_2.LIB`; check the Protected label/status.
   Export an entry and start New aircraft: reading and extraction must work.
2. Edit a field. Ctrl+S should suggest `HANGAR.LIB`. Enter `FA_2.LIB`, then
   `fa_2.lib` in another folder: both must report a protected retail name.
   The document should remain Modified and the original file unchanged.
3. Save as `MYJET.LIB` instead. Edit and save again; check the backup and reopen.
4. Try a read-only custom LIB and an unwritable folder. A failed save must keep
   the original bytes and leave edits available to save elsewhere.

## Real LIB and new-aircraft workflow

Use a copy of your own game installation for the game check.

1. Open FA_1.LIB and confirm its 2,001 entries load. Then open FA_2.LIB,
   filter/select `A10.PT`, and click **New aircraft**.
2. The first field must request a **new aircraft ID**, not a file path. Use
   `A10V1`, then a display name such as `My A-10`.
3. Review the copied resources and new filenames. The tested retail A-10
   produces 44 entries with FA_1/FA_2 available. Verify the main/damage/shadow
   shapes, private textures, HUD/cockpit, weapons, sounds and palette are listed.
4. Try Back and Cancel. The original LIB must remain unchanged. Rebuild the
   review and click **Export new LIB**. The suggested name is `A10V1.LIB`.
5. Save to a new path and reopen. Confirm `A10V1.PT` is selected and its linked
   model/textures load. Numeric characteristics should remain donor values.
6. Repeat with `F18.PT`; the tested source produces 58 private resources.
7. If files are missing, use **Add source LIB**. If the ID already exists, the
   wizard should reject it with a naming error, not a Windows file-open error.
8. In the copied game installation, place the new LIB beside FA.EXE and launch
   with that folder as the working directory. Check the new aircraft's selection,
   cockpit, stores, flight, model LODs, damage and shadow. Verify stock aircraft
   still use their original resources. Remove the new LIB to uninstall the test.

The default workflow no longer requires exporting a loose SH first. For that
separate use case, select **Lib > From loose SH file**. The new clone rewrites
stored resource references but still relies on the game's procedures and any
runtime-generated lookups not encoded as resolvable resource names. Review
reported unresolved HUD names and test them in the actual game.

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
