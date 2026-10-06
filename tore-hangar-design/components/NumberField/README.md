# NumberField

The Blender-style scrub field: drag horizontally to change the value, click to type, arrows step by the field's increment.

- Label sits left in `label` / `ink-muted`; the value sits right in `value` (mono) / `ink`, unit in `ink-muted`.
- Fill is `gm-950` with a `line-strong` border; hover darkens to `gm-1000` and reveals the ‹ › step arrows.
- **Changed** (`is-changed`): the value turns `amber` when it differs from the value on disk in the source lib. This is how a user sees unsaved edits at a glance.
- **Bounded** (`th-num-fill`): a `steel-deep` bar shows position within a known range (fuel load, roll rate). Omit for unbounded values.
- **Locked** (`is-locked`): dashed border, `ink-muted` value, lock icon. Use for fields derived from another entry (engine count follows the shape).
- **Vector** (`th-vec`): X, Y, Z stacked and joined, labels in `axis-x`, `axis-y`, `axis-z`.

Interaction: drag = coarse, Shift+drag = fine (×0.1), Ctrl+drag = snap to increment, double-click = type, Esc = revert, Backspace = reset to file value. The consumer provides label, value, unit, min/max (optional), increment, and the on-disk value for the changed state.
