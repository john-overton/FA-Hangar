# Changelog

User-visible changes by version, newest first. The workspace version lives in
the root [`Cargo.toml`](../Cargo.toml). What was verified for each version is
recorded in [VALIDATION.md](VALIDATION.md); manual acceptance steps are in
[WINDOWS-TEST.md](WINDOWS-TEST.md).

## Unreleased

- **Vertex handles sit on their corners at every zoom.** In Edit Mesh the
  handles (and picking, box select, the gizmo pivot, snap targets and the
  other overlay markers) turned whole source units through the camera
  before zooming, so on a small shape such as the F-5 at 250% they sat on a
  regular screen lattice, some in empty space, while the shaded view drew
  the corners elsewhere. The raster, the wireframe and every overlay now
  share one integer projection with 1/256-unit precision; a handle is
  within a pixel of its drawn corner at 100%, 250% and 800%.
- **Only shown vertices get handles.** In Solid and Textured shading a
  vertex shows when a face using it is drawn and nothing nearer hides it;
  hidden vertices and those outside the view draw nothing (they were dim
  dots). X-ray still shows every corner of a drawn face. Box select, picking
  and **L** follow what is shown; **L** in vertex select also works on the
  face under the pointer.
- **Add vertex is a click tool.** Hover a panel to preview the point under
  the pointer, snapped with the magnet to the panel's corners, edge
  midpoints or centre (shown as Corner, Edge midpoint or Face centre; Alt
  places freely); click to add the vertex there, selected and ready for F or
  G. It replaces Add vertex at median.
- **Split face at point and Split edge at midpoint.** Shift+click with Add
  vertex (or the Split face tool) replaces the panel by a fan of triangles
  around the new vertex, keeping its colour and texture with interpolated
  UVs; a point on an edge also splits the panels sharing it. Split edge at
  midpoint does this for two selected vertices.
- **Connect vertices (J).** Two selected corners of a panel that are not
  neighbours cut it along that diagonal into two faces, as Blender's J;
  from the Mesh menu, the inspector, the right-click menu or J. Adjacent
  corners, corners with no shared panel and concave cuts are refused with
  the reason.
- A refused Edit Mesh operation now shows its reason directly under the
  Mesh panel. `--edit-check` also adds vertices, splits faces and connects
  vertices; snapshot states take an `@ZOOM` suffix and `vertex-add`,
  `vertex-placed`, `vertex-split`, `vertex-split-edge` and `vertex-connect`
  states show the new tools.

- **CI runs on demand or when a release is published, and names builds by
  version.** The Windows workflow no longer runs on every push and pull
  request. It reads the version from `Cargo.toml`, requires a release tag of
  `v<version>`, and produces `tore-hangar-<version>-win98-me-pentium4` and
  `tore-hangar-<version>-win64` packages holding
  `tore-hangar-<version>.exe` and a `SHA256.txt` in `sha256sum` format. On a
  release it also attaches both ZIPs to the release. See
  [RELEASING.md](RELEASING.md).
- **Help > About T.O.R.E Hangar.** A window with the app icon, the version,
  the build target, the licence and the project address; the version also
  shows at the right of the status bar.

- **Make paintable fills the sheet with the colour the marking sits on.**
  The new sheet was filled with the marking face's stored colour, which on
  the F-5 is index 0, so a painted roundel sat in a black square. A new
  **Fill** row (Runtime markings panel) now defaults to **From the surface
  below**: Hangar finds the drawn face under the marking (the wing panel),
  reads its flat colour or the texel at the marking's centre through its
  UVs, and fills the panel with that index, falling back to the shape's skin
  index and then the stored colour. **Panel colour** keeps the old fill and
  **Pick…** opens a palette grid. A swatch and an **Index** line show the
  result before you click. The fill is solid because index 255 transparency
  is not verified in the game
  ([Runtime markings](MANUAL.md#runtime-markings) limits).
  `--markings-check` counts where each slot's fill came from and, with a
  shape, renders the old fill beside the new.

- **Runtime markings** (Model inspector, Edit Mesh and Paint): a new panel
  lists the markings the game fills in at run time, such as national
  roundels and tail art, one row per slot (`E0` record). Clear **Shown**
  to hide a marking, choose another **Slot** (0 to 4), **Select faces** to
  move them with G, R and S in Edit Mesh, or **Make paintable** to draw
  them from a new PIC of your own, which **Restore runtime marking**
  reverses. **Apply to damage family** repeats an action on the `_A` to
  `_D` shapes with the same slot. While the panel is open the viewport
  outlines markings in steel, and hidden ones dashed. Hidden and paintable
  markings are recognised after save and reopen, and showing or restoring
  them gives back the shape's original bytes (146 of the 150 FA_2.LIB
  shapes with markings). Package counts them. `--decal-census` lists which
  aircraft use which slots and `--markings-check` exercises every action on
  real LIBs. The manual's
  [Hide, move and paint runtime markings](MANUAL.md#hide-move-and-paint-runtime-markings)
  and [Runtime markings](MANUAL.md#runtime-markings) limits explain who
  decides what: the shape decides whether, where and which slot; the game
  picks the image (roundels by nation, tail and nose art from the player's
  pilot record, read from FA.EXE and not yet confirmed in the game).

- **Transform gizmo in Edit Mesh.** With vertices or faces selected, drag
  X/Y/Z arrows, plane squares or the centre circle to move, rings to rotate
  (a sweep and the angle show; Ctrl turns in 15° steps) and axis squares or
  the centre to scale (Ctrl in 10% steps). Shift is fine, Ctrl steps moves by
  10 source units, Esc or right-click cancels, and typing a number continues
  as the exact **G**/**R**/**S** entry on that axis. A dim outline of the
  original faces and a readout follow the drag; the release is one undo step
  through the same verified writers. Pick the mode in the viewport tool
  strip, the header (where it fits) or a new right-click menu (**Move**,
  **Rotate**, **Scale**, **Snap to vertices**, **Pivot**, **Cancel**). The
  gizmo dims and says why when the selection is in a posed part or the region
  writer refuses a vertex. See
  [Transform gizmo](MANUAL.md#transform-gizmo).
- **Magnetic snap.** Moves, including plain vertex drags, snap the moving
  vertex nearest the pointer onto a vertex within 8 px, judged in 3D within
  the drag's line or plane, so a vertex that only looks close on screen is
  never a target. A ring and guide mark the target and the status names it.
  Hold Alt to move freely; the magnet button in the header turns it off. See
  [Magnetic snap](MANUAL.md#magnetic-snap).
- **Vertex handles and X-ray.** Vertex select draws keylined 7 px handles
  (selected amber, the active vertex larger, crowded ones smaller), outlines
  the one under the pointer, picks the nearest within 9 px (the nearer in
  depth on overlap), and draws one handle per stored point and position, for
  corners of drawn faces only. In Solid and Textured shading vertices behind
  faces are dim and cannot be picked or boxed until **X-ray** (Alt+Z, or the
  header button) is on.

- **Neg-G cut-out** (Model inspector, Propulsion panel) shows the PT field
  `negGLimit`, which is now also in Flight's Propulsion field group. It
  is the time in 1/256 s of continuous negative G before FA cuts the throttle
  to 0; 0 means never. Graft now carries it with Propulsion instead of
  Handling. `throttleAcc` and `throttleDacc` show their unit, %/s. The
  manual's [Negative-G engine cut-out](MANUAL.md#negative-g-engine-cut-out)
  explains how to set it and its limits.

- **Colors without PALETTE.PAL in the LIB.** One resolver chooses the
  display palette: Load palette, `PALETTE.PAL` in the LIB, the owning
  object's `<ID>.PAL` (any number of aircraft per LIB; shapes and textures
  find their aircraft through its references), the only PAL in the LIB,
  `PALETTE.PAL` in another open LIB or in `FA_2.LIB`/`FA_1.LIB` beside it,
  and the last game palette, remembered beside the executable. The Paint
  palette panel and Details name the source; grayscale shows a warning
  with **Load palette…**. Package checks report aircraft without a palette
  and a custom `PALETTE.PAL` that would recolor the game.
- **Copies bring their palette.** Copy to, Move to, drag and drop, paste,
  Ctrl+D on an object and **Duplicate aircraft** add the object's
  `<ID>.PAL` (or the palette Hangar showed it with, under that name) as a
  review row with **Copy** or **Skip**. **Export object** carries it too
  when the source LIB has no palette of its own. Never `PALETTE.PAL`, which
  FA would apply to every aircraft. `--palette-check` shows the resolution
  and a Copy to on a real LIB.
- **Fix: Clone, Assign and Remap refused panels with "its control flow
  loops"**, for example the F-5's tail fin. Hangar now proves which texture
  draws a face (and which vertex each corner shows) by following every path
  through the shape at once, loops and nested calls included, instead of
  walking back from the face. Across the retail FA_2.LIB, 134,546 of 137,959
  drawn faces are now provable (was 116,558); every answer the old proof gave
  is unchanged. The F-5's six tail-fin faces clone, and each side of the fin
  remaps from the side view with square texels, so one painted pixel stays
  one dot instead of a line along the fin.
- **Readable refusals.** A face that still cannot take a new texture says why
  in plain terms, for example "different textures reach it on different
  paths: KIT.PIC (E2 at CODE+E) and SAME.PIC (E2 at CODE+A5)", without
  repeated phrases. **Clone texture for selected faces**, **Assign
  texture…** and **Remap selected panels from view…** check the selection
  when their dialog opens: a refused selection shows the reason in full and
  the primary button is off. Long errors in these dialogs and in other
  prompts wrap instead of being cut off with "…".
- **Fix: the viewport mode menu read "Texture P…"**; it now shows **Texture
  Paint** whole at every window size.
- **Command line.** `--proof-census INPUT.LIB NEW_OUTPUT.txt [BASELINE.txt]`
  lists the texture and slot proof of every drawn face and compares it with
  an earlier list. `--remap-check` takes `--faces HEX,...`, `--clone
  HEX,...` and `--view YAW,PITCH` after an SH name (the SH may be named again
  for its other side) and `--palette PALETTE.PAL|LIB`, clones and remaps
  exactly those faces, and reports how far one painted texel reaches on
  screen before and after.

- **Fix: FA crashed drawing Hangar's generated panel textures** (for example
  the external view of a painted F-5). Generated panel sheets were raw PICs
  with an embedded palette and no row table, which FA's texture mapper reads
  through. They are now retail SH textures: 256 wide, as tall as the panel,
  with the panel at the left edge, a row table and no palette.
- **Package checks flag textures FA cannot map** as errors ("Would crash FA's
  texture mapper") with the reasons and the shapes that draw them. **Repair
  textures for FA** (Package) and `repair-textures INPUT.LIB NEW_OUTPUT.LIB
  [PALETTE]` rewrite them in the retail layout without changing any SH: each
  pixel keeps its UV, the palette is dropped (colors are mapped only when it
  differs from the game palette, and the status says so), and stored
  originals follow. One undo step. **Assign texture…** warns when the PIC it
  assigns is not an FA texture.
- **Fix: FA crashed at startup with Hangar backups in its folder.** FA loads
  every file whose name contains `.LIB` as a LIB, including `X.LIB.bak`.
  Backups are now `<STEM>.BAK`, then `<STEM>.B01` to `.B99`, and the save
  stage `<STEM>.TMP`; no companion name contains `.LIB`.
- **Game folder checks on save.** Saving into a folder with `FA.EXE` or a
  retail LIB counts what FA would load: at most 20 LIBs, 9,950 resources in
  all and 13-character LIB names. The status shows the totals, old
  `X.LIB.bak` backups get a warning to move them, and a save past a limit
  asks for **Save anyway** (the CLI refuses it with the numbers).

- **Select panels outside Edit Mesh.** In the Model viewport and the Paint
  workspace's model preview, click picks a panel and Shift+click adds or
  removes one; Esc or a click on empty space clears. With the brush,
  eraser or Replace on, a plain click still paints and Shift+click selects
  without painting. Selected panels show amber edges (and an amber-deep
  fill with no paint tool on) and the count shows in the overlay and the
  Face textures panel. It is the same selection as Edit Mesh's faces and
  feeds Clone texture, Assign texture, Use shape texture and Replace
  color's Selected panels.
- **Remap selected panels from view…** (Face textures panel, Paint panel,
  Mesh menu) gives stretched panels a new texture laid out as the viewport
  shows them, with square texels at the shape's density: **Bake current
  look** copies what the panels show now, **Blank** fills their most common
  colour. The new PIC has the retail SH texture layout (256 wide, up to
  1,280 rows, row table, no palette); the SH and the PIC are one undo step
  and Use shape texture reverses it. Panels seen edge-on are refused with
  "Turn the view to face the panel".
- **`--remap-check`** lists each shape's most stretched textured faces and
  remaps the worst side faces from the side view on a user LIB, with
  before/after renders.

- **Replace a color.** The Paint tool control adds **Replace** beside Brush
  and Eraser, and so does the Model inspector's 3D brush row. Alt+click (or
  Pick) the color to replace; strokes then paint the current color over
  matching pixels only. **Tolerance** (0 to 64 palette steps) widens the
  match to similar colors, and Panel lock keeps a stroke inside the panel's
  UV footprint. **Replace color…** (Paint panel, **Mesh** menu and **Face
  textures** panel) replaces a color in the whole texture, inside the
  selected panels' UV footprints, or in every texture the selected faces
  draw from, with From and To pickers and a live pixel count. Each stroke or
  apply is one undo step, keeps `X.ORG` on a first edit, and changes raster
  bytes only; the eraser and Restore texture bring replaced pixels back.
- **`--replace-check`** runs Replace color and the 3D Replace brush on a
  shape's main PIC and checks the bytes, stored original, undo and Restore
  texture; snapshot workspaces `replace`, `replace-model` and
  `replace-dialog`.

- **Per-panel textures.** Selected faces can draw from their own PIC while
  the rest of the aircraft keeps its atlas. **Clone texture for selected
  faces** (Edit Mesh **Mesh** menu and inspector, and the inspector for a
  face picked in the Model workspace) copies the faces' PIC to a free private
  8.3 name with its stored original and moves only those faces to it, in one
  undo step. **Assign texture…** picks any PIC in the LIB from a filterable
  list with Keep, Scale or Project UVs (Keep is off, with the reason, when
  the sizes differ); untextured faces can be projected. **Use shape
  texture** puts the faces' original records back. Painting those faces
  changes only their PIC.
- **Clone texture for whole shape.** The former **Clone texture for this
  shape** is renamed and sits beside the per-face clone with a hint saying
  which faces each one affects.
- **Generated panel sheets fit the panel.** Painting an untextured panel no
  longer makes a 64 × 64 square that skewed rectangular panels: the sheet
  takes the panel's proportions and orientation at the shape's own texel
  density (8 to 256 pixels a side), and with Panel lock off coplanar
  neighbours of the same color share one sheet. Panels after native code
  (most flat faces of retail aircraft, for example 87 of 143 on the A-10),
  which were refused for an unresolved material state, now convert. Sheets
  made by earlier versions are unchanged.
- **`--face-texture-check`** clones, paints and restores tail-face textures
  of named shapes through the app and saves the result create-new; snapshot
  workspaces `paint-side` and `assign-texture`.

- **Identity panel.** PT, NT, JT and OT definitions open with an Identity
  panel: the short and long names edit in place (one undo step each, amber
  with the saved value and a reset) and the reference ID shows beside
  **Rename…**; aircraft add **Duplicate aircraft…**.
- **Rename reference ID.** Renames an aircraft and the files private to it
  (shapes, damage family, skins, cockpit art, HUD, unshared sensors and
  stores with their icons, stored originals) in place, and rewrites every
  recognized reference in the LIB, including the aircraft's own name. The
  review lists renamed, rewritten and shared resources, refuses collisions,
  names beyond 8.3 and names too long for their stored slot, and warns that
  missions and other LIBs still name the old ID. One undo step.
- **Duplicate aircraft.** Copies an aircraft inside the same LIB under a new
  ID and names. A review offers Copy or Share per resource: private shapes,
  skins and HUD are copied by default; weapons, sounds, sensors and anything
  already shared keep their names. One undo step; the new PT is selected.
  On a PT the context menu's Duplicate opens it, and the Entry menu lists
  both new actions.
- **Short and long names in the export.** Export object asks for the short
  and the long name separately instead of writing one title into both.
  `export-object` keeps its TITLE argument and adds `--short NAME` and
  `--long NAME`.
- **App icon.** A gold outline on a gunmetal plate with TORE over HANGAR
  ([design](../tore-hangar-design/icons/app/README.md)). Windows builds embed
  it at 16, 24, 32 and 48 px in 8-bit and 32-bit color plus a 256 px PNG, so
  Explorer, the title bar and the taskbar show it on Windows 98/ME and current
  Windows. The EXE also carries version information (product name, version,
  GPL notice) for its Properties dialog. On Linux the X11 window sets
  `_NET_WM_ICON`.
- **Copy is the default transfer.** Dropping an entry on another LIB opens the
  review with Copy selected; Move is one click away.
- **Drag feedback.** A dragged outliner entry shows a chip with its icon, name
  and the release action. Copy targets in other LIBs are filled amber-deep
  with an amber outline, same-type graft targets are outlined in steel, and
  invalid spots give a short reason. The status bar says what releasing will
  do; Esc or RMB cancels.
- **Outliner context menu.** Right-click an entry for Copy to and Move to
  submenus listing the other open LIBs, Copy, Paste, Duplicate, Rename,
  Export entry, Export object and Delete; right-click a LIB root for Paste,
  Collapse, Package LIB and Close LIB. Unavailable items are dimmed.
- **Fewer phantom texture names.** SH texture names count only when they are
  E2 texture records in the shape's record inventory. Bytes in face or vertex
  data that looked like `B.PIC` (in `F14_C.SH`), `!.PIC` (in the MiG-29
  shapes) and similar no longer appear as references; they blocked exporting
  32 of the 145 aircraft in `FA_2.LIB`. The References dock labels each link
  with its evidence.
- **Unresolved in source.** A name that no searched LIB provides no longer
  stops **Export object** outright. The review lists each one with its
  resource and kind (and *not drawn by any pose* for unreached texture
  records). Keep it as in the source, byte for byte, or retarget a texture
  reference to a PIC in the package or a source LIB. Export stays disabled
  until **Export with N unresolved references, as in the source LIB** is
  ticked. Missing damage-family members and HUD names take the same path, so
  `MIG31.PT` (`Y141.HUD`) and `~BGUN.PT` (`EJECT_A.SH` to `EJECT_D.SH`) export
  with them kept.
- **CLI export flags and large source LIBs.** `export-object` and
  `clone-aircraft` accept `--keep-unresolved` and repeatable
  `--substitute OLD.PIC=NEW.PIC`, and refuse with the list of names
  otherwise. Their source LIBs are read directory-first with bounded range
  reads, as in the GUI, so `FA_7.LIB`, `FA_10.LIB`, `FA_10B.LIB`, `FA_11.LIB`
  and `FA_11B.LIB` (140 to 186 MiB) work as sources instead of failing with
  "File exceeds 128 MiB limit".

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
