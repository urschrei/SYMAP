//! Geometry for the importer: hulls, interior points and clipping on the
//! sheet, and paths from TopoJSON arcs.
//!
//! On the sheet, locations are `Point`s (down and across); `geo` takes them
//! as (down, across) pairs.

use super::read::Ring;
use super::topology::{self, P, arc_points};
use crate::Point;
use geo::{Contains, ConvexHull, InteriorPoint, LineString, MultiPoint, MultiPolygon, Polygon};

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

/// A point inside a zone of `outers` (closed or not) with `holes`, away
/// from its edges where possible: a centroid can lie outside a concave
/// zone, or in a hole. `None` if there are no outer rings.
pub(crate) fn interior_point(outers: &[Vec<Point>], holes: &[Vec<Point>]) -> Option<Point> {
    let polygons = outers.iter().map(|outer| {
        let exterior = LineString::from(ring_pairs(outer));
        let shell = Polygon::new(exterior.clone(), vec![]);
        let inside: Vec<LineString> = holes
            .iter()
            .filter(|h| {
                h.first()
                    .is_some_and(|p| shell.contains(&geo::Coord::from(pair(p))))
            })
            .map(|h| LineString::from(ring_pairs(h)))
            .collect();
        Polygon::new(exterior, inside)
    });
    MultiPolygon::new(polygons.collect())
        .interior_point()
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

    fn points(v: &[(f64, f64)]) -> Vec<Point> {
        v.iter().map(|&(d, a)| Point::new(d, a)).collect()
    }

    fn inside(p: Point, ring: &[Point]) -> bool {
        Polygon::new(LineString::from(ring_pairs(ring)), vec![])
            .contains(&geo::Coord::from(pair(&p)))
    }

    #[test]
    fn interior_points_avoid_holes_and_concavities() {
        // A C shape, whose centroid lies in its mouth.
        let c = points(&[
            (0.0, 0.0),
            (0.0, 10.0),
            (2.0, 10.0),
            (2.0, 2.0),
            (8.0, 2.0),
            (8.0, 10.0),
            (10.0, 10.0),
            (10.0, 0.0),
        ]);
        let p = interior_point(std::slice::from_ref(&c), &[]).unwrap();
        assert!(inside(p, &c), "{p:?}");
        // A square around an enclave at its centre.
        let outer = points(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]);
        let hole = points(&[(3.0, 3.0), (3.0, 7.0), (7.0, 7.0), (7.0, 3.0)]);
        let p = interior_point(std::slice::from_ref(&outer), std::slice::from_ref(&hole)).unwrap();
        assert!(inside(p, &outer) && !inside(p, &hole), "{p:?}");
        assert_eq!(interior_point(&[], &[]), None);
    }

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
