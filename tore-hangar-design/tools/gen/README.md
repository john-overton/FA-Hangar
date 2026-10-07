# Generators

`tokens/tokens.json` is the source of truth for the design tokens. Generated
files are committed; regenerate them after any token change and commit the
JSON together with the output.

## Theme (`theme.py`)

```sh
python3 tore-hangar-design/tools/gen/theme.py
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

## Previews (`build.py`, `render.py`, `icons.py`)

Reference only. `build.py OUT` writes the HTML component previews into
`OUT/project`, copying `tokens.json` rather than defining tokens itself;
`render.py` screenshots them with Playwright into `screens/`. `icons.py` holds
the icon path data the previews and `icons/*.svg` were produced from.
