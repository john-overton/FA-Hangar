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
6. Close with unsaved edits. Esc should return; clicking Discard changes should close.

The synthetic demo is only an editor test; do not install it in the game.

## Protected sources

1. Open a disposable copy named `FA_2.LIB`; check the Protected label/status.
   Export an entry and start Export object: reading and extraction must work.
2. Edit a field. Ctrl+S should suggest `HANGAR.LIB`. Enter `FA_2.LIB`, then
   `fa_2.lib` in another folder: both must report a protected retail name.
   The document should remain Modified and the original file unchanged.
3. Save as `MYJET.LIB` instead. Edit and save again; check the backup and reopen.
4. Try a read-only custom LIB and an unwritable folder. A failed save must keep
   the original bytes and leave edits available to save elsewhere.

## Real LIB and new-aircraft workflow

Use a copy of your own game installation for the game check.

1. Open FA_1.LIB and confirm its 2,001 entries load. Then open FA_2.LIB,
   filter/select `A10.PT`, and click **Export object**.
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

## Resource relationships and package checks

1. Load Demo and select `DEMO.SH`. Open **References** in the lower dock
   (**Links** at 800x600), then click `DEMO.PIC`. Confirm Paint opens, the dock
   reports `DEMO.SH` as a direct user and `DEMO.PT` as an aircraft user, and
   the **3D preview** tab still shows the originating model.
2. Scroll the Links dock and click `DEMO.PT` under aircraft users. Confirm the
   linked model opens. Navigation must not mark the document modified.
3. Remove `DEMO.PIC`. In Package, confirm a **Removed** row and run package
   checks. `DEMO.SH` should report `DEMO.PIC` as not in this LIB. Undo, rerun,
   and confirm the warning disappears. Test report scrolling at 800x600.
4. On a disposable custom LIB, replace a PIC with malformed bytes. Package
   checks should report a changed-payload error. Undo restores the original.
   An unknown encoding must be marked unverified, not silently passed.
5. Inspect a shared retail shape or texture. Follow its direct and aircraft
   users. Counts cover observed stored names in the current LIB only; resources
   outside this LIB are not evidence that the installed game is missing them.

## Multiple LIBs and structured grafts

1. Open two disposable LIBs. Edit a field in the first, switch roots, edit the
   second, and switch back. Confirm each selection/camera and its own undo
   history survive. Ctrl+Z must affect only the active LIB. Closing the app must
   warn even when only an inactive LIB has edits.
2. Select a shape, Ctrl+C, switch to the other LIB, Ctrl+V. Inspect the dependency
   list. Different same-name resources must require Keep target or Take source.
   Cancel must change nothing; Apply resources must be one undo operation.
3. Repeat by dragging an entry onto the other LIB root. Also drag a definition
   onto a same-type definition in the active LIB to prepare a graft.
4. In Flight, select a group and inspect recognized linked hardpoint/envelope
   rows. Pin a donor, select a differently named compatible target and choose
   Weights or Propulsion in Graft. Verify the before/after values, Apply, then
   undo once. Identity, shape names and unselected groups must remain unchanged.
5. Rename a disposable texture from Entry > Rename resource. Review its shape
   reference updates, apply, reopen and verify them. Undo must restore both name
   and references together. An overlong compiled name or known implicit-family
   rename must fail without modifying the LIB.
6. At 800x600, inspect library roots, field groups, graft review, copy collisions,
   and paginated notes. Try New empty LIB, Ctrl+W, and Save As to another open
   library's path; the latter must be rejected.

## Hardpoint, material and decal acceptance

1. Select DEMO.PT and click Hardpoints. Verify the two steel station markers,
   amber selection, XYZ fields, and no dirty state from a click without movement.
   Drag a station, then undo once. Try G/X/10, H placement, duplicate, remove and
   a default-store assignment. Count and station records must reopen together.
2. In a disposable real aircraft clone, compare station positions and stores
   before/after saving in FA. The editor preserves source coordinates; it does
   not validate the game's full weapon compatibility or launch behavior.
3. Select a textured face, Materials, and transform its UVs. Check the atlas and
   model, then undo; the original footprint must return immediately. Clone the
   texture across the aircraft family and inspect damage/other-LOD references.
4. Edit a saved palette color and undo. Confirm Materials identifies the owning
   resource. A private aircraft-ID PAL is editor preview data; game-global palette
   lookup is unchanged. Test intended colors in the original game.
5. Import a transparent squadron PNG, place it on the atlas, and drag it on a
   model panel. Change width, rotation, mirror and opacity. Nothing should become
   dirty until Apply decal. Cancel/Esc must restore the original preview.
6. Apply the decal, undo once, redo, save and reopen. Check holes stay transparent,
   pixels outside the stamp stay unchanged, and shared/mirrored panels behave as
   the preview showed. Try tail-number text, ink selection, and all four national
   presets. Test the final livery in FA.
7. Restart and reload a squadron PNG from the imported-art library. Forgetting a
   path must not delete its PNG. Test missing paths and a read-only EXE folder.
   Interlaced/animated PNGs and missing target palettes should explain the limit.
8. Check all three livery tabs and the hardpoint inspector at 800x600 and 1280x800.

## Envelope, object export and initial SH tools (0.7)

1. Open an aircraft PT and switch to Flight. Check the G-row arrows, point
   count and Speed/Altitude table. Edit a cell, confirm amber feedback and
   undo to its saved value. Scroll at 800x600 and compare the Raw fields view.
2. Click Base color in the Model inspector. Pick a swatch, apply and undo.
   Only matching flat-color faces should change. Select another plain panel
   and use Panel color to change that face alone. Fully textured models should
   direct you to Materials. Verify loaded game palettes, not grayscale fallback.
3. On a private aircraft copy with its palette loaded, select a supported
   untextured panel and enable Paint model / auto-create texture. Brush on it:
   the new mapping should preview immediately. Esc must discard the sheet and
   stroke; release must add the PIC and SH edit together. One undo must restore
   the original SH and remove the sheet. Save/reopen and inspect the mapping.
   Unsupported material state or CODE limits must explain the refusal.
4. Use Create paintable panel texture, then place a PNG/text decal on the new
   sheet. Check neighboring panels keep their original materials.
5. Select AIM9M.JT and click Export object. Review the private weapon, shape,
   icon and sound filenames. Export/reopen; compare its numeric characteristics
   with the donor. Repeat with a standalone PIC and an opaque resource: opaque
   dependency-discovery limits must appear in review. The source stays open.
6. Select a supported static SH and press Tab. Click a vertex, drag in an
   orthographic view, then undo. Test G with X/Y/Z offsets and A for all vertices.
   Esc or Ctrl+Z during a drag must cancel without committing. Check the selected
   source coordinates and mode labels. Unsupported retail geometry must remain
   inspection-only. Switch LIBs and verify no stale vertex selection survives.
7. Load the generated panel package in original Fighters Anthology. Verify
   visibility, materials, damage variants and camera angles independently of
   editor preview; record original-game acceptance separately.

## Viewport brush, NPC stations and states (0.8)

1. Click the brush icon below Frame before selecting a panel. Verify the palette
   and Panel lock switch appear. Paint across adjacent faces on the same and
   different PICs, and across two supported flat panels. Release once; one undo
   must restore every PIC and the SH, removing any generated sheets. Repeat
   with Esc cancellation. Panel lock must constrain the stroke.
2. Reopen the user's painted A-10. Compare the same wing and letter orientation
   with FA, with the nose pointing the same way. Confirm the editor has not
   altered existing UV/pixel data merely by opening or changing views. Check
   front/side/top, orbiting, skin cutouts and overlapping layers.
3. Use Hardpoints on a ship, tank, AAA and launcher NT. Toggle Loadout/station
   flags to slew fields. Test position, heading/pitch, limits, stores and undo.
4. Open Parts on an aircraft. Set _PLgearDown to 1, select a revealed part,
   change one placement coordinate and undo. Preview-state changes must not dirty
   the LIB. Unsupported programs must report their limits. Angles remain read-only.
5. Test close active LIB and close application with unsaved edits. Cancel must
   preserve all edits; clicking Discard changes must complete the requested
   close without typed confirmation. Other open LIBs retain their own changes.
6. Follow the generated handoff READ-ME.txt and fill in RESULTS.tsv on the actual
   game installation. Test baseline and edited folders separately. Package/parser
   success does not substitute for these original-game results.

## Multiple LIBs and transfer review (0.8.1)

1. Open more than eight LIBs, including while the current LIB has unsaved edits.
   Collapse/expand several roots, expand inactive categories and scroll the
   whole outliner. Check selection, camera and dirty history are retained.
2. Drag an entry onto another LIB. Cancel once, then test Item only and Object
   and linked files with both Copy and Move. Review conflicting names explicitly.
   Copy must retain the source. Move must retain dependencies used by other
   source objects; each changed LIB must undo independently.
3. Check inactive and active workspace tabs look separate from File/Edit menus,
   at 800x600 and a larger window.

## Generated-panel repair (0.8.2)

1. Open an older custom LIB containing automatically generated panel PICs.
   Select its SH and choose Entry > Repair generated panel mappings. Confirm
   the three A10_V2 engine panels retain their painted pixels in the viewport.
2. Undo once and verify the original SH returns; redo and save the repaired
   custom LIB. The repair must not change any PIC or aircraft numeric fields.
3. Load only the repaired copy in FA. Inspect the marked engine panels at the
   same distance/view as the reported missing geometry. Test gear/flaps, nearby
   and distant views and damage states separately.
4. Paint another supported blank panel using the updated editor, save/reopen
   and retest it in FA. Already repaired files must report no repair needed.

## Edit mode and viewport fixes (unreleased)

1. Load the demo, select DEMO.SH and press Tab. Click a vertex without moving:
   it selects and the LIB stays unmodified. Shift+click a second vertex, then
   Shift+click it again to remove it. A selects all; A again clears.
2. Press on a selected vertex slightly off its centre and drag: the vertex
   keeps its offset from the pointer and every selected vertex moves. Press X
   during the drag to lock the X axis; release, then undo once.
3. With two vertices selected, press S, type 200 and Enter: only those two
   vertices scale about their midpoint. Press R, Z, 90, Enter: they rotate
   about their midpoint. Undo restores the exact LIB; the selection remains.
4. Toggle Textured, then press G and type 300 in Object mode: the textured
   preview must move away from the cursor rather than staying centred.
5. Open a File menu and hover a toolbar button outside it: no hover highlight.
   Click the menu's top padding: the menu stays open and nothing else changes.
6. Press 5 for perspective in Edit mode and press a vertex: it selects and
   the status asks for an orthographic view instead of the station message.

## Texture originals, eraser and restore (unreleased)

1. Copy a custom LIB with an aircraft texture (never edit retail files). Paint
   one stroke on its PIC. Confirm `X.ORG` appears under Original textures and
   the Paint inspector shows "Original kept: X.ORG" with its size. Paint again;
   the `.ORG` must not change. Undo both strokes; the `.ORG` must disappear.
2. Paint, then choose **Eraser** and erase the stroke on the atlas and on the 3D
   model. The texture must return to its saved look; each stroke is one undo.
3. Paint again and click **Restore texture**. The status must read "Restored
   X.PIC from X.ORG" and the `.ORG` must disappear. Undo brings both back.
4. Package the LIB with the `.ORG` entries, reopen it and confirm they are
   listed and still restore. Select an `.ORG`: it previews read-only.
5. Load that LIB, containing `.ORG` entries, in the original game. Confirm the
   aircraft list, menus, loadout screens and a flight with the painted aircraft
   are unaffected compared with the same LIB after **Package > Remove stored
   originals**. Report any difference with the LIB name and entry count.

## Edited geometry and part settings (unreleased)

Use copies only. On Linux, run `tore-hangar --geometry-check NEW_DIR FA_2.LIB
F18.SH A10.SH`. Then put the `F18-*.SH` and `A10-*.SH` results you test, renamed
`F18.SH` and `A10.SH`, into a new custom LIB, one variant per LIB.

1. **delete**, **flip**: fly the aircraft and look at the edited panel from
   outside. Deleted: the panel is missing and nothing else is. Flipped: it is
   visible from the other side. Report missing or flickering neighbours.
2. **add**, **duplicate**, **extrude**: the new faces appear with the colour
   of their neighbours, at every view distance where the original face shows.
   No other part of the aircraft distorts, and the game does not crash.
3. **move**: the moved gear-part vertex follows the gear through
   extension and retraction.
4. **scale**: the panel is 10% larger and its neighbours stretch to meet it.
5. Part settings: with the `*-part-*Rotation_axis*` variant, the gear leg
   swings about the other axis. With `*-part-*Shift*`, it travels a different
   angle. With `*-part-*Pivot*`, it hinges one unit to the side. With
   `*-part-*compare*` or `*branch*`, the toggled part shows in the other
   state. Report any crash, a part drawn in the wrong place, or a
   flight-model difference.

## Edit Mesh, Parts and gear encodings (unreleased)

Use a copy of FA_2.LIB and save every result to a new custom LIB, one variant
per LIB.

Editor checks (any Windows box):

1. Select `F18.SH`, press Tab. Press 3: the header face button lights and
   face dots appear. Click a fuselage face: it fills amber-deep with amber
   edges and the overlay reads "1 / … faces". Shift+click it again: cleared.
2. Drag a box over the nose on empty space; Ctrl+drag removes part of it. Press
   B, then drag starting on the aircraft. Point at a gear leg and press L: the
   inspector's Part row reads **Gear left** (or the leg's name).
3. Press Alt+N on one face: the status reports one flipped normal. Ctrl+Z.
   Press X: one face deleted; Ctrl+Z restores the exact LIB (dirty dot clears).
4. Select one face, press E, Z, 4, Enter: one undo step extrudes it. Shift+D,
   X, 6, Enter duplicates it. Press 1, select three vertices, press F.
5. Open **Select** and **Mesh** from the header at 800 x 600 and 1280 x 800:
   both open inside the window and list their keys.
6. Choose **Parts**. Click **Gear down**, then **Gear up**: the legs swing and
   the document stays unmodified. Drag the gearPos field: the legs follow.

In-game checks:

1. **Edited mesh.** In Edit Mesh on `F18.SH`, select one fuselage face and
   extrude it 4 units outwards (E, then the axis and 4, Enter). Save. In the
   game, the bump appears at every view distance where the original panel
   shows, with its neighbours' colour. Nothing else distorts and the game
   does not crash. Report flicker or missing panels nearby.
2. **Part setting.** In **Parts**, select **Hook (state 1)** on `F18.SH` and set
   its gate value to 0. Save. In the game the hook mesh should now show with
   the hook up and hide when it is lowered. Report if it never shows.
3. **Non-retail direction flip.** Select **Gear left** on `F18.SH` and set
   **Direction** to **No NEG** (badge **Not seen in retail**). Save. In the
   game, lower and raise the gear: the left main leg should fold the opposite
   way to the right leg, as the preview shows, and the game must not crash.
   Also try **Swing range** sar 2 (45°) on a variant. Report the leg's
   behaviour, any crash, and the exact variant. Until this passes, these
   encodings stay marked **Not seen in retail**.
4. `--geometry-check` writes `*-form-*.SH` files for every non-retail gear
   form it reaches; any of them can replace the aircraft's SH in a test LIB
   for the same check.

## Please report

Windows version and architecture, CPU/SSE2 or VM setup, whether the window
opened, which checklist step failed, and the exact status/error text. A screenshot
of a drawing/layout problem will help. Original-game results should be recorded
separately from successful editor packaging.
