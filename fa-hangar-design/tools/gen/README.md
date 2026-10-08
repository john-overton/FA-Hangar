# Generators

`tokens/tokens.json` is the source of truth for the design tokens. Generated
files are committed; regenerate them after any token change and commit the
JSON together with the output.

## Theme (`theme.py`)

```sh
python3 fa-hangar-design/tools/gen/theme.py
```

Reads `tokens/tokens.json` and writes:

- `tokens/theme.rs`: the Rust constants the app compiles via `#[path]`
  (`color`, `space`, `radius`, `metric`, `text`). Colors are `Rgb(0xRRGGBB)`;
  the `lip` shadow becomes `color::LIP`. Every number is an integer: the
  native Windows 98 build has no floating point, so ratios are percentages
  (`metric::PROP_LABEL_PCT`) and letter spacing is `tracking` in 1/100 em.
- `tokens/tokens.css`: CSS custom properties and the type style classes used by
  the HTML previews.

To add a token, add it to the matching section of `tokens.json` (`color`,
`spacing`, `radius`, `shadow`, `metric`, `type`), rerun the generator and use
the new constant from `theme::`. Metric values must be integer px. The output
is deterministic, so `git diff --exit-code` after a run checks that the
committed files are current.

## Icons and text advances (`glyphs.py`)

```sh
python3 fa-hangar-design/tools/gen/glyphs.py
```

Writes `crates/hangar-app/src/ui_glyphs.rs`, plain const data the app compiles
in. Needs `rsvg-convert`, ImageMagick `magick` and the Liberation Sans TTFs.

- **Icons.** Every `icons/*.svg` becomes a `Glyph` variant (file stem in
  CamelCase). Each SVG is rasterized by `rsvg-convert` at 16 x 16 and its
  alpha (coverage) is thresholded into two run lists of `(row, x, len)`:
  *full* pixels (coverage >= 166/255, about 65%) and *half* pixels (>= 77/255,
  about 30%). The app draws full runs in the icon color and half runs in the
  icon color mixed halfway into the background (`Rgb::mix`), so the 1.5px
  strokes keep their weight while every pixel stays a solid GDI fill. Nothing
  parses SVG at runtime. Each icon is also rasterized at 12 x 12 for the small
  `th-ic-sm` size (select, twisty, checkbox and status icons): `Glyph::mask(true)`.
  To add an icon, draw it on the 16px grid with a 1.5px
  round stroke (see `icons/README.md`), save it as `icons/<name>.svg`, add the
  path to `icons.py` for the previews, and rerun.
- **Text advances.** Liberation Sans Regular and Bold advance widths for ASCII
  32..126 in 1/64 em (`UI_ADVANCE`, `UI_ADVANCE_BOLD`). Liberation Sans is
  metric-compatible with Arial, which is close to Tahoma; `ui::char_width`
  uses them (bold +8% for Tahoma Bold) to estimate label widths for truncation
  and layout. Mono styles use Lucida Console's fixed 0.6 em cell.

## App icon (`app_icon.py`)

```sh
python3 fa-hangar-design/tools/gen/app_icon.py [--review DIR]
```

Renders `icons/app/fa-hangar.svg` at 256 with `rsvg-convert` (read back
through `magick`), builds 16, 24, 32 and 48 from the hand-tuned pixel art in
the script, and writes `icons/app/fa-hangar-<size>.png` and
`icons/app/fa-hangar.ico` (8-bit and 32-bit DIB entries with AND masks for
the small sizes, PNG for 256). Colors come from `tokens.json`; the script
refuses a master that uses a color outside the tokens. `--review DIR` writes
1x and 8x contact sheets on dark, teal and white grounds. The build reads the
committed `.ico`, never the script. See `icons/app/README.md` for the
per-size design.

## Previews (`build.py`, `render.py`, `icons.py`)

Reference only. `build.py OUT` writes the HTML component previews into
`OUT/project`, copying `tokens.json` rather than defining tokens itself;
`render.py` screenshots them with Playwright into `screens/`. `icons.py` holds
the icon path data the previews and `icons/*.svg` were produced from.
