# App icon

The TORE Hangar application icon: a gold (`amber`) outline on a chamfered
`gm-900` plate, with TORE in `ink` over HANGAR in `amber`, after the patch
logo's lockup. It is the one place amber is used as decoration; inside the
app amber still means selected, active or changed.

| File | Size | Content |
| --- | --- | --- |
| `tore-hangar.svg` | 256 master | TORE over HANGAR, vector geometry (no font needed) |
| `tore-hangar-256.png` | 256 | `rsvg-convert` render of the master |
| `tore-hangar-48.png` | 48 | Pixel art: TORE (3px strokes) over a 5 x 8 HANGAR, 2px outline |
| `tore-hangar-32.png` | 32 | Pixel art: TORE (2px strokes) over a 3 x 5 HANGAR, 1px outline |
| `tore-hangar-24.png` | 24 | Pixel art: TH monogram, 2px strokes, 1px outline |
| `tore-hangar-16.png` | 16 | Pixel art: TH monogram, 1px strokes, 1px outline |
| `tore-hangar.ico` | all | 16/24/32/48 as 8-bit (256-colour) and 32-bit BGRA DIBs with AND masks, 256 as PNG |

The letters are bold condensed square capitals with chamfered corners, drawn
as paths, so no font has to be installed to render the master. Below 64px the
master's strokes fall between pixels, so 16 to 48 are hand-tuned pixel art in
`tools/gen/app_icon.py` rather than downscales: every edge is a solid pixel and
the corner chamfers are single-pixel diagonals. At 16 and 24 the full name is
illegible, so those sizes use the TH monogram (T in `ink`, H in `amber`). 32 is
the smallest size where TORE/HANGAR still reads at 1x.

The 8-bit entries share one palette of the four plate, outline and text colors
(index 0 is black for the transparent corners), so Windows 98/ME shows the
same pixels as the 32-bit entries. Windows XP and earlier ignore the PNG entry.

Regenerate after editing the master or the pixel art, and commit the outputs:

```sh
python3 tore-hangar-design/tools/gen/app_icon.py --review /tmp/app-icon
```

The app embeds `tore-hangar.ico` at build time (`crates/hangar-app/build.rs`
writes the Windows icon resources; the Linux backend reads the 32-bit entries
for `_NET_WM_ICON`). The build never runs Python.
