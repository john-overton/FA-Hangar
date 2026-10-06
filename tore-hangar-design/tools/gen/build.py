import json, os, sys
sys.path.insert(0, os.path.dirname(__file__))
from icons import ICONS, sprite, ic, asset_svg

OUT = sys.argv[1]
P = lambda *a: os.path.join(OUT, "project", *a)

def w(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        f.write(text)

FONTS = '<link rel="preconnect" href="https://fonts.googleapis.com"><link rel="preconnect" href="https://fonts.gstatic.com" crossorigin><link href="https://fonts.googleapis.com/css2?family=Barlow+Condensed:wght@600&family=Barlow+Semi+Condensed:wght@400;500;600&family=JetBrains+Mono:wght@400;500;700&display=swap" rel="stylesheet">'

def doc(marker, title, body, style="", bodycls="th-root", pad=True):
    st = "html,body{margin:0;background:var(--gm-900)}" + ("body{padding:16px}" if pad else "") + style
    return f"""{marker}
<!doctype html>
<html>
<head><meta charset="utf-8"><title>{title}</title>{FONTS}<style>{st}</style></head>
<body class="{bodycls}">
{sprite()}
{body}
</body>
</html>
"""

# ---------------- tokens ----------------
C = [
 ("gm-1000", "#0f1113", "Keylines: the 1px dark edge around buttons, panels, and editor seams. Never a fill for content."),
 ("gm-950", "#15181b", "Viewport background, field and NumberField fill, menubar, status bar, vertical tab strip."),
 ("gm-900", "#1b1f23", "App window background behind editors; zebra rows in the outliner; timeline ruler."),
 ("gm-800", "#23282d", "Editor and panel surface (outliner, properties, timeline). The default ground for text."),
 ("gm-700", "#2c3238", "Raised controls: buttons, selects, keycaps; row hover."),
 ("gm-600", "#383f46", "Button hover; type badge fill; separators inside menus."),
 ("gm-500", "#48515a", "Non-selected inner mesh edges in the viewport; timeline spans. Decorative only, never text."),
 ("line-strong", "#727b84", "Border on fields, NumberFields, checkboxes, and graft slots. 3:1 on gm-800 and gm-700."),
 ("ink", "#e4e8eb", "Primary text and values on every gm surface (10:1 or better) and on amber-deep."),
 ("ink-muted", "#aab3bb", "Property labels, secondary text, idle icons, viewport overlay text. 5:1 or better on gm-600 through gm-1000."),
 ("ink-faint", "#808a93", "Disabled text and placeholders only. 4.2:1 on gm-800; never for live labels."),
 ("amber", "#e9a23b", "Selection and the active state: selected outline in the viewport, active tab icon, changed values, primary button fill, dirty dot. 6:1 on gm-800."),
 ("amber-bright", "#f6c066", "The ACTIVE item inside a selection (active outliner row text, object origin); primary button hover."),
 ("amber-deep", "#3a2f1c", "Solid selection fill behind selected rows, toggled buttons, and hot menu items. Takes ink, amber, or amber-bright text."),
 ("on-amber", "#1b1307", "Text and checkmarks on amber and amber-bright fills."),
 ("steel", "#66aedb", "Time and reference: timeline playhead, hardpoint markers, graft ghost geometry, links. 6:1 on gm-800."),
 ("steel-deep", "#1d3140", "Slider fill inside a NumberField that has a bounded range."),
 ("focus", "#66aedb", "Keyboard focus ring: 1px solid, 1px offset. 6:1 on gm-800, 7:1 on gm-950."),
 ("ok", "#7ccc8a", "Validated / packaged / round-trip clean. Always paired with a check icon or word."),
 ("danger", "#ec6a58", "Errors, destructive actions, failed validation. 4.8:1 on gm-800; always paired with a warning icon or word."),
 ("axis-x", "#e2595e", "X axis: gizmo, grid axis line, X labels in vector fields. 4.9:1 on gm-950."),
 ("axis-y", "#86c24a", "Y axis: gizmo, grid axis line, Y labels in vector fields. 8:1 on gm-950."),
 ("axis-z", "#4f8fe3", "Z axis: gizmo, Z labels in vector fields. 5.4:1 on gm-950."),
]
tokens = {
 "name": "TORE Hangar", "version": 1,
 "color": {"themes": [{"id": "gunmetal", "name": "Gunmetal"}],
           "tokens": [{"name": n, "value": v, "usage": u} for n, v, u in C]},
 "type": {
   "fonts": [],
   "families": {
     "ui": "\"Barlow Semi Condensed\", Tahoma, \"MS Sans Serif\", sans-serif",
     "mono": "\"JetBrains Mono\", \"Lucida Console\", \"Courier New\", monospace",
     "display": "\"Barlow Condensed\", \"Barlow Semi Condensed\", Tahoma, sans-serif"},
   "groups": [
     {"name": "Interface", "family": "ui", "styles": [
       {"name": "title", "fontSize": "14px", "lineHeight": "18px", "fontWeight": 600, "usage": "Dialog titles, the graft target name, workspace tab when active.", "sample": "Graft aspects into F14.PT"},
       {"name": "body", "fontSize": "13px", "lineHeight": "18px", "fontWeight": 400, "usage": "Notices and longer help text.", "sample": "Shape F14.SH is referenced by 3 aircraft entries."},
       {"name": "label", "fontSize": "12px", "lineHeight": "16px", "fontWeight": 500, "usage": "Default UI text: buttons, property labels, tree rows, menus.", "sample": "Stall speed"},
       {"name": "section", "fontSize": "11px", "lineHeight": "14px", "fontWeight": 600, "letterSpacing": "0.06em", "usage": "Uppercase sub-headings inside a panel body and graft slot roles.", "sample": "PROPULSION"},
       {"name": "hint", "fontSize": "11px", "lineHeight": "14px", "fontWeight": 400, "usage": "Status bar, timeline labels.", "sample": "G Move · R Rotate · S Scale"}]},
     {"name": "Data", "family": "mono", "styles": [
       {"name": "value", "fontSize": "12px", "lineHeight": "16px", "fontWeight": 400, "usage": "Every editable number, filename, and hex offset.", "sample": "20,900 lbf"},
       {"name": "value-sm", "fontSize": "11px", "lineHeight": "15px", "fontWeight": 400, "usage": "Viewport overlays, counts, diff summaries.", "sample": "412 verts · 286 faces"},
       {"name": "badge", "fontSize": "10px", "lineHeight": "14px", "fontWeight": 500, "usage": "Lib entry type badges and keycaps.", "sample": "PT SH PIC JT"}]},
     {"name": "Display", "family": "display", "styles": [
       {"name": "display", "fontSize": "64px", "lineHeight": "60px", "fontWeight": 600, "usage": "Splash and about box only.", "sample": "TORE Hangar"}]}]},
 "spacing": {"tokens": [
   {"name": "space-1", "value": "4px", "usage": "Icon to label gap; gap between stacked panels."},
   {"name": "space-2", "value": "8px", "usage": "Panel body inset; label to control gap in a property row."},
   {"name": "space-3", "value": "12px", "usage": "Menu item side padding."},
   {"name": "space-4", "value": "16px", "usage": "Status bar group gap; dialog padding."},
   {"name": "space-6", "value": "24px", "usage": "Gap between dialog sections."}]},
 "radius": {"tokens": [
   {"name": "radius-xs", "value": "2px", "usage": "Badges, keycaps, timeline playhead cap."},
   {"name": "radius-sm", "value": "3px", "usage": "Buttons, fields, panels, rows. The default."},
   {"name": "radius-md", "value": "4px", "usage": "Floating viewport tool strip."}]},
 "shadow": {"note": "No soft shadows anywhere: the target renderer has no alpha. Depth comes from fill steps and keylines.", "tokens": [
   {"name": "lip", "value": "inset 0 1px 0 #3d444b", "usage": "1px top highlight on raised controls (buttons, selects). Drawn as a single line, not a blur."}]},
}
w(P("tokens.json"), json.dumps(tokens, indent=2) + "\n")
w(P("components", "bundle.css"), open(os.path.join(os.path.dirname(__file__), "bundle.css")).read())

# ---------------- shared builders ----------------
L_HALF = [(0,-160),(-7,-130),(-12,-95),(-16,-70),(-46,-18),(-150,48),(-152,60),(-50,40),(-30,64),(-28,96),(-86,128),(-84,142),(-26,134),(-20,150),(0,150)]
def outline():
    right = [(-x, y) for x, y in reversed(L_HALF[1:-1])]
    pts = L_HALF + right
    return " ".join(f"{x},{y}" for x, y in pts)
def mirror_lines(lines):
    out = []
    for (a, b) in lines:
        out.append((a, b)); out.append(((-a[0], a[1]), (-b[0], b[1])))
    return out
INNER = mirror_lines([((-16,-70),(-22,0)),((-22,0),(-26,64)),((-26,64),(-24,134)),((-46,-18),(-22,0)),((-50,40),(-24,40)),
    ((-46,-18),(-50,40)),((-50,40),(-150,48)),((-28,96),(-26,134)),((-28,96),(-84,142)),((-12,-95),(0,-70)),((-16,-70),(0,-40)),((-22,0),(0,20)),((-26,64),(0,90))])
TAILS = mirror_lines([((-14,86),(-20,142))])
CANOPY = "0,-120 -6,-110 -6,-92 0,-86 6,-92 6,-110"
HPS = [(-38,-2,"1"),(38,-2,"2"),(-14,30,"3"),(14,30,"4"),(-14,100,"5"),(14,100,"6")]

def jet(cx, cy, s, hp=True, cls_sel="vp-sel", ghost=False):
    g = [f'<g transform="translate({cx} {cy}) scale({s})">']
    for a, b in INNER:
        g.append(f'<line class="vp-wire-inner" x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" vector-effect="non-scaling-stroke"/>')
    g.append('<line class="vp-wire-inner" x1="0" y1="-160" x2="0" y2="150" vector-effect="non-scaling-stroke"/>')
    g.append(f'<polygon class="vp-wire" points="{CANOPY}" vector-effect="non-scaling-stroke"/>')
    for a, b in TAILS:
        g.append(f'<line class="vp-wire" x1="{a[0]}" y1="{a[1]}" x2="{b[0]}" y2="{b[1]}" vector-effect="non-scaling-stroke"/>')
    g.append(f'<polygon class="{cls_sel}" points="{outline()}" vector-effect="non-scaling-stroke"/>')
    if hp:
        for x, y, n in HPS:
            g.append(f'<g transform="translate({x} {y}) scale({1/s})"><rect class="vp-hp" x="-4" y="-4" width="8" height="8" transform="rotate(45)"/>'
                     f'<text class="vp-hp-label" x="{8 if x>0 else -8}" y="3.5" text-anchor="{"start" if x>0 else "end"}">HP{n}</text></g>')
    g.append(f'<g transform="scale({1/s})"><circle class="vp-origin" r="3.5"/></g>')
    g.append('</g>')
    return "".join(g)

def grid(wd, ht, cx, cy, step=24):
    g = []
    i = 0
    x = cx % step
    while x < wd:
        major = round((x - cx) / step) % 5 == 0
        g.append(f'<line class="{"vp-grid-major" if major else "vp-grid"}" x1="{x}" y1="0" x2="{x}" y2="{ht}"/>'); x += step
    y = cy % step
    while y < ht:
        major = round((y - cy) / step) % 5 == 0
        g.append(f'<line class="{"vp-grid-major" if major else "vp-grid"}" x1="0" y1="{y}" x2="{wd}" y2="{y}"/>'); y += step
    g.append(f'<line class="vp-axis-x" x1="0" y1="{cy}" x2="{wd}" y2="{cy}"/>')
    g.append(f'<line class="vp-axis-y" x1="{cx}" y1="0" x2="{cx}" y2="{ht}"/>')
    return "".join(g)

def gizmo():
    return ('<svg width="84" height="84" viewBox="0 0 84 84" aria-label="Navigation gizmo">'
      '<line class="g-x" x1="42" y1="42" x2="70" y2="42" stroke-width="2"/><line class="g-y" x1="42" y1="42" x2="42" y2="14" stroke-width="2"/>'
      '<circle class="g-neg g-x" cx="14" cy="42" r="6"/><circle class="g-neg g-y" cx="42" cy="70" r="6"/>'
      '<circle class="g-x" cx="70" cy="42" r="8"/><text class="th-gizmo-axis" x="70" y="45.5" text-anchor="middle">X</text>'
      '<circle class="g-y" cx="42" cy="14" r="8"/><text class="th-gizmo-axis" x="42" y="17.5" text-anchor="middle">Y</text>'
      '<circle class="g-z" cx="42" cy="42" r="8"/><text class="th-gizmo-axis" x="42" y="45.5" text-anchor="middle">Z</text></svg>')

def viewport(wd, ht, scale):
    cx, cy = wd // 2 + 20, ht // 2 + 4
    tools = [("select", True), ("move", False), ("rotate", False), ("scale", False), None, ("measure", False), ("hardpoint", False)]
    tl = "".join('<div class="th-toolstrip-sep"></div>' if t is None else f'<button class="th-btn th-btn-icon{" is-on" if t[1] else ""}" title="{t[0]}">{ic(t[0])}</button>' for t in tools)
    return f'''<div class="th-viewport" style="width:{wd}px;height:{ht}px">
<svg class="th-viewport-canvas" width="{wd}" height="{ht}" viewBox="0 0 {wd} {ht}">{grid(wd, ht, cx, cy)}{jet(cx, cy, scale)}</svg>
<div class="th-toolstrip">{tl}</div>
<div class="th-overlay" style="left:48px;top:10px"><b>Top · Orthographic</b><br>F14.SH · LOD 0<br>412 verts · 286 faces · 6 hardpoints</div>
<div class="th-overlay" style="right:8px;top:6px">{gizmo()}</div>
<div class="th-overlay" style="left:48px;bottom:8px">Object Mode · 1 selected</div>
<div class="th-overlay" style="right:12px;bottom:8px">grid 1 m · units ft</div>
</div>'''

def viewport_head():
    return f'''<div class="th-editor-head">
<div class="th-select" style="width:116px">{ic("select","th-ic-sm")}<span class="th-select-value">Object Mode</span>{ic("chevron-down","th-ic-sm")}</div>
<div class="th-row" style="gap:2px"><button class="th-btn th-btn-ghost">View</button><button class="th-btn th-btn-ghost">Select</button><button class="th-btn th-btn-ghost">Mesh</button><button class="th-btn th-btn-ghost">Hardpoints</button></div>
<div class="th-spacer"></div>
<div class="th-select" style="width:72px"><span class="th-select-value">LOD 0</span>{ic("chevron-down","th-ic-sm")}</div>
<div class="th-seg"><button class="th-btn th-btn-icon is-on" title="Hardpoints">{ic("hardpoint")}</button><button class="th-btn th-btn-icon" title="Bounds">{ic("shape")}</button><button class="th-btn th-btn-icon" title="Overlays">{ic("eye")}</button></div>
<div class="th-seg"><button class="th-btn th-btn-icon is-on" title="Wireframe">{ic("wire")}</button><button class="th-btn th-btn-icon" title="Solid">{ic("solid")}</button><button class="th-btn th-btn-icon" title="Textured">{ic("textured")}</button></div>
</div>'''

def num(label, value, unit="", fill=None, changed=False, locked=False, label_cls=""):
    cls = "th-num" + (" is-changed" if changed else "") + (" is-locked" if locked else "")
    f = f'<span class="th-num-fill" style="width:{fill}%"></span>' if fill is not None else ""
    lab = f'<span class="th-num-label {label_cls}">{label}</span>' if label else ""
    u = f'<span class="th-num-unit">{unit}</span>' if unit else ""
    lock = ic("lock", "th-ic-sm") if locked else ""
    return f'<div class="{cls}">{f}<span class="th-num-arrow">‹</span>{lab}<span class="th-num-value">{value}{u}</span>{lock and "<span style=position:relative;padding-right:5px;color:var(--ink-muted)>"+lock+"</span>"}<span class="th-num-arrow">›</span></div>'

def prop(label, control, top=False):
    return f'<div class="th-prop{" is-top" if top else ""}"><span class="th-prop-label">{label}</span>{control}</div>'

def panel(title, body, icon=None, collapsed=False, tools=""):
    chev = ic("chevron-right" if collapsed else "chevron-down", "th-ic-sm")
    i = ic(icon) if icon else ""
    t = f'<span class="th-panel-tools">{tools}</span>' if tools else ""
    return f'<section class="th-panel{" is-collapsed" if collapsed else ""}"><div class="th-panel-head">{chev}{i}<span>{title}</span>{t}</div><div class="th-panel-body">{body}</div></section>'

def flight_panels():
    env = "".join([
        prop("Max speed", num("", "2.34", " M", changed=True)),
        prop("Corner speed", num("", "330", " kt")),
        prop("Stall speed", num("", "115", " kt")),
        prop("Ceiling", num("", "53,000", " ft")),
        prop("G limit", '<div class="th-row" style="flex-wrap:nowrap;gap:4px">' + num("+", "7.5") + num("−", "3.0") + '</div>'),
    ])
    prop_ = "".join([
        prop("Engines", num("", "2", locked=True)),
        prop("Thrust, mil", num("", "12,350", " lbf")),
        prop("Afterburner", num("", "20,900", " lbf")),
        prop("Fuel, internal", num("", "16,200", " lb", fill=62)),
        prop("Burner flame", '<label class="th-check is-on"><span class="th-box">' + ic("check") + '</span>Show on AB</label>'),
    ])
    wts = "".join([prop("Empty", num("", "40,100", " lb")), prop("Max takeoff", num("", "74,350", " lb"))])
    hnd = "".join([prop("Roll rate", num("", "180", " °/s", fill=50)), prop("Pitch rate", num("", "22", " °/s", fill=44))])
    return (panel("Envelope", env, tools=f'<button class="th-btn th-btn-ghost th-btn-icon" title="Reset">{ic("rotate","th-ic-sm")}</button>')
            + panel("Propulsion", prop_) + panel("Weights", wts) + panel("Handling", hnd) + panel("Stores limits", "", collapsed=True))

def vtabs(active="flight"):
    items = [("lib", "Lib entry"), ("sliders", "Object"), None, ("flight", "Flight"), ("engine", "Systems"), ("hardpoint", "Hardpoints"), ("damage", "Damage"), ("textured", "Materials"), None, ("graft", "Graft")]
    return '<nav class="th-vtabs">' + "".join('<div class="th-vtab-sep"></div>' if t is None else f'<div class="th-vtab{" is-active" if t[0]==active else ""}" title="{t[1]}">{ic(t[0])}</div>' for t in items) + '</nav>'

def props_header():
    return f'''<div style="padding:6px 8px 4px;display:flex;flex-direction:column;gap:4px;border-bottom:1px solid var(--gm-1000)">
<div class="th-row" style="flex-wrap:nowrap">{ic("aircraft")}<div class="th-field" style="flex:1"><input value="F14.PT" aria-label="Entry name"></div><span class="th-badge">PT</span></div>
<div class="th-row" style="flex-wrap:nowrap"><span class="th-muted" style="width:52px;text-align:right">Shape</span><div class="th-select" style="flex:1">{ic("shape","th-ic-sm")}<span class="th-select-value">F14.SH</span>{ic("chevron-down","th-ic-sm")}</div><button class="th-btn th-btn-icon" title="Linked">{ic("link","th-ic-sm")}</button></div>
</div>'''

def node(depth, name, icon=None, badge=None, count=None, twisty=None, cls="", dirty=False):
    tw = ""
    if twisty is not None:
        tw = ic("chevron-down" if twisty else "chevron-right")
    meta = ""
    if dirty: meta += '<span class="th-dirty" title="Modified"></span>'
    if count is not None: meta += f'<span class="th-count">{count}</span>'
    if badge: meta += f'<span class="th-badge">{badge}</span>'
    i = ic(icon) if icon else ""
    return f'<div class="th-node {cls}" style="padding-left:{4+depth*14}px"><span class="th-twisty">{tw}</span>{i}<span class="th-node-name">{name}</span><span class="th-node-meta">{meta}</span></div>'

def tree():
    rows = [
        node(0, "USNF97.LIB", "lib", count="1,204", twisty=True, cls="is-lib", dirty=True),
        node(1, "Aircraft", "aircraft", "PT", "42", True),
        node(2, "F14.PT", "aircraft", cls="is-active", dirty=True),
        node(2, "F18.PT", "aircraft"),
        node(2, "A6.PT", "aircraft"),
        node(2, "MIG29.PT", "aircraft", cls="is-selected"),
        node(2, "SU27.PT", "aircraft"),
        node(1, "Shapes", "shape", "SH", "318", False),
        node(1, "Images", "image", "PIC", "211", False),
        node(1, "Weapons", "weapon", "JT", "58", False),
        node(1, "Ground objects", "object", "OT", "96", False),
        node(1, "Palettes", "palette", "PAL", "12", False),
        node(1, "Missions", "mission", "M", "74", False),
        node(1, "Sounds", "sound", "11K", "140", False),
        node(0, "ATF.LIB", "lib", count="866", twisty=True, cls="is-lib"),
        node(1, "Aircraft", "aircraft", "PT", "31", True),
        node(2, "F14A.PT", "aircraft"),
        node(2, "F22.PT", "aircraft"),
        node(1, "Shapes", "shape", "SH", "204", False),
    ]
    return '<div class="th-tree">' + "".join(rows) + '</div>'

def outliner_head():
    return f'''<div class="th-editor-head">
<div class="th-field" style="flex:1">{ic("search","th-ic-sm")}<span class="th-placeholder">Filter entries</span></div>
<div class="th-seg"><button class="th-btn th-btn-icon is-on" title="Aircraft">{ic("aircraft","th-ic-sm")}</button><button class="th-btn th-btn-icon" title="Shapes">{ic("shape","th-ic-sm")}</button><button class="th-btn th-btn-icon" title="Images">{ic("image","th-ic-sm")}</button></div>
<button class="th-btn th-btn-icon" title="Open lib">{ic("plus","th-ic-sm")}</button>
</div>'''

TRACKS = [("Landing gear", [(0, False), (24, True)], (0, 24)), ("Flaps", [(6, False), (18, False)], (6, 18)), ("Wing sweep", [(0, False), (30, False), (48, False)], (0, 48)),
          ("Tail hook", [(36, False), (42, False)], (36, 42)), ("Canopy", [(50, False), (60, False)], (50, 60)), ("Burner", [(12, True)], None)]
def timeline(lane_w, head=24, rows=TRACKS, frames=60):
    px = lambda f: f / frames * 100
    ticks = "".join(f'<span class="th-tl-tick" style="left:calc(120px + (100% - 120px) * {f/frames:.4f})">{f}</span>' for f in range(0, frames + 1, 10))
    tracks = ""
    for name, keys, span in rows:
        sp = f'<span class="th-tl-span" style="left:{px(span[0])}%;width:{px(span[1]-span[0])}%"></span>' if span else ""
        ks = "".join(f'<span class="th-tl-key{" is-selected" if sel else ""}" style="left:{px(f)}%"></span>' for f, sel in keys)
        tracks += f'<div class="th-tl-track"><div class="th-tl-name"><span>{name}</span></div><div class="th-tl-lane">{sp}{ks}</div></div>'
    headpos = f"calc(120px + (100% - 120px) * {head/frames:.4f})"
    return f'''<div class="th-timeline" style="position:relative">
<div class="th-tl-ruler">{ticks}</div>{tracks}
<div class="th-tl-head" style="left:{headpos}"></div><div class="th-tl-headcap" style="left:{headpos}">{head}</div>
</div>'''

def timeline_head():
    return f'''<div class="th-editor-head">
<div class="th-seg"><button class="th-btn is-on">Animation</button><button class="th-btn">Raw fields</button><button class="th-btn">Log</button></div>
<div class="th-spacer"></div>
<div class="th-seg"><button class="th-btn th-btn-icon" title="Start">{ic("skip-start","th-ic-sm")}</button><button class="th-btn th-btn-icon" title="Play">{ic("play","th-ic-sm")}</button><button class="th-btn th-btn-icon" title="End">{ic("skip-end","th-ic-sm")}</button></div>
<div style="width:96px">{num("Frame", "24")}</div>
<div style="width:96px">{num("End", "60")}</div>
</div>'''

def menubar(active="Model"):
    menus = "".join(f'<span class="th-menubar-item">{m}</span>' for m in ["File", "Edit", "Lib", "Entry", "View", "Tools", "Help"])
    tabs = "".join(f'<span class="th-wstab{" is-active" if t==active else ""}">{t}</span>' for t in ["Browse", "Model", "Flight", "Graft", "Package"])
    return f'''<div class="th-menubar">
<span style="display:flex;align-items:center;gap:6px;padding:0 8px 0 4px;color:var(--amber)">{ic("hardpoint")}<b style="color:var(--ink);font-weight:600">Hangar</b></span>
{menus}<div class="th-wstabs">{tabs}</div>
<span style="margin-left:auto;display:flex;align-items:center;gap:6px;color:var(--ink-muted);padding-right:6px" class="th-mono"><span class="th-dirty"></span>USNF97.LIB</span>
</div>'''

def statusbar():
    hints = [("G", "Move"), ("R", "Rotate"), ("S", "Scale"), ("Tab", "Edit mode"), ("MMB", "Orbit"), ("H", "Hardpoint")]
    h = "".join(f'<span class="th-status-hint"><span class="th-kbd">{k}</span>{t}</span>' for k, t in hints)
    return f'''<div class="th-status">{h}<span class="th-spacer"></span>
<span class="th-status-hint th-mono">F14.PT · USNF97.LIB</span>
<span class="th-status-hint"><span class="th-dirty"></span>3 unsaved edits</span>
<span class="th-status-hint" style="color:var(--ok)">{ic("check","th-ic-sm")}Round-trip clean</span></div>'''

# ---------------- previews ----------------
comps = {}

comps["Button"] = ("Actions", 120, f'''<div class="th-col">
<div class="th-row"><button class="th-btn">Revert</button><button class="th-btn th-btn-primary">Apply graft</button><button class="th-btn">{ic("package","th-ic-sm")}Package lib</button><button class="th-btn" disabled>Export</button><button class="th-btn th-btn-ghost">Cancel</button><button class="th-btn th-btn-danger">{ic("close","th-ic-sm")}Remove entry</button></div>
<div class="th-row"><button class="th-btn th-btn-icon" title="Play">{ic("play","th-ic-sm")}</button><button class="th-btn th-btn-icon is-on" title="Hardpoints">{ic("hardpoint")}</button>
<div class="th-seg"><button class="th-btn th-btn-icon is-on" title="Wireframe">{ic("wire")}</button><button class="th-btn th-btn-icon" title="Solid">{ic("solid")}</button><button class="th-btn th-btn-icon" title="Textured">{ic("textured")}</button></div>
<div class="th-seg"><button class="th-btn is-on">Animation</button><button class="th-btn">Raw fields</button><button class="th-btn">Log</button></div></div>
</div>''', """# Button

Buttons are flat gunmetal blocks with a 1px `gm-1000` keyline and a 1px `lip` highlight; the only colored button is the one action a dialog exists for.

- **Default** (`th-btn`): `gm-700` fill, `ink` label, hover `gm-600`, pressed `gm-950` with no lip.
- **Primary** (`th-btn-primary`): `amber` fill, `on-amber` label. At most one per dialog or panel: Apply graft, Save lib, Package.
- **Toggle on** (`is-on`): `amber-deep` fill, `amber` label or icon. Use for viewport overlays, shading modes, and dock tabs.
- **Icon** (`th-btn-icon`): 22 × 22, 16px icon. Always give it a `title`; the status bar shows the title on hover.
- **Segmented** (`th-seg`): joined buttons for mutually exclusive modes. Two to five members.
- **Danger** (`th-btn-danger`): `danger` label on the default fill, always with the close or warning icon.
- **Ghost** (`th-btn-ghost`): editor header menus (View, Select, Mesh) and Cancel.

Height is fixed at 22px. Labels are verbs in sentence case: "Apply graft", not "OK". The consumer provides the label, optional leading icon, and `title`.
""")

comps["NumberField"] = ("Inputs", 196, f'''<div style="display:grid;grid-template-columns:260px 200px;gap:24px">
<div class="th-col">
{prop("Stall speed", num("", "115", " kt"))}
{prop("Max speed", num("", "2.34", " M", changed=True))}
{prop("Fuel, internal", num("", "16,200", " lb", fill=62))}
{prop("Engines", num("", "2", locked=True))}
<div class="th-subhead">Hover shows scrub arrows</div>
<div class="th-num" style="background:var(--gm-1000)"><span class="th-num-arrow" style="display:block">‹</span><span class="th-num-label">Roll rate</span><span class="th-num-value">180<span class="th-num-unit"> °/s</span></span><span class="th-num-arrow" style="display:block">›</span></div>
</div>
<div class="th-col"><div class="th-subhead">Location</div>
<div class="th-vec">{num("X", "0.000", " ft", label_cls="th-axis-x")}{num("Y", "−4.250", " ft", label_cls="th-axis-y", changed=True)}{num("Z", "1.120", " ft", label_cls="th-axis-z")}</div>
<div class="th-subhead">Scale</div>
<div class="th-vec">{num("X", "1.000", label_cls="th-axis-x")}{num("Y", "1.000", label_cls="th-axis-y")}{num("Z", "1.000", label_cls="th-axis-z")}</div></div>
</div>''', """# NumberField

The Blender-style scrub field: drag horizontally to change the value, click to type, arrows step by the field's increment.

- Label sits left in `label` / `ink-muted`; the value sits right in `value` (mono) / `ink`, unit in `ink-muted`.
- Fill is `gm-950` with a `line-strong` border; hover darkens to `gm-1000` and reveals the ‹ › step arrows.
- **Changed** (`is-changed`): the value turns `amber` when it differs from the value on disk in the source lib. This is how a user sees unsaved edits at a glance.
- **Bounded** (`th-num-fill`): a `steel-deep` bar shows position within a known range (fuel load, roll rate). Omit for unbounded values.
- **Locked** (`is-locked`): dashed border, `ink-muted` value, lock icon. Use for fields derived from another entry (engine count follows the shape).
- **Vector** (`th-vec`): X, Y, Z stacked and joined, labels in `axis-x`, `axis-y`, `axis-z`.

Interaction: drag = coarse, Shift+drag = fine (×0.1), Ctrl+drag = snap to increment, double-click = type, Esc = revert, Backspace = reset to file value. The consumer provides label, value, unit, min/max (optional), increment, and the on-disk value for the changed state.
""")

comps["Select"] = ("Inputs", 200, f'''<div class="th-row" style="align-items:flex-start;gap:24px">
<div class="th-col" style="width:180px">
<div class="th-select">{ic("select","th-ic-sm")}<span class="th-select-value">Object Mode</span>{ic("chevron-down","th-ic-sm")}</div>
<div class="th-select">{ic("shape","th-ic-sm")}<span class="th-select-value">F14.SH</span>{ic("chevron-down","th-ic-sm")}</div>
<div class="th-field">{ic("search","th-ic-sm")}<span class="th-placeholder">Filter entries</span></div>
<div class="th-field is-editing"><input value="F14_TOMCAT.PT" aria-label="Name"></div>
</div>
<div class="th-menu">
<div class="th-menu-item">{ic("select")}Object Mode<span class="th-kbd">Tab</span></div>
<div class="th-menu-item is-hot">{ic("shape")}Edit Mesh</div>
<div class="th-menu-item">{ic("hardpoint")}Hardpoints<span class="th-kbd">H</span></div>
<div class="th-menu-sep"></div>
<div class="th-menu-item">{ic("textured")}Texture Paint</div>
</div></div>''', """# Select

Dropdowns use the raised button treatment (`gm-700` + `lip`) so they read as clickable; text fields use the sunken field treatment (`gm-950` + `line-strong`) so they read as editable.

- Leading 12px icon is optional and names the kind of thing chosen (mode, shape entry, palette).
- Menus: `gm-800` surface, 22px items, hot item `amber-deep`, shortcut keycap right-aligned.
- Entry pickers (Shape, Palette, Weapon) list lib entries with their type badge and filter as the user types.
- Text fields show `ink-faint` placeholders; while editing, the border switches to `focus`.

The consumer provides options, the current value, and an optional icon per option.
""")

comps["Checkbox"] = ("Inputs", 64, f'''<div class="th-row" style="gap:20px">
<label class="th-check is-on"><span class="th-box">{ic("check")}</span>Show on AB</label>
<label class="th-check"><span class="th-box"></span>Carrier capable</label>
<label class="th-check is-mixed"><span class="th-box"></span>Mixed (3 entries)</label>
</div>''', """# Checkbox

14px box, `gm-950` fill with `line-strong` border; checked is a solid `amber` box with an `on-amber` check.

- Mixed (`is-mixed`) appears when a multi-entry selection disagrees.
- Label sits right in `label` / `ink`. Click the label or the box.
- For on/off view options in headers, use a toggle icon button instead.
""")

comps["Panel"] = ("Layout", 330, f'''<div style="width:320px">{panel("Envelope", prop("Max speed", num("", "2.34", " M", changed=True)) + prop("Stall speed", num("", "115", " kt")) + prop("Ceiling", num("", "53,000", " ft")), tools=f'<button class="th-btn th-btn-ghost th-btn-icon" title="Reset">{ic("rotate","th-ic-sm")}</button>')}
{panel("Propulsion", '<div class="th-subhead">Engine</div>' + prop("Thrust, mil", num("", "12,350", " lbf")) + prop("Afterburner", num("", "20,900", " lbf")))}
{panel("Weights", "", collapsed=True)}
{panel("Handling", "", collapsed=True)}</div>''', """# Panel

Collapsible property groups, stacked inside the Properties editor exactly like Blender sub-panels.

- Surface `gm-800` with a `gm-1000` keyline; 24px header in `label` weight 600 with a chevron.
- Body rows use `th-prop`: label right-aligned in a 40% column (`ink-muted`), control in the rest. Keep labels to three words.
- `th-subhead` (style `section`) splits a long panel without nesting another panel.
- Header tools (reset, copy, paste values) are ghost icon buttons at the right.
- Panels remember collapsed state per tab. Ctrl+click a header collapses every other panel.

The consumer provides title, rows, optional header tools, and the collapsed state.
""")

comps["PropertyTabs"] = ("Layout", 300, f'''<div style="display:flex;height:270px;width:340px;border:1px solid var(--gm-1000)">{vtabs()}<div style="flex:1;background:var(--gm-800);padding:6px">{panel("Envelope", prop("Max speed", num("", "2.34", " M", changed=True)) + prop("Stall speed", num("", "115", " kt")))}</div></div>''', """# PropertyTabs

The vertical icon strip on the left edge of the Properties editor; each tab is one aspect of the selected lib entry.

Order is fixed: Lib entry, Object | Flight, Systems, Hardpoints, Damage, Materials | Graft. Tabs that do not apply to the entry type are hidden, not disabled (a PIC entry shows only Lib entry and Materials).

- Strip `gm-950`, 24px tabs, idle `ink-muted`, hover `gm-700`, active `gm-800` (it joins the panel) with an `amber` icon.
- Every tab has a `title`; Ctrl+Tab cycles tabs.
""")

comps["Outliner"] = ("Navigation", 470, f'''<div style="width:300px;border:1px solid var(--gm-1000);background:var(--gm-800)">{outliner_head()}{tree()}</div>''', """# Outliner

The lib browser: every open .LIB as a root, its entries grouped by type.

- Rows are 20px with zebra `gm-900`; hover `gm-700`; selected `amber-deep`; the active entry (the one Properties shows) adds `amber-bright` text and an `amber` icon.
- Group rows show the type badge and an entry count in `value-sm`.
- An `amber` dirty dot marks any entry, and its lib, that differs from disk.
- Header: filter field, type filter segmented buttons, and + to open another lib.
- Drag an entry from one lib onto another to copy it; drag onto an entry of the same type to open Graft with that pair.

The consumer provides the lib list, entry groups, selection, active entry, and dirty set.
""")

comps["TypeBadge"] = ("Navigation", 64, '<div class="th-row">' + "".join(f'<span class="th-badge">{t}</span>' for t in ["PT", "SH", "PIC", "JT", "OT", "NT", "PAL", "M", "T2", "11K"]) + '<span style="width:12px"></span><span class="th-badge is-ok">OK</span><span class="th-badge is-warn">2 CONFLICTS</span><span class="th-badge is-danger">CRC FAIL</span></div>', """# TypeBadge

A lib entry's file extension, set in `badge` (mono, caps) on `gm-600` with `ink-muted` text. Badges are neutral on purpose: type is read from the letters and the row icon, not color.

Status variants carry a word, never color alone: `is-ok` (`ok`), `is-warn` (`amber` on `amber-deep`), `is-danger` (`danger`).
""")

comps["Viewport"] = ("Editors", 440, f'<div style="width:760px;border:1px solid var(--gm-1000)">{viewport_head()}{viewport(760, 380, 1.05)}</div>', """# Viewport

The 3D view of the selected shape. Blender navigation, Blender colors for axes, amber for selection.

- Background `gm-950`; minor grid `gm-800`, every fifth line `gm-700`; X and Y axis lines in `axis-x` and `axis-y`.
- Mesh: outer edges `ink-muted`, inner edges `gm-500`; the selected object outline is `amber`, its origin `amber-bright`.
- Hardpoints are `steel` diamonds with `HP<n>` labels in `value-sm`.
- Overlay text (view name, entry, LOD, counts) in `value-sm` / `ink-muted`, top-left; navigation gizmo top-right.
- Tool strip floats top-left: Select, Move, Rotate, Scale | Measure, Place hardpoint.
- Header: mode select, editor menus, LOD select, overlay toggles, shading segmented control (wireframe, solid, textured).

Controls: MMB orbit, Shift+MMB pan, wheel zoom, numpad 1/3/7 front/right/top, numpad 5 ortho, Home frame all, . frame selected, G/R/S with X/Y/Z axis lock, Tab edit mode, H place hardpoint, Alt+H toggle hardpoints.
""")

comps["Timeline"] = ("Editors", 220, f'<div style="width:760px;border:1px solid var(--gm-1000)">{timeline_head()}{timeline(640)}</div>', """# Timeline

Animation tracks for the selected shape's moving parts: gear, flaps, sweep, hook, canopy, burner.

- Ruler `gm-900` with frame numbers in `value-sm`; track names in `label`.
- Keys are 8px diamonds, `ink-muted`; selected keys `amber`. A `gm-500` bar spans each motion.
- The playhead and its frame cap are `steel`, never amber, so time is never confused with selection.
- Space plays, Left/Right step a frame, I inserts a key on the hovered track, drag keys to retime.
- Shares the bottom dock with Raw fields (the entry's decoded record as an editable table) and Log.
""")

graft_rows = [("Geometry", "shape", True, "SH · 412 → 538 verts", None), ("LOD meshes", "shape", True, "3 levels", None), ("Hardpoints", "hardpoint", True, "6 → 8", "2 CONFLICTS"),
              ("Flight envelope", "flight", False, "12 fields", None), ("Propulsion", "engine", False, "5 fields", None), ("Damage model", "damage", False, "", None), ("Textures", "textured", True, "PIC × 4", None)]
def graft_body():
    rows = ""
    for name, icn, on, diff, warn in graft_rows:
        box = f'<label class="th-check{" is-on" if on else ""}"><span class="th-box">{ic("check") if on else ""}</span></label>'
        badge = f'<span class="th-badge is-warn">{warn}</span>' if warn else '<span></span>'
        rows += f'<div class="th-aspect">{box}<span class="th-row" style="gap:6px;flex-wrap:nowrap"><span class="th-muted">{ic(icn,"th-ic-sm")}</span>{name}</span><span class="th-aspect-diff">{diff}</span>{badge}</div>'
    return f'''<div class="th-graft-ends">
<div class="th-graft-end"><div class="th-graft-role">Source</div><div class="th-graft-name">F14A.PT</div><div class="th-graft-lib">ATF.LIB</div></div>
<div style="color:var(--steel);display:grid;place-items:center">{ic("chevron-right")}</div>
<div class="th-graft-end"><div class="th-graft-role">Target</div><div class="th-graft-name">F14.PT</div><div class="th-graft-lib">USNF97.LIB</div></div></div>
<div class="th-subhead">Aspects to carry over</div>
<div>{rows}</div>
<div class="th-notice is-warn" style="margin-top:6px">{ic("warning")}<span>HP3 and HP4 overlap existing target hardpoints. Keep target, take source, or offset.</span></div>
<div class="th-row" style="justify-content:flex-end;margin-top:8px"><button class="th-btn th-btn-ghost">Cancel</button><button class="th-btn">{ic("eye","th-ic-sm")}Preview in viewport</button><button class="th-btn th-btn-primary">Apply graft</button></div>'''

comps["GraftPanel"] = ("Editors", 400, f'<div style="width:400px">{panel("Graft", graft_body(), icon="graft")}</div>', """# GraftPanel

Merges chosen aspects of one lib entry into another, e.g. take ATF's F-14A geometry and hardpoints, keep USNF's flight envelope.

- Source and target slots on `gm-950` with `line-strong`; role in `section`, name in `title`-size label, lib in `value-sm`.
- One row per aspect: checkbox, icon, name, a mono diff summary (`ink-muted`), and a warning badge when the aspect conflicts.
- Conflicts are resolved before Apply: a `th-notice is-warn` lists them with the three choices (keep target, take source, offset).
- Preview in viewport draws the incoming geometry as a dashed `steel` ghost over the target.
- Apply graft is the panel's only primary button. Every graft is one undo step and marks the target dirty.

Aspects available depend on entry type: PT offers geometry (via its SH), LODs, hardpoints, envelope, propulsion, damage, textures; JT offers seeker, motor, and warhead blocks.
""")

comps["Notice"] = ("Feedback", 150, f'''<div class="th-col" style="width:440px">
<div class="th-notice is-ok">{ic("check")}<span>USNF97.LIB repacked: 1,204 entries, round-trip byte-identical for 1,201 untouched entries.</span></div>
<div class="th-notice is-warn">{ic("warning")}<span>F14.SH is referenced by 3 aircraft. Edits apply to all of them.</span></div>
<div class="th-notice is-danger">{ic("warning")}<span>MIG29.PT failed to decode at offset 0x01A4. Opened read-only.</span></div>
</div>''', """# Notice

Inline messages inside a panel or dialog. No toasts: the renderer has no overlay compositing, and messages that vanish are messages missed.

- `gm-900` surface, `gm-600` border; status variants swap the border and icon to `ok`, `amber`, or `danger`.
- Always an icon plus a sentence that says what happened and what it affects. Text stays `ink`.
""")

comps["StatusBar"] = ("Layout", 60, f'<div style="width:900px">{statusbar()}</div>', """# StatusBar

The bottom strip: context key hints on the left, the active entry and save state on the right.

- `gm-950`, 22px, `hint` style. Keys are `th-kbd` caps.
- Hints change with the hovered editor and the current mode (Edit mode shows vertex tools).
- Right side: active entry · lib, unsaved edit count with the dirty dot, and the last round-trip check result.
""")

comps["MenuBar"] = ("Layout", 60, f'<div style="width:900px">{menubar()}</div>', """# MenuBar

Top strip: app mark, menus, workspace tabs, and the active lib.

Workspaces are fixed layouts of the same editors, like Blender's: **Browse** (outliner large, raw fields), **Model** (viewport, properties, timeline), **Flight** (properties large, envelope plots), **Graft** (two viewports side by side, graft panel), **Package** (lib build list, validation log). Active tab joins the `gm-800` editor below it.
""")

# Full workspace screen
def workspace():
    W, H = 1280, 800
    return f'''<div class="th-root" style="width:{W}px;height:{H}px;display:flex;flex-direction:column;background:var(--gm-1000)">
{menubar()}
<div style="flex:1;display:flex;gap:1px;min-height:0">
  <div style="width:280px;display:flex;flex-direction:column;background:var(--gm-800)">{outliner_head()}<div style="flex:1;overflow:hidden">{tree()}</div></div>
  <div style="flex:1;display:flex;flex-direction:column;gap:1px;min-width:0">
    <div style="display:flex;flex-direction:column;flex:1;min-height:0">{viewport_head()}<div style="flex:1;position:relative">{viewport(676, 535, 1.35)}</div></div>
    <div style="height:186px;display:flex;flex-direction:column;background:var(--gm-800)">{timeline_head()}{timeline(556)}</div>
  </div>
  <div style="width:322px;display:flex;background:var(--gm-800)">{vtabs()}<div style="flex:1;display:flex;flex-direction:column;min-width:0">{props_header()}<div style="padding:6px;overflow:hidden">{flight_panels()}</div></div></div>
</div>
{statusbar()}
</div>'''

comps["Workspace"] = ("Screens", 800, workspace(), None)

for name, (group, height, body, readme) in comps.items():
    extra = ' width=1280' if name == "Workspace" else ""
    marker = f'<!-- @dsCard group="{group}" height={height}{extra} -->'
    w(P("components", name, "preview.html"), doc(marker, f"{name} preview", body, pad=(name != "Workspace"), bodycls="th-root" if name != "Workspace" else ""))
    if readme is None:
        readme = """# Workspace

The Model workspace at 1280 × 800, every component in place: outliner left, viewport center with the animation dock below, properties right with the Flight tab open on F14.PT.

Editor seams are 1px `gm-1000` gaps. Splitters between editors drag; double-click a seam to swap the editors on either side. Any editor header can switch its editor type (Outliner, Viewport, Properties, Timeline, Raw fields, Graft, Log), so users can rebuild layouts the way Blender allows. Minimum supported window: 800 × 600, where the outliner and properties collapse to their tab strips.
"""
    w(P("components", name, "README.md"), readme)

# ---------------- cover ----------------
cover = f'''<!-- @dsCard height=300 -->
<!doctype html>
<html><head><meta charset="utf-8"><title>Cover</title>{FONTS}
<style>
html,body{{margin:0;background:var(--gm-900)}}
.cv{{position:relative;width:960px;height:300px;background:var(--gm-900);overflow:hidden}}
.b1{{fill:var(--gm-700)}} .b2{{fill:var(--gm-950)}} .b3{{fill:var(--amber)}} .b4{{fill:var(--gm-600)}} .b5{{fill:var(--steel)}}
.r{{rx:var(--radius-sm)}}
.gr{{stroke:var(--gm-800);stroke-width:1}}
.dot{{fill:var(--gm-500)}}
.name{{position:absolute;left:40px;bottom:40px;max-width:440px;font:600 100px/.92 var(--font-display);color:var(--ink);letter-spacing:-.01em}}
.tag{{display:block;margin-top:10px;font:400 14px/18px var(--font-ui);color:var(--ink-muted);letter-spacing:0}}
</style></head>
<body>
<div class="cv">
<svg width="960" height="300" viewBox="0 0 960 300" style="position:absolute;inset:0">
<!-- blocks: gm-950 viewport slab 288×276, gm-700 panel 160×132, gm-600 strip 160×36, amber 64×92, steel 16×92 (radius-sm)
     arrangement: one tall slab bleeding off the right edge with satellites stacked to its left, like the editor layout
     pattern: a dot grid at space-4 pitch on the viewport slab, because the system is precise, technical, mono, a 4px grid
     steps and radii: space-1 gutters (4px), space-4 pitch (16px), radius-sm corners -->
<rect class="b1 r" x="496" y="24" width="160" height="132"/>
<rect class="b4 r" x="496" y="160" width="160" height="36"/>
<rect class="b3 r" x="496" y="200" width="64" height="76"/>
<rect class="b5 r" x="564" y="200" width="16" height="76"/>
<rect class="b4 r" x="584" y="200" width="72" height="76"/>
<rect class="b2 r" x="660" y="24" width="320" height="252"/>
{"".join(f'<circle class="dot" cx="{x}" cy="{y}" r="1.25"/>' for x in range(676, 961, 16) for y in range(40, 276, 16))}
</svg>
<div class="name">TORE Hangar<span class="tag">Lib editor for Fighters Anthology, built to run on Windows 98.</span></div>
</div>
</body></html>
'''
w(P("components", "Cover", "preview.html"), cover)

# ---------------- icons ----------------
for n in ICONS:
    w(os.path.join(OUT, "icons", f"{n}.svg"), asset_svg(n))
w(P("assets", "Icons", "README.md"), "Hangar's own 16px line icon set: 1.5px stroke, round caps and joins, drawn on a 16 grid so it rasterizes clean at 1x. Files here are inked `ink-muted` (#aab3bb); in the app and previews the same paths are drawn with currentColor (see the `th-ic` class). `play` and `solid` are the only filled glyphs. No emoji anywhere.\n")

print("ok")
