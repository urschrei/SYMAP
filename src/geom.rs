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
    Row::new(ring, p.down).contains(p.across)
}

/// Where a ring crosses the line at one `down`, to test many points on
/// that line as `in_ring` does: the same crossings, found once.
pub struct Row {
    /// Across positions of the crossings, ascending.
    crossings: Vec<f64>,
}

impl Row {
    pub fn new(ring: &[Point], down: f64) -> Self {
        let n = ring.len();
        let mut crossings = Vec::new();
        if n >= 3 {
            let mut j = n - 1;
            for i in 0..n {
                let (a, b) = (ring[i], ring[j]);
                if (a.down > down) != (b.down > down) {
                    crossings.push(
                        (b.across - a.across) * (down - a.down) / (b.down - a.down) + a.across,
                    );
                }
                j = i;
            }
        }
        crossings.sort_by(f64::total_cmp);
        Self { crossings }
    }

    /// Whether the point at `across` on the line is inside the ring: an odd
    /// number of crossings lie beyond it.
    pub fn contains(&self, across: f64) -> bool {
        let beyond = self.crossings.len() - self.crossings.partition_point(|&x| x <= across);
        beyond % 2 == 1
    }
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
    fn a_row_agrees_with_the_point_test() {
        // A concave ring with a vertex and an edge on the test rows.
        let ring = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 10.0),
            Point::new(10.0, 10.0),
            Point::new(5.0, 5.0),
            Point::new(10.0, 0.0),
        ];
        for down in [0.0, 2.5, 5.0, 7.5, 10.0] {
            let row = Row::new(&ring, down);
            for k in -4..=56 {
                let across = f64::from(k) * 0.25 - 1.0;
                let p = Point::new(down, across);
                let slow = {
                    let n = ring.len();
                    let (mut inside, mut j) = (false, n - 1);
                    for i in 0..n {
                        let (a, b) = (ring[i], ring[j]);
                        if (a.down > p.down) != (b.down > p.down)
                            && p.across
                                < (b.across - a.across) * (p.down - a.down) / (b.down - a.down)
                                    + a.across
                        {
                            inside = !inside;
                        }
                        j = i;
                    }
                    inside
                };
                assert_eq!(row.contains(across), slow, "{p:?}");
            }
        }
    }

    #[test]
    fn ring_membership() {
        let outer = square(0.0, 10.0);
        assert!(in_ring(Point::new(5.0, 5.0), &outer));
        assert!(!in_ring(Point::new(11.0, 5.0), &outer));
    }
}
