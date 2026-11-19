//! Shared borders for polygon data: rings split into arcs at junctions, so
//! that a border between two zones is stored, and simplified, once.
//!
//! Arcs are referred to as in TopoJSON: `i` for arc `i`, and `!i` (that is,
//! `-i - 1`) for arc `i` reversed.

use std::collections::HashMap;

pub type P = (f64, f64);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Topology {
    pub arcs: Vec<Vec<P>>,
}

type Key = (i64, i64);

/// An exact key for a coordinate, so that shared vertices match.
fn key(p: P) -> Key {
    ((p.0 * 1e6).round() as i64, (p.1 * 1e6).round() as i64)
}

/// Rings without the closing vertex, with consecutive duplicates removed.
fn clean(ring: &[P]) -> Vec<P> {
    let mut out: Vec<P> = Vec::with_capacity(ring.len());
    for &p in ring {
        if out.last().is_none_or(|&q| key(q) != key(p)) {
            out.push(p);
        }
    }
    while out.len() > 1 && key(out[0]) == key(out[out.len() - 1]) {
        out.pop();
    }
    out
}

impl Topology {
    /// Build arcs from rings. Returns the topology and each ring as arc
    /// references, in the input's order.
    pub fn from_rings(rings: &[Vec<P>]) -> (Topology, Vec<Vec<i64>>) {
        let rings: Vec<Vec<P>> = rings.iter().map(|r| clean(r)).collect();
        // A vertex is a junction where rings that share it disagree about
        // its neighbours (TopoJSON's rule).
        let mut neighbours: HashMap<Key, Vec<(Key, Key)>> = HashMap::new();
        for r in &rings {
            let n = r.len();
            for i in 0..n {
                let (a, b) = (key(r[(i + n - 1) % n]), key(r[(i + 1) % n]));
                let pair = if a <= b { (a, b) } else { (b, a) };
                let e = neighbours.entry(key(r[i])).or_default();
                if !e.contains(&pair) {
                    e.push(pair);
                }
            }
        }
        let junction = |p: P| neighbours.get(&key(p)).is_some_and(|v| v.len() > 1);

        let mut topo = Topology::default();
        let mut index: HashMap<Vec<(i64, i64)>, usize> = HashMap::new();
        let mut add = |chain: Vec<P>, topo: &mut Topology| -> i64 {
            let fwd: Vec<(i64, i64)> = chain.iter().map(|&p| key(p)).collect();
            let rev: Vec<(i64, i64)> = fwd.iter().rev().copied().collect();
            if let Some(&i) = index.get(&fwd) {
                return i as i64;
            }
            if let Some(&i) = index.get(&rev) {
                return !(i as i64);
            }
            index.insert(fwd, topo.arcs.len());
            topo.arcs.push(chain);
            (topo.arcs.len() - 1) as i64
        };

        let mut refs = Vec::with_capacity(rings.len());
        for r in &rings {
            let n = r.len();
            if n < 3 {
                refs.push(Vec::new());
                continue;
            }
            let starts: Vec<usize> = (0..n).filter(|&i| junction(r[i])).collect();
            let mut out = Vec::new();
            if starts.is_empty() {
                // A ring of its own (or shared whole): start at its least
                // vertex, in a canonical direction, so a copy is recognised.
                let s = (0..n).min_by_key(|&i| key(r[i])).unwrap();
                let mut chain: Vec<P> = (0..=n).map(|k| r[(s + k) % n]).collect();
                if key(chain[1]) > key(chain[n - 1]) {
                    chain.reverse();
                    let id = add(chain, &mut topo);
                    out.push(!id);
                } else {
                    out.push(add(chain, &mut topo));
                }
            } else {
                for (j, &s) in starts.iter().enumerate() {
                    let e = starts[(j + 1) % starts.len()];
                    let len = if e > s { e - s } else { e + n - s };
                    let chain: Vec<P> = (0..=len).map(|k| r[(s + k) % n]).collect();
                    out.push(add(chain, &mut topo));
                }
            }
            refs.push(out);
        }
        (topo, refs)
    }

    /// How many times each arc is used by the given rings.
    pub fn uses(&self, rings: &[Vec<i64>]) -> Vec<usize> {
        let mut n = vec![0; self.arcs.len()];
        for r in rings {
            for &a in r {
                n[arc_index(a)] += 1;
            }
        }
        n
    }

    /// Simplify every arc (Douglas-Peucker), keeping its end points.
    pub fn simplify(&mut self, tolerance: f64) {
        for arc in &mut self.arcs {
            *arc = if arc.len() > 2 && key(arc[0]) == key(arc[arc.len() - 1]) {
                // A closed arc: split at its farthest point from the start.
                let far = (1..arc.len() - 1)
                    .max_by(|&i, &j| dist2(arc[0], arc[i]).total_cmp(&dist2(arc[0], arc[j])))
                    .unwrap();
                let mut a = douglas_peucker(&arc[..=far], tolerance);
                a.pop();
                a.extend(douglas_peucker(&arc[far..], tolerance));
                a
            } else {
                douglas_peucker(arc, tolerance)
            };
        }
    }

    /// A ring's vertices from its arcs, closed.
    pub fn ring(&self, refs: &[i64]) -> Vec<P> {
        let mut out: Vec<P> = Vec::new();
        for &a in refs {
            let arc = &self.arcs[arc_index(a)];
            let pts: Box<dyn Iterator<Item = &P>> = if a < 0 {
                Box::new(arc.iter().rev())
            } else {
                Box::new(arc.iter())
            };
            for &p in pts {
                if out.last().is_none_or(|&q| key(q) != key(p)) {
                    out.push(p);
                }
            }
        }
        if let (Some(&f), Some(&l)) = (out.first(), out.last()) {
            if key(f) != key(l) {
                out.push(f);
            }
        }
        out
    }
}

pub fn arc_index(a: i64) -> usize {
    if a < 0 { !a as usize } else { a as usize }
}

fn dist2(a: P, b: P) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

/// Distance from `p` to the segment `a`-`b`.
fn seg_dist(p: P, a: P, b: P) -> f64 {
    let l2 = dist2(a, b);
    if l2 == 0.0 {
        return dist2(p, a).sqrt();
    }
    let t = (((p.0 - a.0) * (b.0 - a.0) + (p.1 - a.1) * (b.1 - a.1)) / l2).clamp(0.0, 1.0);
    dist2(p, (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1))).sqrt()
}

pub fn douglas_peucker(pts: &[P], tol: f64) -> Vec<P> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((s, e)) = stack.pop() {
        let mut best = (0.0, 0);
        for i in s + 1..e {
            let d = seg_dist(pts[i], pts[s], pts[e]);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((s, best.1));
            stack.push((best.1, e));
        }
    }
    pts.iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(p, _)| *p)
        .collect()
}

/// Signed area (positive anticlockwise, in x-y terms).
pub fn area(ring: &[P]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum::<f64>()
        / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two squares sharing a wavy border, and an island.
    fn rings() -> Vec<Vec<P>> {
        let border = [(1.0, 0.0), (1.02, 0.3), (0.98, 0.6), (1.0, 1.0)];
        let mut left = vec![(0.0, 0.0)];
        left.extend(border);
        left.extend([(0.0, 1.0), (0.0, 0.0)]);
        let mut right: Vec<P> = border.iter().rev().copied().collect();
        right.splice(0..0, [(2.0, 1.0)]);
        right.extend([(2.0, 0.0), (2.0, 1.0)]);
        let island = vec![(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 5.0)];
        vec![left, right, island]
    }

    #[test]
    fn shared_borders_become_one_arc() {
        let (topo, refs) = Topology::from_rings(&rings());
        let uses = topo.uses(&refs);
        // The wavy border is one arc used twice, in opposite directions.
        let shared: Vec<usize> = (0..uses.len()).filter(|&i| uses[i] == 2).collect();
        assert_eq!(shared.len(), 1);
        let s = shared[0] as i64;
        assert!(refs[0].contains(&s) != refs[1].contains(&s));
        assert!(refs[0].contains(&!s) || refs[1].contains(&!s));
        assert_eq!(topo.arcs[shared[0]].len(), 4);
        // Rings rebuild to their input.
        assert_eq!(topo.ring(&refs[2]).len(), 4);
        assert!((area(&topo.ring(&refs[0])).abs() - area(&rings()[0]).abs()).abs() < 1e-12);
    }

    #[test]
    fn simplified_neighbours_still_meet() {
        let (mut topo, refs) = Topology::from_rings(&rings());
        topo.simplify(0.05);
        let (a, b) = (topo.ring(&refs[0]), topo.ring(&refs[1]));
        // The wiggle (0.02) is gone from both, and identically so.
        assert!(!a.contains(&(1.02, 0.3)) && !b.contains(&(1.02, 0.3)));
        // (Rings are closed; count each vertex once.)
        let shared: Vec<&P> = a[1..].iter().filter(|p| b.contains(p)).collect();
        assert_eq!(shared.len(), 2);
        // The island keeps enough vertices to stay a ring.
        assert!(topo.ring(&refs[2]).len() >= 4);
    }

    #[test]
    fn douglas_peucker_keeps_corners() {
        let line = [(0.0, 0.0), (1.0, 0.01), (2.0, 0.0), (2.0, 1.0)];
        assert_eq!(
            douglas_peucker(&line, 0.1),
            [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0)]
        );
    }
}
