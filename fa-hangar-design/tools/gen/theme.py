#!/usr/bin/env python3
"""Generate tokens/theme.rs and tokens/tokens.css from tokens/tokens.json.

tokens.json is the source of truth. Run from anywhere:

    python3 fa-hangar-design/tools/gen/theme.py

Output is deterministic; CI-style check: run it and `git diff --exit-code`.
Everything emitted for Rust is integer (the native build has no floating point).
"""
import json
import os
import re

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
TOKENS = os.path.join(ROOT, "tokens", "tokens.json")


def const(name):
    return re.sub(r"[^A-Z0-9]", "_", name.upper())


def px(value):
    if isinstance(value, int):
        return value
    m = re.fullmatch(r"(-?\d+)px", value)
    if not m:
        raise ValueError(f"Expected an integer px value, got {value!r}")
    return int(m.group(1))


def hexcolor(value):
    m = re.fullmatch(r"#([0-9a-fA-F]{6})", value)
    if not m:
        raise ValueError(f"Expected #rrggbb, got {value!r}")
    return m.group(1).upper()


def doc(usage, indent="    "):
    return f"{indent}/// {usage}\n" if usage else ""


def lip_color(t):
    for s in t["shadow"]["tokens"]:
        if s["name"] == "lip":
            m = re.search(r"#([0-9a-fA-F]{6})", s["value"])
            return m.group(1).upper(), s["usage"]
    raise ValueError("tokens.json has no lip shadow")


def text_styles(t):
    for group in t["type"]["groups"]:
        for s in group["styles"]:
            yield group["family"], s


def theme_rs(t):
    out = [
        "//! F.A. Hangar theme tokens, generated from tokens.json by tools/gen/theme.py.",
        "//! Do not edit by hand: change tokens.json and rerun the generator.",
        "//! Colors are 0x00RRGGBB (GDI COLORREF wants 0x00BBGGRR: use `colorref()`).",
        "//! No alpha and no floating point by design: every surface is a solid fill.",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct Rgb(pub u32);",
        "",
        "impl Rgb {",
        "    pub const fn r(self) -> u8 { (self.0 >> 16) as u8 }",
        "    pub const fn g(self) -> u8 { (self.0 >> 8) as u8 }",
        "    pub const fn b(self) -> u8 { self.0 as u8 }",
        "    /// Win32 COLORREF (0x00BBGGRR) for GDI calls.",
        "    pub const fn colorref(self) -> u32 { (self.b() as u32) << 16 | (self.g() as u32) << 8 | self.r() as u32 }",
        "    /// Integer mix: `weight` of 256 parts `other`. Used for icon edge pixels, never for alpha.",
        "    pub const fn mix(self, other: Rgb, weight: u32) -> Rgb {",
        "        const fn c(a: u8, b: u8, w: u32) -> u32 { (a as u32 * (256 - w) + b as u32 * w) / 256 }",
        "        let w = if weight > 256 { 256 } else { weight };",
        "        Rgb(c(self.r(), other.r(), w) << 16 | c(self.g(), other.g(), w) << 8 | c(self.b(), other.b(), w))",
        "    }",
        "}",
        "",
        "pub mod color {",
        "    use super::Rgb;",
    ]
    for c in t["color"]["tokens"]:
        out.append(doc(c["usage"]).rstrip("\n"))
        out.append(f"    pub const {const(c['name'])}: Rgb = Rgb(0x{hexcolor(c['value'])});")
    lip, usage = lip_color(t)
    out.append(doc(usage).rstrip("\n"))
    out.append(f"    pub const LIP: Rgb = Rgb(0x{lip});")
    out += ["}", "", "/// Spacing, in px at 1x.", "pub mod space {"]
    for s in t["spacing"]["tokens"]:
        out.append(doc(s["usage"]).rstrip("\n"))
        out.append(f"    pub const {const(s['name'])}: i32 = {px(s['value'])};")
    out += ["}", "", "/// Corner radii, in px at 1x.", "pub mod radius {"]
    for s in t["radius"]["tokens"]:
        out.append(doc(s["usage"]).rstrip("\n"))
        out.append(f"    pub const {const(s['name'])}: i32 = {px(s['value'])};")
    out += ["}", "", "/// Fixed control metrics, in px at 1x (percent where named PCT).", "pub mod metric {"]
    metrics = {m["name"]: px(m["value"]) for m in t["metric"]["tokens"]}
    for m in t["metric"]["tokens"]:
        out.append(doc(m["usage"]).rstrip("\n"))
        out.append(f"    pub const {const(m['name'])}: i32 = {px(m['value'])};")
    out.append("    /// Minimum window (width, height).")
    out.append(f"    pub const MIN_WINDOW: (i32, i32) = ({metrics['min-window-w']}, {metrics['min-window-h']});")
    out += [
        "}",
        "",
        "/// Type styles: (size px, line height px, weight). Family: Ui or Mono.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub enum Family { Ui, Mono, Display }",
        "",
        "/// `tracking` is letter spacing in 1/100 em (0 for none).",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct TextStyle { pub family: Family, pub size: i32, pub line: i32, pub weight: u16, pub tracking: i32 }",
        "",
        "pub mod text {",
        "    use super::{Family, TextStyle};",
    ]
    for family, s in text_styles(t):
        spacing = s.get("letterSpacing", "0em")
        m = re.fullmatch(r"(\d+)(?:\.(\d+))?em", spacing)
        if not m:
            raise ValueError(f"letterSpacing must be in em: {spacing!r}")
        frac = (m.group(2) or "").ljust(2, "0")[:2]
        tracking = int(m.group(1)) * 100 + int(frac)
        out.append(doc(s["usage"]).rstrip("\n"))
        out.append(
            f"    pub const {const(s['name'])}: TextStyle = TextStyle {{ family: Family::{family.capitalize()}, "
            f"size: {px(s['fontSize'])}, line: {px(s['lineHeight'])}, weight: {int(s['fontWeight'])}, tracking: {tracking} }};"
        )
    out.append("}")
    return "\n".join(out) + "\n"


def tokens_css(t):
    out = ["/* Generated from tokens.json by tools/gen/theme.py. Gunmetal theme. */", ":root {"]
    for c in t["color"]["tokens"]:
        out.append(f"  --{c['name']}: {c['value']};")
    for group in ("spacing", "radius", "shadow"):
        for s in t[group]["tokens"]:
            out.append(f"  --{s['name']}: {s['value']};")
    for m in t["metric"]["tokens"]:
        unit = "" if m["name"].endswith("-pct") or m["name"] == "grid-major" else "px"
        out.append(f"  --{m['name']}: {m['value']}{unit};")
    for k, v in t["type"]["families"].items():
        out.append(f"  --font-{k}: {v};")
    out.append("}")
    for family, s in text_styles(t):
        extra = f" letter-spacing: {s['letterSpacing']};" if "letterSpacing" in s else ""
        out.append(
            f".{s['name']} {{ font: {s['fontWeight']} {s['fontSize']}/{s['lineHeight']} var(--font-{family}); }}"
            if not extra
            else f".{s['name']} {{ font: {s['fontWeight']} {s['fontSize']}/{s['lineHeight']} var(--font-{family});{extra} }}"
        )
    return "\n".join(out) + "\n"


def main():
    t = json.load(open(TOKENS))
    for name, text in (("theme.rs", theme_rs(t)), ("tokens.css", tokens_css(t))):
        path = os.path.join(ROOT, "tokens", name)
        with open(path, "w") as f:
            f.write(text)
        print(f"wrote {os.path.relpath(path, ROOT)}")


if __name__ == "__main__":
    main()
