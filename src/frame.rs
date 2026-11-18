//! Mapping between data space and the line-printer character grid.
//!
//! Printer cells are not square: at the default 10 characters and 6 lines
//! per inch a cell is 0.1 in wide and ~0.167 in tall. All scaling is done in
//! inches so the printed map keeps the data's proportions.

use crate::error::Error;
use crate::geom::{Extent, Point};

/// Largest map we are willing to allocate, in cells.
pub const MAX_CELLS: usize = 4_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub rows: usize,
    pub cols: usize,
    pub rows_per_inch: f64,
    pub cols_per_inch: f64,
    /// Printed inches per data unit.
    pub inches_per_unit: f64,
    /// Data coordinates of the top-left corner.
    pub origin: Point,
}

impl Frame {
    /// Fit `extent` into a box of `size` = (height, width) inches, keeping proportions.
    pub fn fit(
        extent: Extent,
        size: (f64, f64),
        rows_per_inch: f64,
        cols_per_inch: f64,
    ) -> Result<Frame, Error> {
        let (h, w) = size;
        if !(h > 0.0 && w > 0.0 && rows_per_inch > 0.0 && cols_per_inch > 0.0) {
            return Err(Error::general(
                "map size and characters per inch must be positive",
            ));
        }
        let eh = if extent.height() > 0.0 {
            extent.height()
        } else {
            1.0
        };
        let ew = if extent.width() > 0.0 {
            extent.width()
        } else {
            1.0
        };
        let ipu = (h / eh).min(w / ew);
        let cells =
            |len: f64, per_inch: f64| ((len * ipu * per_inch - 1e-9).ceil() as usize).max(1);
        let (rows, cols) = (cells(eh, rows_per_inch), cells(ew, cols_per_inch));
        if rows.saturating_mul(cols) > MAX_CELLS {
            return Err(Error::general(format!(
                "map of {rows} x {cols} characters is too large"
            )));
        }
        Ok(Frame {
            rows,
            cols,
            rows_per_inch,
            cols_per_inch,
            inches_per_unit: ipu,
            origin: extent.min,
        })
    }

    /// Data coordinates of the centre of cell (`row`, `col`).
    pub fn centre(&self, row: usize, col: usize) -> Point {
        Point::new(
            self.origin.down + (row as f64 + 0.5) / self.rows_per_inch / self.inches_per_unit,
            self.origin.across + (col as f64 + 0.5) / self.cols_per_inch / self.inches_per_unit,
        )
    }

    /// The cell containing `p`, if it lies on the map.
    pub fn cell(&self, p: Point) -> Option<(usize, usize)> {
        let r = (p.down - self.origin.down) * self.inches_per_unit * self.rows_per_inch;
        let c = (p.across - self.origin.across) * self.inches_per_unit * self.cols_per_inch;
        // Points on the far edge belong to the last cell.
        let clamp = |x: f64, n: usize| {
            if x < 0.0 || x > n as f64 + 1e-9 {
                None
            } else {
                Some((x as usize).min(n - 1))
            }
        };
        Some((clamp(r, self.rows)?, clamp(c, self.cols)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_extent_prints_square() {
        let e = Extent {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
        };
        let f = Frame::fit(e, (10.0, 13.0), 6.0, 10.0).unwrap();
        // 10 inches each way: 60 lines by 100 characters.
        assert_eq!((f.rows, f.cols), (60, 100));
        assert_eq!(f.cell(f.centre(17, 42)), Some((17, 42)));
        assert_eq!(f.cell(Point::new(10.0, 10.0)), Some((59, 99)));
        assert_eq!(f.cell(Point::new(-0.1, 1.0)), None);
    }

    #[test]
    fn rejects_huge_maps() {
        let e = Extent {
            min: Point::new(0.0, 0.0),
            max: Point::new(1.0, 1.0),
        };
        assert!(Frame::fit(e, (1000.0, 1000.0), 6.0, 10.0).is_err());
    }
}
