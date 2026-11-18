//! A reimplementation of SYMAP, the Synagraphic Mapping System
//! (H. T. Fisher, Northwestern University and the Harvard Laboratory for
//! Computer Graphics, 1963 onwards).
//!
//! The crate reads SYMAP-style card decks and produces line-printer maps:
//! contour (Shepard interpolation), proximal (nearest data point) and
//! conformant (zone fill), with overprinted tone symbols and a level legend.
//!
//! ```
//! let deck = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n99999\n\
//!             E-VALUES\n1\n2\n3\n4\n99999\n\
//!             F-MAP\nEXAMPLE\n\n\n    1       2.0       2.0\n99999\n999999\n";
//! let run = symap::run(deck).unwrap();
//! print!("{}", run.maps[0].to_text());
//! ```

pub mod card;
pub mod classify;
pub mod deck;
pub mod error;
pub mod frame;
pub mod geom;
pub mod interp;
pub mod render;

pub use classify::Classes;
pub use deck::{Deck, Directive, Elective, MapKind, MapSpec, Zone};
pub use error::{Diagnostic, Error};
pub use frame::Frame;
pub use geom::Point;
pub use interp::{Shepard, ShepardParams};
pub use render::{Cell, Line, PrintedMap, Symbol};

use geom::{Extent, in_ring, in_rings};

/// Default maximum printed size (height, width) in inches: a 13-inch line is
/// 130 characters at 10 per inch.
pub const DEFAULT_SIZE: (f64, f64) = (10.0, 13.0);
pub const DEFAULT_LEVELS: usize = 5;

/// Settings resolved from an F-MAP package.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    pub kind: Option<MapKind>,
    pub size: (f64, f64),
    pub rows_per_inch: f64,
    pub cols_per_inch: f64,
    pub margins_in: (f64, f64, f64, f64),
    pub levels: usize,
    pub breaks: Option<Vec<f64>>,
    pub mark_points: bool,
    pub shepard: ShepardParams,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            kind: None,
            size: DEFAULT_SIZE,
            rows_per_inch: 6.0,
            cols_per_inch: 10.0,
            margins_in: (0.0, 0.0, 0.0, 0.0),
            levels: DEFAULT_LEVELS,
            breaks: None,
            mark_points: false,
            shepard: ShepardParams::default(),
        }
    }
}

impl Options {
    /// Resolve electives and directives; unimplemented electives become warnings.
    pub fn from_spec(spec: &MapSpec) -> (Options, Vec<String>) {
        let mut o = Options::default();
        let mut warnings = Vec::new();
        for (line, e) in &spec.electives {
            match *e {
                Elective::MapSize { height, width } => o.size = (height, width),
                Elective::CharsPerInch { rows, cols } => {
                    o.rows_per_inch = rows;
                    o.cols_per_inch = cols;
                }
                Elective::Margins {
                    top,
                    left,
                    bottom,
                    right,
                } => o.margins_in = (top, left, bottom, right),
                Elective::Scale(_) => warnings.push(format!(
                    "line {line}: elective 13 (scale) is not implemented yet"
                )),
                Elective::Other { number, .. } => warnings.push(format!(
                    "line {line}: elective {number} is not implemented; ignored"
                )),
            }
        }
        for d in &spec.directives {
            match d {
                Directive::Kind(k) => o.kind = Some(*k),
                Directive::Levels(n) => o.levels = *n,
                Directive::Breaks(b) => o.breaks = Some(b.clone()),
                Directive::MarkPoints => o.mark_points = true,
                Directive::NoSlope => o.shepard.slope = false,
                Directive::NoDirection => o.shepard.direction = false,
            }
        }
        (o, warnings)
    }
}

/// Draw one map from a parsed deck.
pub fn render(deck: &Deck, spec: &MapSpec) -> Result<PrintedMap, Error> {
    let at = |m: String| Error::at(spec.line, m);
    let (opts, warnings) = Options::from_spec(spec);
    let kind = opts.kind.unwrap_or(if deck.zones.is_empty() {
        MapKind::Contour
    } else {
        MapKind::Conformant
    });
    let values = &spec.values;

    match kind {
        MapKind::Contour | MapKind::Proximal => {
            if deck.points.is_empty() {
                return Err(at("this map needs a B-DATA POINTS package".into()));
            }
            if values.len() != deck.points.len() {
                return Err(at(format!(
                    "{} data points but {} values in E-VALUES",
                    deck.points.len(),
                    values.len()
                )));
            }
        }
        MapKind::Conformant => {
            if deck.zones.is_empty() {
                return Err(at(
                    "a conformant map needs an A-CONFORMOLINES package".into()
                ));
            }
            if values.len() != deck.zones.len() {
                return Err(at(format!(
                    "{} zones but {} values in E-VALUES",
                    deck.zones.len(),
                    values.len()
                )));
            }
        }
    }

    let classes = match &opts.breaks {
        Some(b) => Classes::explicit(b.clone()).map_err(|e| at(e.message))?,
        None => {
            let (lo, hi) = range(values);
            Classes::equal(lo, hi, opts.levels.max(1))
        }
    };
    let n = classes.levels();
    if !(1..=render::MAX_LEVELS).contains(&n) {
        return Err(at(format!(
            "between 1 and {} levels are supported, not {n}",
            render::MAX_LEVELS
        )));
    }

    let zone_pts = || deck.zones.iter().flat_map(|z| z.rings.iter().flatten());
    let extent = if !deck.outline.is_empty() {
        Extent::of(&deck.outline)
    } else if kind == MapKind::Conformant {
        Extent::of(zone_pts())
    } else {
        Extent::of(&deck.points)
    }
    .expect("non-empty geometry");
    let frame = Frame::fit(extent, opts.size, opts.rows_per_inch, opts.cols_per_inch)
        .map_err(|e| at(e.message))?;

    let inside = |p: Point| deck.outline.is_empty() || in_ring(p, &deck.outline);
    let shepard = (kind == MapKind::Contour).then(|| {
        let area = if deck.outline.is_empty() {
            (extent.height() * extent.width()).max(f64::MIN_POSITIVE)
        } else {
            geom::area(&deck.outline)
        };
        Shepard::new(&deck.points, values, area, opts.shepard)
    });

    let mut cells = Vec::with_capacity(frame.rows * frame.cols);
    for r in 0..frame.rows {
        for c in 0..frame.cols {
            let p = frame.centre(r, c);
            let v = if !inside(p) {
                None
            } else {
                match kind {
                    MapKind::Contour => Some(shepard.as_ref().unwrap().eval(p)),
                    MapKind::Proximal => Some(values[interp::nearest(&deck.points, p)]),
                    MapKind::Conformant => deck
                        .zones
                        .iter()
                        .position(|z| in_rings(p, &z.rings))
                        .map(|i| values[i]),
                }
            };
            cells.push(v.map_or(Cell::Blank, |v| Cell::Level(classes.level(v) as u8)));
        }
    }

    if opts.mark_points && kind != MapKind::Conformant {
        for (p, &v) in deck.points.iter().zip(values) {
            if let Some((r, c)) = frame.cell(*p) {
                cells[r * frame.cols + c] = Cell::Mark(classes.level(v) as u8);
            }
        }
    }

    let mut counts = vec![0; n];
    for k in cells.iter().filter_map(|c| c.level()) {
        counts[k] += 1;
    }
    let (t, l, b, rt) = opts.margins_in;
    let lines = |inches: f64, per: f64| (inches.max(0.0) * per).round() as usize;

    Ok(PrintedMap {
        kind,
        titles: spec.titles.clone(),
        rows: frame.rows,
        cols: frame.cols,
        cells,
        classes,
        symbols: render::symbols_for(n),
        counts,
        margins: (
            lines(t, opts.rows_per_inch),
            lines(l, opts.cols_per_inch),
            lines(b, opts.rows_per_inch),
            lines(rt, opts.cols_per_inch),
        ),
        data_range: range(values),
        warnings,
    })
}

fn range(v: &[f64]) -> (f64, f64) {
    v.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        })
}

/// The result of running a deck.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    pub maps: Vec<PrintedMap>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse a deck and draw every map it requests.
pub fn run(text: &str) -> Result<Run, Error> {
    let deck = Deck::parse(text)?;
    if deck.maps.is_empty() {
        return Err(Error::general("deck contains no F-MAP package"));
    }
    let maps = deck
        .maps
        .iter()
        .map(|m| render(&deck, m))
        .collect::<Result<_, _>>()?;
    Ok(Run {
        maps,
        diagnostics: deck.diagnostics,
    })
}
