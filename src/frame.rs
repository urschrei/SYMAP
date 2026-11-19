//! Mapping between source-map coordinates and the printed character grid.
//!
//! The top border is row 0 and the left border column 0; the bottom and
//! right borders are rows `rows` and `cols`. At the standard 8 lines and 10
//! characters per inch a 5 x 8 inch map has borders at rows 0 and 40 and
//! columns 0 and 80, with 39 x 79 characters of content between them.

use crate::error::Error;
use crate::geom::{Extent, Point};

/// Largest map we are willing to allocate, in characters.
pub const MAX_CELLS: usize = 8_000_000;

/// Settings from electives 1, 2, 13, 14, 15 and 16.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    pub size: (Option<f64>, Option<f64>),
    pub scale: Option<f64>,
    pub margins: Margins,
    pub lines_per_inch: f64,
    pub cols_per_inch: f64,
    pub large: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            size: (None, None),
            scale: None,
            margins: Margins::default(),
            lines_per_inch: 8.0,
            cols_per_inch: 10.0,
            large: false,
        }
    }
}

/// Elective 14: how far each border is shifted, in inches.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Margins {
    pub top: f64,
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
}

/// The standard size of the larger map dimension, in inches.
pub const STANDARD_INCHES: f64 = 13.0;
/// Maps larger than this need elective 16.
pub const SAFEGUARD_INCHES: f64 = 72.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    /// Row of the bottom border.
    pub rows: usize,
    /// Column of the right border.
    pub cols: usize,
    pub lines_per_inch: f64,
    pub cols_per_inch: f64,
    /// Output inches per source-map unit.
    pub scale: f64,
    /// Source-map location of the upper-left extreme point.
    pub origin: Point,
    /// Grid position (row, column) of `origin`.
    pub offset: (f64, f64),
}

impl Frame {
    pub fn fit(extent: Extent, layout: &Layout) -> Result<(Frame, Vec<String>), Error> {
        let (lpi, cpi) = (layout.lines_per_inch, layout.cols_per_inch);
        if !(lpi > 0.0 && cpi > 0.0) {
            return Err(Error::general(
                "elective 15: characters per inch must be positive",
            ));
        }
        let Margins {
            top: t,
            left: l,
            bottom: b,
            right: r,
        } = layout.margins;
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
        let (h, w) = layout.size;
        // Elective 1 dimensions are rounded to whole lines and columns.
        let h = h.map(|x| (x * lpi).round() / lpi);
        let w = w.map(|x| (x * cpi).round() / cpi);
        let mut s = match (layout.scale, h, w) {
            (Some(s), _, _) => s,
            (None, Some(h), Some(w)) => ((h - t - b) / eh).min((w - l - r) / ew),
            (None, Some(h), None) => (h - t - b) / eh,
            (None, None, Some(w)) => (w - l - r) / ew,
            (None, None, None) => {
                ((STANDARD_INCHES - t - b) / eh).min((STANDARD_INCHES - l - r) / ew)
            }
        };
        if !(s > 0.0 && s.is_finite()) {
            return Err(Error::general(
                "map size, margins and scale leave no room for the map",
            ));
        }
        let mut h = h.unwrap_or(eh * s + t + b);
        let mut w = w.unwrap_or(ew * s + l + r);
        let mut warnings = Vec::new();
        let (mut mt, mut ml) = (t, l);
        if !layout.large && (h > SAFEGUARD_INCHES || w > SAFEGUARD_INCHES) {
            let f = STANDARD_INCHES / h.max(w);
            warnings.push(format!(
                "a {h:.1} x {w:.1} inch map exceeds {SAFEGUARD_INCHES} inches; reduced to {:.1} x {:.1} (use elective 16 to allow it)",
                h * f,
                w * f
            ));
            h *= f;
            w *= f;
            s *= f;
            mt *= f;
            ml *= f;
        }
        // Check the size before the conversion to usize, which saturates.
        let rows = (h * lpi).round().max(2.0);
        let cols = (w * cpi).round().max(2.0);
        let cells = (rows + 1.0) * (cols + 1.0);
        if cells.is_nan() || cells > MAX_CELLS as f64 {
            return Err(Error::general(format!(
                "a map of {rows:.0} x {cols:.0} characters is too large"
            )));
        }
        let (rows, cols) = (rows as usize, cols as usize);
        Ok((
            Frame {
                rows,
                cols,
                lines_per_inch: lpi,
                cols_per_inch: cpi,
                scale: s,
                origin: extent.min,
                offset: (mt * lpi, ml * cpi),
            },
            warnings,
        ))
    }

    /// Print positions, borders included: `(rows + 1) x (cols + 1)`.
    pub fn positions(&self) -> usize {
        (self.rows + 1) * (self.cols + 1)
    }

    /// Row-major index of print position (`row`, `col`).
    pub fn index(&self, row: usize, col: usize) -> usize {
        row * (self.cols + 1) + col
    }

    /// Fractional grid position (row, column) of a source-map location.
    pub fn grid(&self, p: Point) -> (f64, f64) {
        (
            self.offset.0 + (p.down - self.origin.down) * self.scale * self.lines_per_inch,
            self.offset.1 + (p.across - self.origin.across) * self.scale * self.cols_per_inch,
        )
    }

    /// The content character (inside the border) at a location. Locations
    /// exactly between characters go to the one below or to the right.
    pub fn cell(&self, p: Point) -> Option<(usize, usize)> {
        let (r, c) = self.grid(p);
        self.cell_at(r, c)
    }

    pub fn cell_at(&self, r: f64, c: f64) -> Option<(usize, usize)> {
        let (r, c) = ((r + 0.5).floor(), (c + 0.5).floor());
        let inside = |x: f64, n: usize| x >= 1.0 && x <= (n - 1) as f64;
        (inside(r, self.rows) && inside(c, self.cols)).then_some((r as usize, c as usize))
    }

    /// Source-map location of the centre of character (`row`, `col`).
    pub fn centre(&self, row: usize, col: usize) -> Point {
        Point::new(
            self.origin.down + (row as f64 - self.offset.0) / (self.scale * self.lines_per_inch),
            self.origin.across + (col as f64 - self.offset.1) / (self.scale * self.cols_per_inch),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext(h: f64, w: f64) -> Extent {
        Extent {
            min: Point::new(0.0, 0.0),
            max: Point::new(h, w),
        }
    }

    #[test]
    fn example_3_1_five_by_ten() {
        let layout = Layout {
            size: (Some(5.0), Some(10.0)),
            ..Default::default()
        };
        let (f, _) = Frame::fit(ext(5.0, 10.0), &layout).unwrap();
        // 39 rows and 99 columns of content between the borders.
        assert_eq!((f.rows, f.cols), (40, 100));
        assert_eq!(f.grid(Point::new(5.0, 10.0)), (40.0, 100.0));
        assert_eq!(f.cell(Point::new(2.5, 5.0)), Some((20, 50)));
        assert_eq!(f.cell(Point::new(0.0, 5.0)), None, "on the border");
        assert_eq!(f.cell(f.centre(17, 42)), Some((17, 42)));
    }

    #[test]
    fn standard_size_and_omitted_dimension() {
        let (f, _) = Frame::fit(ext(5.0, 10.0), &Layout::default()).unwrap();
        assert_eq!((f.rows, f.cols), (52, 130));
        let layout = Layout {
            size: (Some(4.0), None),
            ..Default::default()
        };
        let (f, _) = Frame::fit(ext(5.0, 10.0), &layout).unwrap();
        assert_eq!((f.rows, f.cols), (32, 80));
    }

    #[test]
    fn margins_shift_borders_and_scale() {
        let layout = Layout {
            size: (Some(6.0), Some(6.0)),
            margins: Margins {
                top: 0.5,
                left: 0.5,
                bottom: 0.5,
                right: 0.5,
            },
            ..Default::default()
        };
        let (f, _) = Frame::fit(ext(10.0, 10.0), &layout).unwrap();
        assert_eq!(f.scale, 0.5);
        assert_eq!(f.grid(Point::new(0.0, 0.0)), (4.0, 5.0));
        assert_eq!(f.grid(Point::new(10.0, 10.0)), (44.0, 55.0));
    }

    #[test]
    fn safeguard() {
        let layout = Layout {
            size: (Some(100.0), Some(50.0)),
            ..Default::default()
        };
        let (f, w) = Frame::fit(ext(1.0, 1.0), &layout).unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(f.rows, 104);
        let layout = Layout {
            large: true,
            ..layout
        };
        assert!(Frame::fit(ext(1.0, 1.0), &layout).unwrap().1.is_empty());
    }

    #[test]
    fn huge_character_density_is_too_large() {
        let layout = Layout {
            lines_per_inch: 1e20,
            ..Default::default()
        };
        let e = Frame::fit(ext(1.0, 1.0), &layout).unwrap_err();
        assert!(e.message.contains("too large"));
    }
}
