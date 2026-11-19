//! Geometry for the importer: hulls, interior points and clipping on the
//! sheet, and paths from TopoJSON arcs.
//!
//! On the sheet, locations are `Point`s (down and across); `geo` takes them
//! as (down, across) pairs.

use super::read::Ring;
use super::topology::{self, P, arc_points};
use crate::Point;
use geo::algorithm::buffer::{BufferStyle, LineJoin};
use geo::algorithm::validation::InvalidPolygon;
use geo::{
    Buffer, Contains, ConvexHull, InteriorPoint, LineString, MultiPoint, MultiPolygon, Polygon,
    Simplify, Validation,
};

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

/// A ring grown by `distance` on every side, with mitred corners (very
/// sharp corners are bevelled), closed. `None` if nothing is left.
pub(crate) fn grown(ring: &[Point], distance: f64) -> Option<Vec<Point>> {
    let polygon = Polygon::new(LineString::from(ring_pairs(ring)), vec![]);
    // Corners sharper than about 30 degrees are bevelled, not mitred.
    let style = BufferStyle::new(distance).line_join(LineJoin::Miter(0.5));
    let buffered = polygon.buffer_with_style(style);
    // Each mitred corner also has a vertex on each edge either side of it,
    // in line with the corners: simplification removes them.
    let exterior = buffered.0.first()?.exterior().simplify(distance * 1e-3);
    Some(exterior.coords().map(|c| Point::new(c.x, c.y)).collect())
}

/// Whether a ring (in any coordinates) crosses or touches itself.
pub(crate) fn crosses_itself(ring: &[P]) -> bool {
    Polygon::new(LineString::from(ring.to_vec()), vec![])
        .validation_errors()
        .iter()
        .any(|e| matches!(e, InvalidPolygon::SelfIntersection(_)))
}

/// Unsigned area of a ring.
pub(crate) fn area(ring: &[Point]) -> f64 {
    topology::area(&ring_pairs(ring)).abs()
}

/// The parts of a path on the sheet that lie within the rectangle from
/// `min` to `max`, cut at its edges, in order along the path. A path breaks
/// only where it leaves the rectangle. (Geo's `BooleanOps::clip` also
/// breaks it where it touches itself, and two barriers meeting at a vertex
/// both count a path through it.)
pub(crate) fn clip(pts: &[Point], min: Point, max: Point) -> Vec<Vec<Point>> {
    let mut parts: Vec<Vec<Point>> = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let mut end = |cur: &mut Vec<Point>| {
        if cur.len() >= 2 {
            parts.push(std::mem::take(cur));
        }
        cur.clear();
    };
    for w in pts.windows(2) {
        match clip_segment(w[0], w[1], min, max) {
            Some((a, b)) => {
                if cur.last() != Some(&a) {
                    end(&mut cur);
                    cur.push(a);
                }
                cur.push(b);
                if b != w[1] {
                    end(&mut cur);
                }
            }
            None => end(&mut cur),
        }
    }
    end(&mut cur);
    parts
}

/// The part of segment `p`-`q` inside the rectangle from `min` to `max`
/// (Liang-Barsky). An end inside the rectangle is returned unchanged.
/// `None` if no more than a point is inside.
fn clip_segment(p: Point, q: Point, min: Point, max: Point) -> Option<(Point, Point)> {
    let (dd, da) = (q.down - p.down, q.across - p.across);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    let axes = [
        (p.down, dd, min.down, max.down),
        (p.across, da, min.across, max.across),
    ];
    for (start, delta, lo, hi) in axes {
        if delta == 0.0 {
            if start < lo || start > hi {
                return None;
            }
            continue;
        }
        let (ta, tb) = ((lo - start) / delta, (hi - start) / delta);
        let (enter, exit) = if delta > 0.0 { (ta, tb) } else { (tb, ta) };
        t0 = t0.max(enter);
        t1 = t1.min(exit);
    }
    if t0 >= t1 {
        return None;
    }
    let at = |t: f64| match t {
        0.0 => p,
        1.0 => q,
        _ => Point::new(p.down + t * dd, p.across + t * da),
    };
    Some((at(t0), at(t1)))
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
    fn clipping_cuts_at_the_edges_and_keeps_paths_whole() {
        // A path that leaves the rectangle and comes back, and touches
        // itself at (5, 5).
        let path = points(&[
            (5.0, 0.0),
            (5.0, 5.0),
            (8.0, 8.0),
            (5.0, 8.0),
            (5.0, 5.0),
            (5.0, 20.0),
            (8.0, 20.0),
            (8.0, 9.0),
        ]);
        let parts = clip(&path, Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        assert_eq!(
            parts,
            [
                points(&[
                    (5.0, 0.0),
                    (5.0, 5.0),
                    (8.0, 8.0),
                    (5.0, 8.0),
                    (5.0, 5.0),
                    (5.0, 10.0)
                ]),
                points(&[(8.0, 10.0), (8.0, 9.0)]),
            ]
        );
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
