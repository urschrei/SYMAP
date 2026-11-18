//! Planar geometry in SYMAP's (down, across) coordinate space.

/// A location measured down from the top border and across from the left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub down: f64,
    pub across: f64,
}

impl Point {
    pub const fn new(down: f64, across: f64) -> Self {
        Self { down, across }
    }

    pub fn dist(self, o: Point) -> f64 {
        // sqrt is a single WebAssembly instruction; hypot is a slow software
        // routine there, and this is the innermost loop of interpolation.
        let (dd, da) = (self.down - o.down, self.across - o.across);
        (dd * dd + da * da).sqrt()
    }
}

/// Even-odd point-in-ring test. The ring need not repeat its first vertex.
pub fn in_ring(p: Point, ring: &[Point]) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.down > p.down) != (b.down > p.down)
            && p.across < (b.across - a.across) * (p.down - a.down) / (b.down - a.down) + a.across
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Even-odd test over several rings, so later rings can punch holes.
pub fn in_rings(p: Point, rings: &[Vec<Point>]) -> bool {
    rings.iter().filter(|r| in_ring(p, r)).count() % 2 == 1
}

/// Unsigned shoelace area.
pub fn area(ring: &[Point]) -> f64 {
    let n = ring.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        s += a.across * b.down - b.across * a.down;
    }
    s.abs() / 2.0
}

/// Axis-aligned bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: Point,
    pub max: Point,
}

impl Extent {
    pub fn of<'a>(pts: impl IntoIterator<Item = &'a Point>) -> Option<Self> {
        let mut it = pts.into_iter();
        let first = *it.next()?;
        Some(it.fold(
            Extent {
                min: first,
                max: first,
            },
            |e, p| Extent {
                min: Point::new(e.min.down.min(p.down), e.min.across.min(p.across)),
                max: Point::new(e.max.down.max(p.down), e.max.across.max(p.across)),
            },
        ))
    }

    pub fn height(&self) -> f64 {
        self.max.down - self.min.down
    }

    pub fn width(&self) -> f64 {
        self.max.across - self.min.across
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(lo: f64, hi: f64) -> Vec<Point> {
        vec![
            Point::new(lo, lo),
            Point::new(lo, hi),
            Point::new(hi, hi),
            Point::new(hi, lo),
        ]
    }

    #[test]
    fn ring_membership_and_holes() {
        let outer = square(0.0, 10.0);
        let hole = square(4.0, 6.0);
        assert!(in_ring(Point::new(5.0, 5.0), &outer));
        assert!(!in_ring(Point::new(11.0, 5.0), &outer));
        let rings = vec![outer, hole];
        assert!(!in_rings(Point::new(5.0, 5.0), &rings));
        assert!(in_rings(Point::new(1.0, 1.0), &rings));
    }

    #[test]
    fn shoelace() {
        assert_eq!(area(&square(0.0, 3.0)), 9.0);
    }
}
