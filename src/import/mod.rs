//! Turning modern data into a SYMAP deck.
//!
//! The importer reads points or polygons with values from CSV (points),
//! GeoJSON or TopoJSON, projects them, and writes an ordinary, commented
//! deck. Polygons become A-CONFORMOLINES zones for a conformant map, with
//! shared borders simplified once so that neighbours still meet; for a
//! contour or proximal map, each zone gives its centroid as a data point.
//! Points become B-DATA POINTS with an outline around them. The deck is the
//! program's only input, so what the importer produces can be read, edited
//! and run like any other deck.

pub mod csv;
pub mod project;
pub mod read;
pub mod topology;

use crate::Error;
use geo::{Centroid, ConvexHull, LineString, MultiPoint, MultiPolygon, Polygon};
use project::TransverseMercator;
use read::{Feature, Geom, Prop, Ring};
use std::fmt::Write;
use topology::{P, Topology};

/// The kind of map to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    Contour,
    Proximal,
    Conformant,
}

impl MapType {
    pub fn name(self) -> &'static str {
        match self {
            MapType::Contour => "contour",
            MapType::Proximal => "proximal",
            MapType::Conformant => "conformant",
        }
    }

    pub fn parse(s: &str) -> Option<MapType> {
        match s {
            "contour" => Some(MapType::Contour),
            "proximal" => Some(MapType::Proximal),
            "conformant" => Some(MapType::Conformant),
            _ => None,
        }
    }
}

/// What to do with lines (LineString features).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineUse {
    /// Line legends (C-OTOLEGENDS), drawn on the map.
    Legend,
    /// Impermeable barriers to interpolation (D-BARRIERS).
    Barrier,
    Both,
    /// Left out.
    Omit,
}

impl LineUse {
    pub fn name(self) -> &'static str {
        match self {
            LineUse::Legend => "legend",
            LineUse::Barrier => "barrier",
            LineUse::Both => "both",
            LineUse::Omit => "none",
        }
    }

    pub fn parse(s: &str) -> Option<LineUse> {
        match s {
            "legend" => Some(LineUse::Legend),
            "barrier" => Some(LineUse::Barrier),
            "both" => Some(LineUse::Both),
            "none" => Some(LineUse::Omit),
            _ => None,
        }
    }

    fn draws(self) -> bool {
        matches!(self, LineUse::Legend | LineUse::Both)
    }

    fn bars(self) -> bool {
        matches!(self, LineUse::Barrier | LineUse::Both)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The property or column to map; the first numeric one if `None`.
    pub value: Option<String>,
    /// Conformant for polygons and contour for points if `None`.
    pub map: Option<MapType>,
    /// Printed width of the map in inches.
    pub width: f64,
    /// First title card; the source's name if `None`.
    pub title: Option<String>,
    /// Draw an outline: around points, their convex hull enlarged a little;
    /// for zones mapped as points, the zones themselves. Without one, the
    /// whole rectangle is mapped.
    pub outline: bool,
    /// The file's name, recorded in the deck's comments.
    pub source: Option<String>,
    /// The TopoJSON object to read; the first if `None`.
    pub object: Option<String>,
    /// What to do with lines, in the file or in `lines_text`.
    pub lines: LineUse,
    /// The line legends' symbol: up to four characters, struck over one
    /// another.
    pub line_symbol: String,
    /// Lines from a second file (GeoJSON or TopoJSON), and its name.
    pub lines_text: Option<String>,
    pub lines_source: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            value: None,
            map: None,
            width: 8.0,
            title: None,
            outline: true,
            source: None,
            object: None,
            lines: LineUse::Legend,
            line_symbol: "-".into(),
            lines_text: None,
            lines_source: None,
        }
    }
}

/// A deck made from imported data.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    pub deck: String,
    /// Properties or columns with numeric values, any of which can be mapped.
    pub fields: Vec<String>,
    /// The one mapped.
    pub value: String,
    pub map: MapType,
    /// How many points or zones were mapped, and which.
    pub count: usize,
    pub unit: &'static str,
    /// How many line features were used.
    pub lines: usize,
    pub warnings: Vec<String>,
}

const NAMES: &[&str] = &["name", "label", "title", "site", "station", "place"];
const IDS: &[&str] = &["id", "fid", "gid", "objectid", "ogc_fid", "osm_id", "code"];

/// Identifier-like fields are passed over when choosing what to map.
fn is_id(field: &str) -> bool {
    let f = field.to_ascii_lowercase();
    IDS.contains(&f.as_str())
        || ["_id", "_code", "id_str", "_cd"]
            .iter()
            .any(|s| f.ends_with(s))
}

/// The field that names each item: one of the usual names, or else any
/// text field whose name contains "name" or ends in "desc".
fn name_field(features: &[&Feature]) -> Option<String> {
    let text_fields: Vec<&String> = features
        .iter()
        .flat_map(|f| &f.props)
        .filter(|(_, p)| matches!(p, Prop::Text(_)))
        .map(|(k, _)| k)
        .collect();
    NAMES
        .iter()
        .find_map(|n| text_fields.iter().find(|k| k.eq_ignore_ascii_case(n)))
        .or_else(|| {
            text_fields.iter().find(|k| {
                let k = k.to_ascii_lowercase();
                k.contains("name") || k.ends_with("desc")
            })
        })
        .map(|k| (*k).clone())
}

/// Numeric fields in first-seen order: those with at least one number and
/// no text.
fn numeric_fields(features: &[&Feature]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for f in features {
        for (k, _) in &f.props {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    names.retain(|n| {
        let (mut num, mut text) = (0, 0);
        for f in features {
            match f.props.iter().find(|(k, _)| k == n).map(|(_, p)| p) {
                Some(Prop::Number(_)) => num += 1,
                Some(Prop::Text(_)) => text += 1,
                _ => {}
            }
        }
        num > 0 && text == 0
    });
    names
}

/// A number right-justified in a 10-column field, with at most `decimals`
/// decimals (fewer if it would not fit), trailing zeros dropped.
fn field_with(x: f64, decimals: usize) -> Result<String, Error> {
    (0..=decimals)
        .rev()
        .map(|d| {
            let s = format!("{x:.d$}");
            let s = if s.contains('.') {
                let t = s.trim_end_matches('0');
                if t.ends_with('.') {
                    format!("{t}0")
                } else {
                    t.to_string()
                }
            } else {
                format!("{s}.")
            };
            format!("{s:>10}")
        })
        .find(|s| s.len() <= 10)
        .ok_or_else(|| Error::general(format!("{x} does not fit in a 10-column field")))
}

/// A value, with up to six decimals.
fn field(x: f64) -> Result<String, Error> {
    field_with(x, 6)
}

/// A location to the nearest thousandth (a metre, in kilometres), after
/// `head` in columns 1-10.
fn coordinate_card(head: &str, (down, across): P) -> Result<String, Error> {
    Ok(format!(
        "{head:10}{}{}",
        field_with(down, 3)?,
        field_with(across, 3)?
    ))
}

/// Convex hull, counter-clockwise from its least vertex, not closed.
fn hull(points: &[P]) -> Vec<P> {
    let mut h: Vec<P> = MultiPoint::from(points.to_vec())
        .convex_hull()
        .exterior()
        .coords()
        .map(|c| c.x_y())
        .collect();
    h.pop();
    let least = (0..h.len()).min_by(|&i, &j| {
        let ((ix, iy), (jx, jy)) = (h[i], h[j]);
        ix.total_cmp(&jx).then(iy.total_cmp(&jy))
    });
    if let Some(i) = least {
        h.rotate_left(i);
    }
    h
}

/// Area-weighted centroid of rings (closed or not).
fn centroid(rings: &[Vec<P>]) -> Option<P> {
    MultiPolygon::new(
        rings
            .iter()
            .map(|r| Polygon::new(LineString::from(r.clone()), vec![]))
            .collect(),
    )
    .centroid()
    .map(|p| p.x_y())
}

/// Comment lines, wrapped to fit the card.
fn comment(out: &mut String, text: &str) {
    let mut line = String::from("#");
    for word in text.split_whitespace() {
        if line.len() + 1 + word.len() > 78 && line.len() > 1 {
            let _ = writeln!(out, "{line}");
            line = String::from("# ");
            line.push_str(word);
        } else {
            line.push(' ');
            line.push_str(word);
        }
    }
    let _ = writeln!(out, "{line}");
}

fn card_text(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect::<String>()
        .to_uppercase()
}

/// A ring or path's coordinates: given directly, or joined from arcs.
fn path(arcs: &[Vec<P>], r: &Ring) -> Vec<P> {
    match r {
        Ring::Coords(c) => c.clone(),
        Ring::Arcs(refs) => {
            let mut out: Vec<P> = Vec::new();
            for &a in refs {
                let arc = &arcs[topology::arc_index(a)];
                let pts: Box<dyn Iterator<Item = &P>> = if a < 0 {
                    Box::new(arc.iter().rev())
                } else {
                    Box::new(arc.iter())
                };
                for &p in pts {
                    if out.last() != Some(&p) {
                        out.push(p);
                    }
                }
            }
            out
        }
    }
}

/// The parts of a path (in grid coordinates) that fall within `bounds`
/// (down and across, from 0), each keeping the vertex beyond the edge so
/// that it reaches the edge.
fn clip(pts: &[P], (down, across): P) -> Vec<Vec<P>> {
    let inside = |p: P| p.0 >= 0.0 && p.0 <= down && p.1 >= 0.0 && p.1 <= across;
    let overlaps = |a: P, b: P| {
        a.0.min(b.0) <= down && a.0.max(b.0) >= 0.0 && a.1.min(b.1) <= across && a.1.max(b.1) >= 0.0
    };
    let mut parts: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    for w in pts.windows(2) {
        if inside(w[0]) || inside(w[1]) || overlaps(w[0], w[1]) {
            if cur.is_empty() {
                cur.push(w[0]);
            }
            cur.push(w[1]);
        } else if !cur.is_empty() {
            parts.push(std::mem::take(&mut cur));
        }
    }
    if cur.len() >= 2 {
        parts.push(cur);
    }
    parts
}

/// One mapped item: a point, or a zone's rings (arc references).
struct Item {
    value: f64,
    name: Option<String>,
    point: P,
    outer: Vec<Vec<i64>>,
    holes: Vec<Vec<i64>>,
}

/// Read points or polygons from CSV, GeoJSON or TopoJSON and write a deck
/// that maps them.
pub fn import(text: &str, opts: &Options) -> Result<Imported, Error> {
    let src = read::read(text, opts.object.as_deref())?;
    let mut warnings = src.warnings.clone();
    let (line_features, mapped): (Vec<&Feature>, Vec<&Feature>) = src
        .features
        .iter()
        .partition(|f| matches!(f.geom, Geom::Line(_)));
    let (polys, points): (Vec<&Feature>, Vec<&Feature>) = mapped
        .into_iter()
        .partition(|f| matches!(f.geom, Geom::Polygon { .. }));
    if polys.is_empty() && points.is_empty() {
        return Err(Error::general(if line_features.is_empty() {
            "the file has no points or polygons to map".to_string()
        } else {
            "the file has only lines; lines are drawn on a map of points or polygons, \
             given as a second file"
                .to_string()
        }));
    }
    // Lines, as paths in source coordinates, with their names: from this
    // file and from a second one.
    let mut lines: Vec<(Option<String>, Vec<P>)> = Vec::new();
    let line_name = |f: &Feature| {
        name_field(&[f]).and_then(|k| {
            f.props
                .iter()
                .find(|(n, _)| *n == k)
                .and_then(|(_, p)| match p {
                    Prop::Text(t) => Some(t.clone()),
                    _ => None,
                })
        })
    };
    for f in &line_features {
        if let Geom::Line(parts) = &f.geom {
            for r in parts {
                lines.push((line_name(f), path(&src.arcs, r)));
            }
        }
    }
    let mut line_count = line_features.len();
    let mut lines_geographic: Option<bool> = None;
    if let Some(t) = &opts.lines_text {
        let other = read::read(t, None)?;
        let n = other.features.len();
        let mut used = 0;
        let mut in_range = true;
        for f in &other.features {
            if let Geom::Line(parts) = &f.geom {
                used += 1;
                for r in parts {
                    let p = path(&other.arcs, r);
                    in_range &= p.iter().all(|q| q.0.abs() <= 180.0 && q.1.abs() <= 90.0);
                    lines.push((line_name(f), p));
                }
            }
        }
        if used < n {
            warnings.push(format!(
                "{} features in the lines file that are not lines were left out",
                n - used
            ));
        }
        if used == 0 {
            return Err(Error::general("the lines file has no lines"));
        }
        line_count += used;
        lines_geographic = Some(in_range);
    }
    if opts.lines == LineUse::Omit {
        if line_count > 0 {
            warnings.push(format!("{line_count} lines were left out"));
        }
        lines.clear();
        line_count = 0;
    }
    let zonal = !polys.is_empty();
    if zonal && !points.is_empty() {
        warnings.push(format!(
            "{} point features were left out; the polygons are mapped",
            points.len()
        ));
    }
    let map = opts.map.unwrap_or(if zonal {
        MapType::Conformant
    } else {
        MapType::Contour
    });
    if map == MapType::Conformant && !zonal {
        return Err(Error::general(
            "a conformant map needs polygons; this file has only points",
        ));
    }
    let features = if zonal { polys } else { points };
    let unit = if zonal { "zones" } else { "points" };

    let fields = numeric_fields(&features);
    let value = match &opts.value {
        Some(v) => {
            if !fields.contains(v) {
                return Err(Error::general(format!(
                    "'{v}' is not a numeric field; numeric fields: {}",
                    fields.join(", ")
                )));
            }
            v.clone()
        }
        None => fields
            .iter()
            .find(|f| !is_id(f))
            .or(fields.first())
            .cloned()
            .ok_or_else(|| Error::general("no numeric field to map"))?,
    };
    let name_field = name_field(&features);

    // Items with a value for the mapped field.
    let kept: Vec<(&Feature, f64, Option<String>)> = features
        .iter()
        .filter_map(|f| {
            let get = |name: &str| f.props.iter().find(|(k, _)| k == name).map(|(_, p)| p);
            let Some(Prop::Number(v)) = get(&value) else {
                return None;
            };
            let name = name_field.as_deref().and_then(get).and_then(|p| match p {
                Prop::Text(s) => Some(s.clone()),
                Prop::Number(n) => Some(n.to_string()),
                Prop::Missing => None,
            });
            Some((*f, *v, name))
        })
        .collect();
    let missing = features.len() - kept.len();
    if missing > 0 {
        warnings.push(format!(
            "{missing} {unit} without a value for {value} were left out"
        ));
    }
    let needed = if map == MapType::Conformant { 1 } else { 3 };
    if kept.len() < needed {
        return Err(Error::general(format!(
            "{} {unit} have a value for {value}; at least {needed} are needed",
            kept.len()
        )));
    }

    // Source coordinates in use, to choose the projection.
    let mut raw: Vec<P> = Vec::new();
    let mut used_arcs = vec![false; src.arcs.len()];
    for (f, ..) in &kept {
        match &f.geom {
            Geom::Point(p) => raw.push(*p),
            Geom::Polygon { outer, holes } => {
                for r in outer.iter().chain(holes) {
                    match r {
                        Ring::Coords(c) => raw.extend(c),
                        Ring::Arcs(a) => {
                            for &i in a {
                                used_arcs[topology::arc_index(i)] = true;
                            }
                        }
                    }
                }
            }
            // Lines are drawn on the map but do not set its extent.
            Geom::Line(_) => {}
        }
    }
    for (arc, _) in src.arcs.iter().zip(&used_arcs).filter(|(_, u)| **u) {
        raw.extend(arc);
    }
    let in_range = raw.iter().all(|p| p.0.abs() <= 180.0 && p.1.abs() <= 90.0);
    let geographic = src.geographic.unwrap_or(in_range);
    if src.geographic.is_none() && !geographic {
        warnings.push(
            "coordinates are outside longitude and latitude ranges, so they are used as \
             projected coordinates"
                .into(),
        );
    }
    if lines_geographic.is_some_and(|g| g != geographic) {
        return Err(Error::general(if geographic {
            "the lines file is not in longitude and latitude, as the data are"
        } else {
            "the lines file is in longitude and latitude, but the data are projected"
        }));
    }
    let (lo, hi) = raw.iter().fold(
        ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)),
        |(lo, hi), p| {
            (
                (lo.0.min(p.0), lo.1.min(p.1)),
                (hi.0.max(p.0), hi.1.max(p.1)),
            )
        },
    );
    let tm = TransverseMercator {
        lon0: (lo.0 + hi.0) / 2.0,
        lat0: (lo.1 + hi.1) / 2.0,
    };
    let k = if (hi.0 - lo.0).max(hi.1 - lo.1) > 1000.0 {
        1000.0
    } else {
        1.0
    };
    let project = |p: P| -> P {
        if geographic {
            let (e, n) = tm.forward(p.0, p.1);
            (e / 1000.0, n / 1000.0)
        } else {
            (p.0 / k, p.1 / k)
        }
    };
    let projection = if geographic {
        format!(
            "Longitude and latitude are projected with a transverse Mercator on WGS84, \
             centred on {:.4}, {:.4}, in kilometres.",
            tm.lon0, tm.lat0
        )
    } else if k > 1.0 {
        "Projected coordinates are taken to be metres and given in kilometres.".into()
    } else {
        "Projected coordinates are used as given.".into()
    };

    // Projected items, and (for zones) the topology of their borders.
    let mut topo = Topology::default();
    let mut items: Vec<Item> = Vec::new();
    if zonal {
        let topojson = !src.arcs.is_empty();
        let mut coords: Vec<Vec<P>> = Vec::new();
        let mut shapes: Vec<(usize, usize)> = Vec::new(); // outer, hole counts
        let mut arc_rings: Vec<Vec<i64>> = Vec::new();
        for (f, ..) in &kept {
            let Geom::Polygon { outer, holes } = &f.geom else {
                unreachable!()
            };
            shapes.push((outer.len(), holes.len()));
            for r in outer.iter().chain(holes) {
                match r {
                    Ring::Coords(c) => coords.push(c.iter().map(|&p| project(p)).collect()),
                    Ring::Arcs(a) => arc_rings.push(a.clone()),
                }
            }
        }
        let refs = if topojson {
            topo.arcs = src
                .arcs
                .iter()
                .map(|a| a.iter().map(|&p| project(p)).collect())
                .collect();
            arc_rings
        } else {
            let (t, refs) = Topology::from_rings(&coords);
            topo = t;
            refs
        };
        let mut refs = refs.into_iter();
        for ((_, v, name), (no, nh)) in kept.iter().zip(shapes) {
            let outer: Vec<Vec<i64>> = refs.by_ref().take(no).collect();
            let holes: Vec<Vec<i64>> = refs.by_ref().take(nh).collect();
            items.push(Item {
                value: *v,
                name: name.clone(),
                point: (0.0, 0.0),
                outer,
                holes,
            });
        }
    } else {
        for (f, v, name) in &kept {
            let Geom::Point(p) = f.geom else {
                unreachable!()
            };
            items.push(Item {
                value: *v,
                name: name.clone(),
                point: project(p),
                outer: Vec::new(),
                holes: Vec::new(),
            });
        }
    }

    // The frame: a margin around everything mapped.
    let extent: Vec<P> = if zonal {
        items
            .iter()
            .flat_map(|i| &i.outer)
            .flat_map(|r| topo.ring(r))
            .collect()
    } else {
        items.iter().map(|i| i.point).collect()
    };
    let (e0, n0, e1, n1) = extent.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(a, b, c, d), p| (a.min(p.0), b.min(p.1), c.max(p.0), d.max(p.1)),
    );
    let span = (e1 - e0).max(n1 - n0);
    if span.is_nan() || span <= 0.0 {
        return Err(Error::general(format!("the {unit} all lie in one place")));
    }
    let margin = if zonal { 0.02 } else { 0.05 } * span;
    let top = n1 + margin;
    let left = e0 - margin;
    let down_extent = n1 - n0 + 2.0 * margin;
    let across_extent = e1 - e0 + 2.0 * margin;
    let width = opts.width;
    if !(1.0..=30.0).contains(&width) {
        return Err(Error::general("the map width must be from 1 to 30 inches"));
    }
    let height = width * down_extent / across_extent;
    // Down and across from the top-left corner.
    let grid = |(e, n): P| (top - n, e - left);
    // One character across, in source kilometres.
    let char_width = 0.1 * across_extent / width;
    let char_area = char_width * char_width * 1.25;

    // Zones: simplify shared borders once, then rebuild each zone's rings.
    let mut rings: Vec<(usize, Vec<P>)> = Vec::new();
    let tolerance = char_width / 2.0;
    if zonal {
        topo.simplify(tolerance);
        let outer_refs: Vec<Vec<i64>> = items.iter().flat_map(|i| i.outer.clone()).collect();
        let uses = topo.uses(&outer_refs);
        let (mut small, mut filled) = (0, 0);
        for (z, item) in items.iter_mut().enumerate() {
            let mut kept_rings = Vec::new();
            for r in &item.outer {
                let ring: Vec<P> = topo.ring(r).into_iter().map(grid).collect();
                if ring.len() < 4 || topology::area(&ring).abs() < char_area / 2.0 {
                    small += 1;
                } else {
                    kept_rings.push(ring);
                }
            }
            if kept_rings.is_empty() {
                // Keep the largest part, however small, so the zone remains.
                if let Some(r) = item.outer.iter().max_by(|a, b| {
                    topology::area(&topo.ring(a))
                        .abs()
                        .total_cmp(&topology::area(&topo.ring(b)).abs())
                }) {
                    let ring: Vec<P> = topo.ring(r).into_iter().map(grid).collect();
                    if ring.len() >= 4 {
                        small -= 1;
                        kept_rings.push(ring);
                    }
                }
            }
            item.point = centroid(&kept_rings).unwrap_or((0.0, 0.0));
            filled += item
                .holes
                .iter()
                .filter(|h| h.iter().any(|&a| uses[topology::arc_index(a)] == 0))
                .count();
            rings.extend(kept_rings.into_iter().map(|r| (z + 1, r)));
        }
        if small > 0 {
            warnings.push(format!(
                "{small} parts smaller than half a character were left out"
            ));
        }
        if filled > 0 && map == MapType::Conformant {
            warnings.push(format!(
                "{filled} holes with no zone inside are filled by the zone around them"
            ));
        }
        // Smaller rings first: the first zone to claim a character keeps
        // it, so enclaves stay distinct from the zones around them.
        rings.sort_by(|a, b| {
            topology::area(&a.1)
                .abs()
                .total_cmp(&topology::area(&b.1).abs())
        });
    } else {
        for i in &mut items {
            i.point = grid(i.point);
        }
    }

    // Lines: projected, simplified to half a character, and clipped to a
    // character beyond the map's edges.
    let symbol: String = opts.line_symbol.chars().take(4).collect();
    if opts.lines.draws() && symbol.chars().any(|c| c.is_control()) {
        return Err(Error::general(
            "the line symbol must be printable characters",
        ));
    }
    let mut line_parts: Vec<Vec<P>> = Vec::new();
    for (_, pts) in &lines {
        let projected: Vec<P> = pts.iter().map(|&p| project(p)).collect();
        let simple = topology::douglas_peucker(&projected, tolerance);
        let on_grid: Vec<P> = simple
            .into_iter()
            .map(grid)
            .map(|(d, a)| (d + char_width, a + char_width))
            .collect();
        for part in clip(
            &on_grid,
            (
                down_extent + 2.0 * char_width,
                across_extent + 2.0 * char_width,
            ),
        ) {
            line_parts.push(
                part.into_iter()
                    .map(|(d, a)| (d - char_width, a - char_width))
                    .collect(),
            );
        }
    }
    if !lines.is_empty() && line_parts.is_empty() {
        warnings.push("the lines all lie outside the map".into());
    }
    let barriers = opts.lines.bars() && !line_parts.is_empty();
    if barriers && map == MapType::Conformant {
        warnings.push("barriers have no effect on a conformant map, so none were written".into());
    }
    let barriers = barriers && map != MapType::Conformant;
    let legends = opts.lines.draws() && !line_parts.is_empty();

    // The deck.
    let source = opts
        .source
        .as_deref()
        .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s))
        .unwrap_or("the imported data");
    let n = items.len();
    let mut out = String::new();
    comment(
        &mut out,
        &format!("Made by symap import from {source}: {n} {unit}, mapping {value}."),
    );
    comment(&mut out, &projection);
    comment(
        &mut out,
        &format!(
            "Locations are measured down from {top:.3} and across from {left:.3}, with a \
             margin of {margin:.3} around the {unit}."
        ),
    );
    if zonal {
        comment(
            &mut out,
            &format!(
                "Borders are simplified to within {tolerance:.3} (half a character), each \
                 shared border once, so neighbouring zones still meet."
            ),
        );
        if map != MapType::Conformant {
            comment(
                &mut out,
                "Each zone's value is placed at its centroid as a data point.",
            );
        }
    }
    if legends || barriers {
        let from = opts
            .lines_source
            .as_deref()
            .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s))
            .map_or(String::new(), |s| format!(" from {s}"));
        let how = match (legends, barriers) {
            (true, true) => format!(
                "drawn as line legends ({symbol}) and written as impermeable barriers \
                 (strength -1)"
            ),
            (true, false) => format!("drawn as line legends ({symbol})"),
            _ => "written as impermeable barriers (strength -1)".into(),
        };
        let mut names: Vec<&str> = lines.iter().filter_map(|(n, _)| n.as_deref()).collect();
        names.dedup();
        let named = if names.is_empty() {
            String::new()
        } else {
            format!(" ({})", names.join(", "))
        };
        comment(
            &mut out,
            &format!(
                "{line_count} lines{from}{named} are {how}, simplified to within {tolerance:.3} \
                 and cut a character beyond the map's edges."
            ),
        );
    }
    for w in &warnings {
        comment(&mut out, &format!("Note: {w}."));
    }
    if name_field.is_some() {
        let what = if zonal { "Zones" } else { "Data points" };
        let _ = writeln!(out, "#\n# {what}, in order:");
        for (i, item) in items.iter().enumerate() {
            if let Some(name) = &item.name {
                let name: String = name.chars().filter(|c| !c.is_control()).take(70).collect();
                let _ = writeln!(out, "# {:>4} {name}", i + 1);
            }
        }
    }

    let write_ring = |out: &mut String, head: &str, ring: &[P]| -> Result<(), Error> {
        for (j, &p) in ring.iter().enumerate() {
            let _ = writeln!(
                out,
                "{}",
                coordinate_card(if j == 0 { head } else { "" }, p)?
            );
        }
        Ok(())
    };
    if map == MapType::Conformant {
        let _ = writeln!(out, "A-CONFORMOLINES");
        for (z, ring) in &rings {
            write_ring(&mut out, &format!("{z:>5}    A"), ring)?;
        }
        let _ = writeln!(out, "99999");
    } else {
        if opts.outline {
            if zonal {
                let _ = writeln!(out, "A-OUTLINE");
                for (_, ring) in &rings {
                    write_ring(&mut out, "", ring)?;
                }
                let _ = writeln!(out, "99999");
            } else {
                let pts: Vec<P> = items.iter().map(|i| i.point).collect();
                let h = hull(&pts);
                if h.len() >= 3 {
                    let (cd, ca) = h.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
                    let c = (cd / h.len() as f64, ca / h.len() as f64);
                    let mut grown: Vec<P> = h
                        .iter()
                        .map(|&(d, a)| {
                            let (dd, da) = (d - c.0, a - c.1);
                            let len = (dd * dd + da * da).sqrt().max(1e-9);
                            (d + dd / len * margin * 0.6, a + da / len * margin * 0.6)
                        })
                        .collect();
                    grown.push(grown[0]);
                    let _ = writeln!(out, "A-OUTLINE");
                    write_ring(&mut out, "", &grown)?;
                    let _ = writeln!(out, "99999");
                } else {
                    warnings.push("the points are in a line, so no outline was drawn".into());
                }
            }
        }
        let _ = writeln!(out, "B-DATA POINTS");
        for i in &items {
            let _ = writeln!(out, "{}", coordinate_card("", i.point)?);
        }
        let _ = writeln!(out, "99999");
    }
    if legends {
        let _ = writeln!(out, "C-OTOLEGENDS");
        for part in &line_parts {
            write_ring(&mut out, &format!("     {symbol:<4}L"), part)?;
        }
        let _ = writeln!(out, "99999");
    }
    if barriers {
        // Repeating a barrier's last vertex ends it.
        let _ = writeln!(out, "D-BARRIERS");
        for part in &line_parts {
            for &p in part.iter().chain(part.last()) {
                let _ = writeln!(out, "{}{}", coordinate_card("", p)?, field_with(-1.0, 1)?);
            }
        }
        let _ = writeln!(out, "99999");
    }
    let _ = writeln!(out, "E-VALUES");
    for i in &items {
        let _ = writeln!(out, "{:10}{}", "", field(i.value)?);
    }
    let _ = writeln!(out, "99999");

    let title = opts.title.clone().unwrap_or_else(|| {
        source
            .rsplit_once('.')
            .map_or(source, |(s, _)| s)
            .replace(['_', '-'], " ")
    });
    let _ = writeln!(out, "F-MAP");
    let _ = writeln!(out, "{}", card_text(&title));
    let counted = if zonal {
        format!("{value} for {n} zones")
    } else {
        format!("{value} at {n} points")
    };
    let _ = writeln!(out, "{}", card_text(&counted));
    let _ = writeln!(
        out,
        "{}",
        match map {
            MapType::Contour => "CONTOUR MAP - SHEPARD INTERPOLATION",
            MapType::Proximal => "PROXIMAL MAP - NEAREST DATA POINT",
            MapType::Conformant => "CONFORMANT MAP",
        }
    );
    let _ = writeln!(
        out,
        "    1{:5}{}{}",
        "",
        field_with(height, 2)?,
        field_with(width, 2)?
    );
    let _ = writeln!(
        out,
        "    2{:5}{}{}{}{}",
        "",
        field_with(0.0, 3)?,
        field_with(0.0, 3)?,
        field_with(down_extent, 3)?,
        field_with(across_extent, 3)?
    );
    if map == MapType::Proximal {
        let _ = writeln!(out, "   31\n   36\n   37");
    }
    let _ = writeln!(out, "99999\n999999");

    Ok(Imported {
        deck: out,
        fields,
        value,
        map,
        count: n,
        unit,
        lines: line_count,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "site,lat,lon,no2,code\n\
        Marylebone Road,51.5225,-0.1546,48.2,MY1\n\
        Bloomsbury,51.5222,-0.1259,28.1,CLL2\n\
        Greenwich,51.4528,0.0083,24.5,GN0\n\
        Ealing,51.5185,-0.2655,,EA6\n\
        Teddington,51.4243,-0.3395,13.6,TED\n";

    #[test]
    fn csv_points_make_a_deck_that_runs() {
        let opts = Options {
            source: Some("no2.csv".into()),
            ..Options::default()
        };
        let imp = import(CSV, &opts).unwrap();
        assert_eq!(imp.fields, ["no2"]);
        assert_eq!(imp.value, "no2");
        assert_eq!(
            (imp.count, imp.unit, imp.map),
            (4, "points", MapType::Contour)
        );
        assert_eq!(
            imp.warnings,
            ["1 points without a value for no2 were left out"]
        );
        assert!(imp.deck.contains("#    1 Marylebone Road\n"));
        assert!(
            imp.deck
                .contains("\nF-MAP\nNO2\nNO2 AT 4 POINTS\nCONTOUR MAP")
        );
        assert!(imp.deck.lines().all(|l| l.len() <= 80));
        let run = crate::run(&imp.deck).unwrap();
        let m = &run.maps[0];
        // The printed table lists the points with their values.
        let values: Vec<f64> = m.table.iter().filter_map(|t| t.value).collect();
        assert_eq!(values, [48.2, 28.1, 24.5, 13.6]);
        // Eight inches wide at ten characters per inch.
        assert_eq!(m.frame.cols, 80);
    }

    #[test]
    fn geography_is_preserved() {
        // Greenwich is east of Bloomsbury and south of it; Teddington is
        // the furthest west.
        let imp = import(CSV, &Options::default()).unwrap();
        let m = &crate::run(&imp.deck).unwrap().maps[0];
        let t = &m.table;
        assert!(t[2].col > t[1].col && t[2].row > t[1].row);
        assert!(t.iter().all(|r| r.col >= t[3].col));
        // Distances keep their proportions: Bloomsbury to Greenwich is about
        // 11.0 km, Marylebone Road to Bloomsbury about 2.0 km.
        let km = |a: &crate::map::TableRow, b: &crate::map::TableRow| {
            let (d, c) = ((a.row - b.row) as f64 / 8.0, (a.col - b.col) as f64 / 10.0);
            (d * d + c * c).sqrt()
        };
        let ratio = km(&t[1], &t[2]) / km(&t[0], &t[1]);
        assert!((ratio - 11.0 / 2.0).abs() < 0.6, "{ratio}");
    }

    #[test]
    fn geojson_points() {
        let gj = r#"{"type": "FeatureCollection", "features": [
          {"type": "Feature", "properties": {"name": "A", "v": 1},
           "geometry": {"type": "Point", "coordinates": [-6.26, 53.35]}},
          {"type": "Feature", "properties": {"name": "B", "v": "2.5"},
           "geometry": {"type": "Point", "coordinates": [-6.20, 53.34]}},
          {"type": "Feature", "properties": {"name": "C", "v": 4},
           "geometry": {"type": "MultiPoint", "coordinates": [[-6.30, 53.30], [-6.25, 53.38]]}},
          {"type": "Feature", "properties": {"name": "Liffey"},
           "geometry": {"type": "LineString", "coordinates": [[-6.3, 53.34], [-6.2, 53.35]]}}
        ]}"#;
        let opts = Options {
            map: Some(MapType::Proximal),
            ..Options::default()
        };
        let imp = import(gj, &opts).unwrap();
        assert_eq!(imp.count, 4);
        // The Liffey is a line, drawn as a line legend.
        assert!(imp.warnings.is_empty(), "{:?}", imp.warnings);
        assert_eq!(imp.lines, 1);
        assert!(imp.deck.contains("\nC-OTOLEGENDS\n     -   L"));
        assert!(
            imp.deck
                .contains("1 lines (Liffey) are drawn as line legends (-)")
        );
        assert!(imp.deck.contains("   31\n   36\n   37\n"));
        let run = crate::run(&imp.deck).unwrap();
        assert_eq!(run.maps[0].kind, crate::MapKind::Proximal);
        let conformant = Options {
            map: Some(MapType::Conformant),
            ..Options::default()
        };
        assert!(import(gj, &conformant).is_err());
    }

    #[test]
    fn projected_coordinates_and_errors() {
        let csv = "easting,northing,height\n530000,180000,10\n531000,181500,20\n529000,182000,30\n";
        let imp = import(csv, &Options::default()).unwrap();
        assert!(imp.deck.contains("taken to be metres"));
        crate::run(&imp.deck).unwrap();
        assert!(import("a,b\n1,2\n", &Options::default()).is_err());
        let opts = Options {
            value: Some("nope".into()),
            ..Options::default()
        };
        assert!(
            import(csv, &opts)
                .unwrap_err()
                .message
                .contains("numeric fields: height")
        );
        assert!(import("x,y,v\n1,1,1\n1,1,2\n1,1,3\n", &Options::default()).is_err());
    }

    /// Four zones in a 2x2 grid with wavy shared borders, the north-east
    /// one with an enclave (a fifth zone) in a hole, and a lake (a hole
    /// with no zone) in the south-west.
    fn zones_geojson() -> String {
        let wave = |a: P, b: P| -> Vec<P> {
            (0..=20)
                .map(|i| {
                    let t = i as f64 / 20.0;
                    let w = 0.002 * (t * 40.0).sin();
                    (a.0 + t * (b.0 - a.0) + w, a.1 + t * (b.1 - a.1) + w)
                })
                .collect()
        };
        let (x0, x1, x2, y0, y1, y2) = (-0.2, -0.1, 0.0, 51.4, 51.5, 51.6);
        let mid_v_lo = wave((x1, y0), (x1, y1));
        let mid_v_hi = wave((x1, y1), (x1, y2));
        let mid_h_l = wave((x0, y1), (x1, y1));
        let mid_h_r = wave((x1, y1), (x2, y1));
        let rev = |v: &[P]| -> Vec<P> { v.iter().rev().copied().collect() };
        let cat = |parts: &[Vec<P>]| -> Vec<P> {
            let mut out: Vec<P> = Vec::new();
            for p in parts {
                for &q in p {
                    if out.last() != Some(&q) {
                        out.push(q);
                    }
                }
            }
            if out.first() != out.last() {
                out.push(out[0]);
            }
            out
        };
        let sw = cat(&[vec![(x0, y0)], mid_v_lo.clone(), rev(&mid_h_l)]);
        let se = cat(&[vec![(x1, y0), (x2, y0), (x2, y1)], rev(&mid_h_r)]);
        let nw = cat(&[mid_h_l.clone(), mid_v_hi.clone(), vec![(x0, y2)]]);
        let ne = cat(&[mid_h_r.clone(), vec![(x2, y2)], rev(&mid_v_hi)]);
        let enclave = vec![
            (-0.06, 51.54),
            (-0.03, 51.54),
            (-0.03, 51.57),
            (-0.06, 51.57),
            (-0.06, 51.54),
        ];
        let lake = vec![
            (-0.17, 51.43),
            (-0.14, 51.43),
            (-0.14, 51.46),
            (-0.17, 51.43),
        ];
        let ring = |r: &[P]| -> String {
            let pts: Vec<String> = r.iter().map(|p| format!("[{},{}]", p.0, p.1)).collect();
            format!("[{}]", pts.join(","))
        };
        let feat = |name: &str, v: f64, rings: &[&[P]]| -> String {
            let rs: Vec<String> = rings.iter().map(|r| ring(r)).collect();
            format!(
                r#"{{"type":"Feature","properties":{{"name":"{name}","v":{v}}},
                   "geometry":{{"type":"Polygon","coordinates":[{}]}}}}"#,
                rs.join(",")
            )
        };
        format!(
            r#"{{"type":"FeatureCollection","features":[{},{},{},{},{}]}}"#,
            feat("SW", 1.0, &[&sw, &lake]),
            feat("SE", 2.0, &[&se]),
            feat("NW", 3.0, &[&nw]),
            feat("NE", 4.0, &[&ne, &enclave]),
            feat("Enclave", 5.0, &[&enclave]),
        )
    }

    #[test]
    fn polygons_make_a_conformant_map() {
        let imp = import(&zones_geojson(), &Options::default()).unwrap();
        assert_eq!(
            (imp.map, imp.count, imp.unit),
            (MapType::Conformant, 5, "zones")
        );
        assert_eq!(
            imp.warnings,
            ["1 holes with no zone inside are filled by the zone around them"]
        );
        assert!(imp.deck.contains("\nA-CONFORMOLINES\n"));
        assert!(imp.deck.contains("#    5 Enclave\n"));
        assert!(imp.deck.lines().all(|l| l.len() <= 80));
        let run = crate::run(&imp.deck).unwrap();
        let m = &run.maps[0];
        assert_eq!(m.kind, crate::MapKind::Conformant);
        // The table follows the cards, which put smaller zones first.
        let mut values: Vec<f64> = m.table.iter().filter_map(|t| t.value).collect();
        assert_eq!(values[0], 5.0);
        values.sort_by(f64::total_cmp);
        assert_eq!(values, [1.0, 2.0, 3.0, 4.0, 5.0]);
        // Every zone prints its own level: with five levels, the enclave
        // takes the top one (O+X+A+V) inside the zone around it (O+-).
        let text = m.to_text();
        let enclave_row = text.lines().find(|l| l.contains('█')).unwrap();
        assert!(enclave_row.contains('Θ'), "{text}");
    }

    #[test]
    fn shared_borders_stay_shared() {
        let imp = import(&zones_geojson(), &Options::default()).unwrap();
        // Every vertex of the middle (wavy) border is used by both zones on
        // either side, so the cards for each pair of neighbours agree.
        let deck = &imp.deck;
        let zones: Vec<Vec<&str>> = deck
            .split("    A")
            .skip(1)
            .map(|z| z.lines().take_while(|l| l.starts_with(' ')).collect())
            .collect();
        let coords = |z: &Vec<&str>| -> Vec<String> {
            z.iter().map(|l| l.trim_start().to_string()).collect()
        };
        let all: Vec<Vec<String>> = zones.iter().map(coords).collect();
        // The four grid zones each share at least three vertices (two
        // corners and the centre) with a neighbour.
        let shared = |a: &Vec<String>, b: &Vec<String>| a.iter().filter(|p| b.contains(p)).count();
        let big: Vec<&Vec<String>> = all.iter().filter(|z| z.len() > 6).collect();
        assert_eq!(big.len(), 4);
        for z in &big {
            assert!(big.iter().filter(|o| shared(z, o) >= 2).count() >= 3);
        }
    }

    #[test]
    fn zones_as_points() {
        let opts = Options {
            map: Some(MapType::Contour),
            ..Options::default()
        };
        let imp = import(&zones_geojson(), &opts).unwrap();
        assert!(imp.deck.contains("\nA-OUTLINE\n") && imp.deck.contains("\nB-DATA POINTS\n"));
        assert!(imp.deck.contains("placed at its centroid"));
        let m = &crate::run(&imp.deck).unwrap().maps[0];
        assert_eq!(m.kind, crate::MapKind::Contour);
        assert_eq!(m.table.len(), 5);
    }

    #[test]
    fn topojson_zones() {
        let t = r#"{"type": "Topology",
          "transform": {"scale": [0.001, 0.001], "translate": [-0.2, 51.4]},
          "objects": {"zones": {"type": "GeometryCollection", "geometries": [
            {"type": "Polygon", "arcs": [[0, 1]], "properties": {"name": "West", "v": 1}},
            {"type": "Polygon", "arcs": [[2, -2]], "properties": {"name": "East", "v": 2}}
          ]}},
          "arcs": [
            [[100, 0], [-100, 0], [0, 100], [100, 0]],
            [[100, 100], [3, -30], [-6, -30], [3, -40]],
            [[100, 100], [100, 0], [0, -100], [-100, 0]]
          ]}"#;
        let imp = import(t, &Options::default()).unwrap();
        assert_eq!((imp.count, imp.map), (2, MapType::Conformant));
        let m = &crate::run(&imp.deck).unwrap().maps[0];
        assert!(m.to_text().contains("CONFORMANT MAP"));
    }

    /// A river across the CSV's sites, west to east, as GeoJSON.
    const RIVER: &str = r#"{"type": "FeatureCollection", "features": [
      {"type": "Feature", "properties": {"name": "River"},
       "geometry": {"type": "LineString", "coordinates":
         [[-0.40, 51.45], [-0.25, 51.47], [-0.12, 51.50], [0.00, 51.49], [0.20, 51.47]]}},
      {"type": "Feature", "properties": {"name": "Far"},
       "geometry": {"type": "LineString", "coordinates": [[5.0, 48.0], [5.1, 48.1]]}},
      {"type": "Feature", "properties": {"name": "Gauge"},
       "geometry": {"type": "Point", "coordinates": [-0.1, 51.5]}}
    ]}"#;

    #[test]
    fn lines_from_a_second_file() {
        let mut opts = Options {
            lines_text: Some(RIVER.into()),
            lines_source: Some("river.geojson".into()),
            line_symbol: "~".into(),
            ..Options::default()
        };
        let imp = import(CSV, &opts).unwrap();
        assert_eq!(imp.lines, 2);
        assert!(
            imp.deck
                .contains("from river.geojson (River, Far) are drawn as line legends (~)")
        );
        assert!(imp.deck.contains("\nC-OTOLEGENDS\n     ~   L"));
        assert!(!imp.deck.contains("D-BARRIERS"));
        assert!(
            imp.warnings
                .contains(&"1 features in the lines file that are not lines were left out".into())
        );
        // The far line is clipped away; the river crosses the map.
        let legends = imp.deck.lines().filter(|l| l.starts_with("     ~   L"));
        assert_eq!(legends.count(), 1);
        let m = &crate::run(&imp.deck).unwrap().maps[0];
        assert!(m.to_text().contains('~'));

        opts.lines = LineUse::Barrier;
        let imp = import(CSV, &opts).unwrap();
        assert!(!imp.deck.contains("C-OTOLEGENDS"));
        let bars: Vec<&str> = imp
            .deck
            .split("D-BARRIERS\n")
            .nth(1)
            .unwrap()
            .lines()
            .take_while(|l| *l != "99999")
            .collect();
        assert!(bars.iter().all(|l| l.ends_with("-1.0")));
        // Repeating the last vertex ends the barrier.
        assert_eq!(bars[bars.len() - 1], bars[bars.len() - 2]);
        crate::run(&imp.deck).unwrap();

        opts.lines = LineUse::Omit;
        let imp = import(CSV, &opts).unwrap();
        assert!(!imp.deck.contains("C-OTOLEGENDS") && !imp.deck.contains("D-BARRIERS"));
        assert!(imp.warnings.contains(&"2 lines were left out".into()));
    }

    #[test]
    fn lines_must_match_the_data() {
        let projected = r#"{"type": "Feature", "properties": {},
          "geometry": {"type": "LineString", "coordinates": [[530000, 180000], [531000, 181000]]}}"#;
        let opts = Options {
            lines_text: Some(projected.into()),
            ..Options::default()
        };
        assert!(
            import(CSV, &opts)
                .unwrap_err()
                .message
                .contains("not in longitude and latitude")
        );
        // A file of lines alone has nothing to map.
        assert!(
            import(
                RIVER.replace("Point", "LineString").as_str(),
                &Options::default()
            )
            .is_err()
        );
    }

    #[test]
    fn barriers_are_left_off_conformant_maps() {
        let opts = Options {
            lines_text: Some(RIVER.into()),
            lines: LineUse::Both,
            ..Options::default()
        };
        let imp = import(&zones_geojson(), &opts).unwrap();
        assert!(imp.deck.contains("C-OTOLEGENDS") && !imp.deck.contains("D-BARRIERS"));
        assert!(
            imp.warnings
                .iter()
                .any(|w| w.starts_with("barriers have no effect"))
        );
        crate::run(&imp.deck).unwrap();
    }

    #[test]
    fn topojson_lines() {
        let t = r#"{"type": "Topology",
          "objects": {"sites": {"type": "GeometryCollection", "geometries": [
            {"type": "Point", "coordinates": [-0.2, 51.4], "properties": {"v": 1}},
            {"type": "Point", "coordinates": [0.0, 51.6], "properties": {"v": 2}},
            {"type": "Point", "coordinates": [-0.1, 51.5], "properties": {"v": 3}},
            {"type": "LineString", "arcs": [0, 1], "properties": {"name": "Brook"}}
          ]}},
          "arcs": [[[-0.2, 51.5], [-0.1, 51.45]], [[-0.1, 51.45], [0.0, 51.5]]]}"#;
        let imp = import(t, &Options::default()).unwrap();
        assert_eq!((imp.count, imp.lines), (3, 1));
        let legend: Vec<&str> = imp
            .deck
            .split("C-OTOLEGENDS\n")
            .nth(1)
            .unwrap()
            .lines()
            .take_while(|l| *l != "99999")
            .collect();
        // The two arcs join into one line of three vertices.
        assert_eq!(legend.len(), 3);
        crate::run(&imp.deck).unwrap();
    }

    #[test]
    fn hull_is_convex() {
        let h = hull(&[
            (0.0, 0.0),
            (2.0, 0.0),
            (1.0, 1.0),
            (2.0, 2.0),
            (0.0, 2.0),
            (1.0, 0.0),
        ]);
        assert_eq!(h, [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    }

    #[test]
    fn fields_fit_ten_columns() {
        assert_eq!(field(3.5).unwrap(), "       3.5");
        assert_eq!(field(48.0).unwrap(), "      48.0");
        assert_eq!(field(-12345.6789).unwrap(), "-12345.679");
        assert_eq!(field(0.000012).unwrap(), "  0.000012");
        assert_eq!(field(123456789.0).unwrap(), "123456789.");
        assert_eq!(field_with(2.0 / 3.0, 3).unwrap(), "     0.667");
        assert!(field(1e12).is_err());
    }
}
