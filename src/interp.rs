//! Shepard's interpolation, as devised for SYMAP's contour maps.
//!
//! Follows Shepard (1968), doi:10.1145/800186.810616:
//!
//! * search radius `r` chosen so that on average `target` (7) points fall
//!   inside it, then widened or narrowed so that between `min_points` (4) and
//!   `max_points` (10) points contribute;
//! * a distance weight `s(d)` equal to `1/d` near the point and tapering
//!   smoothly to zero at `r`;
//! * a direction factor that increases the weight of points that are not
//!   shadowed by others in the same direction;
//! * a slope correction that extrapolates each point's value along a
//!   locally estimated gradient, damped with distance.
//!
//! SYMAP exposes these as electives: 31 (the extrapolation fraction, 0.1),
//! 34 and 35 (initial and maximum search radius) and 36 (4 to 10 points).

use crate::geom::Point;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShepardParams {
    pub target: f64,
    pub min_points: usize,
    pub max_points: usize,
    pub direction: bool,
    pub slope: bool,
    /// Elective 31: how far the slope term may reach, as a fraction of the data range.
    pub fraction: f64,
    /// Elective 34: initial search radius (otherwise from `target`).
    pub initial_radius: Option<f64>,
    /// Elective 35: never search beyond this distance.
    pub max_radius: Option<f64>,
}

impl Default for ShepardParams {
    fn default() -> Self {
        Self {
            target: 7.0,
            min_points: 4,
            max_points: 10,
            direction: true,
            slope: true,
            fraction: 0.1,
            initial_radius: None,
            max_radius: None,
        }
    }
}

pub struct Shepard {
    pts: Vec<Point>,
    z: Vec<f64>,
    r0: f64,
    params: ShepardParams,
    grad: Vec<(f64, f64)>,
    /// Slope damping distance; zero disables the slope term.
    v: f64,
    /// Total range of the data values.
    range: f64,
}

fn by_dist(a: &(usize, f64), b: &(usize, f64)) -> std::cmp::Ordering {
    a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))
}

impl Shepard {
    /// `area` is the area of the study region, in source-map units squared.
    pub fn new(points: &[Point], values: &[f64], area: f64, params: ShepardParams) -> Self {
        assert_eq!(points.len(), values.len());
        assert!(!points.is_empty());
        assert!(params.max_points >= 1 && params.max_points >= params.min_points);
        let r0 = params
            .initial_radius
            .unwrap_or_else(|| (params.target * area / (PI * points.len() as f64)).sqrt());
        let mut s = Shepard {
            pts: points.to_vec(),
            z: values.to_vec(),
            r0,
            params,
            grad: vec![(0.0, 0.0); points.len()],
            v: 0.0,
            range: 0.0,
        };
        let (lo, hi) = values
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| {
                (l.min(v), h.max(v))
            });
        s.range = hi - lo;
        if params.slope && params.fraction > 0.0 && points.len() > 1 {
            let grads: Vec<_> = (0..points.len()).map(|i| s.gradient(i)).collect();
            let gmax = grads.iter().map(|(a, b)| a.hypot(*b)).fold(0.0, f64::max);
            if gmax > 0.0 && s.range > 0.0 {
                s.v = params.fraction * s.range / gmax;
                s.grad = grads;
            }
        }
        s
    }

    /// Contributing points around `p` (excluding `skip` and anything coincident
    /// with it), and the radius used.
    fn neighbours(&self, p: Point, skip: Option<usize>) -> (Vec<(usize, f64)>, f64) {
        let mut d: Vec<(usize, f64)> = self
            .pts
            .iter()
            .enumerate()
            .map(|(i, q)| (i, p.dist(*q)))
            .filter(|&(i, di)| skip.is_none_or(|s| i != s && di > 0.0))
            .collect();
        let (lo, hi) = (self.params.min_points, self.params.max_points);
        if d.len() > hi + 1 {
            d.select_nth_unstable_by(hi, by_dist);
            d.truncate(hi + 1);
        }
        d.sort_by(by_dist);
        let within = d.iter().filter(|x| x.1 < self.r0).count();
        let r = if within > hi {
            d[hi].1
        } else if within < lo {
            match d.get(lo) {
                Some(x) => x.1,
                None => d.last().map_or(1.0, |x| x.1 * 1.5).max(f64::MIN_POSITIVE),
            }
        } else {
            self.r0
        };
        let r = self.params.max_radius.map_or(r, |m| r.min(m));
        d.retain(|x| x.1 < r);
        (d, r)
    }

    fn weights(&self, p: Point, nb: &[(usize, f64)], r: f64) -> Vec<f64> {
        let s: Vec<f64> = nb
            .iter()
            .map(|&(_, d)| {
                if d <= r / 3.0 {
                    1.0 / d
                } else {
                    27.0 / (4.0 * r) * (d / r - 1.0).powi(2)
                }
            })
            .collect();
        if !self.params.direction {
            return s.iter().map(|x| x * x).collect();
        }
        let ssum: f64 = s.iter().sum();
        nb.iter()
            .zip(&s)
            .map(|(&(i, di), &si)| {
                let a = self.pts[i];
                let t: f64 = nb
                    .iter()
                    .zip(&s)
                    .map(|(&(j, dj), &sj)| {
                        let b = self.pts[j];
                        let cos = ((p.across - a.across) * (p.across - b.across)
                            + (p.down - a.down) * (p.down - b.down))
                            / (di * dj);
                        sj * (1.0 - cos)
                    })
                    .sum::<f64>()
                    / ssum;
                si * si * (1.0 + t)
            })
            .collect()
    }

    /// Weighted mean of directional derivatives towards neighbours: (d/dacross, d/ddown).
    fn gradient(&self, i: usize) -> (f64, f64) {
        let p = self.pts[i];
        let (nb, r) = self.neighbours(p, Some(i));
        if nb.is_empty() {
            return (0.0, 0.0);
        }
        let w = self.weights(p, &nb, r);
        let (mut a, mut b, mut ws) = (0.0, 0.0, 0.0);
        for (&(j, d), &wj) in nb.iter().zip(&w) {
            let q = self.pts[j];
            let dz = (self.z[j] - self.z[i]) / (d * d);
            a += wj * dz * (q.across - p.across);
            b += wj * dz * (q.down - p.down);
            ws += wj;
        }
        (a / ws, b / ws)
    }

    /// Interpolated value at `p`, or `None` if no data point is within the
    /// maximum search radius.
    pub fn eval(&self, p: Point) -> Option<f64> {
        if let Some(i) = self.pts.iter().position(|q| *q == p) {
            return Some(self.z[i]);
        }
        let (nb, r) = self.neighbours(p, None);
        if nb.is_empty() {
            return None;
        }
        let w = self.weights(p, &nb, r);
        let (mut num, mut den) = (0.0, 0.0);
        for (&(i, d), &wi) in nb.iter().zip(&w) {
            let mut zi = self.z[i];
            if self.v > 0.0 {
                let (a, b) = self.grad[i];
                let q = self.pts[i];
                zi += (a * (p.across - q.across) + b * (p.down - q.down)) * self.v / (self.v + d);
            }
            num += wi * zi;
            den += wi;
        }
        let v = if den > 0.0 && num.is_finite() {
            num / den
        } else {
            // Degenerate weights: fall back to the nearest point.
            self.z[nb[0].0]
        };
        // Extrapolate beyond the local extremes (the highest and lowest of the
        // points used here) by at most the elective 31 fraction of the range.
        let (zlo, zhi) = nb
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &(i, _)| {
                (l.min(self.z[i]), h.max(self.z[i]))
            });
        let reach = self.params.fraction * self.range;
        Some(v.clamp(zlo - reach, zhi + reach))
    }
}

/// Index of the nearest point (lowest index on ties).
pub fn nearest(points: &[Point], p: Point) -> usize {
    points
        .iter()
        .enumerate()
        .map(|(i, q)| (i, p.dist(*q)))
        .min_by(by_dist)
        .map(|x| x.0)
        .expect("at least one point")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_points() -> Vec<Point> {
        (0..6)
            .flat_map(|i| {
                (0..6)
                    .map(move |j| Point::new(i as f64 * 2.0 + (j % 2) as f64 * 0.3, j as f64 * 2.0))
            })
            .collect()
    }

    #[test]
    fn exact_at_data_points() {
        let pts = grid_points();
        let z: Vec<f64> = pts
            .iter()
            .map(|p| (p.down * 0.7).sin() + p.across)
            .collect();
        let s = Shepard::new(&pts, &z, 100.0, ShepardParams::default());
        for (p, v) in pts.iter().zip(&z) {
            assert_eq!(s.eval(*p), Some(*v));
        }
    }

    #[test]
    fn constant_field_is_reproduced() {
        let pts = grid_points();
        let z = vec![3.25; pts.len()];
        let s = Shepard::new(&pts, &z, 100.0, ShepardParams::default());
        for q in [
            Point::new(1.1, 4.4),
            Point::new(9.9, 0.2),
            Point::new(5.0, 5.0),
        ] {
            assert!((s.eval(q).unwrap() - 3.25).abs() < 1e-12);
        }
    }

    #[test]
    fn without_slope_stays_within_data_range() {
        let pts = grid_points();
        let z: Vec<f64> = (0..pts.len()).map(|i| ((i * 37) % 11) as f64).collect();
        let params = ShepardParams {
            fraction: 0.0,
            ..Default::default()
        };
        let s = Shepard::new(&pts, &z, 100.0, params);
        for k in 0..200 {
            let q = Point::new((k % 20) as f64 * 0.55, (k / 20) as f64 * 1.1);
            let v = s.eval(q).unwrap();
            assert!((0.0..=10.0).contains(&v), "{v}");
        }
    }

    #[test]
    fn weight_function_is_continuous_at_a_third() {
        let r = 3.0;
        let inner = 1.0 / (r / 3.0);
        let outer = 27.0 / (4.0 * r) * ((r / 3.0) / r - 1.0f64).powi(2);
        assert!((inner - outer).abs() < 1e-12);
    }

    #[test]
    fn few_points_still_interpolate() {
        let pts = [Point::new(0.0, 0.0), Point::new(0.0, 10.0)];
        let s = Shepard::new(&pts, &[0.0, 10.0], 100.0, ShepardParams::default());
        let v = s.eval(Point::new(0.0, 5.0)).unwrap();
        assert!((v - 5.0).abs() < 1e-9, "{v}");
    }

    #[test]
    fn extrapolation_is_limited_near_local_extremes() {
        // A steep ramp: slope correction would overshoot beyond the end points.
        let pts: Vec<Point> = (0..8).map(|i| Point::new(0.0, i as f64)).collect();
        let z: Vec<f64> = (0..8).map(|i| (i * i) as f64).collect();
        let s = Shepard::new(&pts, &z, 64.0, ShepardParams::default());
        let reach = 0.1 * 49.0;
        for k in 0..70 {
            let p = Point::new(0.3, k as f64 * 0.1);
            let v = s.eval(p).unwrap();
            assert!(v >= -reach - 1e-9 && v <= 49.0 + reach + 1e-9, "{v}");
        }
    }

    #[test]
    fn max_radius_leaves_gaps() {
        let pts = [Point::new(0.0, 0.0), Point::new(0.0, 1.0)];
        let params = ShepardParams {
            max_radius: Some(2.0),
            ..Default::default()
        };
        let s = Shepard::new(&pts, &[1.0, 2.0], 100.0, params);
        assert!(s.eval(Point::new(0.0, 0.5)).is_some());
        assert_eq!(s.eval(Point::new(9.0, 9.0)), None);
    }

    #[test]
    fn nearest_breaks_ties_low() {
        let pts = [Point::new(0.0, 0.0), Point::new(0.0, 2.0)];
        assert_eq!(nearest(&pts, Point::new(0.0, 1.0)), 0);
        assert_eq!(nearest(&pts, Point::new(0.0, 1.5)), 1);
    }
}
