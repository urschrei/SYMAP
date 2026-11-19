//! Drawing one map: resolve electives, compute every character, overlay
//! lines, data points and legends.

use crate::classify::{Classes, Level};
use crate::deck::{Direction, Legend, MapKind, MapSpec, Packages, ZoneShape};
use crate::elective::{DotMode, Elective, Extrapolation, PointCount};
use crate::error::Error;
use crate::frame::{Frame, Layout};
use crate::geom::{self, Extent, Point};
use crate::interp::{PointIndex, Shepard, ShepardParams};
use crate::symbols::{MAX_LEVELS, Strikes, Symbolism};

/// White contour and boundary lines (electives 8 and 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lines {
    None,
    /// Only where the band is wide enough to keep its symbolism.
    Standard,
    Continuous,
}

/// Every elective, resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub layout: Layout,
    pub extremes: Option<(Point, Point)>,
    pub levels: usize,
    pub value_min: Option<f64>,
    pub value_max: Option<f64>,
    pub intervals: Option<Vec<f64>>,
    pub symbolism: Option<Symbolism>,
    pub lines: Lines,
    pub bars: bool,
    pub text: Vec<String>,
    pub print_values: bool,
    pub invalid_blank: bool,
    pub invalid_min: Option<f64>,
    pub invalid_max: Option<f64>,
    pub invalid_symbol: bool,
    pub numeric: bool,
    pub conformant_points: bool,
    pub force_contour: bool,
    pub extrapolation: Option<Extrapolation>,
    pub extrapolation_min: Option<f64>,
    pub extrapolation_max: Option<f64>,
    pub initial_radius: Option<f64>,
    pub max_radius: Option<f64>,
    pub point_count: Option<PointCount>,
    pub independence: bool,
    /// Elective 21: keep each character's computed value.
    pub store_values: bool,
    /// Elective 28: dot map.
    pub dots: Option<DotMode>,
    /// Elective 17 turns off the table printed before the map.
    pub tabular: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            layout: Layout::default(),
            extremes: None,
            levels: 5,
            value_min: None,
            value_max: None,
            intervals: None,
            symbolism: None,
            lines: Lines::Standard,
            bars: true,
            text: Vec::new(),
            print_values: false,
            invalid_blank: false,
            invalid_min: None,
            invalid_max: None,
            invalid_symbol: true,
            numeric: true,
            conformant_points: true,
            force_contour: false,
            extrapolation: None,
            extrapolation_min: None,
            extrapolation_max: None,
            initial_radius: None,
            max_radius: None,
            point_count: None,
            independence: false,
            store_values: false,
            dots: None,
            tabular: true,
        }
    }
}

impl Options {
    pub fn from_spec(spec: &MapSpec) -> (Options, Vec<String>) {
        let mut o = Options::default();
        let mut warnings = Vec::new();
        let mut continuous = false;
        for entry in &spec.electives {
            match &entry.elective {
                Elective::Size { height, width } => o.layout.size = (*height, *width),
                Elective::Extremes { min, max } => o.extremes = Some((*min, *max)),
                Elective::Levels(n) => o.levels = *n,
                Elective::ValueMin(v) => o.value_min = Some(*v),
                Elective::ValueMax(v) => o.value_max = Some(*v),
                Elective::Intervals(v) => o.intervals = Some(v.clone()),
                Elective::Symbolism(s) => o.symbolism = Some((**s).clone()),
                Elective::NoContourLines => o.lines = Lines::None,
                Elective::NoHistogram => o.bars = false,
                Elective::Text(t) => o.text = t.clone(),
                Elective::PrintValues => o.print_values = true,
                Elective::Repeat | Elective::OverprintAlignment => {}
                Elective::NoTabular => o.tabular = false,
                Elective::StoreValues => o.store_values = true,
                Elective::DotMap(mode) => o.dots = Some(*mode),
                Elective::Scale(s) => o.layout.scale = Some(*s),
                Elective::Margins(m) => o.layout.margins = *m,
                Elective::CharsPerInch { rows, cols } => {
                    o.layout.lines_per_inch = rows.unwrap_or(8.0);
                    o.layout.cols_per_inch = cols.unwrap_or(10.0);
                }
                Elective::LargeMaps => o.layout.large = true,
                Elective::InvalidBlank => o.invalid_blank = true,
                Elective::InvalidMin(v) => o.invalid_min = Some(*v),
                Elective::InvalidMax(v) => o.invalid_max = Some(*v),
                Elective::ContinuousContours => continuous = true,
                Elective::NoInvalidSymbol => o.invalid_symbol = false,
                Elective::NoNumericInterpretation => o.numeric = false,
                Elective::NoDataPoints => o.conformant_points = false,
                Elective::ForceContour => o.force_contour = true,
                Elective::Extrapolation(f) => o.extrapolation = Some(*f),
                Elective::ExtrapolationMin(v) => o.extrapolation_min = Some(*v),
                Elective::ExtrapolationMax(v) => o.extrapolation_max = Some(*v),
                Elective::InitialRadius(r) => o.initial_radius = Some(*r),
                Elective::MaxRadius(r) => o.max_radius = Some(*r),
                Elective::PointCount(count) => o.point_count = Some(*count),
                Elective::Independence => o.independence = true,
                Elective::Unsupported { number, .. } => warnings.push(format!(
                    "line {}: elective {number} is not supported; ignored",
                    entry.line
                )),
            }
        }
        o.lines = match (o.lines, continuous) {
            (Lines::Standard, true) => Lines::Continuous,
            (lines @ (Lines::None | Lines::Standard | Lines::Continuous), _) => lines,
        };
        (o, warnings)
    }

    /// Electives 31, 36 and 37 together, with no values, request a proximal map.
    pub fn proximal(&self) -> bool {
        self.extrapolation == Some(Extrapolation::Blank)
            && self.point_count == Some(PointCount::Blank)
            && self.independence
    }
}

/// Counts of data points (or zones) by level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frequency {
    pub low: usize,
    pub levels: Vec<usize>,
    pub high: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintedMap {
    pub kind: MapKind,
    pub titles: [String; 3],
    pub text: Vec<String>,
    pub frame: Frame,
    /// `(rows + 1) x (cols + 1)` print positions including the border, row-major.
    pub grid: Vec<Strikes>,
    /// Level of each print position before lines, points and legends.
    pub levels: Vec<Option<Level>>,
    pub classes: Classes,
    pub symbolism: Symbolism,
    /// Extremes of the valid data values.
    pub data_range: (f64, f64),
    pub frequency: Frequency,
    pub numeric: bool,
    pub bars: bool,
    pub warnings: Vec<String>,
    /// Elective 21: each print position's computed value (row-major, like `grid`).
    pub values: Option<Vec<Option<f64>>>,
    /// The table printed before the map: data points, or zones for a
    /// conformant map (unless elective 17).
    pub table: Vec<TableRow>,
    pub tabular: bool,
    /// The initial search radius of a contour map, in source-map units.
    pub search_radius: Option<f64>,
}

/// One row of the table printed before a map.
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    /// Point or zone number, in input order.
    pub number: usize,
    /// `None` for data points.
    pub shape: Option<ZoneShape>,
    /// Print position (of a zone's centre).
    pub row: i64,
    pub col: i64,
    /// The E-VALUES reference used, if any.
    pub datum: Option<usize>,
    /// `None` for invalid data.
    pub value: Option<f64>,
    pub level: Option<Level>,
}

/// The E-VALUES entry a reference maps to after E1-VALUES INDEX.
fn datum(pk: &Packages, r: usize) -> Option<usize> {
    match pk.index.iter().rev().find(|(from, _)| *from == r) {
        Some((_, to)) => *to,
        None => Some(r),
    }
}

/// Centre of a zone outline: the area centroid, or the mean of its vertices.
fn centre_of(z: &crate::deck::Conformoline) -> Point {
    let v = &z.vertices;
    let n = v.len() as f64;
    let mean = Point::new(
        v.iter().map(|p| p.down).sum::<f64>() / n,
        v.iter().map(|p| p.across).sum::<f64>() / n,
    );
    if z.shape != ZoneShape::Area || v.len() < 3 {
        return mean;
    }
    let (mut a, mut cd, mut ca) = (0.0, 0.0, 0.0);
    for i in 0..v.len() {
        let (p, q) = (v[i], v[(i + 1) % v.len()]);
        let cross = p.across * q.down - q.across * p.down;
        a += cross;
        cd += (p.down + q.down) * cross;
        ca += (p.across + q.across) * cross;
    }
    if a.abs() < 1e-12 {
        mean
    } else {
        Point::new(cd / (3.0 * a), ca / (3.0 * a))
    }
}

/// A deterministic scatter order for dot maps.
fn scatter(r: usize, c: usize) -> u64 {
    let mut h = (r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (c as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h.wrapping_mul(0x94D0_49BB_1331_11EB) ^ (h >> 29)
}

impl PrintedMap {
    pub fn width(&self) -> usize {
        self.frame.cols + 1
    }

    pub fn at(&self, row: usize, col: usize) -> Strikes {
        self.grid[self.frame.index(row, col)]
    }

    pub fn level(&self, row: usize, col: usize) -> Option<Level> {
        self.levels[self.frame.index(row, col)]
    }
}

/// Values per item (data point or zone reference), `None` where invalid.
fn resolve_values(
    pk: &Packages,
    refs: &[usize],
    o: &Options,
    line: usize,
) -> Result<Vec<Option<f64>>, Error> {
    refs.iter()
        .map(|&r| {
            let Some(target) = datum(pk, r) else {
                return Ok(None);
            };
            let raw = *pk.values.get(target - 1).ok_or_else(|| {
                Error::at(
                    line,
                    format!(
                        "value {target} is needed but E-VALUES has only {}",
                        pk.values.len()
                    ),
                )
            })?;
            let v = match raw {
                None if o.invalid_blank => return Ok(None),
                None => 0.0,
                Some(v) if o.invalid_blank && v == 0.0 && v.is_sign_negative() => return Ok(None),
                Some(v) => v,
            };
            let invalid = match (o.invalid_min, o.invalid_max) {
                (Some(a), Some(b)) if a <= b => (a..=b).contains(&v),
                (Some(a), Some(b)) => v >= a || v <= b,
                (Some(a), None) => v >= a,
                (None, Some(b)) => v <= b,
                (None, None) => false,
            };
            Ok((!invalid).then_some(v))
        })
        .collect()
}

/// Character positions along a polyline, in grid coordinates, in no
/// particular order.
fn trace(frame: &Frame, vertices: &[Point]) -> Vec<(usize, usize)> {
    let g: Vec<(f64, f64)> = vertices.iter().map(|p| frame.grid(*p)).collect();
    let along = g.windows(2).flat_map(|w| raster(frame, w[0], w[1]));
    let mut out: Vec<(usize, usize)> = g
        .iter()
        .copied()
        .chain(along)
        .filter_map(|(r, c)| frame.cell_at(r, c))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The characters a segment passes through, as SYMAP draws lines: the
/// nearest character on each printed line crossed (on each column, for a
/// shallow segment). Grid coordinates. Only lines (or columns) from the top
/// (or left) border to the bottom (or right) border are given; the other
/// coordinate may lie off the map.
fn raster(
    frame: &Frame,
    (r0, c0): (f64, f64),
    (r1, c1): (f64, f64),
) -> impl Iterator<Item = (f64, f64)> {
    let steep = (r1 - r0).abs() > (c1 - c0).abs();
    let (a0, a1, n) = if steep {
        (r0, r1, frame.rows)
    } else {
        (c0, c1, frame.cols)
    };
    // Clamp before the conversion: a zoomed map can put a segment's ends
    // far outside the frame.
    let lo = a0.min(a1).ceil().max(0.0);
    let hi = a0.max(a1).floor().min(n as f64);
    let steps = lo as usize..(hi + 1.0).max(lo) as usize;
    steps.map(move |a| {
        let a = a as f64;
        let t = if a1 != a0 { (a - a0) / (a1 - a0) } else { 0.0 };
        if steep {
            (a, ((c0 + t * (c1 - c0)) + 0.5).floor())
        } else {
            (((r0 + t * (r1 - r0)) + 0.5).floor(), a)
        }
    })
}

/// The content characters, row by row, within a character of a polygon's
/// bounding box: the only ones whose centres can be inside it.
fn cells_around(frame: &Frame, vertices: &[Point]) -> impl Iterator<Item = (usize, usize)> {
    let (mut lo, mut hi) = (
        (f64::INFINITY, f64::INFINITY),
        (f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for &p in vertices {
        let (r, c) = frame.grid(p);
        lo = (lo.0.min(r), lo.1.min(c));
        hi = (hi.0.max(r), hi.1.max(c));
    }
    // Clamp in floating point, then convert: positions can be far off the map.
    let span = |lo: f64, hi: f64, n: usize| {
        let first = (lo - 1.0).floor().max(1.0);
        let end = (hi + 2.0).floor().min(n as f64).max(first);
        first as usize..end as usize
    };
    let (rows, cols) = (span(lo.0, hi.0, frame.rows), span(lo.1, hi.1, frame.cols));
    rows.flat_map(move |r| cols.clone().map(move |c| (r, c)))
}

/// Draw one map.
pub fn draw(spec: &MapSpec) -> Result<PrintedMap, Error> {
    let at = |m: String| Error::at(spec.line, m);
    let pk = &spec.packages;
    let (o, mut warnings) = Options::from_spec(spec);

    let kind = if !pk.conformolines.is_empty() && !o.force_contour {
        MapKind::Conformant
    } else if o.proximal() {
        MapKind::Proximal
    } else {
        MapKind::Contour
    };
    match kind {
        MapKind::Conformant if o.print_values => warnings.push(
            "elective 11 prints the values of data points, which a conformant map does not \
             have; ignored"
                .into(),
        ),
        MapKind::Contour | MapKind::Proximal if o.dots.is_some() => warnings
            .push("elective 28 makes a dot map of conformant zones; ignored on this map".into()),
        MapKind::Contour | MapKind::Proximal | MapKind::Conformant => {}
    }
    if kind != MapKind::Conformant && pk.points.is_empty() {
        return Err(at(
            "a contour or proximal map needs a B-DATA POINTS package".into(),
        ));
    }

    // Values for the items being mapped.
    let (item_values, point_values) = match kind {
        MapKind::Conformant => {
            let mut zone_refs: Vec<usize> = pk.conformolines.iter().map(|z| z.reference).collect();
            zone_refs.sort_unstable();
            zone_refs.dedup();
            let zv = resolve_values(pk, &zone_refs, &o, spec.line)?;
            let lookup = move |r: usize| zv[zone_refs.binary_search(&r).unwrap()];
            let items: Vec<Option<f64>> = pk
                .conformolines
                .iter()
                .map(|z| lookup(z.reference))
                .collect();
            (items, Vec::new())
        }
        MapKind::Contour | MapKind::Proximal => {
            let point_refs: Vec<usize> = (1..=pk.points.len()).collect();
            let v = resolve_values(pk, &point_refs, &o, spec.line)?;
            (v.clone(), v)
        }
    };
    let valid: Vec<f64> = item_values.iter().flatten().copied().collect();
    if valid.is_empty() {
        return Err(at("there are no valid data values to map".into()));
    }
    let data_range = valid
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &v| {
            (l.min(v), h.max(v))
        });

    // Levels.
    if !(1..=MAX_LEVELS).contains(&o.levels) {
        return Err(at(format!(
            "between 1 and {MAX_LEVELS} levels are supported"
        )));
    }
    let freq_values: Vec<f64> = match kind {
        MapKind::Conformant => {
            // One value per zone reference, not per outline.
            let mut seen = std::collections::BTreeMap::new();
            for (z, v) in pk.conformolines.iter().zip(&item_values) {
                if let Some(v) = v {
                    seen.insert(z.reference, *v);
                }
            }
            seen.into_values().collect()
        }
        MapKind::Contour | MapKind::Proximal => valid,
    };
    let classes = Classes::new(
        o.value_min.unwrap_or(data_range.0),
        o.value_max.unwrap_or(data_range.1),
        o.levels,
        o.intervals.as_deref(),
        &freq_values,
    )
    .map_err(|e| at(e.message))?;
    let symbolism = o
        .symbolism
        .clone()
        .unwrap_or_else(|| Symbolism::standard(classes.levels()));

    // Frame.
    let extent = match o.extremes {
        Some((min, max)) => Extent { min, max },
        None => {
            let pts: Vec<Point> = if kind == MapKind::Conformant {
                pk.conformolines
                    .iter()
                    .flat_map(|z| z.vertices.clone())
                    .collect()
            } else if !pk.outlines.is_empty() {
                pk.outlines.concat()
            } else {
                pk.points.clone()
            };
            Extent::of(&pts).ok_or_else(|| at("nothing to map".into()))?
        }
    };
    let (frame, w) = Frame::fit(extent, &o.layout).map_err(|e| at(e.message))?;
    warnings.extend(w);

    let d = Drawing {
        pk,
        o: &o,
        kind,
        frame,
        classes: &classes,
        symbolism: &symbolism,
        item_values: &item_values,
        point_values: &point_values,
    };
    let (surface, search_radius) = d.fill(data_range);
    let levels: Vec<Option<Level>> = surface
        .fill
        .iter()
        .map(|f| match f {
            Fill::Value(l) => Some(*l),
            Fill::Background | Fill::NoData => None,
        })
        .collect();
    let mut grid = d.paint(&surface.fill);
    if let (Some(mode), MapKind::Conformant) = (o.dots, kind) {
        d.dot_map(&mut grid, &surface.zone_of, mode);
    }
    match (o.lines, kind) {
        (Lines::None, _) => {}
        (Lines::Standard | Lines::Continuous, MapKind::Conformant) => {
            d.zone_boundaries(&mut grid, &surface.zone_of);
        }
        (lines @ (Lines::Standard | Lines::Continuous), MapKind::Contour | MapKind::Proximal) => {
            d.level_lines(&mut grid, &levels, lines == Lines::Standard);
        }
    }
    if kind != MapKind::Conformant || o.conformant_points {
        d.data_points(&mut grid);
    }
    d.legends(&mut grid);
    let frequency = frequency(&classes, &freq_values);
    let table = d.table();

    Ok(PrintedMap {
        kind,
        titles: spec.titles.clone(),
        text: o.text.clone(),
        frame,
        grid,
        levels,
        classes,
        symbolism,
        data_range,
        frequency,
        numeric: o.numeric,
        bars: o.bars,
        warnings,
        values: o.store_values.then_some(surface.value),
        table,
        tabular: o.tabular,
        search_radius,
    })
}

/// What fills a print position before lines, points and legends.
#[derive(Clone, Copy, PartialEq)]
enum Fill {
    Background,
    NoData,
    Value(Level),
}

/// The fill of every print position, row-major.
struct Surface {
    fill: Vec<Fill>,
    /// Which conformant zone (value reference) fills each position.
    zone_of: Vec<Option<usize>>,
    /// Each position's computed value, before classing.
    value: Vec<Option<f64>>,
}

impl Surface {
    /// Fill position `i` with a zone's value.
    fn set_zone(&mut self, i: usize, reference: usize, value: f64, level: Level) {
        self.fill[i] = Fill::Value(level);
        self.zone_of[i] = Some(reference);
        self.value[i] = Some(value);
    }
}

/// One map's settings and data, resolved, for the stages of drawing it.
struct Drawing<'a> {
    pk: &'a Packages,
    o: &'a Options,
    kind: MapKind,
    frame: Frame,
    classes: &'a Classes,
    symbolism: &'a Symbolism,
    /// Values per item mapped: data points, or zone outlines.
    item_values: &'a [Option<f64>],
    /// Values per data point; empty for a conformant map.
    point_values: &'a [Option<f64>],
}

impl Drawing<'_> {
    /// Fill every position inside the outlines (or zones), and give the
    /// initial search radius of a contour map.
    fn fill(&self, data_range: (f64, f64)) -> (Surface, Option<f64>) {
        let n = self.frame.positions();
        let mut s = Surface {
            fill: vec![Fill::Background; n],
            zone_of: vec![None; n],
            value: vec![None; n],
        };
        let valid_pts: Vec<(Point, f64)> = self
            .pk
            .points
            .iter()
            .zip(self.point_values)
            .filter_map(|(p, v)| v.map(|v| (*p, v)))
            .collect();
        let mut search_radius = None;
        match self.kind {
            MapKind::Contour => {
                search_radius = Some(self.fill_contour(&mut s, &valid_pts, data_range))
            }
            MapKind::Proximal => self.fill_proximal(&mut s, &valid_pts),
            MapKind::Conformant => self.fill_conformant(&mut s),
        }
        (s, search_radius)
    }

    /// Whether a location is inside the A-OUTLINE rings (or there are none).
    fn inside(&self, p: Point) -> bool {
        self.pk.outlines.is_empty() || self.pk.outlines.iter().any(|r| geom::in_ring(p, r))
    }

    fn fill_contour(
        &self,
        s: &mut Surface,
        valid_pts: &[(Point, f64)],
        data_range: (f64, f64),
    ) -> f64 {
        let (o, frame) = (self.o, &self.frame);
        let (pts, zs): (Vec<Point>, Vec<f64>) = valid_pts.iter().copied().unzip();
        // The initial radius is based on "the area occupied by the data
        // points" (Shepard 1970, note 21): their bounding rectangle.
        let area = {
            let e = Extent::of(&pts).unwrap();
            (e.height() * e.width()).max(f64::MIN_POSITIVE)
        };
        let (min_points, max_points) = match o.point_count {
            Some(PointCount::Given { min, max }) => (min, max),
            Some(PointCount::Blank) | None => (4, 10),
        };
        let params = ShepardParams {
            min_points,
            max_points,
            fraction: match o.extrapolation {
                None => 0.1,
                Some(Extrapolation::Blank) => 0.0,
                Some(Extrapolation::Fraction(f)) => f,
            },
            initial_radius: o.initial_radius,
            max_radius: o.max_radius,
            // 1.5 characters, in source-map units.
            slope_min_distance: 1.5 / (frame.scale * frame.cols_per_inch),
            ..Default::default()
        };
        let lo = o.extrapolation_min.unwrap_or(data_range.0);
        let hi = o.extrapolation_max.unwrap_or(data_range.1);
        let shepard = Shepard::new(&pts, &zs, area, params, &self.pk.barriers);
        // A location within half a character of data points takes their
        // (average) value; elsewhere the surface is interpolated.
        let grid_pts: Vec<((f64, f64), f64)> =
            valid_pts.iter().map(|&(p, v)| (frame.grid(p), v)).collect();
        let at_location = |r: usize, c: usize| -> Option<f64> {
            // The sum starts from -0.0, as `Sum` for f64 does.
            let (sum, n) = grid_pts
                .iter()
                .filter(|((gr, gc), _)| {
                    (gr - r as f64).abs() <= 0.5 && (gc - c as f64).abs() <= 0.5
                })
                .fold((-0.0, 0usize), |(sum, n), x| (sum + x.1, n + 1));
            if n == 0 {
                shepard.eval(frame.centre(r, c))
            } else {
                Some(sum / n as f64)
            }
        };
        // Standard SYMAP computes about one character in six (every second
        // line, every third column) and derives the rest; elective 37
        // computes every character independently.
        let (sr, sc) = if o.independence { (1, 1) } else { (2, 3) };
        let anchor = |i: usize, step: usize, n: usize| -> (usize, usize) {
            let a = i / step * step;
            (a, (a + step).min(n))
        };
        // Values at the computed characters; the others stay `None`.
        let mut samples: Vec<Option<f64>> = vec![None; frame.positions()];
        let sample_rows = (0..=frame.rows).filter(|r| r % sr == 0 || *r == frame.rows);
        for r in sample_rows {
            let sample_cols = (0..=frame.cols).filter(|c| c % sc == 0 || *c == frame.cols);
            for c in sample_cols {
                samples[frame.index(r, c)] = at_location(r, c);
            }
        }
        let value = |r: usize, c: usize| -> Option<f64> {
            let ((r0, r1), (c0, c1)) = (anchor(r, sr, frame.rows), anchor(c, sc, frame.cols));
            let g = |r, c| samples[frame.index(r, c)];
            let (a, b, cc, d) = (g(r0, c0)?, g(r0, c1)?, g(r1, c0)?, g(r1, c1)?);
            let t = if r1 > r0 {
                (r - r0) as f64 / (r1 - r0) as f64
            } else {
                0.0
            };
            let u = if c1 > c0 {
                (c - c0) as f64 / (c1 - c0) as f64
            } else {
                0.0
            };
            Some((a * (1.0 - u) + b * u) * (1.0 - t) + (cc * (1.0 - u) + d * u) * t)
        };
        for r in 1..frame.rows {
            for c in 1..frame.cols {
                if self.inside(frame.centre(r, c)) {
                    let i = frame.index(r, c);
                    s.fill[i] = match value(r, c) {
                        Some(v) => {
                            let v = v.clamp(lo, hi.max(lo));
                            s.value[i] = Some(v);
                            Fill::Value(self.classes.level(v))
                        }
                        None => Fill::NoData,
                    };
                }
            }
        }
        shepard.initial_radius()
    }

    fn fill_proximal(&self, s: &mut Surface, valid_pts: &[(Point, f64)]) {
        let frame = &self.frame;
        let pts: Vec<Point> = valid_pts.iter().map(|x| x.0).collect();
        let index = PointIndex::new(&pts);
        for r in 1..frame.rows {
            for c in 1..frame.cols {
                let p = frame.centre(r, c);
                if let Some(k) = index.nearest(&pts, p).filter(|_| self.inside(p)) {
                    let v = valid_pts[k].1;
                    let i = frame.index(r, c);
                    s.value[i] = Some(v);
                    s.fill[i] = Fill::Value(self.classes.level(v));
                }
            }
        }
    }

    /// Fill each zone in deck order: an area keeps the positions it claims
    /// first; lines and points are drawn over what is there.
    fn fill_conformant(&self, s: &mut Surface) {
        let frame = &self.frame;
        for (z, v) in self.pk.conformolines.iter().zip(self.item_values) {
            let Some(v) = v else { continue };
            let level = self.classes.level(*v);
            match z.shape {
                ZoneShape::Area => {
                    for (r, c) in cells_around(frame, &z.vertices) {
                        let i = frame.index(r, c);
                        if s.fill[i] == Fill::Background
                            && geom::in_ring(frame.centre(r, c), &z.vertices)
                        {
                            s.set_zone(i, z.reference, *v, level);
                        }
                    }
                }
                ZoneShape::Line | ZoneShape::Point => {
                    for (r, c) in trace(frame, &z.vertices) {
                        s.set_zone(frame.index(r, c), z.reference, *v, level);
                    }
                }
            }
        }
    }

    /// The symbolism of every position.
    fn paint(&self, fill: &[Fill]) -> Vec<Strikes> {
        let s = self.symbolism;
        fill.iter()
            .map(|f| match f {
                Fill::Background => s.background,
                Fill::NoData => s.no_data,
                Fill::Value(level) => s.fill(*level),
            })
            .collect()
    }

    /// Elective 28: each zone as scattered dots instead of tone.
    fn dot_map(&self, grid: &mut [Strikes], zone_of: &[Option<usize>], mode: DotMode) {
        let width = self.frame.cols + 1;
        let mut cells: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
        for (i, z) in zone_of.iter().enumerate() {
            if let Some(z) = z {
                cells.entry(*z).or_default().push(i);
                grid[i] = self.symbolism.background;
            }
        }
        let (lo, hi) = (self.classes.min(), self.classes.max());
        let n = self.classes.levels();
        for (z, mut list) in cells {
            let Some(v) = self
                .item_values
                .iter()
                .zip(&self.pk.conformolines)
                .find(|(_, c)| c.reference == z)
                .and_then(|(v, _)| *v)
            else {
                continue;
            };
            let share = match mode {
                DotMode::PerDot(k) => (v / k).round().max(0.0) / list.len() as f64,
                DotMode::Percent => (v - lo) / (hi - lo),
                DotMode::ByLevel => match self.classes.level(v) {
                    Level::Low => 0.0,
                    Level::In(k) => (k as f64 + 1.0) / n as f64,
                    Level::High => 1.0,
                },
            };
            let k = ((share.clamp(0.0, 1.0) * list.len() as f64).round() as usize).min(list.len());
            list.sort_by_key(|&i| scatter(i / width, i % width));
            for &i in &list[..k] {
                grid[i] = Strikes::single('.');
            }
        }
    }

    /// Conformant boundaries are drawn where they run: each zone edge is
    /// traced to the nearest character on every line it crosses (or every
    /// column, for shallow edges), and a traced character becomes a line
    /// where it separates two different zones. Outer edges stay filled.
    /// (Matches Example 1-22 of the 1971 manual, including lines between
    /// zones on the same level.)
    fn zone_boundaries(&self, grid: &mut [Strikes], zone_of: &[Option<usize>]) {
        let frame = &self.frame;
        let zone = |r: i64, c: i64| -> Option<usize> {
            (r >= 1 && c >= 1 && r < frame.rows as i64 && c < frame.cols as i64)
                .then(|| zone_of[frame.index(r as usize, c as usize)])
                .flatten()
        };
        let areas = self
            .pk
            .conformolines
            .iter()
            .filter(|z| z.shape == ZoneShape::Area);
        for z in areas {
            let n = z.vertices.len();
            for k in 0..n {
                let (p0, p1) = (
                    frame.grid(z.vertices[k]),
                    frame.grid(z.vertices[(k + 1) % n]),
                );
                let steep = (p1.0 - p0.0).abs() > (p1.1 - p0.1).abs();
                for (r, c) in raster(frame, p0, p1) {
                    let (r, c) = (r as i64, c as i64);
                    let (x, y) = if steep {
                        (zone(r, c - 1), zone(r, c + 1))
                    } else {
                        (zone(r - 1, c), zone(r + 1, c))
                    };
                    if let (Some(x), Some(y)) = (x, y) {
                        if x != y {
                            grid[frame.index(r as usize, c as usize)] = self.symbolism.contour;
                        }
                    }
                }
            }
        }
    }

    /// Lines are made in printing order. A character becomes a line where
    /// its level differs from the character above it, or from the one to
    /// its left unless that one is already a line. So a boundary takes one
    /// character, on whichever side the scan reaches second, rather than
    /// always the higher level. (Matches Examples 1-5, 1-16 and 1-17 of the
    /// 1971 manual better than either side by level.) With `band`, as in
    /// standard SYMAP, a line that would leave no symbol between it and the
    /// next is dropped; elective 22 keeps them.
    fn level_lines(&self, grid: &mut [Strikes], levels: &[Option<Level>], band: bool) {
        let frame = &self.frame;
        let level = |r: usize, c: usize| levels[frame.index(r, c)];
        let mut line = vec![false; levels.len()];
        for r in 1..frame.rows {
            for c in 1..frame.cols {
                let Some(me) = level(r, c) else { continue };
                let differs = |(nr, nc): (usize, usize), (br, bc): (usize, usize)| {
                    level(nr, nc).is_some_and(|x| x != me) && (!band || level(br, bc) == Some(me))
                };
                if differs((r - 1, c), (r + 1, c))
                    || (!line[frame.index(r, c - 1)] && differs((r, c - 1), (r, c + 1)))
                {
                    line[frame.index(r, c)] = true;
                    grid[frame.index(r, c)] = self.symbolism.contour;
                }
            }
        }
    }

    /// Data point symbols, or their values (elective 11).
    fn data_points(&self, grid: &mut [Strikes]) {
        let (pk, frame, o) = (self.pk, &self.frame, self.o);
        let mut by_cell: std::collections::BTreeMap<(usize, usize), Vec<usize>> =
            Default::default();
        for (i, p) in pk.points.iter().enumerate() {
            if let Some(cell) = frame.cell(*p) {
                by_cell.entry(cell).or_default().push(i);
            }
        }
        let point_level = |i: usize| -> Option<Option<Level>> {
            match self.kind {
                MapKind::Conformant => {
                    let p = pk.points[i];
                    let z = pk
                        .conformolines
                        .iter()
                        .zip(self.item_values)
                        .find(|(z, _)| {
                            z.shape == ZoneShape::Area && geom::in_ring(p, &z.vertices)
                        })?;
                    Some(z.1.map(|v| self.classes.level(v)))
                }
                MapKind::Contour | MapKind::Proximal => {
                    Some(self.point_values[i].map(|v| self.classes.level(v)))
                }
            }
        };
        for (&(r, c), items) in &by_cell {
            let i = items[0];
            let Some(level) = point_level(i) else {
                continue;
            };
            if o.print_values && items.len() == 1 {
                if let Some(v) = self.point_values.get(i).copied().flatten() {
                    let s = format!("{v:.2}");
                    let dot = s.find('.').unwrap();
                    for (k, ch) in s.chars().enumerate() {
                        let col = c as i64 + k as i64 - dot as i64;
                        if (1..frame.cols as i64).contains(&col) {
                            grid[frame.index(r, col as usize)] = Strikes::single(ch);
                        }
                    }
                    continue;
                }
            }
            grid[frame.index(r, c)] = if items.len() > 1 {
                self.symbolism.superimposed
            } else {
                match level {
                    None if o.invalid_symbol => self.symbolism.invalid,
                    None => continue,
                    Some(level) => self.symbolism.point_symbol(level),
                }
            };
        }
    }

    /// C-OTOLEGENDS, on top of everything.
    fn legends(&self, grid: &mut [Strikes]) {
        let frame = &self.frame;
        for l in &self.pk.legends {
            match l {
                Legend::Symbol { strikes, at, shift } => {
                    let (r, c) = frame.grid(*at);
                    if let Some((r, c)) = frame.cell_at(r + shift.0, c + shift.1) {
                        grid[frame.index(r, c)] = *strikes;
                    }
                }
                Legend::Text {
                    text,
                    direction,
                    at,
                    shift,
                } => {
                    let (r0, c0) = frame.grid(*at);
                    for (k, ch) in text.chars().enumerate() {
                        let (dr, dc) = match direction {
                            Direction::Down => (k as f64, 0.0),
                            Direction::Across => (0.0, k as f64),
                        };
                        if let Some((r, c)) = frame.cell_at(r0 + shift.0 + dr, c0 + shift.1 + dc) {
                            grid[frame.index(r, c)] = Strikes::single(ch);
                        }
                    }
                }
                Legend::Line { strikes, vertices } => {
                    for (r, c) in trace(frame, vertices) {
                        grid[frame.index(r, c)] = *strikes;
                    }
                }
                Legend::Area { strikes, vertices } => {
                    for (r, c) in cells_around(frame, vertices) {
                        if geom::in_ring(frame.centre(r, c), vertices) {
                            grid[frame.index(r, c)] = *strikes;
                        }
                    }
                }
            }
        }
    }

    /// The table printed before the map: zones for a conformant map,
    /// otherwise data points.
    fn table(&self) -> Vec<TableRow> {
        let (pk, frame) = (self.pk, &self.frame);
        let level_of = |v: Option<f64>| v.map(|v| self.classes.level(v));
        match self.kind {
            MapKind::Conformant => pk
                .conformolines
                .iter()
                .zip(self.item_values)
                .enumerate()
                .map(|(i, (z, v))| {
                    let (r, c) = frame.grid(centre_of(z));
                    TableRow {
                        number: i + 1,
                        shape: Some(z.shape),
                        row: (r + 0.5).floor() as i64,
                        col: (c + 0.5).floor() as i64,
                        datum: datum(pk, z.reference),
                        value: *v,
                        level: level_of(*v),
                    }
                })
                .collect(),
            MapKind::Contour | MapKind::Proximal => pk
                .points
                .iter()
                .zip(self.point_values)
                .enumerate()
                .map(|(i, (p, v))| {
                    let (r, c) = frame.grid(*p);
                    TableRow {
                        number: i + 1,
                        shape: None,
                        row: (r + 0.5).floor() as i64,
                        col: (c + 0.5).floor() as i64,
                        datum: datum(pk, i + 1),
                        value: *v,
                        level: level_of(*v),
                    }
                })
                .collect(),
        }
    }
}

/// Counts of data values (zones, for a conformant map) by level.
fn frequency(classes: &Classes, values: &[f64]) -> Frequency {
    let mut frequency = Frequency {
        levels: vec![0; classes.levels()],
        ..Default::default()
    };
    for &v in values {
        match classes.level(v) {
            Level::Low => frequency.low += 1,
            Level::High => frequency.high += 1,
            Level::In(k) => frequency.levels[k as usize] += 1,
        }
    }
    frequency
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_stops_at_the_frame() {
        let extent = Extent {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
        };
        let (frame, _) = Frame::fit(extent, &Layout::default()).unwrap();
        let steps = raster(&frame, (-1e12, 5.0), (1e12, 6.0)).count();
        assert_eq!(steps, frame.rows + 1);
        assert_eq!(raster(&frame, (-9.0, 5.0), (-2.0, 6.0)).count(), 0);
        let far = [Point::new(-1e9, 5.0), Point::new(1e9, 5.0)];
        assert_eq!(trace(&frame, &far).len(), frame.rows - 1);
    }

    #[test]
    fn cells_around_a_polygon() {
        let extent = Extent {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
        };
        let (frame, _) = Frame::fit(extent, &Layout::default()).unwrap();
        let square = [
            Point::new(2.0, 2.0),
            Point::new(2.0, 4.0),
            Point::new(4.0, 4.0),
            Point::new(4.0, 2.0),
        ];
        let cells: Vec<_> = cells_around(&frame, &square).collect();
        let inside = (1..frame.rows)
            .flat_map(|r| (1..frame.cols).map(move |c| (r, c)))
            .filter(|&(r, c)| geom::in_ring(frame.centre(r, c), &square));
        assert!(inside.into_iter().all(|cell| cells.contains(&cell)));
        assert!(cells.len() < (frame.rows - 1) * (frame.cols - 1) / 4);
        let off = square.map(|p| Point::new(p.down + 1e9, p.across));
        assert_eq!(cells_around(&frame, &off).count(), 0);
        let huge = square.map(|p| Point::new((p.down - 3.0) * 1e9, (p.across - 3.0) * 1e9));
        assert_eq!(
            cells_around(&frame, &huge).count(),
            (frame.rows - 1) * (frame.cols - 1)
        );
    }
}
