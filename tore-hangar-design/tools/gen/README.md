Hangar is the TORE dev tool for Fighters Anthology LIB files: open libs, browse entries, edit entry fields, view and adjust 3D shapes, graft aspects between entries, and repackage. It borrows Blender's layout and navigation and swaps Blender's scene concepts for lib entries.

## Principles

- **Blender muscle memory, lib-entry nouns.** Outliner, viewport, properties, timeline, workspaces, G/R/S, Tab, MMB orbit all behave the way a Blender user expects. What gets selected is a lib entry (`F14.PT`, `F14.SH`), not a scene object.
- **Every edit is visible against the file on disk.** A changed value turns `amber`; a changed entry and its lib get the `amber` dirty dot. Nothing is written until Save or Package.
- **Draws on a GDI.** Hangar targets Windows 98/ME as well as current systems. Every surface is a solid fill, every edge a 1px line. No alpha, no gradients, no blurred shadows, no animation beyond the 3D view and timeline playback.
- **Dense, not cramped.** 20px rows, 22px buttons, 12px labels. Spacing comes from the 4px scale only.

## Content

- Voice is terse and technical. Labels are nouns ("Stall speed", "Afterburner"), actions are verbs in sentence case ("Apply graft", "Package lib"). Never "OK" or "Submit".
- Lib, entry, and file names are written exactly as stored, in mono caps: `USNF97.LIB`, `F14.PT`, `MIG29.PT`. Type extensions are always shown as a TypeBadge, never spelled out in prose ("PT", not "plane type file").
- Units always follow values, in `ink-muted`: kt, ft, lb, lbf, °/s, M (Mach). Thousands use commas.
- Messages say what happened and what it affects: "F14.SH is referenced by 3 aircraft. Edits apply to all of them." No exclamation marks, no emoji.

## Color

One theme, Gunmetal. The surface ladder runs dark to light: `gm-1000` keylines, `gm-950` viewport and fields, `gm-900` window, `gm-800` editors and panels, `gm-700` raised controls, `gm-600` hover.

- Text is `ink` on any gm surface, `ink-muted` for labels and secondary text. `ink-faint` is for disabled text and placeholders only.
- `amber` means selected, active, or changed. Nothing decorative is amber. Selection fills are the solid `amber-deep`, never a translucent amber.
- `steel` means time or reference: the playhead, hardpoints, the graft ghost, focus. It never means selected.
- `axis-x`, `axis-y`, `axis-z` are reserved for axes: gizmo, grid axis lines, and vector field labels.
- `ok` and `danger` always travel with an icon or a word.
- Focus ring: 1px solid `focus`, 1px offset, on every focusable control.

## Type

- Interface text is Barlow Semi Condensed (`font-ui`), which falls back to Tahoma and MS Sans Serif on Windows 98. Use `label` for nearly everything, `title` for dialog titles, `section` for uppercase sub-heads, `hint` for the status bar.
- Every number, file name, offset, and count is JetBrains Mono (`font-mono`, falls back to Lucida Console): `value` in fields, `value-sm` in overlays and diffs, `badge` in badges and keycaps.
- `display` (Barlow Condensed) appears only on the splash and about box.
- For the native build, ship bitmap rasterizations of `font-ui` at 12 and 13px and `font-mono` at 10, 11, and 12px rather than relying on system fonts.

## Shape and depth

- `radius-sm` (3px) on buttons, fields, panels, and rows; `radius-xs` (2px) on badges and keycaps; `radius-md` (4px) only on the floating viewport tool strip. Never fully rounded pills.
- Raised controls (buttons, selects) get the `lip`: a single 1px top highlight line. Sunken controls (fields) get a `line-strong` border on `gm-950`. Pressed controls lose the lip and drop to `gm-950`.
- Editors are separated by 1px `gm-1000` seams, not margins.

## Layout

- Workspaces are fixed arrangements of the same editors: Browse, Model, Flight, Graft, Package. The Model workspace (see the Workspace screen) is outliner left 280px, viewport center, animation dock below at 186px, properties right 322px.
- Every editor has a 28px header (`th-editor-head`) holding its menus and toggles. The status bar (22px) and menu bar (26px) frame the window.
- Property rows put the label right-aligned in a 40% column and the control in the rest.
- Minimum window 800 × 600.

## Interaction

- Viewport: MMB orbit, Shift+MMB pan, wheel zoom, numpad 1/3/7 views, numpad 5 ortho toggle, Home frame all, period frame selected.
- Transform: G move, R rotate, S scale, then X/Y/Z to lock an axis, type a number for exact values, Enter to confirm, Esc or RMB to cancel.
- Tab toggles Object and Edit mode. H places a hardpoint at the cursor; Alt+H toggles hardpoint display.
- NumberField: drag to scrub, Shift for fine, Ctrl to snap, double-click to type, Backspace resets to the on-disk value.
- Ctrl+Z / Ctrl+Shift+Z undo and redo across every editor; a graft is a single undo step.
- Drag an entry between libs to copy it; drop it on a same-type entry to open Graft with that pair.

## Iconography

Hangar's own 16px line set in `assets/Icons`: 1.5px stroke, round caps and joins, 16px grid, drawn with currentColor in the app. Idle icons are `ink-muted`, active ones `amber`. Every entry type has one icon: aircraft (PT), shape (SH), image (PIC), weapon (JT), object (OT), palette (PAL), mission (M), sound (11K), lib (.LIB). Icon-only buttons always carry a `title`.
