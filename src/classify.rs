//! Grouping values into levels (electives 3 to 6).

use crate::error::Error;

/// Where a value falls: below the range (L), in a level, or above it (H).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Low,
    In(u8),
    High,
}

impl Level {
    /// The label printed for the level: `L`, `1` to `10`, or `H`.
    pub fn label(self) -> String {
        match self {
            Level::Low => "L".into(),
            Level::In(k) => (k + 1).to_string(),
            Level::High => "H".into(),
        }
    }
}

/// Level boundaries, lowest first; `levels() == breaks.len() - 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Classes {
    pub breaks: Vec<f64>,
}

impl Classes {
    /// `n` levels over `min..=max`.
    ///
    /// `intervals` is elective 6: `None` for equal ranges, `Some(&[])` for
    /// equal numbers of `data` values per level, otherwise relative sizes.
    pub fn new(
        min: f64,
        max: f64,
        n: usize,
        intervals: Option<&[f64]>,
        data: &[f64],
    ) -> Result<Self, Error> {
        assert!(n >= 1);
        if max < min {
            return Err(Error::general(format!(
                "value range minimum {min} exceeds maximum {max}"
            )));
        }
        let max = if max == min { min + 1.0 } else { max };
        let span = max - min;
        let mut breaks = vec![min];
        match intervals {
            None => breaks.extend((1..n).map(|k| min + span * k as f64 / n as f64)),
            Some([]) => {
                let mut v: Vec<f64> = data
                    .iter()
                    .copied()
                    .filter(|x| (min..=max).contains(x))
                    .collect();
                v.sort_by(f64::total_cmp);
                if v.is_empty() {
                    breaks.extend((1..n).map(|k| min + span * k as f64 / n as f64));
                } else {
                    // Each boundary value belongs to the higher level.
                    breaks.extend((1..n).map(|k| v[((k * v.len()).div_ceil(n)).min(v.len() - 1)]));
                }
            }
            Some(r) => {
                if r.len() != n {
                    return Err(Error::general(format!(
                        "elective 6 gives {} ranges for {n} levels",
                        r.len()
                    )));
                }
                let total: f64 = r.iter().sum();
                let mut acc = 0.0;
                for x in &r[..n - 1] {
                    acc += x;
                    breaks.push(min + span * acc / total);
                }
            }
        }
        breaks.push(max);
        Ok(Self { breaks })
    }

    pub fn levels(&self) -> usize {
        self.breaks.len() - 1
    }

    pub fn min(&self) -> f64 {
        self.breaks[0]
    }

    pub fn max(&self) -> f64 {
        *self.breaks.last().unwrap()
    }

    /// A value on a boundary goes to the higher level; the maximum belongs to
    /// the top level.
    pub fn level(&self, v: f64) -> Level {
        if v < self.min() {
            return Level::Low;
        }
        if v > self.max() {
            return Level::High;
        }
        let n = self.levels();
        Level::In(self.breaks[1..n].iter().take_while(|&&b| v >= b).count() as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_3_3_equal_ranges() {
        let c = Classes::new(0.0, 30.0, 3, None, &[]).unwrap();
        assert_eq!(c.breaks, vec![0.0, 10.0, 20.0, 30.0]);
        assert_eq!(c.level(10.0), Level::In(1));
        assert_eq!(c.level(30.0), Level::In(2));
        assert_eq!(c.level(-0.1), Level::Low);
        assert_eq!(c.level(30.1), Level::High);
    }

    #[test]
    fn example_3_4_proportions() {
        let c = Classes::new(100.0, 500.0, 5, Some(&[25.0, 25.0, 12.5, 12.5, 25.0]), &[]).unwrap();
        assert_eq!(c.breaks, vec![100.0, 200.0, 300.0, 350.0, 400.0, 500.0]);
    }

    #[test]
    fn example_3_6_seven_levels() {
        let c = Classes::new(
            0.0,
            1000.0,
            7,
            Some(&[150.0, 50.0, 71.5, 228.5, 250.0, 139.0, 111.0]),
            &[],
        )
        .unwrap();
        let want = [0.0, 150.0, 200.0, 271.5, 500.0, 750.0, 889.0, 1000.0];
        for (a, b) in c.breaks.iter().zip(want) {
            assert!((a - b).abs() < 1e-9, "{a} {b}");
        }
        assert_eq!(c.level(150.0), Level::In(1));
    }

    #[test]
    fn equal_counts() {
        let data: Vec<f64> = (1..=10).map(f64::from).collect();
        let c = Classes::new(1.0, 10.0, 5, Some(&[]), &data).unwrap();
        let mut counts = [0; 5];
        for v in &data {
            let Level::In(k) = c.level(*v) else { panic!() };
            counts[k as usize] += 1;
        }
        assert_eq!(counts, [2; 5]);
        assert!(Classes::new(0.0, 1.0, 3, Some(&[1.0]), &[]).is_err());
    }
}
