//! Points on a face: the point under a view position, located in the fan
//! of triangles `(0, j, j + 1)` the renderer draws and interpolated from the
//! corners with integer barycentrics (as Remap's Bake samples the old UVs),
//! and the snap targets of a face (corners, edge midpoints, the centre)
//! judged in 3D within the face's plane by `gizmo::snap_search`.
use crate::gizmo::{div_round, snap_search, Constraint};
use crate::model::face_normal;
use alloc::vec::Vec;

/// A located point: the fan triangle `(0, tri, tri + 1)` and its weights
/// (non-negative, summing to `area`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Located {
    pub tri: usize,
    pub weights: [i64; 3],
    pub area: i64,
}
impl Located {
    /// The corner indices of the triangle.
    pub fn ids(&self) -> [usize; 3] {
        [0, self.tri, self.tri + 1]
    }
    /// Interpolate per-corner values (coordinates, UVs) at the point,
    /// rounded to the nearest integer.
    pub fn mix<const N: usize>(&self, values: &[[i32; N]]) -> Option<[i32; N]> {
        let ids = self.ids();
        if ids[2] >= values.len() || self.area <= 0 {
            return None;
        }
        Some(core::array::from_fn(|c| {
            let sum: i128 = (0..3)
                .map(|k| self.weights[k] as i128 * values[ids[k]][c] as i128)
                .sum();
            let a = self.area as i128;
            let r = (2 * sum + sum.signum() * a) / (2 * a);
            r.clamp(i32::MIN as i128, i32::MAX as i128) as i32
        }))
    }
}
fn edge(a: [i64; 2], b: [i64; 2], p: [i64; 2]) -> i128 {
    (p[0] - a[0]) as i128 * (b[1] - a[1]) as i128 - (p[1] - a[1]) as i128 * (b[0] - a[0]) as i128
}
/// Locate `at` in a polygon's fan given its corners in 2D (any integer
/// plane: view 1/16 px, or a 3D face dropped onto its dominant plane).
/// Inside a triangle the weights are its edge functions; outside every
/// triangle (a pixel at the face's border) the triangle with the least
/// negative weight is used with its negative weights clamped to 0, so the
/// point lies on the face's edge. None for fewer than 3 corners or a
/// polygon with no area.
pub fn locate(corners: &[[i64; 2]], at: [i64; 2]) -> Option<Located> {
    let mut best: Option<(i128, Located)> = None;
    for j in 1..corners.len().saturating_sub(1) {
        let t = [corners[0], corners[j], corners[j + 1]];
        let area = edge(t[0], t[1], t[2]);
        if area == 0 {
            continue;
        }
        let sign = area.signum();
        let w = [
            edge(t[1], t[2], at) * sign,
            edge(t[2], t[0], at) * sign,
            edge(t[0], t[1], at) * sign,
        ];
        let worst = *w.iter().min()?;
        // Normalise the worst weight by the triangle's size so the choice
        // between triangles does not favour large ones.
        let score = if worst >= 0 {
            0
        } else {
            worst * 1024 / area.abs()
        };
        if best.as_ref().is_some_and(|(s, _)| *s >= score) {
            continue;
        }
        let clamped: [i128; 3] = w.map(|v| v.max(0));
        let sum: i128 = clamped.iter().sum();
        if sum == 0 {
            continue;
        }
        // Rescale to fit i64 for very large coordinates.
        let shift = (0..64).find(|s| (sum >> s) < (1i128 << 60)).unwrap_or(63);
        let weights = clamped.map(|v| (v >> shift) as i64);
        let area = weights.iter().sum::<i64>();
        if area == 0 {
            continue;
        }
        best = Some((
            score,
            Located {
                tri: j,
                weights,
                area,
            },
        ));
        if worst >= 0 {
            break;
        }
    }
    best.map(|(_, l)| l)
}
/// Locate a 3D point of a face in its fan, by dropping the axis the face's
/// normal is longest along.
pub fn locate3(points: &[[i32; 3]], p: [i32; 3]) -> Option<Located> {
    let n = face_normal(points)?;
    let drop = (0..3).max_by_key(|k| n[*k].abs())?;
    let keep = [(drop + 1) % 3, (drop + 2) % 3];
    let flat = |q: [i32; 3]| keep.map(|k| q[k] as i64);
    let corners: Vec<[i64; 2]> = points.iter().map(|q| flat(*q)).collect();
    locate(&corners, flat(p))
}
/// Distance of `p` from the plane of `points`, rounded up to whole units;
/// None when the corners are collinear.
pub fn plane_distance(points: &[[i32; 3]], p: [i32; 3]) -> Option<i64> {
    let n = face_normal(points)?.map(|v| v as i64);
    let a = points[0];
    let d: i64 = (0..3).map(|k| n[k] * (p[k] as i64 - a[k] as i64)).sum();
    let len = crate::gizmo::isqrt(n.iter().map(|v| v * v).sum()).max(1);
    Some((d.abs() + len - 1) / len)
}
/// What a placed point snapped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    /// An existing corner of the face.
    Corner,
    /// The midpoint of an edge.
    EdgeMidpoint,
    /// The face's centre: the average of its corners, as its stored centre.
    FaceCentre,
}
impl SnapKind {
    pub fn label(self) -> &'static str {
        match self {
            SnapKind::Corner => "Corner",
            SnapKind::EdgeMidpoint => "Edge midpoint",
            SnapKind::FaceCentre => "Face centre",
        }
    }
}
/// A snap target: its kind, the corner (or the edge from that corner to
/// the next) it belongs to, and its point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub kind: SnapKind,
    pub index: usize,
    pub point: [i32; 3],
}
/// The midpoint of `a` and `b`, rounded to the nearest unit.
pub fn midpoint(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    core::array::from_fn(|k| div_round(a[k] as i64 + b[k] as i64, 2) as i32)
}
/// The face's centre: the corner average truncated as stored face centres are.
pub fn centre(points: &[[i32; 3]]) -> [i32; 3] {
    let n = points.len().max(1) as i64;
    core::array::from_fn(|k| (points.iter().map(|p| p[k] as i64).sum::<i64>() / n) as i32)
}
/// Every snap target of a face: corners, edge midpoints, then the centre.
pub fn targets(points: &[[i32; 3]]) -> Vec<Target> {
    let n = points.len();
    let mut out: Vec<Target> = (0..n)
        .map(|i| Target {
            kind: SnapKind::Corner,
            index: i,
            point: points[i],
        })
        .collect();
    out.extend((0..n).map(|i| Target {
        kind: SnapKind::EdgeMidpoint,
        index: i,
        point: midpoint(points[i], points[(i + 1) % n]),
    }));
    if n >= 3 {
        out.push(Target {
            kind: SnapKind::FaceCentre,
            index: 0,
            point: centre(points),
        });
    }
    out
}
/// Snap `point` on a face to the target nearest it within `tolerance`
/// source units in the face's plane: `gizmo::snap_search` under the
/// view-plane constraint along the face normal, the rule vertex drags use.
/// Ties go to corners, then midpoints, then the centre.
pub fn snap(points: &[[i32; 3]], point: [i32; 3], tolerance: i32) -> Option<Target> {
    let normal = face_normal(points)?;
    let all = targets(points);
    let candidates: Vec<(usize, [i32; 3])> =
        all.iter().enumerate().map(|(i, t)| (i, t.point)).collect();
    let s = snap_search(
        Constraint::View(normal),
        point,
        point,
        &candidates,
        tolerance,
    )?;
    all.get(s.target).copied()
}
/// The edge (corner `i` to `i + 1`) `p` lies on, when it is on one: inside
/// the segment, not at a corner, and within one unit of the line (a rounded
/// midpoint is up to 0.87 units off it).
pub fn on_edge(points: &[[i32; 3]], p: [i32; 3]) -> Option<usize> {
    let n = points.len();
    (0..n).find(|i| {
        let (a, b) = (points[*i], points[(i + 1) % n]);
        if p == a || p == b {
            return false;
        }
        let d: [i64; 3] = core::array::from_fn(|k| b[k] as i64 - a[k] as i64);
        let w: [i64; 3] = core::array::from_fn(|k| p[k] as i64 - a[k] as i64);
        let dd: i64 = d.iter().map(|v| v * v).sum();
        let t: i64 = (0..3).map(|k| d[k] * w[k]).sum();
        if dd == 0 || t <= 0 || t >= dd {
            return false;
        }
        // |d x w|^2 / |d|^2 is the squared distance from the line.
        let c = [
            d[1] as i128 * w[2] as i128 - d[2] as i128 * w[1] as i128,
            d[2] as i128 * w[0] as i128 - d[0] as i128 * w[2] as i128,
            d[0] as i128 * w[1] as i128 - d[1] as i128 * w[0] as i128,
        ];
        let cc: i128 = c.iter().map(|v| v * v).sum();
        cc <= dd as i128
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A square in the X-Y plane at z 10, corners counter-clockwise.
    fn square() -> Vec<[i32; 3]> {
        alloc::vec![[0, 0, 10], [100, 0, 10], [100, 60, 10], [0, 60, 10]]
    }
    #[test]
    fn locates_points_with_integer_barycentrics() {
        let s = square();
        let flat: Vec<[i64; 2]> = s.iter().map(|p| [p[0] as i64, p[1] as i64]).collect();
        // Inside the second fan triangle (0, 2, 3).
        let l = locate(&flat, [20, 40]).unwrap();
        assert_eq!(l.tri, 2);
        assert_eq!(l.mix(&s), Some([20, 40, 10]));
        // Inside the first triangle (0, 1, 2).
        let l = locate(&flat, [75, 25]).unwrap();
        assert_eq!((l.tri, l.mix(&s)), (1, Some([75, 25, 10])));
        // Outside: clamped onto the nearest edge.
        let l = locate(&flat, [130, 30]).unwrap();
        let p = l.mix(&s).unwrap();
        assert!(
            p[0] == 100 && (0..=60).contains(&p[1]) && p[2] == 10,
            "{p:?}"
        );
        // UVs interpolate with the same weights.
        let uv = [[0, 0], [200, 0], [200, 120], [0, 120]];
        let l = locate(&flat, [50, 30]).unwrap();
        assert_eq!(l.mix(&uv), Some([100, 60]));
        // 3D: the square tilted out of every principal plane.
        let tilted: Vec<[i32; 3]> = alloc::vec![[0, 0, 0], [100, 0, 50], [100, 60, 50], [0, 60, 0]];
        let p = [40, 30, 20];
        let l = locate3(&tilted, p).unwrap();
        assert_eq!(l.mix(&tilted), Some(p));
        assert_eq!(plane_distance(&tilted, p), Some(0));
        assert!(plane_distance(&tilted, [40, 30, 40]).unwrap() >= 15);
        // Degenerate input.
        assert!(locate(&flat[..2], [0, 0]).is_none());
        assert!(locate(&[[0, 0], [5, 5], [10, 10]], [5, 5]).is_none());
    }
    #[test]
    fn snaps_to_midpoints_centre_and_corners() {
        let s = square();
        let t = targets(&s);
        assert_eq!(t.len(), 9);
        assert_eq!(t[4].point, [50, 0, 10]);
        assert_eq!(t[8].kind, SnapKind::FaceCentre);
        assert_eq!(t[8].point, [50, 30, 10]);
        // Near the bottom edge's midpoint.
        let hit = snap(&s, [53, 3, 10], 6).unwrap();
        assert_eq!(
            (hit.kind, hit.index, hit.point),
            (SnapKind::EdgeMidpoint, 0, [50, 0, 10])
        );
        assert_eq!(hit.kind.label(), "Edge midpoint");
        // Near the centre.
        let hit = snap(&s, [48, 33, 10], 6).unwrap();
        assert_eq!(hit.kind, SnapKind::FaceCentre);
        // Near a corner.
        let hit = snap(&s, [97, 58, 10], 6).unwrap();
        assert_eq!((hit.kind, hit.index), (SnapKind::Corner, 2));
        // Out of reach: no snap.
        assert!(snap(&s, [25, 15, 10], 6).is_none());
        // Off the plane by more than the tolerance: no snap (3D rule).
        assert!(snap(&s, [50, 0, 30], 6).is_none());
        // Midpoints round to the nearest unit.
        assert_eq!(midpoint([0, 0, 0], [3, -3, 5]), [2, -2, 3]);
    }
    #[test]
    fn finds_the_edge_a_point_lies_on() {
        let s = square();
        assert_eq!(on_edge(&s, [50, 0, 10]), Some(0));
        assert_eq!(on_edge(&s, [100, 30, 10]), Some(1));
        assert_eq!(on_edge(&s, [0, 30, 10]), Some(3));
        assert_eq!(on_edge(&s, [50, 30, 10]), None);
        assert_eq!(
            on_edge(&s, [100, 0, 10]),
            None,
            "a corner is not on an edge"
        );
        assert_eq!(on_edge(&s, [150, 0, 10]), None, "beyond the segment");
    }
}
