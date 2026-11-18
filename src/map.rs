//! Drawing one map: resolve electives, compute every character, overlay
//! lines, data points and legends.

use crate::classify::{Classes, Level};
use crate::deck::{Legend, MapKind, MapSpec, Packages, ZoneShape};
use crate::elective::Elective;
use crate::error::Error;
use crate::frame::{Frame, Layout};
use crate::geom::{self, Extent, Point};
use crate::interp::{self, Shepard, ShepardParams};
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
    pub extrapolation: Option<Option<f64>>,
    pub extrapolation_min: Option<f64>,
    pub extrapolation_max: Option<f64>,
    pub initial_radius: Option<f64>,
    pub max_radius: Option<f64>,
    pub point_count: Option<(Option<f64>, Option<f64>)>,
    pub independence: bool,
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
                Elective::Repeat | Elective::NoTabular | Elective::OverprintAlignment => {}
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
                Elective::PointCount { min, max } => o.point_count = Some((*min, *max)),
                Elective::Independence => o.independence = true,
                Elective::Unsupported { number, .. } => warnings.push(format!(
                    "line {}: elective {number} is not supported; ignored",
                    entry.line
                )),
            }
        }
        if continuous && o.lines != Lines::None {
            o.lines = Lines::Continuous;
        }
        (o, warnings)
    }

    /// Electives 31, 36 and 37 together, with no values, request a proximal map.
    pub fn proximal(&self) -> bool {
        self.extrapolation == Some(None)
            && self.point_count == Some((None, None))
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
}

impl PrintedMap {
    pub fn width(&self) -> usize {
        self.frame.cols + 1
    }

    pub fn at(&self, row: usize, col: usize) -> Strikes {
        self.grid[row * self.width() + col]
    }

    pub fn level(&self, row: usize, col: usize) -> Option<Level> {
        self.levels[row * self.width() + col]
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
            let target = match pk.index.iter().rev().find(|(from, _)| *from == r) {
                Some((_, Some(to))) => *to,
                Some((_, None)) => return Ok(None),
                None => r,
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

/// Character positions along a polyline, in grid coordinates.
fn trace(frame: &Frame, vertices: &[Point]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let g: Vec<(f64, f64)> = vertices.iter().map(|p| frame.grid(*p)).collect();
    if g.len() == 1 {
        out.extend(frame.cell_at(g[0].0, g[0].1));
    }
    for w in g.windows(2) {
        let (a, b) = (w[0], w[1]);
        let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 4.0)
            .ceil()
            .max(1.0) as usize;
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            if let Some(c) = frame.cell_at(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t) {
                if out.last() != Some(&c) {
                    out.push(c);
                }
            }
        }
    }
    out
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
    if kind != MapKind::Conformant && pk.points.is_empty() {
        return Err(at(
            "a contour or proximal map needs a B-DATA POINTS package".into(),
        ));
    }
    if kind == MapKind::Contour && !pk.barriers.is_empty() {
        warnings
            .push("D-BARRIERS is not implemented yet; interpolation ignores the barriers".into());
    }

    // Values for the items being mapped.
    let point_refs: Vec<usize> = (1..=pk.points.len()).collect();
    let (item_values, point_values) = if kind == MapKind::Conformant {
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
    } else {
        let v = resolve_values(pk, &point_refs, &o, spec.line)?;
        (v.clone(), v)
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
    let freq_values: Vec<f64> = if kind == MapKind::Conformant {
        // One value per zone reference, not per outline.
        let mut seen = std::collections::BTreeMap::new();
        for (z, v) in pk.conformolines.iter().zip(&item_values) {
            if let Some(v) = v {
                seen.insert(z.reference, *v);
            }
        }
        seen.into_values().collect()
    } else {
        valid.clone()
    };
    let classes = Classes::new(
        o.value_min.unwrap_or(data_range.0),
        o.value_max.unwrap_or(data_range.1),
        o.levels,
        o.intervals.as_deref(),
        &freq_values,
    )
    .map_err(|e| at(e.message))?;
    let n = classes.levels();
    let symbolism = o
        .symbolism
        .clone()
        .unwrap_or_else(|| Symbolism::standard(n));

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
    let width = frame.cols + 1;
    let idx = |r: usize, c: usize| r * width + c;

    // Fill.
    #[derive(Clone, Copy, PartialEq)]
    enum Fill {
        Background,
        NoData,
        Value(Level),
    }
    let mut fill = vec![Fill::Background; (frame.rows + 1) * width];
    let inside =
        |p: Point| pk.outlines.is_empty() || pk.outlines.iter().any(|r| geom::in_ring(p, r));
    let valid_pts: Vec<(Point, f64)> = pk
        .points
        .iter()
        .zip(&point_values)
        .filter_map(|(p, v)| v.map(|v| (*p, v)))
        .collect();
    match kind {
        MapKind::Contour => {
            let (pts, zs): (Vec<Point>, Vec<f64>) = valid_pts.iter().copied().unzip();
            let area = if pk.outlines.is_empty() {
                let e = Extent::of(&pts).unwrap();
                (e.height() * e.width()).max(f64::MIN_POSITIVE)
            } else {
                pk.outlines.iter().map(|r| geom::area(r)).sum()
            };
            let (pmin, pmax) = o.point_count.unwrap_or((None, None));
            let params = ShepardParams {
                min_points: pmin.unwrap_or(4.0) as usize,
                max_points: pmax.unwrap_or(10.0) as usize,
                fraction: match o.extrapolation {
                    None => 0.1,
                    Some(None) => 0.0,
                    Some(Some(f)) => f,
                },
                initial_radius: o.initial_radius,
                max_radius: o.max_radius,
                ..Default::default()
            };
            if params.max_points < 1 || params.min_points > params.max_points {
                return Err(at(
                    "elective 36: need 1 to 10 points, minimum no more than maximum".into(),
                ));
            }
            if !(0.0..1.0).contains(&params.fraction) {
                return Err(at(
                    "elective 31: the fraction must be at least 0 and less than 1".into(),
                ));
            }
            let lo = o.extrapolation_min.unwrap_or(data_range.0);
            let hi = o.extrapolation_max.unwrap_or(data_range.1);
            let s = Shepard::new(&pts, &zs, area, params);
            // Standard SYMAP computes about one character in six (every second
            // line, every third column) and derives the rest; elective 37
            // computes every character independently.
            let (sr, sc) = if o.independence { (1, 1) } else { (2, 3) };
            let anchor = |i: usize, step: usize, n: usize| -> (usize, usize) {
                let a = i / step * step;
                (a, (a + step).min(n))
            };
            let samples: std::collections::HashMap<(usize, usize), Option<f64>> = (0..=frame.rows)
                .filter(|r| r % sr == 0 || *r == frame.rows)
                .flat_map(|r| {
                    (0..=frame.cols)
                        .filter(|c| c % sc == 0 || *c == frame.cols)
                        .map(move |c| (r, c))
                })
                .map(|(r, c)| ((r, c), s.eval(frame.centre(r, c))))
                .collect();
            let value = |r: usize, c: usize| -> Option<f64> {
                let ((r0, r1), (c0, c1)) = (anchor(r, sr, frame.rows), anchor(c, sc, frame.cols));
                let g = |r, c| samples[&(r, c)];
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
                    let p = frame.centre(r, c);
                    if inside(p) {
                        fill[idx(r, c)] = match value(r, c).or_else(|| s.eval(p)) {
                            Some(v) => Fill::Value(classes.level(v.clamp(lo, hi.max(lo)))),
                            None => Fill::NoData,
                        };
                    }
                }
            }
        }
        MapKind::Proximal => {
            let pts: Vec<Point> = valid_pts.iter().map(|x| x.0).collect();
            for r in 1..frame.rows {
                for c in 1..frame.cols {
                    let p = frame.centre(r, c);
                    if inside(p) {
                        fill[idx(r, c)] =
                            Fill::Value(classes.level(valid_pts[interp::nearest(&pts, p)].1));
                    }
                }
            }
        }
        MapKind::Conformant => {
            for (z, v) in pk.conformolines.iter().zip(&item_values) {
                let Some(v) = v else { continue };
                let f = Fill::Value(classes.level(*v));
                match z.shape {
                    ZoneShape::Area => {
                        for r in 1..frame.rows {
                            for c in 1..frame.cols {
                                if fill[idx(r, c)] == Fill::Background
                                    && geom::in_ring(frame.centre(r, c), &z.vertices)
                                {
                                    fill[idx(r, c)] = f;
                                }
                            }
                        }
                    }
                    ZoneShape::Line | ZoneShape::Point => {
                        for (r, c) in trace(&frame, &z.vertices) {
                            fill[idx(r, c)] = f;
                        }
                    }
                }
            }
        }
    }
    let levels: Vec<Option<Level>> = fill
        .iter()
        .map(|f| {
            if let Fill::Value(l) = f {
                Some(*l)
            } else {
                None
            }
        })
        .collect();

    // Symbolism, then white lines between adjacent levels.
    let mut grid: Vec<Strikes> = fill
        .iter()
        .map(|f| match f {
            Fill::Background => symbolism.background,
            Fill::NoData => symbolism.no_data,
            Fill::Value(Level::Low) => symbolism.low,
            Fill::Value(Level::High) => symbolism.high,
            Fill::Value(Level::In(k)) => symbolism.general[*k as usize],
        })
        .collect();
    if o.lines != Lines::None {
        let rank = |r: usize, c: usize| levels[idx(r, c)].map(Level::rank);
        for r in 1..frame.rows {
            for c in 1..frame.cols {
                let Some(me) = rank(r, c) else { continue };
                let line = [(0i64, 1i64), (1, 0), (0, -1), (-1, 0)]
                    .iter()
                    .any(|&(dr, dc)| {
                        let (nr, nc) = ((r as i64 + dr) as usize, (c as i64 + dc) as usize);
                        let lower = rank(nr, nc).is_some_and(|x| x < me);
                        // Keep symbolism in bands one character wide.
                        let (br, bc) = ((r as i64 - dr) as usize, (c as i64 - dc) as usize);
                        lower && (o.lines == Lines::Continuous || rank(br, bc) == Some(me))
                    });
                if line {
                    grid[idx(r, c)] = symbolism.contour;
                }
            }
        }
    }

    // Data points.
    let show_points = kind != MapKind::Conformant || o.conformant_points;
    if show_points {
        let mut by_cell: std::collections::BTreeMap<(usize, usize), Vec<usize>> =
            Default::default();
        for (i, p) in pk.points.iter().enumerate() {
            if let Some(cell) = frame.cell(*p) {
                by_cell.entry(cell).or_default().push(i);
            }
        }
        let point_level = |i: usize| -> Option<Option<Level>> {
            if kind == MapKind::Conformant {
                let p = pk.points[i];
                let z =
                    pk.conformolines.iter().zip(&item_values).find(|(z, _)| {
                        z.shape == ZoneShape::Area && geom::in_ring(p, &z.vertices)
                    })?;
                Some(z.1.map(|v| classes.level(v)))
            } else {
                Some(point_values[i].map(|v| classes.level(v)))
            }
        };
        for (&(r, c), items) in &by_cell {
            let i = items[0];
            let Some(level) = point_level(i) else {
                continue;
            };
            if o.print_values && items.len() == 1 {
                if let Some(v) = point_values.get(i).copied().flatten() {
                    let s = format!("{v:.2}");
                    let dot = s.find('.').unwrap();
                    for (k, ch) in s.chars().enumerate() {
                        let col = c as i64 + k as i64 - dot as i64;
                        if (1..frame.cols as i64).contains(&col) {
                            grid[idx(r, col as usize)] = Strikes::single(ch);
                        }
                    }
                    continue;
                }
            }
            grid[idx(r, c)] = if items.len() > 1 {
                symbolism.superimposed
            } else {
                match level {
                    None if o.invalid_symbol => symbolism.invalid,
                    None => continue,
                    Some(Level::Low) => symbolism.low_point,
                    Some(Level::High) => symbolism.high_point,
                    Some(Level::In(k)) => symbolism.point[k as usize],
                }
            };
        }
    }

    // Legends, on top of everything.
    for l in &pk.legends {
        match l {
            Legend::Symbol { strikes, at, shift } => {
                let (r, c) = frame.grid(*at);
                if let Some((r, c)) = frame.cell_at(r + shift.0, c + shift.1) {
                    grid[idx(r, c)] = *strikes;
                }
            }
            Legend::Text {
                text,
                vertical,
                at,
                shift,
            } => {
                let (r0, c0) = frame.grid(*at);
                for (k, ch) in text.chars().enumerate() {
                    let (dr, dc) = if *vertical {
                        (k as f64, 0.0)
                    } else {
                        (0.0, k as f64)
                    };
                    if let Some((r, c)) = frame.cell_at(r0 + shift.0 + dr, c0 + shift.1 + dc) {
                        grid[idx(r, c)] = Strikes::single(ch);
                    }
                }
            }
            Legend::Line { strikes, vertices } => {
                for (r, c) in trace(&frame, vertices) {
                    grid[idx(r, c)] = *strikes;
                }
            }
            Legend::Area { strikes, vertices } => {
                for r in 1..frame.rows {
                    for c in 1..frame.cols {
                        if geom::in_ring(frame.centre(r, c), vertices) {
                            grid[idx(r, c)] = *strikes;
                        }
                    }
                }
            }
        }
    }

    // Frequency of data values (zones for a conformant map) by level.
    let mut frequency = Frequency {
        levels: vec![0; n],
        ..Default::default()
    };
    for &v in &freq_values {
        match classes.level(v) {
            Level::Low => frequency.low += 1,
            Level::High => frequency.high += 1,
            Level::In(k) => frequency.levels[k as usize] += 1,
        }
    }

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
    })
}
