//! Geometry for the importer: hulls, centroids, clipping, and paths from
//! TopoJSON arcs.

use super::read::Ring;
use super::topology::{P, arc_points};
use geo::{Centroid, ConvexHull, LineString, MultiPoint, MultiPolygon, Polygon};

/// Convex hull, counter-clockwise from its least vertex, not closed.
pub(crate) fn hull(points: &[P]) -> Vec<P> {
    let mut h: Vec<P> = MultiPoint::from(points.to_vec())
        .convex_hull()
        .exterior()
        .coords()
        .map(|c| c.x_y())
        .collect();
    h.pop();
    let least = (0..h.len()).min_by(|&i, &j| {
        let ((ix, iy), (jx, jy)) = (h[i], h[j]);
        ix.total_cmp(&jx).then(iy.total_cmp(&jy))
    });
    if let Some(i) = least {
        h.rotate_left(i);
    }
    h
}

/// Area-weighted centroid of rings (closed or not).
pub(crate) fn centroid(rings: &[Vec<P>]) -> Option<P> {
    MultiPolygon::new(
        rings
            .iter()
            .map(|r| Polygon::new(LineString::from(r.clone()), vec![]))
            .collect(),
    )
    .centroid()
    .map(|p| p.x_y())
}

/// The parts of a path (in grid coordinates) that fall within `bounds`
/// (down and across, from 0), each keeping the vertex beyond the edge so
/// that it reaches the edge.
pub(crate) fn clip(pts: &[P], (down, across): P) -> Vec<Vec<P>> {
    let inside = |p: P| p.0 >= 0.0 && p.0 <= down && p.1 >= 0.0 && p.1 <= across;
    let overlaps = |a: P, b: P| {
        a.0.min(b.0) <= down && a.0.max(b.0) >= 0.0 && a.1.min(b.1) <= across && a.1.max(b.1) >= 0.0
    };
    let mut parts: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    for w in pts.windows(2) {
        if inside(w[0]) || inside(w[1]) || overlaps(w[0], w[1]) {
            if cur.is_empty() {
                cur.push(w[0]);
            }
            cur.push(w[1]);
        } else if !cur.is_empty() {
            parts.push(std::mem::take(&mut cur));
        }
    }
    if cur.len() >= 2 {
        parts.push(cur);
    }
    parts
}

/// A ring or path's coordinates: given directly, or joined from arcs.
pub(crate) fn path(arcs: &[Vec<P>], r: &Ring) -> Vec<P> {
    match r {
        Ring::Coords(c) => c.clone(),
        Ring::Arcs(refs) => {
            let mut out: Vec<P> = Vec::new();
            for &a in refs {
                for p in arc_points(arcs, a) {
                    if out.last() != Some(&p) {
                        out.push(p);
                    }
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hull_is_convex() {
        let h = hull(&[
            (0.0, 0.0),
            (2.0, 0.0),
            (1.0, 1.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 0.0),
        ]);
        assert_eq!(h, [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    }
}
