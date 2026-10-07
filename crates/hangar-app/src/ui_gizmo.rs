//! Edit Mesh transform gizmo: move arrows, plane squares and a free-move
//! circle, rotate rings, scale handles; magnetic vertex snapping; vertex
//! handles with occlusion (X-ray) and the viewport context menu. Integer
//! only: screen directions are the viewport projection (`hp_project`) of
//! world steps at the pivot, angles come from `gizmo::atan2_deg`, and every
//! release commits one undo step through `commit_points`, the writers the
//! numeric G/R/S transforms use.
use super::view::{Action, Icon, Layout};
use super::widgets::Item;
use super::*;
use core::cell::RefCell;
use hangar_core::gizmo::{self as gm, Constraint};
use hangar_core::shape_geometry as geo;

pub(super) const G_NONE: u8 = 0;
pub(super) const G_MOVE: u8 = 1;
pub(super) const G_ROTATE: u8 = 2;
pub(super) const G_SCALE: u8 = 3;
/// The viewport context menu (`App::gizmo.menu_at`).
pub(super) const MENU_GIZMO: usize = 16;
/// Screen size of the gizmo, px: arrow length, ring radius, plane square
/// offset and half size, free-move circle radius.
const ARROW: i64 = 64;
const RING: i64 = 52;
const PLANE_AT: i64 = 24;
const PLANE_HALF: i32 = 3;
const FREE: i32 = 8;
/// Pick tolerance around a handle, px.
const PICK: i64 = 6;
/// Magnetic snap reach, px at the current zoom.
pub(super) const SNAP_PX: i64 = 8;
/// Vertex handle pick radius, px.
pub(super) const HANDLE_HIT: i32 = 9;
/// Ring sample count; 48 segments read as a circle at 52 px.
const RING_STEPS: i32 = 48;
/// A gizmo handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Handle {
    /// Move along a principal axis.
    Axis(usize),
    /// Move in the plane whose normal is this axis.
    Plane(usize),
    /// Move in the view plane.
    Free,
    /// Rotate about a principal axis.
    Ring(usize),
    /// Scale along a principal axis.
    Scale(usize),
    /// Scale uniformly.
    Uniform,
}
impl Handle {
    fn op(self) -> char {
        match self {
            Handle::Axis(_) | Handle::Plane(_) | Handle::Free => 'g',
            Handle::Ring(_) => 'r',
            _ => 's',
        }
    }
    /// The axis a typed value applies to (3: none, as the G/R/S prompt).
    fn axis(self) -> usize {
        match self {
            Handle::Axis(a) | Handle::Ring(a) | Handle::Scale(a) => a,
            _ => 3,
        }
    }
}
/// A gizmo drag in progress.
pub(super) struct GizmoDrag {
    pub entry: usize,
    pub original: Entry,
    /// The committed model at the press; previews derive from it.
    pub model: Model,
    pub handle: Handle,
    pub pivot: [i32; 3],
    /// Pivot on screen at the press.
    pub centre: [i32; 2],
    pub start: [i32; 2],
    pub last: [i32; 2],
    /// Effective pointer offset from `start` in tenths of a pixel: Shift
    /// adds a tenth of each motion.
    pub eff: [i32; 2],
    pub moving: bool,
    /// Rotation: last cursor angle and the accumulated screen turn.
    pub angle: i32,
    pub turn: i32,
    /// The previewed transform and its readout.
    pub transform: Option<Transform>,
    pub readout: String,
}
/// Gizmo, magnet and X-ray state, boxed in `App`.
pub(super) struct GizmoState {
    pub mode: u8,
    pub magnet: bool,
    pub xray: bool,
    pub drag: Option<Box<GizmoDrag>>,
    pub menu_at: [i32; 2],
    /// The live snap and the unsnapped position of the snapping vertex.
    pub snap: Option<(gm::Snap, [i32; 3])>,
    /// Region-writer refusals of the selection: entry storage, offsets, reason.
    check: RefCell<Option<Checked>>,
    /// Shaded-view visibility of vertices and faces, by view key.
    occlusion: RefCell<Option<Occlusion>>,
}
/// The entry storage and selected offsets a refusal was checked for.
type Checked = (Entry, Vec<usize>, Option<String>);
/// View key, vertex visibility and face visibility.
type Occlusion = (u64, Vec<bool>, Vec<bool>);
impl Default for GizmoState {
    fn default() -> Self {
        Self {
            mode: G_MOVE,
            magnet: true,
            xray: false,
            drag: None,
            menu_at: [0; 2],
            snap: None,
            check: RefCell::new(None),
            occlusion: RefCell::new(None),
        }
    }
}
/// Screen directions at a world point: where world steps of `k` units
/// along each axis and the camera's right and up land, in px.
pub(super) struct Basis {
    pub c: [i32; 2],
    pub axes: [[i64; 2]; 3],
    pub right: [i64; 2],
    pub up: [i64; 2],
    /// Camera right and up as world vectors of length `k`.
    pub rv: [i32; 3],
    pub uv: [i32; 3],
    pub k: i64,
    /// Pixel length of `k` units in the view plane.
    pub r: i64,
    /// Each world axis's towards-the-viewer component, of 1024.
    pub camz: [i32; 3],
}
impl Basis {
    fn tip(&self, d: [i64; 2], len: i64) -> [i32; 2] {
        [
            self.c[0] + gm::div_round(d[0] * len, self.r) as i32,
            self.c[1] + gm::div_round(d[1] * len, self.r) as i32,
        ]
    }
    /// Axis `a` points nearly at the viewer: its arrow is too short to drag.
    fn foreshortened(&self, a: usize) -> bool {
        let d = self.axes[a];
        (d[0] * d[0] + d[1] * d[1]) * 100 * 100 < self.r * self.r * 15 * 15
    }
    /// A plane or ring around axis `a` is seen edge-on.
    fn edge_on(&self, a: usize) -> bool {
        self.camz[a].abs() < 205
    }
    /// Point `deg` degrees round the ring about axis `a`, radius `len` px.
    fn ring_point(&self, a: usize, deg: i32, len: i64) -> [i32; 2] {
        let (s, c) = model::sin_cos(deg);
        let (u, v) = (self.axes[(a + 1) % 3], self.axes[(a + 2) % 3]);
        let d = [
            (u[0] * c as i64 + v[0] * s as i64) / 1024,
            (u[1] * c as i64 + v[1] * s as i64) / 1024,
        ];
        self.tip(d, len)
    }
}
/// Squared distance from `p` to segment `a`-`b`, px.
fn seg_dist2(p: [i32; 2], a: [i32; 2], b: [i32; 2]) -> i64 {
    let (ab, ap) = (
        [(b[0] - a[0]) as i64, (b[1] - a[1]) as i64],
        [(p[0] - a[0]) as i64, (p[1] - a[1]) as i64],
    );
    let den = ab[0] * ab[0] + ab[1] * ab[1];
    let t = ap[0] * ab[0] + ap[1] * ab[1];
    if den == 0 || t <= 0 {
        return ap[0] * ap[0] + ap[1] * ap[1];
    }
    if t >= den {
        let bp = [(p[0] - b[0]) as i64, (p[1] - b[1]) as i64];
        return bp[0] * bp[0] + bp[1] * bp[1];
    }
    let cross = ap[0] * ab[1] - ap[1] * ab[0];
    cross * cross / den
}
/// One drawn handle: segments, filled squares and the hit square.
struct Shape {
    handle: Handle,
    axis: Option<usize>,
    lines: Vec<([i32; 2], [i32; 2])>,
    fills: Vec<[i32; 4]>,
    hit: [i32; 4],
    pickable: bool,
}
fn square(c: [i32; 2], half: i32) -> [i32; 4] {
    [c[0] - half, c[1] - half, 2 * half + 1, 2 * half + 1]
}
fn polyline(points: &[[i32; 2]]) -> Vec<([i32; 2], [i32; 2])> {
    (0..points.len())
        .map(|i| (points[i], points[(i + 1) % points.len()]))
        .collect()
}
fn circle(c: [i32; 2], r: i32) -> Vec<[i32; 2]> {
    (0..16)
        .map(|i| {
            let (s, co) = model::sin_cos(i * 360 / 16);
            [c[0] + r * co / 1024, c[1] - r * s / 1024]
        })
        .collect()
}
const AXIS_COLORS: [Rgb; 3] = [c::AXIS_X, c::AXIS_Y, c::AXIS_Z];
/// Dashed 1px line, 3 on, 3 off.
pub(super) fn dashed(d: &mut Canvas, a: [i32; 2], b: [i32; 2], color: Rgb) {
    let n = (b[0] - a[0]).abs().max((b[1] - a[1]).abs());
    if n == 0 {
        return;
    }
    let at = |t: i32| [a[0] + (b[0] - a[0]) * t / n, a[1] + (b[1] - a[1]) * t / n];
    let mut t = 0;
    while t < n {
        let (p, q) = (at(t), at((t + 3).min(n)));
        d.line(p[0], p[1], q[0], q[1], color);
        t += 6;
    }
}
/// "−3" with a typographic minus for readouts.
fn signed(v: i32) -> String {
    if v < 0 {
        format!("\u{2212}{}", -(v as i64))
    } else {
        format!("{v}")
    }
}
/// Vertex handle on screen: model vertex, position, depth (larger is
/// nearer) and whether the shaded view shows it.
#[derive(Clone, Copy, Debug)]
pub(super) struct VertexHandle {
    pub index: usize,
    pub at: [i32; 2],
    pub depth: i32,
}
impl App {
    fn view_rect(&self) -> [i32; 4] {
        [self.left() + 1, 54, self.right() - 2, self.dock_y() - 1]
    }
    /// The model the overlay shows: the live preview, else the committed one.
    fn shown_model(&self) -> Option<&Model> {
        self.preview.as_deref().or(self.model.as_ref())
    }
    pub(super) fn basis(&self, p: [i32; 3]) -> Option<Basis> {
        let (_, span) = self.model_bounds()?;
        // Long steps keep the rounding of projected pixels small; perspective
        // uses short ones so the local scale holds.
        let k = if self.perspective && !self.textured {
            (span / 8).max(16)
        } else {
            span.max(16)
        };
        let c = self.hp_project(p)?;
        let d = |v: [i32; 3]| -> Option<[i64; 2]> {
            let q = self.hp_project(core::array::from_fn(|j| p[j].saturating_add(v[j])))?;
            Some([(q[0] - c[0]) as i64, (q[1] - c[1]) as i64])
        };
        let mut axes = [[0i64; 2]; 3];
        let mut camz = [0; 3];
        for a in 0..3 {
            let mut v = [0; 3];
            v[a] = k;
            axes[a] = d(v)?;
            v[a] = 1024;
            camz[a] = self.camera_point(v)[2];
        }
        let rv = self.camera_inverse([k, 0, 0]);
        let uv = self.camera_inverse([0, k, 0]);
        let (right, up) = (d(rv)?, d(uv)?);
        let r = gm::isqrt(right[0] * right[0] + right[1] * right[1]);
        (r >= 2).then_some(Basis {
            c,
            axes,
            right,
            up,
            rv,
            uv,
            k: k as i64,
            r,
            camz,
        })
    }
    /// Why the gizmo cannot edit the selection, if it cannot: another LIB's
    /// shape, a part the preview pose rotates, or a vertex the region
    /// writer refuses (`vertex_status`).
    pub(super) fn gizmo_refusal(&self) -> Option<String> {
        if let Some(reason) = self.mesh_blocked() {
            return Some(reason);
        }
        let model = self.model.as_ref()?;
        if self
            .mesh_vertices
            .iter()
            .any(|i| *i < model.vertices.len() && !geo::unrotated(model, *i))
        {
            return Some(
                "A selected vertex is in a part the preview pose rotates; reset the pose (Parts) to edit it"
                    .into(),
            );
        }
        if model.writable {
            return None;
        }
        let entry = self.doc.archive.entries.get(self.model_entry?)?;
        let mut offsets: Vec<usize> = self
            .mesh_vertices
            .iter()
            .filter_map(|i| model.vertices.get(*i).map(|v| v.offset))
            .collect();
        offsets.sort_unstable();
        offsets.dedup();
        let mut cache = self.gizmo.check.borrow_mut();
        if let Some((e, o, why)) = cache.as_ref() {
            if e.same_storage(entry) && e.name == entry.name && *o == offsets {
                return why.clone();
            }
        }
        let why = (|| {
            let source = entry.read().ok()?;
            let g = match geo::Geometry::parse(&source) {
                Ok(g) => g,
                Err(e) => return Some(e),
            };
            for o in &offsets {
                let Some((b, i)) = g.vertex_at(*o) else {
                    return Some(format!("No stored vertex at {o:X}"));
                };
                if let Some(e) = g.vertex_status(b, i).refusal {
                    return Some(format!("Vertex at {o:X}: {e}"));
                }
            }
            None
        })();
        *cache = Some((entry.clone(), offsets, why.clone()));
        why
    }
    /// Where the gizmo sits: the selection's median, moved with a live move.
    fn gizmo_pivot(&self) -> Option<[i32; 3]> {
        if let Some(d) = &self.gizmo.drag {
            if let Some(Transform::Translate(t)) = d.transform {
                return Some(core::array::from_fn(|k| d.pivot[k] + t[k]));
            }
            return Some(d.pivot);
        }
        self.model.as_ref()?.median(&self.mesh_vertices)
    }
    /// The gizmo's handles for the current mode, or none when hidden.
    fn gizmo_shapes(&self) -> Option<(Basis, Vec<Shape>)> {
        let shown = self.mesh_edit
            && self.mode == Mode::Model
            && self.gizmo.mode != G_NONE
            && !self.mesh_vertices.is_empty()
            && self.prompt.is_none()
            && self.mesh_drag.is_none()
            && self.ed.mesh_box.is_none();
        if !shown {
            return None;
        }
        let b = self.basis(self.gizmo_pivot()?)?;
        let [l, t, r, bottom] = self.view_rect();
        if b.c[0] < l || b.c[0] > r || b.c[1] < t || b.c[1] > bottom {
            return None;
        }
        let mut out = Vec::new();
        match self.gizmo.mode {
            G_MOVE => {
                for a in 0..3 {
                    // Plane squares sit between the other two axes.
                    let (u, v) = (b.axes[(a + 1) % 3], b.axes[(a + 2) % 3]);
                    let p = [u[0] + v[0], u[1] + v[1]];
                    let at = b.tip(p, PLANE_AT);
                    out.push(Shape {
                        handle: Handle::Plane(a),
                        axis: Some(a),
                        lines: Vec::new(),
                        fills: vec![square(at, PLANE_HALF)],
                        hit: square(at, PLANE_HALF),
                        pickable: !b.edge_on(a),
                    });
                }
                for a in 0..3 {
                    let tip = b.tip(b.axes[a], ARROW);
                    let d = [(tip[0] - b.c[0]) as i64, (tip[1] - b.c[1]) as i64];
                    let n = gm::isqrt(d[0] * d[0] + d[1] * d[1]).max(1);
                    // Arrow head: a filled triangle 9 px long, 4 px each side.
                    let base = [
                        tip[0] - (d[0] * 9 / n) as i32,
                        tip[1] - (d[1] * 9 / n) as i32,
                    ];
                    let side = [(-d[1] * 4 / n) as i32, (d[0] * 4 / n) as i32];
                    let mut lines = vec![(
                        [
                            b.c[0] + (d[0] * FREE as i64 / n) as i32,
                            b.c[1] + (d[1] * FREE as i64 / n) as i32,
                        ],
                        base,
                    )];
                    for s in -4..=4 {
                        let q = [base[0] + side[0] * s / 4, base[1] + side[1] * s / 4];
                        lines.push((q, tip));
                    }
                    out.push(Shape {
                        handle: Handle::Axis(a),
                        axis: Some(a),
                        lines,
                        fills: Vec::new(),
                        hit: square(tip, 5),
                        pickable: !b.foreshortened(a),
                    });
                }
                out.push(Shape {
                    handle: Handle::Free,
                    axis: None,
                    lines: polyline(&circle(b.c, FREE)),
                    fills: Vec::new(),
                    hit: square(b.c, FREE),
                    pickable: true,
                });
            }
            G_ROTATE => {
                for a in 0..3 {
                    let points: Vec<[i32; 2]> = (0..RING_STEPS)
                        .map(|i| b.ring_point(a, i * 360 / RING_STEPS, RING))
                        .collect();
                    // The hit square sits on the ring between the other axes.
                    let at = b.ring_point(a, 45, RING);
                    out.push(Shape {
                        handle: Handle::Ring(a),
                        axis: Some(a),
                        lines: polyline(&points),
                        fills: Vec::new(),
                        hit: square(at, 4),
                        pickable: !b.edge_on(a),
                    });
                }
            }
            _ => {
                for a in 0..3 {
                    let tip = b.tip(b.axes[a], ARROW);
                    let d = [(tip[0] - b.c[0]) as i64, (tip[1] - b.c[1]) as i64];
                    let n = gm::isqrt(d[0] * d[0] + d[1] * d[1]).max(1);
                    let from = [
                        b.c[0] + (d[0] * 14 / n) as i32,
                        b.c[1] + (d[1] * 14 / n) as i32,
                    ];
                    out.push(Shape {
                        handle: Handle::Scale(a),
                        axis: Some(a),
                        lines: vec![(from, tip)],
                        fills: vec![square(tip, 3)],
                        hit: square(tip, 5),
                        pickable: !b.foreshortened(a),
                    });
                }
                out.push(Shape {
                    handle: Handle::Uniform,
                    axis: None,
                    lines: polyline(&circle(b.c, 11)),
                    fills: Vec::new(),
                    hit: square(b.c, 8),
                    pickable: true,
                });
            }
        }
        Some((b, out))
    }
    /// The gizmo handle under the pointer: the nearest within 6 px.
    pub(super) fn gizmo_pick(&self, x: i32, y: i32) -> Option<Handle> {
        let (_, shapes) = self.gizmo_shapes()?;
        let p = [x, y];
        let mut best: Option<(i64, Handle)> = None;
        for s in shapes.iter().filter(|s| s.pickable) {
            let mut d = s
                .lines
                .iter()
                .map(|(a, b)| seg_dist2(p, *a, *b))
                .min()
                .unwrap_or(i64::MAX);
            for [fx, fy, fw, fh] in s.fills.iter().chain(core::iter::once(&s.hit)) {
                if x >= *fx - 1 && y >= *fy - 1 && x <= fx + fw && y <= fy + fh {
                    d = 0;
                }
            }
            // The free circle and the uniform scale take their inside too.
            if matches!(s.handle, Handle::Free | Handle::Uniform) {
                let c = s.hit;
                let (cx, cy) = (c[0] + c[2] / 2, c[1] + c[3] / 2);
                if (x - cx).abs().max((y - cy).abs()) <= c[2] / 2 {
                    d = d.min(1);
                }
            }
            if d <= PICK * PICK && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, s.handle));
            }
        }
        best.map(|(_, h)| h)
    }
    /// Draw the gizmo, the snap target and the drag readouts.
    pub(super) fn gizmo_overlay(&self, o: &mut Layout) {
        let view = self.view_rect();
        let line = |o: &mut Layout, a: [i32; 2], b: [i32; 2], color: Rgb| {
            if let Some((a, b)) = super::clip(a, b, view) {
                o.canvas.line(a[0], a[1], b[0], b[1], color);
            }
        };
        if let Some((b, shapes)) = self.gizmo_shapes() {
            let refusal = self.gizmo_refusal();
            let hover = (self.gizmo.drag.is_none() && o.mouse[0] != i32::MIN)
                .then(|| self.gizmo_pick(o.mouse[0], o.mouse[1]))
                .flatten();
            let active = self.gizmo.drag.as_ref().map(|d| d.handle);
            for s in &shapes {
                if active.is_some_and(|h| h != s.handle) {
                    continue;
                }
                let base = s.axis.map_or(c::INK, |a| AXIS_COLORS[a]);
                let lit = hover == Some(s.handle) || active == Some(s.handle);
                let color = if refusal.is_some() || !s.pickable {
                    base.mix(c::GM_950, 150)
                } else if lit {
                    base.mix(c::INK, 110)
                } else {
                    base
                };
                // Hidden when the axis points at the viewer.
                if matches!(s.handle, Handle::Axis(a) | Handle::Scale(a) if b.foreshortened(a))
                    || matches!(s.handle, Handle::Plane(a) if b.edge_on(a))
                {
                    continue;
                }
                // Axis strokes are 2 px (parallel 1 px lines), the centre
                // circles 1 px; the hovered or dragged handle gains 1 px.
                let width = i32::from(s.axis.is_some()) + 1 + i32::from(lit && refusal.is_none());
                for (p, q) in &s.lines {
                    let horizontal = (q[0] - p[0]).abs() >= (q[1] - p[1]).abs();
                    for k in [0, 1, -1].iter().take(width as usize) {
                        let (ox, oy) = if horizontal { (0, *k) } else { (*k, 0) };
                        line(o, [p[0] + ox, p[1] + oy], [q[0] + ox, q[1] + oy], color);
                    }
                }
                for [x, y, w, h] in &s.fills {
                    if *x >= view[0] && *y >= view[1] && x + w <= view[2] && y + h <= view[3] {
                        o.canvas.rect(*x, *y, *w, *h, color);
                    }
                }
                let [hx, hy, hw, hh] = s.hit;
                if s.pickable
                    && hx >= view[0] + 40
                    && hy >= view[1]
                    && hx + hw <= view[2]
                    && hy + hh <= view[3]
                {
                    o.hit(s.hit, Action::Gizmo(s.handle));
                }
            }
            if let (Some(why), Some(_)) = (&refusal, hover) {
                let text = fit(why, view[2] - b.c[0] - 16, Style::ValueSm);
                o.canvas
                    .styled(b.c[0] + 12, b.c[1] - 12, &text, c::AMBER, Style::ValueSm);
            }
        }
        // Drag feedback: ghost, sweep, cursor line and readout.
        if let Some(d) = &self.gizmo.drag {
            if d.moving {
                self.ghost(o, &d.model);
                let cursor = self.mouse;
                if let Handle::Ring(a) = d.handle {
                    self.sweep(o, d, AXIS_COLORS[a]);
                }
                if matches!(
                    d.handle,
                    Handle::Ring(_) | Handle::Scale(_) | Handle::Uniform
                ) {
                    dashed(&mut o.canvas, d.centre, cursor, c::INK_FAINT);
                }
                self.readout(o, &d.readout);
            }
        } else if let Some(d) = self.mesh_drag.as_ref().filter(|d| d.moving) {
            self.ghost(o, &d.model);
        }
        if let Some((s, from)) = &self.gizmo.snap {
            if let (Some(v), Some(p)) = (self.hp_project(s.vertex), self.hp_project(*from)) {
                if v[0] > view[0] + 6
                    && v[0] < view[2] - 6
                    && v[1] > view[1] + 6
                    && v[1] < view[3] - 6
                {
                    super::chrome::ring(&mut o.canvas, v[0], v[1], 6, c::AMBER_BRIGHT, c::GM_1000);
                    super::chrome::ring(&mut o.canvas, v[0], v[1], 3, c::AMBER_BRIGHT, c::AMBER);
                }
                if let Some((a, b)) = super::clip(p, v, view) {
                    dashed(&mut o.canvas, a, b, c::AMBER_BRIGHT);
                }
            }
        }
    }
    /// Dim outline of the faces the drag moves, where they were.
    fn ghost(&self, o: &mut Layout, model: &Model) {
        let view = self.view_rect();
        let mut moving = vec![false; model.vertices.len()];
        for i in &self.mesh_vertices {
            if let Some(m) = moving.get_mut(*i) {
                *m = true;
            }
        }
        for f in model
            .faces
            .iter()
            .filter(|f| f.indices.iter().any(|i| moving.get(*i) == Some(&true)))
            .take(4096)
        {
            let pts: Vec<Option<[i32; 2]>> = f
                .indices
                .iter()
                .map(|i| self.hp_project(model.vertices.get(*i)?.point))
                .collect();
            for k in 0..pts.len() {
                if let (Some(a), Some(b)) = (pts[k], pts[(k + 1) % pts.len()]) {
                    if let Some((a, b)) = super::clip(a, b, view) {
                        o.canvas.line(a[0], a[1], b[0], b[1], c::INK_FAINT);
                    }
                }
            }
        }
    }
    /// Filled sweep of the rotation from its start angle, as spokes.
    fn sweep(&self, o: &mut Layout, d: &GizmoDrag, color: Rgb) {
        let turn = d.turn.clamp(-360, 360);
        let start = gm::atan2_deg(
            -(d.start[1] - d.centre[1]) as i64,
            (d.start[0] - d.centre[0]) as i64,
        );
        let view = self.view_rect();
        let dim = color.mix(c::GM_950, 170);
        let spoke = |o: &mut Layout, deg: i32, color: Rgb| {
            let (s, co) = model::sin_cos(deg);
            let r = RING as i32 / 2;
            let p = [d.centre[0] + r * co / 1024, d.centre[1] - r * s / 1024];
            if let Some((a, b)) = super::clip(d.centre, p, view) {
                o.canvas.line(a[0], a[1], b[0], b[1], color);
            }
        };
        let step = if turn < 0 { -1 } else { 1 };
        let mut t = 0;
        while t != turn {
            spoke(o, start + t, dim);
            t += step;
        }
        spoke(o, start, color);
        spoke(o, start + turn, color);
    }
    /// The live readout beside the cursor.
    fn readout(&self, o: &mut Layout, text: &str) {
        let view = self.view_rect();
        let w = text_width(text, Style::ValueSm) + 8;
        let x = (self.mouse[0] + 16).min(view[2] - w - 2).max(view[0] + 2);
        let y = (self.mouse[1] + 18).min(view[3] - 18).max(view[1] + 2);
        o.canvas.rect(x, y, w, 16, c::GM_950);
        view::border(&mut o.canvas, x, y, w, 16, c::GM_600);
        o.canvas.styled(x + 4, y + 12, text, c::INK, Style::ValueSm);
    }
    /// Press on a gizmo handle: refuse with the reason, or start the drag.
    pub(super) fn gizmo_press(&mut self, h: Handle) {
        self.gizmo.snap = None;
        if let Some(why) = self.gizmo_refusal() {
            self.status = format!("Gizmo unavailable: {why}");
            self.ed.mesh_refusal = Some(why);
            return;
        }
        let (Some(entry), Some(model)) = (self.model_entry, self.model.as_ref()) else {
            return;
        };
        let Some(pivot) = model.median(&self.mesh_vertices) else {
            return;
        };
        let Some(centre) = self.hp_project(pivot) else {
            return;
        };
        let original = self.doc.archive.entries[entry].clone();
        let model = model.clone();
        let angle = gm::atan2_deg(
            -(self.mouse[1] - centre[1]) as i64,
            (self.mouse[0] - centre[0]) as i64,
        );
        self.gizmo.drag = Some(Box::new(GizmoDrag {
            entry,
            original,
            model,
            handle: h,
            pivot,
            centre,
            start: self.mouse,
            last: self.mouse,
            eff: [0; 2],
            moving: false,
            angle,
            turn: 0,
            transform: None,
            readout: String::new(),
        }));
        self.ed.mesh_refusal = None;
        let what = match h {
            Handle::Axis(a) => format!("Move along {}", ['X', 'Y', 'Z'][a]),
            Handle::Plane(a) => format!("Move in the {} plane", ["YZ", "ZX", "XY"][a]),
            Handle::Free => "Move in the view plane".into(),
            Handle::Ring(a) => format!("Rotate about {}", ['X', 'Y', 'Z'][a]),
            Handle::Scale(a) => format!("Scale along {}", ['X', 'Y', 'Z'][a]),
            Handle::Uniform => "Scale".into(),
        };
        self.status = format!(
            "{what}: Shift fine, Ctrl steps, Alt skips snapping, type a value, Esc or right-click cancels"
        );
    }
    /// Pointer motion during a gizmo drag: rebuild the preview.
    pub(super) fn gizmo_motion(&mut self, x: i32, y: i32, shift: bool) {
        let Some(mut d) = self.gizmo.drag.take() else {
            return;
        };
        let step = [x - d.last[0], y - d.last[1]];
        d.last = [x, y];
        let scale = if shift { 1 } else { 10 };
        d.eff = [d.eff[0] + step[0] * scale, d.eff[1] + step[1] * scale];
        if !d.moving && (x - d.start[0]).abs().max((y - d.start[1]).abs()) < 3 {
            self.gizmo.drag = Some(d);
            return;
        }
        d.moving = true;
        let result = self.gizmo_compute(&mut d);
        match result {
            Ok((t, preview, text)) => {
                d.transform = Some(t);
                d.readout = text.clone();
                self.preview = Some(Box::new(preview));
                self.status = text;
            }
            // Out of range (16-bit coordinates) or refused: nothing to
            // release onto until the pointer comes back.
            Err(e) => {
                d.transform = None;
                self.preview = None;
                self.status = format!("Error: {e}");
            }
        }
        self.gizmo.drag = Some(d);
    }
    /// The transform, preview and readout for the drag's pointer.
    fn gizmo_compute(&mut self, d: &mut GizmoDrag) -> Result<(Transform, Model, String)> {
        let ctrl = self.ctrl;
        let alt = self.replace.alt;
        let b = self.basis(d.pivot).ok_or("The pivot is off the view")?;
        // Effective pointer, tenths of a pixel from the press.
        let m = [d.eff[0] as i64, d.eff[1] as i64];
        let cursor = [
            d.start[0] + gm::div_round(m[0], 10) as i32,
            d.start[1] + gm::div_round(m[1], 10) as i32,
        ];
        self.gizmo.snap = None;
        let (t, text) = match d.handle {
            Handle::Axis(_) | Handle::Plane(_) | Handle::Free => {
                let mut delta = [0i32; 3];
                match d.handle {
                    Handle::Axis(a) => {
                        delta[a] = gm::div_round(gm::along(m, b.axes[a], b.k), 10) as i32;
                    }
                    Handle::Plane(n) => {
                        let (u, v) = ((n + 1) % 3, (n + 2) % 3);
                        let s = gm::solve2(m, b.axes[u], b.axes[v], b.k)
                            .ok_or("The plane is edge-on; orbit the view")?;
                        delta[u] = gm::div_round(s[0], 10) as i32;
                        delta[v] = gm::div_round(s[1], 10) as i32;
                    }
                    _ => {
                        let s = gm::solve2(m, b.right, b.up, b.k).ok_or("Degenerate view")?;
                        for (j, x) in delta.iter_mut().enumerate() {
                            *x = gm::div_round(
                                s[0] * b.rv[j] as i64 + s[1] * b.uv[j] as i64,
                                b.k * 10,
                            ) as i32;
                        }
                    }
                }
                if ctrl {
                    delta = delta.map(|v| gm::step(v, 10));
                }
                let constraint = match d.handle {
                    Handle::Axis(a) => Constraint::Axis(a),
                    Handle::Plane(n) => Constraint::Plane(n),
                    _ => Constraint::View(self.camera_inverse([0, 0, 1024])),
                };
                let (delta, note) = if self.gizmo.magnet && !alt {
                    self.snap_delta(&d.model, delta, constraint, d.last)
                } else {
                    (delta, None)
                };
                let mut text = format!(
                    "Move X {} \u{b7} Y {} \u{b7} Z {} (source units)",
                    signed(delta[0]),
                    signed(delta[1]),
                    signed(delta[2])
                );
                if let Some(note) = note {
                    text = format!("{text}. {note}");
                } else if alt && self.gizmo.magnet {
                    text.push_str(". Snapping off while Alt is held");
                }
                (Transform::Translate(delta), text)
            }
            Handle::Ring(a) => {
                let now = gm::atan2_deg(
                    -(cursor[1] - d.centre[1]) as i64,
                    (cursor[0] - d.centre[0]) as i64,
                );
                d.turn += gm::angle_delta(d.angle, now);
                d.angle = now;
                // A counter-clockwise turn on screen is positive about an
                // axis that points at the viewer.
                let mut deg = if b.camz[a] >= 0 { d.turn } else { -d.turn };
                if ctrl {
                    deg = gm::step(deg, 15);
                }
                (
                    Transform::Rotate(a, deg),
                    format!("Rotate {} {}\u{b0}", ['X', 'Y', 'Z'][a], signed(deg)),
                )
            }
            Handle::Scale(_) | Handle::Uniform => {
                let rel = |p: [i32; 2]| [(p[0] - d.centre[0]) as i64, (p[1] - d.centre[1]) as i64];
                let (s0, s1) = match d.handle {
                    Handle::Scale(a) => {
                        let u = b.axes[a];
                        let n = gm::isqrt(u[0] * u[0] + u[1] * u[1]).max(1);
                        let dot = |v: [i64; 2]| (v[0] * u[0] + v[1] * u[1]) / n;
                        (dot(rel(d.start)), dot(rel(cursor)))
                    }
                    _ => {
                        let len = |v: [i64; 2]| gm::isqrt(v[0] * v[0] + v[1] * v[1]);
                        (len(rel(d.start)), len(rel(cursor)))
                    }
                };
                // A press near the centre scales by 30 px per 100%.
                let s0 = s0.max(30);
                let mut pct = gm::div_round(s1 * 100, s0).clamp(1, 10000) as i32;
                if ctrl {
                    pct = gm::step(pct, 10).max(10);
                }
                match d.handle {
                    Handle::Scale(a) => (
                        Transform::Scale(Some(a), pct),
                        format!("Scale {} {pct}%", ['X', 'Y', 'Z'][a]),
                    ),
                    _ => (Transform::Scale(None, pct), format!("Scale {pct}%")),
                }
            }
        };
        let preview = self.edit_transform(&d.model, d.handle.op(), t)?;
        Ok((t, preview, text))
    }
    /// Snap a move: the moving vertex nearest the pointer lands on the best
    /// target under `gizmo::snap_search`. Returns the delta and a note.
    pub(super) fn snap_delta(
        &mut self,
        base: &Model,
        delta: [i32; 3],
        constraint: Constraint,
        cursor: [i32; 2],
    ) -> ([i32; 3], Option<String>) {
        self.gizmo.snap = None;
        let moving: Vec<usize> = self
            .mesh_vertices
            .iter()
            .copied()
            .filter(|i| *i < base.vertices.len())
            .collect();
        let shifted = |p: [i32; 3]| -> [i32; 3] { core::array::from_fn(|k| p[k] + delta[k]) };
        let Some(snapper) = moving
            .iter()
            .filter_map(|i| {
                let p = self.hp_project(shifted(base.vertices[*i].point))?;
                let d = (p[0] - cursor[0]) as i64;
                let e = (p[1] - cursor[1]) as i64;
                Some((d * d + e * e, *i))
            })
            .min()
            .map(|(_, i)| i)
        else {
            return (delta, None);
        };
        let p0 = base.vertices[snapper].point;
        let Some(b) = self.basis(p0) else {
            return (delta, None);
        };
        let tolerance = gm::div_round(SNAP_PX * b.k, b.r).max(1) as i32;
        let mut offsets: Vec<usize> = moving.iter().map(|i| base.vertices[*i].offset).collect();
        offsets.sort_unstable();
        let mut starts: Vec<[i32; 3]> = moving.iter().map(|i| base.vertices[*i].point).collect();
        starts.sort_unstable();
        let mut used = vec![false; base.vertices.len()];
        for f in &base.faces {
            for i in &f.indices {
                if let Some(u) = used.get_mut(*i) {
                    *u = true;
                }
            }
        }
        let view = self.view_rect();
        let (center, span) = self.model_bounds().unwrap_or(([0; 3], 1));
        let candidates: Vec<(usize, [i32; 3])> = base
            .vertices
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                used[*i]
                    && offsets.binary_search(&v.offset).is_err()
                    // Copies of a moving vertex never pull it back.
                    && starts.binary_search(&v.point).is_err()
            })
            .filter(|(_, v)| {
                // Behind the perspective camera, or off the view: not a target.
                let z = self.camera_point(core::array::from_fn(|k| v.point[k] - center[k]))[2];
                !(self.perspective && !self.textured && z >= span * 4)
                    && self.hp_project(v.point).is_some_and(|[x, y]| {
                        x >= view[0] && x <= view[2] && y >= view[1] && y <= view[3]
                    })
            })
            .map(|(i, v)| (i, v.point))
            .collect();
        let Some(s) = gm::snap_search(constraint, p0, shifted(p0), &candidates, tolerance) else {
            return (delta, None);
        };
        let snapped: [i32; 3] = core::array::from_fn(|k| s.point[k] - p0[k]);
        let [x, y, z] = s.vertex;
        let note = if s.off == 0 {
            format!("Snapped to vertex at ({x}, {y}, {z})")
        } else {
            format!(
                "Snapped to vertex at ({x}, {y}, {z}) along the {}; {} off it, not coincident",
                match constraint {
                    Constraint::Axis(a) => ["X axis", "Y axis", "Z axis"][a.min(2)],
                    _ => "plane",
                },
                view::count(s.off as usize, "unit", "units")
            )
        };
        self.gizmo.snap = Some((s, shifted(p0)));
        (snapped, Some(note))
    }
    /// Release: commit the preview as one undo step.
    pub(super) fn finish_gizmo(&mut self) -> Result<()> {
        let Some(d) = self.gizmo.drag.take() else {
            return Ok(());
        };
        self.gizmo.snap = None;
        let preview = self.preview.take();
        if !d.moving {
            return Ok(());
        }
        let Some(after) = preview else {
            return Err(self.status.trim_start_matches("Error: ").to_string());
        };
        if !self
            .doc
            .archive
            .entries
            .get(d.entry)
            .is_some_and(|e| e.same_storage(&d.original))
        {
            return Err("Shape changed during drag".into());
        }
        let n = self.commit_points(&d.model, &after)?;
        self.status = if n == 0 {
            "Nothing moved".into()
        } else {
            format!(
                "{}: {} changed. One undo step.",
                d.readout,
                view::count(n, "stored vertex", "stored vertices")
            )
        };
        Ok(())
    }
    /// Esc, Ctrl+Z or right-click during a drag: restore and say so.
    pub(super) fn cancel_gizmo(&mut self) {
        self.gizmo.drag = None;
        self.gizmo.snap = None;
        self.preview = None;
        self.status = "Transform cancelled; nothing changed".into();
    }
    /// A digit or minus during a gizmo drag: continue as the numeric G/R/S
    /// prompt on the dragged axis, as Blender does.
    pub(super) fn gizmo_typed(&mut self, ch: char) {
        let Some(d) = self.gizmo.drag.take() else {
            return;
        };
        self.gizmo.snap = None;
        self.preview = None;
        self.mesh_transform_prompt(d.handle.op());
        if let Some(p) = self.prompt.as_mut() {
            p.axis = d.handle.axis();
            p.value.push(ch);
        }
        self.transform_preview();
    }
    pub(super) fn gizmo_mode(&mut self, mode: u8) {
        self.gizmo.mode = mode.min(G_SCALE);
        self.status = match self.gizmo.mode {
            G_MOVE => "Move gizmo: drag an arrow, a plane square or the centre",
            G_ROTATE => "Rotate gizmo: drag a ring; Ctrl snaps to 15\u{b0}",
            G_SCALE => "Scale gizmo: drag an axis square or the centre; Ctrl snaps to 10%",
            _ => "Gizmo off: G, R and S still transform",
        }
        .into();
    }
    pub(super) fn toggle_magnet(&mut self) {
        self.gizmo.magnet = !self.gizmo.magnet;
        self.status = if self.gizmo.magnet {
            "Snap to vertices on: moves snap within 8 px; hold Alt to move freely"
        } else {
            "Snap to vertices off"
        }
        .into();
    }
    pub(super) fn toggle_xray(&mut self) {
        self.gizmo.xray = !self.gizmo.xray;
        self.status = if self.gizmo.xray {
            "X-ray on: hidden vertices can be picked and boxed (Alt+Z)"
        } else {
            "X-ray off: vertices behind faces are dimmed and skipped (Alt+Z)"
        }
        .into();
    }
    /// Right-click in the Edit Mesh viewport while idle.
    pub(super) fn open_gizmo_menu(&mut self, x: i32, y: i32) {
        self.gizmo.menu_at = [x, y];
        self.menu = Some(MENU_GIZMO);
    }
    pub(super) fn gizmo_menu_items(&self) -> Vec<Item<'static>> {
        vec![
            Item::new("Move", Action::GizmoMode(G_MOVE))
                .icon(Icon::Move)
                .on(self.gizmo.mode == G_MOVE),
            Item::new("Rotate", Action::GizmoMode(G_ROTATE))
                .icon(Icon::Rotate)
                .on(self.gizmo.mode == G_ROTATE),
            Item::new("Scale", Action::GizmoMode(G_SCALE))
                .icon(Icon::Scale)
                .on(self.gizmo.mode == G_SCALE),
            Item::sep(),
            Item::new("Snap to vertices", Action::Magnet)
                .icon(Icon::Magnet)
                .on(self.gizmo.magnet),
            Item::new("Pivot: Median", Action::Pivot(false)).on(!self.ed.pivot_individual),
            Item::new("Pivot: Individual", Action::Pivot(true)).on(self.ed.pivot_individual),
            Item::sep(),
            Item::new("Cancel", Action::Cancel),
        ]
    }
    /// A hash of everything the shaded raster depends on.
    fn view_key(&self, m: &Model) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: i64| {
            h ^= v as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        };
        for v in [
            self.yaw,
            self.pitch,
            self.zoom,
            self.pan[0],
            self.pan[1],
            self.width,
            self.height,
            self.left(),
            self.right(),
            self.dock_y(),
            self.flat as i32,
            m.faces.len() as i32,
        ] {
            eat(v as i64);
        }
        for v in &m.vertices {
            eat(v.point[0] as i64 | (v.point[1] as i64) << 20 | (v.point[2] as i64) << 40);
        }
        h
    }
    /// Which vertices and faces the shaded view shows. None in wireframe or
    /// with X-ray on, where every corner of a drawn face counts as shown.
    /// A face shows when it has pixels in the raster's face buffer. A
    /// vertex shows when a shown face has it as a corner and it is not
    /// occluded: a pixel within one raster px of it is drawn by such a face
    /// (it lies on that face's visible boundary), or nothing nearer is drawn
    /// at its pixel (depth test with two raster px of tolerance).
    pub(super) fn occlusion(&self) -> Option<(Vec<bool>, Vec<bool>)> {
        if !self.textured || self.gizmo.xray {
            return None;
        }
        let m = self.shown_model()?;
        let key = self.view_key(m);
        if let Some((k, v, f)) = self.gizmo.occlusion.borrow().as_ref() {
            if *k == key {
                return Some((v.clone(), f.clone()));
            }
        }
        let [w, h] = self.view_size();
        let rw = (w.max(1) as usize).min(512);
        let rh = (h.max(1) as usize * rw / w.max(1) as usize).max(1);
        let frame = self.raster(rw, rh);
        let view = self.raster_frame(self.view_size())?;
        let mut faces = vec![false; m.faces.len()];
        for f in &frame.faces {
            if let Some(v) = faces.get_mut(*f) {
                *v = true;
            }
        }
        let mut cornered = vec![false; m.vertices.len()];
        for (f, shown) in m.faces.iter().zip(&faces) {
            for i in f.indices.iter().filter(|_| *shown) {
                if let Some(c) = cornered.get_mut(*i) {
                    *c = true;
                }
            }
        }
        let tolerance = (2 * view.units_per_px(rw)).max(2 * model::VIEW_FIXED);
        let corner = |f: usize, p: [i32; 3]| {
            faces.get(f) == Some(&true)
                && m.faces[f]
                    .indices
                    .iter()
                    .any(|c| m.vertices.get(*c).is_some_and(|c| c.point == p))
        };
        let verts: Vec<bool> = m
            .vertices
            .iter()
            .zip(&cornered)
            .map(|(v, cornered)| {
                if !cornered {
                    return false;
                }
                let [px, py, z] = view.raster16(v.point, [rw, rh]);
                let (x, y) = (px.div_euclid(16), py.div_euclid(16));
                if x < 0 || y < 0 || x >= rw as i32 || y >= rh as i32 {
                    // Off the raster (and the viewport): its faces decide.
                    return true;
                }
                let on_boundary = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (x, y) = (x + dx, y + dy);
                        x >= 0
                            && y >= 0
                            && x < rw as i32
                            && y < rh as i32
                            && corner(frame.faces[y as usize * rw + x as usize], v.point)
                    })
                });
                let i = y as usize * rw + x as usize;
                on_boundary
                    || frame.faces[i] == usize::MAX
                    || z as i64 >= frame.depth[i] - tolerance
            })
            .collect();
        *self.gizmo.occlusion.borrow_mut() = Some((key, verts.clone(), faces.clone()));
        Some((verts, faces))
    }
    /// Vertex handles: one per screen position and point, only for shown
    /// corners (`occlusion`; every corner of a drawn face with X-ray or in
    /// wireframe) inside the viewport.
    pub(super) fn vertex_handles(&self) -> Vec<VertexHandle> {
        let mut all = self.handle_points();
        // Copies of a vertex in several frames or buffers draw once.
        all.sort_unstable_by_key(|(h, p)| (h.at, *p, h.index));
        all.dedup_by_key(|(h, p)| (h.at, *p));
        all.into_iter().map(|(h, _)| h).collect()
    }
    /// Every shown corner inside the viewport with its point; box select
    /// takes all copies, so it uses these rather than the handles.
    pub(super) fn handle_points(&self) -> Vec<(VertexHandle, [i32; 3])> {
        let Some(model) = self.shown_model() else {
            return Vec::new();
        };
        let visible = self.occlusion().map(|(v, _)| v);
        let mut used = vec![false; model.vertices.len()];
        for f in &model.faces {
            for i in &f.indices {
                if let Some(u) = used.get_mut(*i) {
                    *u = true;
                }
            }
        }
        let inside = |[x, y]: [i32; 2]| {
            x >= self.left() + 40 && x < self.right() - 8 && y >= 60 && y < self.dock_y() - 10
        };
        let Some(frame) = self.view_frame(self.view_size()) else {
            return Vec::new();
        };
        // A loose vertex (no face uses it, as one Add vertex just placed)
        // shows while it is selected.
        let mut selected = vec![false; model.vertices.len()];
        for i in &self.mesh_vertices {
            if let Some(s) = selected.get_mut(*i) {
                *s = true;
            }
        }
        let mut all: Vec<(VertexHandle, [i32; 3])> = Vec::new();
        for (i, v) in model.vertices.iter().enumerate() {
            let shown = if used[i] {
                visible.as_ref().is_none_or(|s| s.get(i) == Some(&true))
            } else {
                selected[i]
            };
            if !shown {
                continue;
            }
            let Some(at) = self.hp_project(v.point).filter(|p| inside(*p)) else {
                continue;
            };
            all.push((
                VertexHandle {
                    index: i,
                    at,
                    depth: frame.project16(v.point)[2] as i32,
                },
                v.point,
            ));
        }
        all
    }
    /// The vertex handle nearest the pointer within `HANDLE_HIT` px; ties go
    /// to the nearer vertex in depth. Hidden vertices only with X-ray.
    pub(super) fn pick_handle(&self, x: i32, y: i32) -> Option<usize> {
        self.vertex_handles()
            .into_iter()
            .filter_map(|h| {
                let (dx, dy) = ((h.at[0] - x) as i64, (h.at[1] - y) as i64);
                let d = dx * dx + dy * dy;
                (d <= (HANDLE_HIT * HANDLE_HIT) as i64).then_some((d, -h.depth, h.index))
            })
            .min()
            .map(|(.., i)| i)
    }
    /// Vertex handles over the mesh: ink squares with a gm-1000 keyline,
    /// selected amber with an amber-bright ring, the active one 2 px larger,
    /// the hovered one outlined. Hidden vertices draw nothing.
    pub(super) fn draw_vertex_handles(&self, o: &mut Layout) {
        let handles = self.vertex_handles();
        let hover = (o.mouse[0] != i32::MIN
            && self.mesh_drag.is_none()
            && self.gizmo.drag.is_none()
            && self.ed.mesh_box.is_none())
        .then(|| {
            if self.gizmo_pick(o.mouse[0], o.mouse[1]).is_some() {
                None
            } else {
                self.pick_handle(o.mouse[0], o.mouse[1])
            }
        })
        .flatten();
        let mut selected = vec![false; self.shown_model().map_or(0, |m| m.vertices.len())];
        for i in &self.mesh_vertices {
            if let Some(s) = selected.get_mut(*i) {
                *s = true;
            }
        }
        let active = self.mesh_vertices.first().copied();
        // Crowded: another shown handle within 6 px. Crowded unselected
        // handles draw 2 px smaller so dense meshes stay readable.
        let mut order: Vec<usize> = (0..handles.len()).collect();
        order.sort_unstable_by_key(|k| handles[*k].at);
        let mut crowded = vec![false; handles.len()];
        for (n, k) in order.iter().enumerate() {
            let a = handles[*k].at;
            for j in &order[n + 1..] {
                let b = handles[*j].at;
                if b[0] - a[0] > 6 {
                    break;
                }
                if (b[1] - a[1]).abs() <= 6 {
                    crowded[*k] = true;
                    crowded[*j] = true;
                }
            }
        }
        // Unselected first, then selected, so selections stay on top.
        for pass in 1..3 {
            for (k, h) in handles.iter().enumerate() {
                let on = selected.get(h.index) == Some(&true);
                if 1 + usize::from(on) != pass {
                    continue;
                }
                let [x, y] = h.at;
                // 7 px with the keyline (5 when crowded); selected 9, the
                // active vertex 11.
                let hovered = hover == Some(h.index);
                let half = if on {
                    4 + i32::from(active == Some(h.index))
                } else {
                    3 - i32::from(crowded[k] && !hovered)
                };
                o.canvas
                    .rect(x - half, y - half, 2 * half + 1, 2 * half + 1, c::GM_1000);
                if on {
                    o.canvas.rect(
                        x - half + 1,
                        y - half + 1,
                        2 * half - 1,
                        2 * half - 1,
                        c::AMBER_BRIGHT,
                    );
                    // The fill: 5 px, 7 px for the active vertex.
                    o.canvas.rect(
                        x - half + 2,
                        y - half + 2,
                        2 * half - 3,
                        2 * half - 3,
                        c::AMBER,
                    );
                } else {
                    o.canvas.rect(
                        x - half + 1,
                        y - half + 1,
                        2 * half - 1,
                        2 * half - 1,
                        c::INK,
                    );
                }
                if hovered {
                    view::border(
                        &mut o.canvas,
                        x - half - 2,
                        y - half - 2,
                        2 * half + 5,
                        2 * half + 5,
                        c::INK,
                    );
                }
                o.hit(
                    [
                        x - HANDLE_HIT,
                        y - HANDLE_HIT,
                        2 * HANDLE_HIT + 1,
                        2 * HANDLE_HIT + 1,
                    ],
                    Action::MeshVertex(h.index),
                );
            }
        }
    }
}

#[cfg(not(windows))]
impl App {
    /// Snapshot states (`--native-snapshot OUT LIB SH gizmo-...`): Edit Mesh
    /// in Solid shading with one face's corners selected and the gizmo mode
    /// in the name; `-drag` holds a drag on its first handle, `-wire` uses
    /// Wireframe, `-xray` turns X-ray on, `-hover` points at a handle, `-800`
    /// renders at 800 x 600 (otherwise 1280 x 800). `gizmo-snap` drags the
    /// demo shape's tail vertex up Y onto the nose vertex.
    pub(super) fn snapshot_gizmo(&mut self, name: &str) -> Result<()> {
        (self.width, self.height) = if name.ends_with("-800") {
            (800, 600)
        } else {
            (1280, 800)
        };
        if name.contains("snap") && self.path.starts_with("Synthetic") {
            self.doc.replace(0, model::demo_shape())?;
            self.doc.mark_saved();
            self.select_entry(0);
        }
        self.mode = Mode::Model;
        if !self.mesh_edit {
            self.act(Action::MeshMode);
        }
        self.textured = !name.contains("wire");
        self.flat = true;
        self.perspective = false;
        self.gizmo.xray = name.contains("xray");
        self.yaw = 35;
        self.pitch = 25;
        self.ed.face_select = false;
        let mode = if name.contains("rotate") {
            G_ROTATE
        } else if name.contains("scale") {
            G_SCALE
        } else if name.contains("move") || name.contains("snap") {
            G_MOVE
        } else {
            G_NONE
        };
        self.gizmo.mode = mode;
        if name.contains("snap") {
            // The demo shape from above: the tail (0, -70, 0) dragged up the
            // Y arrow to 4 px short of the nose snaps onto it.
            self.yaw = 0;
            self.pitch = 90;
            self.textured = false;
            self.mesh_vertices = vec![5];
            let hit = self
                .layout()
                .hits
                .into_iter()
                .find(|h| matches!(h.action, Action::Gizmo(Handle::Axis(1))))
                .ok_or("No Y arrow")?
                .rect;
            let (x, y) = (hit[0] + hit[2] / 2, hit[1] + hit[3] / 2);
            let model = self.model.as_ref().ok_or("No model")?;
            let tail = self.hp_project(model.vertices[5].point).ok_or("Tail")?;
            let nose = self.hp_project(model.vertices[0].point).ok_or("Nose")?;
            let short = if nose[1] > tail[1] { -4 } else { 4 };
            self.motion(x, y, false);
            self.pointer(x, y, 1, true, false);
            self.motion(x, y + (nose[1] - tail[1]) / 2, false);
            self.motion(x + 6, y + (nose[1] - tail[1]) + short, false);
            return Ok(());
        }
        // The face drawn a fifth of the way right of the viewport centre.
        let (cx, cy) = (
            (self.left() + self.right()) / 2 + (self.right() - self.left()) / 5,
            (54 + self.dock_y()) / 2,
        );
        let face = (0..40)
            .find_map(|k| self.pick_face(cx - k * 4, cy + k))
            .ok_or("No face near the centre")?;
        let model = self.model.as_ref().ok_or("No model")?;
        self.mesh_vertices = model.faces[face].indices.clone();
        let first = self
            .layout()
            .hits
            .into_iter()
            .find(|h| matches!(h.action, Action::Gizmo(_)))
            .map(|h| h.rect);
        if let Some(r) = first {
            let (x, y) = (r[0] + r[2] / 2, r[1] + r[3] / 2);
            if name.contains("drag") {
                self.motion(x, y, false);
                self.pointer(x, y, 1, true, false);
                let (dx, dy) = match mode {
                    G_ROTATE => (-30, -40),
                    G_SCALE => (24, 10),
                    _ => (40, 18),
                };
                self.motion(x + dx / 2, y + dy / 2, false);
                self.motion(x + dx, y + dy, false);
            } else if name.contains("hover") {
                self.motion(x, y, false);
            }
        }
        if name.contains("hover") && mode == G_NONE {
            if let Some(h) = self.vertex_handles().into_iter().next() {
                self.motion(h.at[0] + 3, h.at[1] + 2, false);
            }
        }
        Ok(())
    }
}

/// The demo LIB with the writable demo shape in Edit Mesh, wireframe,
/// orthographic, at `w` x `h`.
#[inline(never)]
fn gizmo_app(w: i32, h: i32) -> Box<App> {
    let mut a = Box::new(App::new());
    a.demo();
    a.doc.replace(0, model::demo_shape()).unwrap();
    a.doc.mark_saved();
    a.select_entry(0);
    a.mode = Mode::Model;
    a.width = w;
    a.height = h;
    a.textured = false;
    a.perspective = false;
    a.yaw = 35;
    a.pitch = 25;
    a.act(Action::MeshMode);
    a
}
fn points(a: &App) -> Vec<[i32; 3]> {
    a.model
        .as_ref()
        .unwrap()
        .vertices
        .iter()
        .map(|v| v.point)
        .collect()
}
impl App {
    /// Centre of the hit region of gizmo handle `h`.
    fn smoke_handle(&self, h: Handle) -> [i32; 2] {
        let r = self
            .layout()
            .hits
            .into_iter()
            .rev()
            .find(|x| matches!(x.action, Action::Gizmo(g) if g == h))
            .unwrap_or_else(|| panic!("Gizmo handle {h:?} missing"))
            .rect;
        [r[0] + r[2] / 2, r[1] + r[3] / 2]
    }
    /// Press at `from`, move through the midpoint to `to`, optionally release.
    fn smoke_gizmo_drag(&mut self, from: [i32; 2], to: [i32; 2], shift: bool, release: bool) {
        self.motion(from[0], from[1], false);
        self.pointer(from[0], from[1], 1, true, false);
        let mid = [(from[0] + to[0]) / 2, (from[1] + to[1]) / 2];
        self.motion(mid[0], mid[1], shift);
        self.motion(to[0], to[1], shift);
        if release {
            self.pointer(to[0], to[1], 1, false, false);
        }
    }
    /// Drag an arrow, a plane square, the centre, a ring with Ctrl and a
    /// scale handle; typed values; Esc and right-click cancel; snapping,
    /// Alt and Shift; the context menu; header controls; the disabled
    /// gizmo; vertex handles, overlap, X-ray and box select.
    #[inline(never)]
    pub(super) fn smoke_gizmo(&mut self) {
        for (w, h) in [(800, 600), (1280, 800)] {
            smoke_gizmo_moves(w, h);
            smoke_gizmo_rotate_scale(w, h);
            smoke_gizmo_snap(w, h);
            smoke_gizmo_controls(w, h);
            smoke_vertex_handles(w, h);
            smoke_handle_precision(w, h);
        }
        smoke_gizmo_refused();
    }
}
#[inline(never)]
fn smoke_gizmo_moves(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let start = points(&a);
    // The X arrow: only X of the selected vertex changes, one undo step.
    a.mesh_vertices = vec![1];
    let tip = a.smoke_handle(Handle::Axis(0));
    let c = a.hp_project(start[1]).unwrap();
    let dir = [tip[0] - c[0], tip[1] - c[1]];
    a.smoke_gizmo_drag(tip, [tip[0] + dir[0] / 2, tip[1] + dir[1] / 2], false, true);
    let moved = points(&a);
    assert!(a.doc.dirty(), "{}", a.status);
    assert!(moved[1][0] != start[1][0], "X moves");
    assert_eq!([moved[1][1], moved[1][2]], [start[1][1], start[1][2]]);
    assert!((0..start.len())
        .filter(|i| *i != 1)
        .all(|i| moved[i] == start[i]));
    assert!(a.status.contains("One undo step") && a.status.contains("Move X"));
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // A typed value during the drag becomes exact numeric entry on X.
    let tip = a.smoke_handle(Handle::Axis(0));
    a.smoke_gizmo_drag(tip, [tip[0] + 9, tip[1] + 3], false, false);
    a.key(Key::Char('2'), false, false);
    a.key(Key::Char('5'), false, false);
    assert!(a.gizmo.drag.is_none());
    assert!(a
        .prompt
        .as_ref()
        .is_some_and(|p| p.axis == 0 && p.value == "25"));
    a.pointer(tip[0] + 9, tip[1] + 3, 1, false, false);
    a.key(Key::Enter, false, false);
    assert_eq!(points(&a)[1], [start[1][0] + 25, start[1][1], start[1][2]]);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // The XY square moves X and Y, never Z.
    a.mesh_vertices = vec![1];
    let sq = a.smoke_handle(Handle::Plane(2));
    a.smoke_gizmo_drag(sq, [sq[0] + 25, sq[1] + 20], false, true);
    let moved = points(&a);
    assert!(moved[1][0] != start[1][0] || moved[1][1] != start[1][1]);
    assert_eq!(moved[1][2], start[1][2], "XY plane keeps Z");
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // The centre moves in the view plane.
    a.mesh_vertices = vec![0, 1];
    let centre = a.smoke_handle(Handle::Free);
    assert!(a.pick_handle(centre[0], centre[1]).is_none());
    a.smoke_gizmo_drag(centre, [centre[0] + 30, centre[1] - 20], false, true);
    let moved = points(&a);
    let d: [i64; 3] = core::array::from_fn(|k| (moved[0][k] - start[0][k]) as i64);
    assert!(d != [0; 3]);
    assert_eq!(
        core::array::from_fn::<i32, 3, _>(|k| moved[1][k] - start[1][k]),
        d.map(|v| v as i32),
        "Selection moves together"
    );
    let n = a.camera_inverse([0, 0, 1024]);
    let along = (0..3).map(|k| d[k] * n[k] as i64).sum::<i64>().abs();
    let len = hangar_core::gizmo::isqrt(d.iter().map(|v| v * v).sum::<i64>());
    assert!(
        along <= len * 1024 / 20 + 1024,
        "Free move stays in the view plane"
    );
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Esc mid-drag restores everything.
    let tip = a.smoke_handle(Handle::Axis(1));
    a.smoke_gizmo_drag(tip, [tip[0] + 30, tip[1] + 30], false, false);
    assert!(a.preview.is_some());
    a.key(Key::Escape, false, false);
    assert!(a.preview.is_none() && a.gizmo.drag.is_none());
    a.pointer(tip[0] + 30, tip[1] + 30, 1, false, false);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Right-click mid-drag cancels too, and opens no menu.
    a.smoke_gizmo_drag(tip, [tip[0] + 30, tip[1] + 30], false, false);
    a.pointer(tip[0] + 30, tip[1] + 30, 3, true, false);
    assert!(a.preview.is_none() && a.gizmo.drag.is_none() && a.menu.is_none());
    a.pointer(tip[0] + 30, tip[1] + 30, 1, false, false);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Shift moves a tenth as far as the same drag without it.
    a.gizmo.magnet = false;
    a.mesh_vertices = vec![1];
    let tip = a.smoke_handle(Handle::Axis(0));
    let c = a.hp_project(start[1]).unwrap();
    let to = [tip[0] + (tip[0] - c[0]) * 2, tip[1] + (tip[1] - c[1]) * 2];
    a.smoke_gizmo_drag(tip, to, false, true);
    let coarse = points(&a)[1][0] - start[1][0];
    a.act(Action::Undo);
    let tip = a.smoke_handle(Handle::Axis(0));
    a.smoke_gizmo_drag(tip, to, true, true);
    let fine = points(&a)[1][0] - start[1][0];
    assert!(coarse.abs() >= 20, "{coarse}");
    assert!(
        (fine * 10 - coarse).abs() <= 25,
        "Shift is fine: {fine} vs {coarse}"
    );
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Ctrl steps by 10 source units.
    a.modifiers(true);
    let tip = a.smoke_handle(Handle::Axis(0));
    a.smoke_gizmo_drag(tip, to, false, true);
    a.modifiers(false);
    assert_eq!((points(&a)[1][0] - start[1][0]) % 10, 0);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Past the signed 16-bit range: no preview, and release refuses.
    a.zoom = 10;
    let tip = a.smoke_handle(Handle::Axis(0));
    let c = a.hp_project(start[1]).unwrap();
    let far = [
        tip[0] + (tip[0] - c[0]) * 400,
        tip[1] + (tip[1] - c[1]) * 400,
    ];
    a.smoke_gizmo_drag(tip, far, false, false);
    assert!(
        a.preview.is_none() && a.status.contains("16-bit"),
        "{}",
        a.status
    );
    a.pointer(far[0], far[1], 1, false, false);
    assert!(
        a.status.contains("16-bit") && !a.doc.dirty(),
        "{}",
        a.status
    );
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
#[inline(never)]
fn smoke_gizmo_rotate_scale(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let start = points(&a);
    a.mesh_vertices = vec![1, 2];
    let pivot = a.model.as_ref().unwrap().median(&[1, 2]).unwrap();
    // Rotate ring with Ctrl: 15 degree steps, the vertex follows the cursor.
    a.gizmo_mode(G_ROTATE);
    let on = a.smoke_handle(Handle::Ring(2));
    let c = a.hp_project(pivot).unwrap();
    let r = [(on[0] - c[0]) as i64, (on[1] - c[1]) as i64];
    // Turn the press point 20 degrees counter-clockwise on screen.
    let (s, co) = model::sin_cos(20);
    let to = [
        c[0] + ((r[0] * co as i64 + r[1] * s as i64) / 1024) as i32,
        c[1] + ((r[1] * co as i64 - r[0] * s as i64) / 1024) as i32,
    ];
    let screen_angle = |a: &App, p: [i32; 3]| {
        let q = a.hp_project(p).unwrap();
        hangar_core::gizmo::atan2_deg(-(q[1] - c[1]) as i64, (q[0] - c[0]) as i64)
    };
    let before = screen_angle(&a, start[2]);
    a.modifiers(true);
    a.smoke_gizmo_drag(on, to, false, false);
    assert!(
        a.status.contains("Rotate Z") && a.status.contains("15\u{b0}"),
        "{}",
        a.status
    );
    a.pointer(to[0], to[1], 1, false, false);
    a.modifiers(false);
    let turned = points(&a);
    let deg = if turned[2] == hangar_core::gizmo::rotate_about(start[2], pivot, 2, 15) {
        15
    } else {
        -15
    };
    for i in [1, 2] {
        assert_eq!(
            turned[i],
            hangar_core::gizmo::rotate_about(start[i], pivot, 2, deg)
        );
    }
    let after = screen_angle(&a, turned[2]);
    let turn = hangar_core::gizmo::angle_delta(before, after);
    assert!(turn > 0, "The ring turns with the cursor: {turn}");
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Scale handle: X grows about the median, Y and Z stay.
    a.gizmo_mode(G_SCALE);
    let tip = a.smoke_handle(Handle::Scale(0));
    let to = [tip[0] + (tip[0] - c[0]) / 2, tip[1] + (tip[1] - c[1]) / 2];
    a.modifiers(true);
    a.smoke_gizmo_drag(tip, to, false, false);
    assert!(a.status.contains("Scale X") && a.status.contains('%'));
    let pct: i32 = a.status["Scale X ".len()..a.status.find('%').unwrap()]
        .trim()
        .parse()
        .unwrap();
    assert!(pct % 10 == 0 && (130..=170).contains(&pct), "{pct}");
    a.pointer(to[0], to[1], 1, false, false);
    a.modifiers(false);
    let scaled = points(&a);
    for i in [1, 2] {
        assert_eq!(
            scaled[i][0] - pivot[0],
            ((start[i][0] - pivot[0]) as i64 * pct as i64 / 100) as i32
        );
        assert_eq!(&scaled[i][1..], &start[i][1..]);
    }
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
#[inline(never)]
fn smoke_gizmo_snap(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    let original = a.doc.archive.bytes().unwrap();
    let start = points(&a);
    // Top view: drag the tail (0, -70, 0) up the Y arrow to 6 px short of
    // the nose (0, 100, 0); it lands exactly on the nose.
    a.yaw = 0;
    a.pitch = 90;
    a.mesh_vertices = vec![5];
    let nose = a.hp_project(start[0]).unwrap();
    let tail = a.hp_project(start[5]).unwrap();
    let tip = a.smoke_handle(Handle::Axis(1));
    let short = if nose[1] > tail[1] { -6 } else { 6 };
    let to = [tip[0], tip[1] + nose[1] - tail[1] + short];
    a.smoke_gizmo_drag(tip, to, false, false);
    assert!(
        a.status.contains("Snapped to vertex at (0, 100, 0)"),
        "{}",
        a.status
    );
    assert!(a.gizmo.snap.is_some());
    // The target ring and guide are drawn.
    assert!(a
        .draw()
        .commands
        .iter()
        .any(|d| matches!(d, Draw::Rect(.., color) if *color == c::AMBER_BRIGHT.0)));
    // Alt bypasses the snap for this drag.
    a.alt_modifier(true);
    a.motion(to[0], to[1] + 1, false);
    a.motion(to[0], to[1], false);
    assert!(
        !a.status.contains("Snapped") && a.gizmo.snap.is_none(),
        "{}",
        a.status
    );
    a.pointer(to[0], to[1], 1, false, false);
    a.alt_modifier(false);
    assert!(points(&a)[5] != start[0], "Alt leaves it where dropped");
    assert_eq!([points(&a)[5][0], points(&a)[5][2]], [0, 0]);
    a.act(Action::Undo);
    let tip = a.smoke_handle(Handle::Axis(1));
    a.smoke_gizmo_drag(tip, to, false, true);
    assert_eq!(points(&a)[5], start[0], "Snapped exactly");
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // A plain vertex drag snaps in the view plane: the nose onto the left
    // wing tip (-80, -35, 0).
    a.mesh_vertices.clear();
    let wing = a.hp_project(start[1]).unwrap();
    a.motion(nose[0], nose[1], false);
    a.pointer(nose[0], nose[1], 1, true, false);
    a.motion(wing[0] + 1, wing[1] - 1, false);
    assert!(
        a.status.contains("Snapped to vertex at (-80, -35, 0)"),
        "{}",
        a.status
    );
    a.pointer(wing[0] + 1, wing[1] - 1, 1, false, false);
    assert_eq!(points(&a)[0], start[1]);
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Near on screen, far in depth: the top (0, -50, 20) dragged onto the
    // keel (0, -30, -12), 32 units below it, does not snap.
    let top = a.hp_project(start[3]).unwrap();
    let keel = a.hp_project(start[4]).unwrap();
    a.mesh_vertices.clear();
    a.motion(top[0], top[1], false);
    a.pointer(top[0], top[1], 1, true, false);
    a.motion(keel[0], keel[1], false);
    assert!(!a.status.contains("Snapped"), "{}", a.status);
    a.key(Key::Escape, false, false);
    a.pointer(keel[0], keel[1], 1, false, false);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
#[inline(never)]
fn smoke_gizmo_controls(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    a.mesh_vertices = vec![1, 2];
    a.smoke_geometry("gizmo move");
    // Right-click on empty viewport space: the gizmo menu switches modes.
    let empty = [a.right() - 30, a.dock_y() - 30];
    assert!(
        a.gizmo_pick(empty[0], empty[1]).is_none() && a.pick_handle(empty[0], empty[1]).is_none()
    );
    a.motion(empty[0], empty[1], false);
    a.pointer(empty[0], empty[1], 3, true, false);
    a.pointer(empty[0], empty[1], 3, false, false);
    assert_eq!(a.menu, Some(MENU_GIZMO));
    a.smoke_geometry("gizmo menu");
    let item = a
        .chrome_hit(&|x| matches!(x, Action::GizmoMode(G_ROTATE)))
        .expect("Rotate item");
    a.chrome_click(item);
    assert_eq!(a.gizmo.mode, G_ROTATE);
    assert!(a.menu.is_none());
    a.smoke_geometry("gizmo rotate");
    a.pointer(empty[0], empty[1], 3, true, false);
    let snap = a.chrome_hit(&|x| matches!(x, Action::Magnet)).unwrap();
    a.chrome_click(snap);
    assert!(!a.gizmo.magnet);
    a.pointer(empty[0], empty[1], 3, true, false);
    let cancel = a.chrome_hit(&|x| matches!(x, Action::Cancel)).unwrap();
    a.chrome_click(cancel);
    assert!(a.menu.is_none() && a.gizmo.mode == G_ROTATE);
    a.gizmo_mode(G_SCALE);
    a.smoke_geometry("gizmo scale");
    // The tool strip picks the gizmo at both sizes; Select hides it.
    let tool = a
        .chrome_hit(&|x| matches!(x, Action::GizmoMode(G_NONE)))
        .unwrap();
    a.chrome_click(tool);
    assert_eq!(a.gizmo.mode, G_NONE);
    assert!(!a
        .layout()
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::Gizmo(_))));
    let tool = a
        .chrome_hit(&|x| matches!(x, Action::GizmoMode(G_MOVE)))
        .unwrap();
    a.chrome_click(tool);
    assert_eq!(a.gizmo.mode, G_MOVE);
    // Header toggles at 1280; the shading overflow menu at 800.
    if w >= 1280 {
        let s = a.viewport_header_slots();
        assert!(s.gizmo.is_some() && s.magnet.is_some() && s.xray.is_some());
        let x = a.chrome_hit(&|x| matches!(x, Action::Xray)).unwrap();
        assert_eq!(x, s.xray.unwrap());
        a.chrome_click(x);
        assert!(a.gizmo.xray);
        let g = a.chrome_hit(&|x| matches!(x, Action::Magnet)).unwrap();
        a.chrome_click(g);
        assert!(a.gizmo.magnet);
    } else {
        assert!(a.viewport_header_slots().overflow.is_some());
        let more = a
            .chrome_hit(&|x| matches!(x, Action::Menu(chrome::MENU_SHADING)))
            .unwrap();
        a.chrome_click(more);
        a.smoke_geometry("shading overflow");
        let x = a.chrome_hit(&|x| matches!(x, Action::Xray)).unwrap();
        a.chrome_click(x);
        assert!(a.gizmo.xray);
    }
    a.alt_key('z');
    assert!(!a.gizmo.xray, "Alt+Z toggles X-ray");
}
#[inline(never)]
fn smoke_vertex_handles(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    a.gizmo.mode = G_NONE;
    let start = points(&a);
    // Front view: the nose (y 100) and the tail (y -70) share a screen
    // position; the nearer, the nose, is picked.
    a.yaw = 0;
    a.pitch = 0;
    let p = a.hp_project(start[0]).unwrap();
    assert_eq!(p, a.hp_project(start[5]).unwrap());
    assert_eq!(a.pick_handle(p[0] + 2, p[1] + 1), Some(0));
    a.smoke_press(p[0] + 2, p[1] + 1, false);
    assert_eq!(a.mesh_vertices, vec![0]);
    // Hover outlines the handle under the pointer in ink.
    a.mesh_vertices.clear();
    let wing = a.hp_project(start[1]).unwrap();
    a.motion(wing[0] + 3, wing[1] - 2, false);
    let ring = |a: &App| {
        a.draw()
            .commands
            .iter()
            .any(|d| matches!(d, Draw::Line(x, y, ..) if *x == wing[0] - 5 && *y == wing[1] - 5))
    };
    assert!(ring(&a), "Hovered handle outline");
    a.motion(wing[0] + 40, wing[1] + 40, false);
    assert!(!ring(&a));
    // Adjacent handles: zoom out until two sit within the hit radius; a
    // click on each still picks that one.
    let mut pair = None;
    (a.yaw, a.pitch) = (35, 25);
    for zoom in (10..=100).rev().step_by(5) {
        a.zoom = zoom;
        let all = a.vertex_handles();
        // Handles with no other handle on top of them.
        let hs: Vec<VertexHandle> = all
            .iter()
            .filter(|h| {
                all.iter()
                    .filter(|o| (o.at[0] - h.at[0]).abs().max((o.at[1] - h.at[1]).abs()) < 3)
                    .count()
                    == 1
            })
            .copied()
            .collect();
        pair = hs.iter().enumerate().find_map(|(i, p)| {
            hs[i + 1..]
                .iter()
                .find(|q| {
                    let d = (p.at[0] - q.at[0]).abs().max((p.at[1] - q.at[1]).abs());
                    (4..=HANDLE_HIT).contains(&d)
                })
                .map(|q| (*p, *q))
        });
        if pair.is_some() {
            break;
        }
    }
    let (p, q) = pair.expect("Adjacent handles");
    for h in [p, q] {
        a.smoke_press(h.at[0], h.at[1], false);
        assert_eq!(a.mesh_vertices, vec![h.index], "Nearest handle wins");
    }
    a.zoom = 100;
    // Solid shading from above: the keel vertex (0, -30, -12) is hidden by
    // the top faces: not drawn, not pickable until X-ray.
    a.yaw = 0;
    a.pitch = 90;
    a.textured = true;
    a.flat = true;
    let keel = a.hp_project(start[4]).unwrap();
    assert!(
        a.vertex_handles().iter().all(|h| h.index != 4),
        "Keel hidden in Solid shading"
    );
    let keel_drawn = |a: &App| {
        a.draw().commands.iter().any(|d| {
            matches!(d, Draw::Rect(x, y, w, ..) if x + w / 2 == keel[0] && y + w / 2 == keel[1])
        })
    };
    assert!(!keel_drawn(&a), "Hidden vertices draw nothing");
    assert_ne!(a.pick_handle(keel[0], keel[1]), Some(4));
    let boxed = |a: &mut App| {
        a.mesh_vertices.clear();
        a.key(Key::Char('b'), false, false);
        a.motion(keel[0] - 3, keel[1] - 3, false);
        a.pointer(keel[0] - 3, keel[1] - 3, 1, true, false);
        a.motion(keel[0] + 3, keel[1] + 3, false);
        a.pointer(keel[0] + 3, keel[1] + 3, 1, false, false);
        a.mesh_vertices.contains(&4)
    };
    assert!(!boxed(&mut a), "Box select skips hidden vertices");
    a.alt_key('z');
    assert!(a.gizmo.xray);
    assert_eq!(a.pick_handle(keel[0], keel[1]), Some(4));
    assert!(keel_drawn(&a), "X-ray draws every corner");
    assert!(boxed(&mut a), "X-ray boxes hidden vertices");
    a.smoke_press(keel[0], keel[1], false);
    assert_eq!(a.mesh_vertices, vec![4]);
    a.smoke_geometry("vertex handles");
}
/// Where the shaded raster draws `p`, in screen px: its raster corner
/// mapped back over the viewport as the blit stretches it.
fn raster_corner(a: &App, p: [i32; 3], [rw, rh]: [usize; 2]) -> [i32; 2] {
    let [w, h] = a.view_size();
    let r = a.raster_frame([w, h]).unwrap().raster16(p, [rw, rh]);
    [
        a.left() + 1 + (r[0] as i64 * w as i64).div_euclid(rw as i64 * 16) as i32,
        54 + (r[1] as i64 * h as i64).div_euclid(rh as i64 * 16) as i32,
    ]
}
/// Handles, picking and the raster share one projection: at 100, 250 and
/// 800% every handle sits within 1 px of its rasterized corner, on a pixel
/// of a face with that corner; a selected face's handles draw at its
/// corners at high zoom.
#[inline(never)]
fn smoke_handle_precision(w: i32, h: i32) {
    let mut a = gizmo_app(w, h);
    a.gizmo.mode = G_NONE;
    a.textured = true;
    a.flat = true;
    let [vw, vh] = a.view_size();
    let rw = (vw as usize).min(512);
    let rh = (vh as usize * rw / vw as usize).max(1);
    let (cx, cy) = (a.left() + 1 + vw / 2, 54 + vh / 2);
    let m = a.model.clone().unwrap();
    let mut solid = [0; 3];
    for (z, k, xray) in [100, 250, 800]
        .into_iter()
        .enumerate()
        .flat_map(|z| (0..m.vertices.len()).flat_map(move |k| [(z, k, false), (z, k, true)]))
    {
        // Each corner in turn at the viewport centre, X-ray off and on.
        let zoom = z.1;
        a.zoom = zoom;
        a.gizmo.xray = xray;
        a.pan = [0, 0];
        let at = a.hp_project(m.vertices[k].point).unwrap();
        a.pan = [cx - at[0], cy - at[1]];
        let frame = a.raster(rw, rh);
        let handles = a.vertex_handles();
        assert!(!xray || !handles.is_empty(), "zoom {zoom}: X-ray handles");
        if !xray {
            solid[z.0] += handles.len();
        }
        for hd in &handles {
            let p = m.vertices[hd.index].point;
            let c = raster_corner(&a, p, [rw, rh]);
            let d = (c[0] - hd.at[0]).abs().max((c[1] - hd.at[1]).abs());
            assert!(d <= 1, "zoom {zoom}: handle {:?} vs corner {c:?}", hd.at);
            assert_eq!(Some(hd.at), a.hp_project(p));
            // A shown handle sits on a pixel of a face with that corner
            // (within 2 raster px: sharp tips cover few pixel centres).
            let x = ((hd.at[0] - a.left() - 1) as usize * rw / vw as usize) as i32;
            let y = ((hd.at[1] - 54) as usize * rh / vh as usize) as i32;
            let on = (-2..=2).any(|dy| {
                (-2..=2).any(|dx| {
                    let (x, y) = (x + dx, y + dy);
                    x >= 0
                        && y >= 0
                        && (x as usize) < rw
                        && (y as usize) < rh
                        && m.faces
                            .get(frame.faces[y as usize * rw + x as usize])
                            .is_some_and(|f| f.indices.iter().any(|i| m.vertices[*i].point == p))
                })
            });
            assert!(xray || on, "zoom {zoom}: handle {} off its faces", hd.index);
        }
    }
    assert!(solid.iter().all(|n| *n > 0), "Solid handles at every zoom");
    a.gizmo.xray = false;
    // A selected face at 800%, each corner panned to the viewport centre
    // in turn: an amber handle and a pick region on the rasterized corner.
    a.zoom = 100;
    a.pan = [0, 0];
    let face = (0..40)
        .find_map(|k| a.pick_face(cx + k * 3, cy + k))
        .expect("A face near the centre");
    let corners: Vec<usize> = m.faces[face].indices.clone();
    a.mesh_vertices = corners.clone();
    a.zoom = 800;
    for i in &corners {
        let p = m.vertices[*i].point;
        a.pan = [0, 0];
        let at = a.hp_project(p).unwrap();
        a.pan = [cx - at[0], cy - at[1]];
        let c = raster_corner(&a, p, [rw, rh]);
        assert!(
            (c[0] - cx).abs() <= 1 && (c[1] - cy).abs() <= 1,
            "Corner {i} at {c:?}"
        );
        let layout = a.layout();
        let hit = layout
            .hits
            .iter()
            .find_map(|x| match x.action {
                Action::MeshVertex(v) if m.vertices[v].point == p => Some(x.rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("Corner {i} has no handle"));
        let centre = [hit[0] + hit[2] / 2, hit[1] + hit[3] / 2];
        assert!((centre[0] - c[0]).abs() <= 1 && (centre[1] - c[1]).abs() <= 1);
        assert!(
            layout.canvas.commands.iter().any(|d| matches!(d,
                Draw::Rect(x, y, w, _, color) if *color == c::AMBER.0
                    && x + w / 2 == centre[0] && *y + w / 2 == centre[1])),
            "Amber handle at corner {i}"
        );
    }
    a.smoke_geometry("handle precision");
}
#[inline(never)]
fn smoke_gizmo_refused() {
    let mut a = super::edit_ui::parts_app();
    a.width = 1280;
    a.height = 800;
    a.act(Action::MeshMode);
    assert!(a.mesh_edit && !a.model.as_ref().unwrap().writable);
    let original = a.doc.archive.bytes().unwrap();
    // Gear down: the body moves through the region writer.
    let model = a.model.as_ref().unwrap();
    let body = (0..model.vertices.len())
        .find(|i| {
            model.vertex_tags[*i].group.is_none()
                && a.hp_project(model.vertices[*i].point)
                    .is_some_and(|p| a.in_viewport(p[0], p[1]))
        })
        .unwrap();
    a.mesh_vertices = vec![body];
    assert!(a.gizmo_refusal().is_none(), "{:?}", a.gizmo_refusal());
    let before = points(&a)[body];
    let tip = a.smoke_handle(Handle::Axis(0));
    a.smoke_gizmo_drag(tip, [tip[0] + 25, tip[1] + 6], false, true);
    let after = points(&a)[body];
    assert!(
        after[0] != before[0] && after[1..] == before[1..],
        "{}",
        a.status
    );
    a.act(Action::Undo);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
    // Legs folded (gear down, gearPos -8192) are rotated: their vertices
    // disable the gizmo.
    a.ed.pose.insert("_PLgearDown".into(), 1);
    a.ed.pose.insert("_PLgearPos".into(), -8192);
    a.refresh();
    let model = a.model.as_ref().unwrap();
    let leg = (0..model.vertices.len())
        .find(|i| {
            !hangar_core::shape_geometry::unrotated(model, *i)
                && a.hp_project(model.vertices[*i].point)
                    .is_some_and(|p| a.in_viewport(p[0], p[1]))
        })
        .expect("A rotated leg vertex");
    a.mesh_vertices = vec![leg];
    let why = a.gizmo_refusal().expect("Refused");
    assert!(why.contains("rotates"));
    let dim = c::AXIS_X.mix(c::GM_950, 150).0;
    assert!(a
        .draw()
        .commands
        .iter()
        .any(|d| matches!(d, Draw::Line(.., color) if *color == dim)));
    let tip = a.smoke_handle(Handle::Axis(0));
    a.motion(tip[0], tip[1], false);
    assert!(a.status.starts_with("Gizmo unavailable"), "{}", a.status);
    a.smoke_gizmo_drag(tip, [tip[0] + 25, tip[1] + 6], false, true);
    assert!(a.gizmo.drag.is_none() && a.preview.is_none());
    assert!(a.status.starts_with("Gizmo unavailable"), "{}", a.status);
    assert_eq!(a.doc.archive.bytes().unwrap(), original);
}
