# Viewport

The 3D view of the selected shape. Blender navigation, Blender colors for axes, amber for selection.

- Background `gm-950`; minor grid `gm-800`, every fifth line `gm-700`; X and Y axis lines in `axis-x` and `axis-y`.
- Mesh: outer edges `ink-muted`, inner edges `gm-500`; the selected object outline is `amber`, its origin `amber-bright`.
- Hardpoints are `steel` diamonds with `HP<n>` labels in `value-sm`.
- Overlay text (view name, entry, LOD, counts) in `value-sm` / `ink-muted`, top-left; navigation gizmo top-right.
- Tool strip floats top-left: Select, Move, Rotate, Scale | Measure, Place hardpoint. In Edit Mesh, Select hides the transform gizmo and Move, Rotate and Scale pick it.
- Transform gizmo (Edit Mesh): arrows, plane squares and rings in `axis-x`/`axis-y`/`axis-z` (2px strokes from 1px lines, one more px when hovered or dragged), the centre circle in `ink`; a disabled gizmo mixes toward `gm-950`. Snap target: `amber-bright` ring and dashed guide. Vertex handles: `ink` squares on a `gm-1000` keyline, selected `amber` with an `amber-bright` ring.
- Header: mode select, editor menus, LOD select, overlay toggles, shading segmented control (wireframe, solid, textured).

Controls: MMB orbit, Shift+MMB pan, wheel zoom, numpad 1/3/7 front/right/top, numpad 5 ortho, Home frame all, . frame selected, G/R/S with X/Y/Z axis lock, Tab edit mode, H place hardpoint, Alt+H toggle hardpoints.
