//! Integer math for the viewport transform gizmo and vertex snapping:
//! cursor angles, screen-to-world drag conversion and the 3D snap search.
//! No floating point; angles are whole degrees and match `model::sin_cos`.
use crate::model::{rotate, sin_cos};

/// Rounded integer division (half away from zero); 0 when `d` is 0.
pub fn div_round(n: i64, d: i64) -> i64 {
    if d == 0 {
        return 0;
    }
    let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
    if n >= 0 {
        (n + d / 2) / d
    } else {
        -((-n + d / 2) / d)
    }
}
/// Integer square root (floor).
pub fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
/// Angle of `(x, y)` in whole degrees, 0..360, counter-clockwise from +x:
/// the quadrant by sign, then a binary search of `sin_cos` within it, so
/// `atan2_deg(sin d, cos d) == d`. (0, 0) gives 0.
pub fn atan2_deg(y: i64, x: i64) -> i32 {
    if x == 0 && y == 0 {
        return 0;
    }
    // Turn the vector by -90 degree steps until it lies in [0, 90).
    let (mut x, mut y, mut base) = (x, y, 0);
    while !(x > 0 && y >= 0) {
        (x, y) = (y, -x);
        base += 90;
    }
    // Largest d in 0..=89 at or before the vector: y cos d - x sin d >= 0.
    let cross = |d: i32| {
        let (s, c) = sin_cos(d);
        y * c as i64 - x * s as i64
    };
    let (mut lo, mut hi) = (0, 89);
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if cross(mid) >= 0 {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    // Round to the nearer of d and d + 1 by perpendicular distance.
    let d = if cross(lo + 1).abs() < cross(lo).abs() {
        lo + 1
    } else {
        lo
    };
    (base + d) % 360
}
/// Signed shortest turn from `from` to `to` degrees, -179..=180.
pub fn angle_delta(from: i32, to: i32) -> i32 {
    let d = (to - from).rem_euclid(360);
    if d > 180 {
        d - 360
    } else {
        d
    }
}
/// The drag `m` (pixels) measured along a screen direction `d`, in units of
/// the world length `k` that `d` is the projection of: `k (m.d) / (d.d)`.
pub fn along(m: [i64; 2], d: [i64; 2], k: i64) -> i64 {
    div_round(k * (m[0] * d[0] + m[1] * d[1]), d[0] * d[0] + d[1] * d[1])
}
/// Solve `m = s a + t b` for two screen directions (Cramer's rule) and
/// return `[s k, t k]`: the world offsets along the two vectors `a` and `b`
/// project. None when the directions are parallel on screen.
pub fn solve2(m: [i64; 2], a: [i64; 2], b: [i64; 2], k: i64) -> Option<[i64; 2]> {
    let det = a[0] * b[1] - a[1] * b[0];
    if det == 0 {
        return None;
    }
    Some([
        div_round(k * (m[0] * b[1] - m[1] * b[0]), det),
        div_round(k * (a[0] * m[1] - a[1] * m[0]), det),
    ])
}
/// `v` rounded to the nearest multiple of `step` (step <= 1 leaves it).
pub fn step(v: i32, step: i32) -> i32 {
    if step <= 1 {
        return v;
    }
    (div_round(v as i64, step as i64) * step as i64) as i32
}
/// Rotate `p` about `pivot` around a principal axis, as `transform_selection`.
pub fn rotate_about(p: [i32; 3], pivot: [i32; 3], axis: usize, degrees: i32) -> [i32; 3] {
    let local = core::array::from_fn(|k| p[k] - pivot[k]);
    let r = rotate(local, axis, degrees);
    core::array::from_fn(|k| r[k] + pivot[k])
}
/// The set a dragged vertex moves in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    /// The line through the start along a principal axis.
    Axis(usize),
    /// The plane through the start whose normal is this principal axis.
    Plane(usize),
    /// The plane through the start perpendicular to this view direction
    /// (any length; the camera's towards-the-viewer vector).
    View([i32; 3]),
}
/// A snap target and where the snapping vertex lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snap {
    /// Index into the candidate list's first field.
    pub target: usize,
    pub vertex: [i32; 3],
    /// New position of the snapping vertex.
    pub point: [i32; 3],
    /// Distance of the target from the constraint, rounded source units:
    /// 0 means the result coincides exactly.
    pub off: i64,
}
fn sub(a: [i32; 3], b: [i32; 3]) -> [i64; 3] {
    core::array::from_fn(|k| a[k] as i64 - b[k] as i64)
}
fn dot(a: [i64; 3], b: [i64; 3]) -> i64 {
    (0..3).map(|k| a[k] * b[k]).sum()
}
/// The snap rule, judged in 3D: a candidate qualifies only when it lies
/// within `tolerance` source units of the constraint through `start` (so a
/// vertex that only looks near on screen, far away in depth, never does);
/// among those the one nearest `current` within the constraint wins,
/// provided that distance is at most `tolerance`. Ties go to the smaller
/// off-constraint distance, then the lower index. The snapping vertex takes
/// the target's coordinates along the free axes: one for an axis, two for
/// a principal plane, all three for the view plane.
pub fn snap_search(
    constraint: Constraint,
    start: [i32; 3],
    current: [i32; 3],
    candidates: &[(usize, [i32; 3])],
    tolerance: i32,
) -> Option<Snap> {
    let t = tolerance.max(0) as i64;
    let t2 = t * t;
    let mut best: Option<(i64, i64, usize, Snap)> = None;
    for (index, v) in candidates {
        let w0 = sub(*v, start);
        let w = sub(*v, current);
        // (off-constraint distance squared, in-constraint distance squared,
        // landing point); squares are compared against t2.
        let (off2, in2, point) = match constraint {
            Constraint::Axis(a) if a < 3 => {
                let off2 = (0..3).filter(|k| *k != a).map(|k| w0[k] * w0[k]).sum();
                let mut p = start;
                p[a] = v[a];
                (off2, w[a] * w[a], p)
            }
            Constraint::Plane(n) if n < 3 => {
                let in2 = (0..3).filter(|k| *k != n).map(|k| w[k] * w[k]).sum();
                let mut p = *v;
                p[n] = start[n];
                (w0[n] * w0[n], in2, p)
            }
            Constraint::View(n) => {
                let n = n.map(|x| x as i64);
                let nn = dot(n, n);
                if nn == 0 {
                    return None;
                }
                let along0 = dot(w0, n);
                let along = dot(w, n);
                let off2 = div_round(along0 * along0, nn);
                let in2 = dot(w, w) - div_round(along * along, nn);
                (off2, in2.max(0), *v)
            }
            _ => return None,
        };
        if off2 > t2 || in2 > t2 {
            continue;
        }
        let snap = Snap {
            target: *index,
            vertex: *v,
            point,
            off: isqrt(off2) + i64::from(isqrt(off2) * isqrt(off2) != off2),
        };
        if best.is_none_or(|(i, o, k, _)| (in2, off2, *index) < (i, o, k)) {
            best = Some((in2, off2, *index, snap));
        }
    }
    best.map(|(.., s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atan2_inverts_sin_cos() {
        for d in 0..360 {
            let (s, c) = sin_cos(d);
            assert_eq!(atan2_deg(s as i64, c as i64), d, "{d}");
            // Any length.
            assert_eq!(atan2_deg(s as i64 * 37, c as i64 * 37), d, "{d}");
        }
        assert_eq!(atan2_deg(0, 0), 0);
        assert_eq!(atan2_deg(0, 5), 0);
        assert_eq!(atan2_deg(5, 0), 90);
        assert_eq!(atan2_deg(0, -5), 180);
        assert_eq!(atan2_deg(-5, 0), 270);
        assert_eq!(atan2_deg(1, 1), 45);
        assert_eq!(atan2_deg(-3, 3), 315);
        // Screen-sized vectors round to the nearest degree.
        assert_eq!(atan2_deg(10, 57), 10);
        assert_eq!(angle_delta(350, 10), 20);
        assert_eq!(angle_delta(10, 350), -20);
        assert_eq!(angle_delta(0, 180), 180);
    }
    #[test]
    fn drags_convert_to_world_units() {
        // 40 px along a 20 px projection of 100 units is 200 units.
        assert_eq!(along([40, 0], [20, 0], 100), 200);
        // Perpendicular motion does not move along the axis.
        assert_eq!(along([0, 40], [20, 0], 100), 0);
        assert_eq!(along([7, 7], [0, 0], 100), 0);
        // A plane: X projects to (10, 0), Y to (5, 5); the drag (25, 15) is 1 X + 3 Y.
        assert_eq!(solve2([25, 15], [10, 0], [5, 5], 100), Some([100, 300]));
        assert_eq!(solve2([1, 1], [1, 1], [2, 2], 100), None);
        assert_eq!(step(14, 10), 10);
        assert_eq!(step(-15, 10), -20);
        assert_eq!(step(7, 1), 7);
        assert_eq!(div_round(-7, 2), -4);
        assert_eq!(div_round(7, -2), -4);
    }
    #[test]
    fn rotates_about_a_pivot() {
        let p = rotate_about([110, 10, 5], [10, 10, 5], 2, 90);
        assert_eq!(p, [10, 110, 5]);
        assert_eq!(rotate_about([3, 4, 5], [3, 4, 5], 0, 33), [3, 4, 5]);
        // A half turn about X through (0, 50, 0).
        assert_eq!(rotate_about([0, 60, 10], [0, 50, 0], 0, 180), [0, 40, -10]);
    }
    #[test]
    fn snaps_in_three_dimensions() {
        let start = [0, 0, 0];
        // Front view: the viewer looks along -Y, so depth is Y.
        let view = Constraint::View([0, -1024, 0]);
        let near_screen_far_depth = (0, [10, 400, 0]);
        let near_in_plane = (1, [12, 2, 1]);
        // On screen both are about 10 units from the cursor; only the second
        // lies near the view plane through the start.
        assert_eq!(
            snap_search(view, start, [10, 0, 0], &[near_screen_far_depth], 4),
            None
        );
        let s = snap_search(
            view,
            start,
            [10, 0, 0],
            &[near_screen_far_depth, near_in_plane],
            4,
        )
        .unwrap();
        assert_eq!((s.target, s.point, s.off), (1, [12, 2, 1], 2));
        // Too far from the dragged position within the plane.
        assert_eq!(
            snap_search(view, start, [0, 0, 0], &[near_in_plane], 4),
            None
        );
        // Axis: the candidate on the X line snaps exactly along X.
        let s = snap_search(
            Constraint::Axis(0),
            start,
            [47, 0, 0],
            &[(5, [50, 0, 0])],
            4,
        )
        .unwrap();
        assert_eq!((s.point, s.off), ([50, 0, 0], 0));
        // Slightly off the line: X matches, the rest stays on the line.
        let s = snap_search(
            Constraint::Axis(0),
            start,
            [47, 0, 0],
            &[(5, [50, 3, 0])],
            4,
        )
        .unwrap();
        assert_eq!((s.point, s.off), ([50, 0, 0], 3));
        // Far off the line in depth: refused even though X matches.
        assert_eq!(
            snap_search(
                Constraint::Axis(0),
                start,
                [50, 0, 0],
                &[(5, [50, 0, 90])],
                4
            ),
            None
        );
        // Plane XY (normal Z): lands on the target's X and Y.
        let s = snap_search(
            Constraint::Plane(2),
            start,
            [30, 29, 0],
            &[(2, [31, 30, 2])],
            4,
        )
        .unwrap();
        assert_eq!((s.point, s.off), ([31, 30, 0], 2));
        assert_eq!(
            snap_search(
                Constraint::Plane(2),
                start,
                [30, 30, 0],
                &[(2, [30, 30, 9])],
                4
            ),
            None
        );
        // The nearest in-constraint candidate wins.
        let s = snap_search(
            Constraint::Axis(1),
            start,
            [0, 20, 0],
            &[(1, [0, 23, 0]), (2, [0, 21, 1]), (3, [0, 19, 0])],
            4,
        )
        .unwrap();
        assert_eq!(s.target, 3);
    }
}
