# First Windows test

Use `fa-hangar-<version>-win64.zip` on a modern 64-bit Windows box. Use
`fa-hangar-<version>-win98-me-pentium4.zip` on Windows 98/ME with an
SSE2-capable CPU. Extract to a writable folder with an ASCII path and launch
`fa-hangar-<version>.exe`.
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
   Confirm the new edits reopen and `MYMOD.BAK` contains the previous
   version. A third save should keep `MYMOD.BAK` and add `MYMOD.B01`; no
   file name beside it may contain `.LIB` except `MYMOD.LIB` itself.
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
4. Drag an entry slowly over another LIB's rows, a same-type definition, the
   same LIB and the viewport, and outside the window: the chip follows the
   pointer, targets highlight, and the status bar names the release action.
   Release on another LIB opens the review with Copy; Esc and RMB cancel.
5. Right-click an entry and a LIB root. Check the menu opens at the pointer,
   stays inside the window near the edges, Copy to and Move to submenus list
   the other LIBs and open the review with Copy or Move, and a click outside
   closes it. RMB in the viewport still cancels a G/R/S transform.

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

## Edit mode and viewport fixes (0.9.0)

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

## Texture originals, eraser and restore (0.9.0)

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

## Edited geometry and part settings (0.9.0)

Use copies only. On Linux, run `fa-hangar --geometry-check NEW_DIR FA_2.LIB
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

## Edit Mesh, Parts and gear encodings (0.9.0)

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

## App icon and version information (0.9.0)

1. In Explorer, view the folder holding `fa-hangar.exe` as Large Icons and
   as Small Icons (on current Windows also Extra large): the gold-outlined
   F.A. / HANGAR plate shows at large sizes and the FH monogram at small
   sizes, with clear corners and no black box.
2. Launch it: the title bar and taskbar button show the icon. On Windows
   98/ME also check Alt+Tab and a 256-colour display mode.
3. Right-click the EXE, **Properties**: the Version tab (98/ME) or Details tab
   shows F.A. Hangar, the release version and the GPL-3.0-only notice.

## Unresolved in source exports (0.9.0)

1. Open FA_2.LIB, select `F14.PT` and click **Export object**. The review has
   no Unresolved in source section; export it as `F14X` and fly it: the
   damaged (C) model and every livery texture must look as on the stock F-14.
2. Select `MIG31.PT`, click **Export object**: the review lists `Y141.HUD`
   (BRF string) and **Export new LIB** is disabled. Tick **Export with 1
   unresolved reference, as in the source LIB**, export, and fly the new
   aircraft: the cockpit should behave as the stock MiG-31's.
3. On a synthetic or modded aircraft with a missing texture, choose **Use
   texture…** and pick its own skin; after export the face that used the
   missing name shows that skin in game.

## Names, reference ID rename and duplicate aircraft (0.9.0)

Open `FA_2.LIB`, make the edits below and save them under a custom name
(for example `IDENT.LIB`). In a copied game installation, replace `FA_2.LIB`
with that file, renaming it there (Hangar never saves under a retail name;
keeping both would leave two copies of every other entry). On Linux,
`fa-hangar --identity-check FA_2.LIB F14.PT F14Z F18.PT F18Z IDENT.LIB`
makes the same renames with the default names instead (`F-14` / `F- 14D
Tomcat` kept, the duplicate listed as `F/A- 18D Hornet variant`).

1. Select `F14.PT`. In **Identity**, set the short name to `F-14Z` and the
   long name to `F-14Z Test`; both show amber with the saved value. Click
   **Rename…**, enter `F14Z`, check the review (14 renamed, the shared
   weapons, sounds and `F14CC.HUD` left alone, the mission warning) and click
   **Rename**.
2. Select `F18.PT`, click **Duplicate aircraft…**, enter `F18Z`, `F/A-18Z`
   and `F/A-18Z Test`, keep the defaults and click **Duplicate aircraft**.
   `F18Z.PT` is selected.
3. Save, install as above, start Fighters Anthology and open the aircraft
   list (single mission or quick mission): **F-14Z Test** and **F/A-18Z Test** appear
   with their new names, and the stock F/A-18 is still listed. Fly each: the
   F-14Z shows the F-14 model, skins, damage states and cockpit; the F/A-18Z
   behaves like the stock F/A-18.
4. Load a stock mission that uses the F-14 (one of the 69 `FA_2.LIB`
   missions that name `F14.PT`) and record what the game does without
   `F14.PT`: an error, a substitute aircraft or a missing flight.

## Per-panel textures and sized panel sheets (0.9.0)

Use a copy of a LIB holding the aircraft (save under a new custom name).

1. Select `F18.SH`, press **Tab**, then **3**. Click one tail fin face
   (Shift+click to add its neighbours). The **Face textures** panel lists
   `_F18.PIC`. Choose **Mesh > Clone texture for selected faces**, accept
   `_F18T1.PIC` and check that only the selected faces are listed on it.
2. Leave Edit Mesh, turn on **Paint on the model** and **Lock strokes to the
   panel**, and paint a bright mark on the fin. Check that the other fin,
   other aircraft using `_F18.PIC` and the rest of the F-18 are unchanged.
   Ctrl+Z twice returns the LIB to unchanged; repeat steps 1 and 2.
3. Select another face and use **Mesh > Assign texture…** with a PIC of a
   different size: **Keep** is off with its reason; apply **Scale**. Select
   it again and apply **Use shape texture**; it returns to `_F18.PIC`.
4. On an untextured rectangular panel (for example a flat underside panel of
   a ground object or a long fuselage strip), paint with the brush. The new
   sheet's size in the status follows the panel's proportions, and the brush
   dabs stay round on the model.
5. Save, install the LIB, start Fighters Anthology and fly the F/A-18 (or
   view it in the hangar/arming screen). Check the painted fin shows the
   mark, the other fin and the rest of the livery are stock, the fin is
   drawn in the right order, damage states still appear, and no panel is
   missing. Record whether a generated sheet narrower than 256
   pixels displays (every retail shape texture is 256 wide).

## Replace a color (0.9.0)

Use a copy of a LIB holding the aircraft (save under a new custom name).

1. Select `_F18.PIC`. In Paint choose **Replace**, hold **Alt** and click a
   skin color on the atlas: the **Replace** row shows that index and the
   paint color. On Windows 98/ME check that Alt+click picks rather than
   painting and does not open the window menu.
2. Pick a bright palette color and stroke across a panel line: only the
   pixels of that index change, the line stays. Raise **Tolerance** and
   stroke again: similar shades change too. Ctrl+Z undoes each stroke.
3. Select `F18.SH`, Tab, 3, click the tail fin faces and choose **Mesh >
   Replace color…**. The dialog opens on **Selected panels** with a pixel
   count; **Replace pixels**, then check in Textured view that only the fins
   changed and `_F18.ORG` appears under Original textures.
4. **Eraser** over part of the fin and **Restore texture** bring the skin
   color back; Ctrl+Z and Ctrl+Shift+Z step through both.
5. Save, install the LIB and view the F/A-18 in Fighters Anthology: the
   replaced color shows only where it was applied, and the `.ORG` entry does
   not disturb loading.

## Panel selection and Remap from view (0.9.0)

Use a copy of a LIB holding the aircraft (save under a new custom name).

1. Select `F18.SH` in the Model workspace, Textured. Click a panel, then
   Shift+click two neighbours: the overlay reads "3 panels selected" and
   they fill amber-deep. Shift+click one again to drop it; Esc clears. Turn
   on the brush: Shift+click still selects without painting, a plain click
   paints. On Windows 98/ME check Shift+click is not taken by the system.
2. Press **3** (side view), select the left or right intake side panels and
   choose **Remap selected panels from view…** in the Face textures panel.
   The dialog suggests `_F18T1.PIC` and shows a size 256 pixels wide. Choose
   **Bake current look** and **Remap**: the panels look the same, and
   Paint shows `_F18T1.PIC` with the panels as seen from the side. Paint a
   straight line across them: it stays straight and even on the model.
3. Press **1** (front view) with panels on the side selected and try again:
   the status reads "turn the view to face the panel" and nothing changes.
4. Save, install the LIB and view the F/A-18 in Fighters Anthology, in the
   external view and in flight: the remapped panels show the bake and the
   paint, nothing else changes, and the game does not crash when the
   aircraft is drawn (the new PIC has the retail texture layout). Repeat
   with **Blank**.

## FA crash fixes: textures and the game folder (0.9.0)

1. Copy `TopGun-repaired.LIB` (made by `repair-textures` from the user's
   `TopGun.LIB`) into a test copy of the game folder in place of
   `TopGun.LIB`, and move every `*.LIB.bak*` file out of that folder.
   Launch FA: it must start. Fly the F-5 and switch to the external view:
   it must not crash, and the generated panels show their colors as before.
2. Open `TopGun.LIB` in Hangar, run Package checks: 14 ERROR rows "Would
   crash FA's texture mapper". Click **Repair textures for FA**: the rows go,
   the model looks the same, Ctrl+Z brings them back.
3. Create a new generated panel (paint an untextured panel) on a copy of an
   aircraft, save, and view it in FA's external view.
4. Save a custom LIB into the game folder twice: `MYMOD.BAK`, then
   `MYMOD.B01` appear; the status ends with "Game folder: … of 20 LIBs, … of
   9,950 resources". Put a copy named `MYMOD.LIB.bak` there and save again:
   the dialog names the 14-character LIB name; Cancel writes nothing, Save
   anyway saves and the status says to move it out. Remove it afterwards.

## Negative-G engine cut-out (0.9.0)

1. Open a copy of a custom LIB with an aircraft PT and select it in Model.
   The Propulsion panel shows **Neg-G cut-out** with `1/256 s`. In Flight,
   the Propulsion group lists `plane.negGLimit` and the throttle rates show
   `%/s`.
2. Set **Neg-G cut-out** to 1280 (5 s), save, and fly that aircraft in FA at
   military power. Push over into steady negative G: after about 5 s the
   throttle falls to 0 and the engine spools down. Ease to 0 G or above: the
   throttle returns to the lever setting with no restart. Set it back to 0
   and repeat: the engine keeps running however long the push lasts.

## Display palettes and palette companions (0.9.0)

1. Put a copy of a two-aircraft mod LIB without `PALETTE.PAL` (for example
   `TOPGUNFX.LIB` with F14.PT and F5EV.PT) in a folder with copies of
   `FA_2.LIB` and `FA_1.LIB`. Open it: both aircraft's textures show in
   color, and the Paint palette panel and Details read **PALETTE.PAL from
   FA_2.LIB**.
2. Close Hangar, move the mod LIB to a folder without retail LIBs and open
   it: the palette reads **PALETTE.PAL from FA_2.LIB, remembered**. Delete
   `fa-hangar-palette.txt` beside the executable and open it again: the
   panel shows grayscale with **No game palette found; colors are
   approximate.**; **Load palette…** with `FA_2.LIB` restores color.
3. Copy F5EV.PT to a new LIB with **Copy to**: the review has an `F5EV.PAL`
   row with **Copy** selected; apply and save. The new LIB has `F5EV.PAL`
   and no `PALETTE.PAL`, and shows in color on its own.
4. Load the new LIB in FA next to the retail LIBs: every aircraft keeps the
   game's colors (FA does not read `F5EV.PAL`).
5. Repeat 3 into a LIB that has `PALETTE.PAL`: no palette row; the notes
   say why. Duplicate an aircraft in a LIB without a palette: the review has
   a `<ID>.PAL` row; **Skip** leaves it out.

## F-5 tail fin: proofs through loops (0.9.0)

Use a copy of `TOPGUNFX.LIB` (or a custom LIB holding `F5EV.SH`) with the
game palette loaded.

1. Select `F5EV.SH`, Textured, press **3** and orbit half a turn so the
   fin's left side faces you. Click the three fin panels (Shift+click), then
   orbit back and add the three on the right. **Clone texture for selected
   faces** opens with no refusal; Apply: "6 selected faces now draw from
   `_F5EVT1.PIC`". Undo.
2. Select the three left fin panels from the left side and **Remap selected
   panels from view…**: no refusal notice, **Remap** on. Remap with **Bake
   current look**: the fin looks the same. Paint one dot on it with the
   smallest brush: a dot, not a line the height of the fin. Repeat for the
   right side.
3. Select a face of the right side while looking at the left and open Remap:
   the status reads "faces away from the view" and nothing changes. At
   800x600 in paint mode the viewport header reads **Texture Paint** whole.
4. Save, install, and view the F-5 in FA's external view and in flight: the
   fin shows the bake and the dot, nothing else changes, and the game does
   not crash.

## Runtime markings (0.9.0)

Use a copy of `FA_2.LIB` (or a custom LIB holding `F5EV.SH`) and the game
palette. FA must also load the retail `FA_1.LIB`, which holds the roundels.

1. Select `F5EV.SH` in Model. The **Runtime markings** panel lists Wing
   marking left (slot 3) and Wing marking right (slot 4), one face each.
   Press **7**: the left-wing roundel is outlined in steel; orbit underneath
   for the right one.
2. Clear **Shown** on both rows: the outlines turn dim and dashed. Save as a
   custom LIB, put it in the game folder and fly the F-5 (any nation): both
   roundels are gone, nothing else changes, and the game does not crash.
3. Reopen the LIB in Hangar: both rows read hidden. Check **Shown** on both
   and save: the entry matches the retail `F5EV.SH` byte for byte (compare
   with `extract`).
4. Set slot 4's **Slot** to 2 (nose art) and fly: the roundel under the
   right wing shows the player's nose art for the player's aircraft and
   nothing (`BLANK.PIC`) for other F-5s. Set it back to 4.
5. **Make paintable** on slot 4, paint the panel of `F5EVM4.PIC`, save and
   fly as two different nations: the painted image shows under the right
   wing for both, and the left wing keeps each nation's roundel. Note
   whether the unpainted panel colour (index 0, black, on the F-5) or index
   255 shows through.
6. **Restore runtime marking**, save and fly: the nation's roundel is back;
   `F5EVM4.PIC` and `F5EVM4.ORG` are gone from the LIB.
7. On `SU27.SH`, check **Apply to damage family** and clear **Shown** on
   Tail art left (slot 0): the status names `SU27_A.SH` and `SU27_C.SH` as
   changed and `SU27_B.SH` as having no slot 0. Fly the Su-27 as the player
   and get it damaged: no left tail art on the intact or damaged shapes.

## Transform gizmo, magnetic snap and X-ray (0.9.0)

Use a copy of a LIB with an editable aircraft shape (or the synthetic demo).

1. Edit Mesh (Tab), vertex select, Solid shading. Hover a vertex: its
   square handle gains an outline. Click two handles a few pixels apart:
   each click selects the one under the pointer.
2. Select a panel's corners. Drag the red arrow: only X changes, and the
   status reads **Move X … · Y 0 · Z 0 (source units)**. Ctrl+Z restores it.
   Drag a square and the white centre circle; drag with Shift held (fine)
   and with Ctrl held (steps of 10).
3. Hold **Alt** while dragging a vertex towards another: no snap, and no
   window menu opens when Alt is released. Without Alt, release within a
   few pixels of another vertex in the drag plane: **Snapped to vertex at
   (x, y, z)** and the two coincide.
4. Right-click in the viewport: the menu opens; choose **Rotate**, drag a
   ring with Ctrl: the angle steps by 15°. Right-click during a drag cancels.
5. Press **Alt+Z**: X-ray turns on (header button lit), hidden vertices
   become pickable; Alt+Z again turns it off and no window menu opens.
6. Repeat at 800x600: the X-ray and magnet toggles are in the shading menu.

## Vertex handles, Add vertex, split and connect (0.9.0)

Use a copy of a LIB with an editable aircraft shape (the F-5's F5EV.SH, or
the synthetic demo).

1. Edit Mesh, vertex select, Solid shading. Zoom in to about 250% and 800%:
   every handle sits on a panel corner, none floats in empty space, and
   corners behind the aircraft show no handle. Alt+Z shows them all.
2. **Mesh > Add vertex**. Hover a wing panel: it is outlined and a ring
   follows the pointer on it. Near an edge's middle the ring doubles and
   reads **Edge midpoint**; hold Alt and it moves freely. Click: one vertex
   is added and selected; Ctrl+Z removes it.
3. Turn Add vertex on again and Shift+click inside a flat or textured panel:
   the status reads **Split 1 face into N triangles**; the panel looks the
   same, its texture unbroken. Ctrl+Z restores it. Esc and right-click turn
   the tool off.
4. Select two opposite corners of a four-corner panel and press **J**: the
   status reads **Connected 2 vertices**. Two neighbouring corners give
   **Already connected by an edge of face …**.
5. Save the split and connected shape to a new custom LIB and fly it in the
   game: the panels draw where they did, with their textures, and nothing
   flickers or vanishes. Report any difference.

## Please report

Windows version and architecture, CPU/SSE2 or VM setup, whether the window
opened, which checklist step failed, and the exact status/error text. A screenshot
of a drawing/layout problem will help. Original-game results should be recorded
separately from successful editor packaging.
