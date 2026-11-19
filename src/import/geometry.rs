//! Geometry for the importer: hulls, centroids and clipping on the sheet,
//! and paths from TopoJSON arcs.
//!
//! On the sheet, locations are `Point`s (down and across); `geo` takes them
//! as (down, across) pairs.

use super::read::Ring;
use super::topology::{self, P, arc_points};
use crate::Point;
use geo::{Centroid, ConvexHull, LineString, MultiPoint, MultiPolygon, Polygon};

fn pair(p: &Point) -> P {
    (p.down, p.across)
}

fn ring_pairs(ring: &[Point]) -> Vec<P> {
    ring.iter().map(pair).collect()
}

/// Convex hull, counter-clockwise from its least vertex, not closed.
pub(crate) fn hull(points: &[Point]) -> Vec<Point> {
    let mut h: Vec<P> = MultiPoint::from(ring_pairs(points))
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
    h.into_iter().map(|(d, a)| Point::new(d, a)).collect()
}

/// Area-weighted centroid of rings (closed or not).
pub(crate) fn centroid(rings: &[Vec<Point>]) -> Option<Point> {
    MultiPolygon::new(
        rings
            .iter()
            .map(|r| Polygon::new(LineString::from(ring_pairs(r)), vec![]))
            .collect(),
    )
    .centroid()
    .map(|p| Point::new(p.x(), p.y()))
}

/// Unsigned area of a ring.
pub(crate) fn area(ring: &[Point]) -> f64 {
    topology::area(&ring_pairs(ring)).abs()
}

/// The parts of a path (in grid coordinates) that fall within `bounds`
/// (down and across, from 0), each keeping the vertex beyond the edge so
/// that it reaches the edge.
pub(crate) fn clip(pts: &[Point], bounds: Point) -> Vec<Vec<Point>> {
    let (down, across) = (bounds.down, bounds.across);
    let inside =
        |p: Point| p.down >= 0.0 && p.down <= down && p.across >= 0.0 && p.across <= across;
    let overlaps = |a: Point, b: Point| {
        a.down.min(b.down) <= down
            && a.down.max(b.down) >= 0.0
            && a.across.min(b.across) <= across
            && a.across.max(b.across) >= 0.0
    };
    let mut parts: Vec<Vec<Point>> = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
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
        let pts = [
            (0.0, 0.0),
            (2.0, 0.0),
            (1.0, 1.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 0.0),
        ]
        .map(|(d, a)| Point::new(d, a));
        let h = hull(&pts);
        let want = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)].map(|(d, a)| Point::new(d, a));
        assert_eq!(h, want);
    }
}
