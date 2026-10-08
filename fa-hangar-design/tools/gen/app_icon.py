#!/usr/bin/env python3
"""Generate the F.A. Hangar application icon.

    python3 fa-hangar-design/tools/gen/app_icon.py [--review DIR]

Reads `icons/app/fa-hangar.svg` (the 256px master) and the hand-tuned pixel
art below, and writes into `icons/app/`:

- `fa-hangar-{16,24,32,48,256}.png`
- `fa-hangar.ico`: 16, 24, 32 and 48 as uncompressed DIB entries, each in an
  8-bit 256-colour palette version (Windows 98/ME) and a 32-bit BGRA version,
  both with AND masks, plus 256 as PNG for current Windows.

The app's build script embeds the committed `.ico` as Windows resources, so
the build never runs this. Needs `rsvg-convert` and ImageMagick `magick`.
`--review DIR` also writes 1x and zoomed contact sheets on light and dark
grounds. Output is deterministic: rerun and `git diff --exit-code` to check
the committed files are current.
"""
import argparse
import json
import os
import re
import struct
import subprocess
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
DESIGN = os.path.normpath(os.path.join(HERE, "..", ".."))
APP = os.path.join(DESIGN, "icons", "app")
MASTER = os.path.join(APP, "fa-hangar.svg")

TOKENS = {t["name"]: t["value"] for t in json.load(open(os.path.join(DESIGN, "tokens", "tokens.json")))["color"]["tokens"]}


def rgb(name):
    v = TOKENS[name]
    return (int(v[1:3], 16), int(v[3:5], 16), int(v[5:7], 16))


PLATE, GOLD, INK = rgb("gm-900"), rgb("amber"), rgb("ink")

# Small sizes are pixel art, not downscaled vectors: the 256 master's strokes
# land between pixels below 64px. Each size is a chamfered plate whose edge is
# the gold outline (`gold` px thick, eroded 4-connected so the 45-degree
# corners are single-pixel diagonals), with glyph bitmaps placed on it.
#   16: "FH" monogram (F ink, H amber), 1px strokes like the outline.
#   24: "FH" monogram, 2px strokes.
#   32: F.A. (2px strokes) over HANGAR in a 3x5 face (N is 4 wide): the
#       smallest size where the full name still reads at 1x.
#   48: F.A. (3px strokes, chamfered A) over a 5x8 HANGAR.
# Glyph rows use '#' for ink and '.' for plate; lowercase keys are the
# HANGAR line's smaller face.
GLYPHS = {
    16: {
        "F": ["#####", "#....", "#....", "#....", "####.", "#....", "#....", "#....", "#....", "#...."],
        "H": ["#...#", "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#", "#...#", "#...#"],
    },
    24: {
        "F": ["#######"] * 2 + ["##....."] * 3 + ["######."] * 2 + ["##....."] * 6,
        "H": ["##...##", "##...##", "##...##", "##...##", "##...##", "#######", "#######", "##...##",
              "##...##", "##...##", "##...##", "##...##", "##...##"],
    },
    32: {
        "F": ["#####", "#####", "##...", "##...", "####.", "##...", "##...", "##...", "##..."],
        "A": [".###.", "#####", "##.##", "##.##", "#####", "##.##", "##.##", "##.##", "##.##"],
        ".": [".."] * 7 + ["##"] * 2,
        "h": ["#.#", "#.#", "###", "#.#", "#.#"],
        "a": ["###", "#.#", "###", "#.#", "#.#"],
        "n": ["#..#", "##.#", "#.##", "#..#", "#..#"],
        "g": ["###", "#..", "#.#", "#.#", "###"],
        "r": ["##.", "#.#", "##.", "#.#", "#.#"],
    },
    48: {
        "F": ["#######"] * 3 + ["###...."] * 4 + ["######."] * 3 + ["###...."] * 6,
        "A": ["..####..", ".######.", "########"] + ["###..###"] * 4 + ["########"] * 3 + ["###..###"] * 6,
        ".": ["..."] * 13 + ["###"] * 3,
        "h": ["#...#", "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        "a": [".###.", "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
        "n": ["#...#", "##..#", "##..#", "#.#.#", "#.#.#", "#..##", "#..##", "#...#"],
        "g": [".###.", "#...#", "#....", "#....", "#.###", "#...#", "#...#", ".###."],
        "r": ["####.", "#...#", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
    },
}

# Per size: (gold outline px, corner chamfer px, [(text, x, y, gap, color)]).
LAYOUT = {
    16: (1, 2, [("FH", 2, 3, 2, None)]),
    24: (1, 3, [("FH", 4, 5, 2, None)]),
    32: (1, 4, [("F.A.", 8, 8, 1, INK), ("hangar", 4, 20, 1, GOLD)]),
    48: (2, 6, [("F.A.", 11, 9, 2, INK), ("hangar", 5, 30, 2, GOLD)]),
}
MONOGRAM = {"F": INK, "H": GOLD}


def plate(n, gold, chamfer):
    """RGBA rows of the chamfered plate: gold edge, gm-900 inside, clear corners."""
    inside = [[x + y >= chamfer and (n - 1 - x) + y >= chamfer and x + (n - 1 - y) >= chamfer
               and (n - 1 - x) + (n - 1 - y) >= chamfer for x in range(n)] for y in range(n)]
    core = inside
    for _ in range(gold):
        core = [[core[y][x] and 0 < x < n - 1 and 0 < y < n - 1 and core[y - 1][x] and core[y + 1][x]
                 and core[y][x - 1] and core[y][x + 1] for x in range(n)] for y in range(n)]
    return [[(PLATE + (255,)) if core[y][x] else (GOLD + (255,)) if inside[y][x] else (0, 0, 0, 0)
             for x in range(n)] for y in range(n)]


def pixel_art(n):
    gold, chamfer, lines = LAYOUT[n]
    img = plate(n, gold, chamfer)
    for text, x, y, gap, color in lines:
        for ch in text:
            rows = GLYPHS[n][ch]
            ink = color or MONOGRAM[ch]
            for dy, row in enumerate(rows):
                for dx, c in enumerate(row):
                    if c == "#":
                        assert img[y + dy][x + dx] == PLATE + (255,), f"{n}px glyph {ch} leaves the plate"
                        img[y + dy][x + dx] = ink + (255,)
            x += len(rows[0]) + gap
    return img


def check_master():
    used = {c.lower() for c in re.findall(r'fill="(#[0-9a-fA-F]{6})"', open(MASTER).read())}
    known = {v.lower() for v in TOKENS.values()}
    assert used <= known, f"master uses colors outside tokens.json: {sorted(used - known)}"


def render(svg, n):
    png = subprocess.run(["rsvg-convert", "-w", str(n), "-h", str(n), svg], check=True, capture_output=True).stdout
    raw = subprocess.run(["magick", "png:-", "-depth", "8", "rgba:-"], input=png, check=True,
                         capture_output=True).stdout
    return [[tuple(raw[(y * n + x) * 4:(y * n + x) * 4 + 4]) for x in range(n)] for y in range(n)]


def png_bytes(img):
    h, w = len(img), len(img[0])
    raw = b"".join(b"\0" + bytes(c for px in row for c in px) for row in img)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def and_mask(img):
    n = len(img)
    stride = (n + 31) // 32 * 4
    out = b""
    for row in reversed(img):
        bits = bytearray(stride)
        for x, px in enumerate(row):
            if px[3] < 128:
                bits[x // 8] |= 0x80 >> (x % 8)
        out += bytes(bits)
    return out


def dib(img, bpp, palette=None):
    """BITMAPINFOHEADER + (palette) + XOR rows bottom-up + AND mask, as stored in .ico."""
    n = len(img)
    mask = and_mask(img)
    if bpp == 32:
        xor = b"".join(bytes((b, g, r, a) if a else (0, 0, 0, 0)) for row in reversed(img) for r, g, b, a in row)
        pal = b""
    else:
        index = {c: i for i, c in enumerate(palette)}
        stride = (n + 3) // 4 * 4
        xor = b"".join(bytes(index[px[:3]] if px[3] >= 128 else 0 for px in row).ljust(stride, b"\0")
                       for row in reversed(img))
        pal = b"".join(bytes((b, g, r, 0)) for r, g, b in palette)
    head = struct.pack("<IiiHHIIiiII", 40, n, 2 * n, 1, bpp, 0, len(xor) + len(mask), 0, 0,
                       256 if bpp == 8 else 0, 0)
    return head + pal + xor + mask


def ico(entries):
    """entries: [(size, bpp, data)] -> .ico bytes."""
    out = struct.pack("<HHH", 0, 1, len(entries))
    offset = 6 + 16 * len(entries)
    body = b""
    for size, bpp, data in entries:
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, bpp, len(data), offset + len(body))
        body += data
    return out + body


def sheet(imgs, path, zoom, grounds):
    """Contact sheet: every size side by side at `zoom`, one row per ground color."""
    gap = 8 * zoom
    width = sum(len(i) * zoom for i in imgs) + gap * (len(imgs) + 1)
    height = max(len(i) for i in imgs) * zoom + 2 * gap
    rows = []
    for ground in grounds:
        canvas = [[ground] * width for _ in range(height)]
        x0 = gap
        for img in imgs:
            for y, row in enumerate(img):
                for x, (r, g, b, a) in enumerate(row):
                    c = tuple((v * a + w * (255 - a)) // 255 for v, w in zip((r, g, b), ground))
                    for yy in range(zoom):
                        canvas[gap + y * zoom + yy][x0 + x * zoom:x0 + (x + 1) * zoom] = [c] * zoom
            x0 += len(img) * zoom + gap
        rows += canvas
    with open(path, "wb") as f:
        f.write(png_bytes([[c + (255,) for c in row] for row in rows]))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--review", help="also write contact sheets into this directory")
    args = ap.parse_args()
    check_master()
    imgs = {n: pixel_art(n) for n in (16, 24, 32, 48)}
    imgs[256] = render(MASTER, 256)
    for n, img in imgs.items():
        with open(os.path.join(APP, f"fa-hangar-{n}.png"), "wb") as f:
            f.write(png_bytes(img))
    small = (48, 32, 24, 16)
    colors = sorted({px[:3] for n in small for row in imgs[n] for px in row if px[3] >= 128} - {(0, 0, 0)})
    palette = [(0, 0, 0)] + colors
    assert len(palette) <= 256
    palette += [(0, 0, 0)] * (256 - len(palette))
    entries = [(256, 32, png_bytes(imgs[256]))]
    for n in small:
        entries.append((n, 32, dib(imgs[n], 32)))
    for n in small:
        entries.append((n, 8, dib(imgs[n], 8, palette)))
    with open(os.path.join(APP, "fa-hangar.ico"), "wb") as f:
        f.write(ico(entries))
    if args.review:
        os.makedirs(args.review, exist_ok=True)
        grounds = [PLATE, (0, 128, 128), (255, 255, 255), (32, 32, 32)]
        order = [imgs[n] for n in (256, 48, 32, 24, 16)]
        sheet(order, os.path.join(args.review, "sizes-1x.png"), 1, grounds)
        sheet([imgs[n] for n in (48, 32, 24, 16)], os.path.join(args.review, "sizes-zoom8.png"), 8, grounds[1:3])


if __name__ == "__main__":
    main()
