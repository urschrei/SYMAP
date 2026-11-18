//! Grouping values into printed levels.

use crate::error::Error;

/// Class boundaries, lowest first; `levels() == breaks.len() - 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Classes {
    pub breaks: Vec<f64>,
}

impl Classes {
    /// `n` equal-width levels spanning `min..=max`.
    pub fn equal(min: f64, mut max: f64, n: usize) -> Self {
        assert!(n >= 1);
        if max <= min {
            max = min + 1.0;
        }
        let w = (max - min) / n as f64;
        let mut breaks: Vec<f64> = (0..n).map(|k| min + w * k as f64).collect();
        breaks.push(max);
        Self { breaks }
    }

    pub fn explicit(breaks: Vec<f64>) -> Result<Self, Error> {
        if breaks.len() < 2 || breaks.windows(2).any(|w| w[0] >= w[1]) {
            return Err(Error::general(
                "class breaks must be at least two strictly increasing values",
            ));
        }
        Ok(Self { breaks })
    }

    pub fn levels(&self) -> usize {
        self.breaks.len() - 1
    }

    /// 0-based level of `v`. Each level includes its lower bound; the top level
    /// also includes its upper bound. Values outside the range are clamped.
    pub fn level(&self, v: f64) -> usize {
        let n = self.levels();
        self.breaks[1..n].iter().take_while(|&&b| v >= b).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_intervals() {
        let c = Classes::equal(0.0, 10.0, 5);
        assert_eq!(c.breaks, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(c.level(-1.0), 0);
        assert_eq!(c.level(1.99), 0);
        assert_eq!(c.level(2.0), 1);
        assert_eq!(c.level(10.0), 4);
        assert_eq!(c.level(99.0), 4);
    }

    #[test]
    fn degenerate_range() {
        let c = Classes::equal(3.0, 3.0, 4);
        assert_eq!(c.level(3.0), 0);
    }

    #[test]
    fn explicit_breaks_validated() {
        assert!(Classes::explicit(vec![0.0, 1.0, 1.0]).is_err());
        assert_eq!(
            Classes::explicit(vec![0.0, 1.0, 5.0]).unwrap().level(2.0),
            1
        );
    }
}
