# Panel

Collapsible property groups, stacked inside the Properties editor exactly like Blender sub-panels.

- Surface `gm-800` with a `gm-1000` keyline; 24px header in `label` weight 600 with a chevron.
- Body rows use `th-prop`: label right-aligned in a 40% column (`ink-muted`), control in the rest. Keep labels to three words.
- `th-subhead` (style `section`) splits a long panel without nesting another panel.
- Header tools (reset, copy, paste values) are ghost icon buttons at the right.
- Panels remember collapsed state per tab. Ctrl+click a header collapses every other panel.

The consumer provides title, rows, optional header tools, and the collapsed state.
