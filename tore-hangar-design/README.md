# TORE Hangar: GUI design system

Visual spec for Hangar, the TORE dev tool for reading, editing, grafting, and repackaging Fighters Anthology LIB files. Gunmetal theme, Blender-style layout and controls, flat enough to draw with GDI on Windows 98/ME.

Start with `screens/workspace.png`, then `BRAND.md`.

## Layout

```
BRAND.md                 Brand book: principles, color roles, type, shape, layout, interaction, keymap
tokens/
  tokens.json            Source of truth: colors, type styles, spacing, radii, lip
  tokens.css             CSS custom properties + type style classes, generated from tokens.json
  theme.rs               Rust constants (colors as 0xRRGGBB with colorref() for GDI, spacing,
                         radii, control metrics, type styles). Compiles standalone, no deps.
css/hangar.css           Reference component styles (th-* classes) used by the HTML previews
components/<Name>/
  README.md              Behavior, states, token usage, what the caller provides
  preview.html           Standalone HTML reference render (open in a browser)
icons/*.svg              16px line icon set, 1.5px stroke, inked #aab3bb
screens/*.png            Rendered previews of every component plus the full Model workspace (2x)
tools/gen/               Python generator that produced the previews and tokens (reference only)
```

## Components

Button, NumberField, Select, Checkbox, Panel, PropertyTabs, Outliner, TypeBadge, Viewport, Timeline, GraftPanel, Notice, StatusBar, MenuBar, and the full Workspace screen (Model workspace at 1280 × 800). Cover is the design system's title card.

## Using it in the Rust app

- Drop `tokens/theme.rs` into the GUI crate as `theme.rs` and draw from `theme::color`, `theme::metric`, and `theme::text`. Nothing in the spec needs alpha, gradients, or blurred shadows.
- The HTML previews are the visual reference, not runtime code. Match pixel metrics from `theme::metric` and the component READMEs.
- Fonts: Barlow Semi Condensed (UI) and JetBrains Mono (data), both SIL OFL. For Win9x, pre-rasterize them to bitmap fonts at the sizes in `theme::text`; fallbacks are Tahoma/MS Sans Serif and Lucida Console.
- The flight values shown in mockups (Mach 2.34, 20,900 lbf, etc.) are placeholders, not decoded PT data.

## Live version

The editable design system lives in the "TORE Hangar" Design System artifact on claude.ai. If you change tokens there, regenerate `theme.rs` and `tokens.css` from the new `tokens.json`.
