//! Turning modern point data into a SYMAP deck.
//!
//! The importer reads points with values from CSV (a header row, then
//! longitude and latitude or easting and northing columns) or GeoJSON
//! (Point and MultiPoint features), projects them, and writes an ordinary,
//! commented deck: an outline around the points, B-DATA POINTS, E-VALUES and
//! an F-MAP package. The deck is the program's only input, so what the
//! importer produces can be read, edited and run like any other deck.

pub mod csv;
pub mod json;
pub mod project;

use crate::Error;
use project::TransverseMercator;
use std::fmt::Write;

/// The kind of map to draw from the points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapType {
    Contour,
    Proximal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The property or column to map; the first numeric one if `None`.
    pub value: Option<String>,
    pub map: MapType,
    /// Printed width of the map in inches.
    pub width: f64,
    /// First title card; the source's name if `None`.
    pub title: Option<String>,
    /// Draw an outline around the points (their convex hull, enlarged a
    /// little); without one, the whole rectangle is mapped.
    pub outline: bool,
    /// The file's name, recorded in the deck's comments.
    pub source: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            value: None,
            map: MapType::Contour,
            width: 8.0,
            title: None,
            outline: true,
            source: None,
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
    pub points: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum Prop {
    Number(f64),
    Text(String),
    Missing,
}

impl Prop {
    fn from_text(s: &str) -> Prop {
        let t = s.trim();
        if t.is_empty() {
            Prop::Missing
        } else {
            match t.parse::<f64>() {
                Ok(n) if n.is_finite() => Prop::Number(n),
                _ => Prop::Text(t.to_string()),
            }
        }
    }
}

struct Record {
    x: f64,
    y: f64,
    props: Vec<(String, Prop)>,
}

struct Points {
    records: Vec<Record>,
    /// Coordinates are longitude and latitude (otherwise already projected).
    geographic: bool,
    warnings: Vec<String>,
}

const LON: &[&str] = &["lon", "lng", "long", "longitude"];
const LAT: &[&str] = &["lat", "latitude"];
const EAST: &[&str] = &["easting", "east", "x"];
const NORTH: &[&str] = &["northing", "north", "y"];
const NAMES: &[&str] = &["name", "label", "title", "site", "station", "place"];
const IDS: &[&str] = &["id", "fid", "gid", "objectid", "ogc_fid", "osm_id", "code"];

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| header.iter().position(|h| h.eq_ignore_ascii_case(n)))
}

fn read_csv(text: &str) -> Result<Points, Error> {
    let t = csv::parse(text)?;
    let (ix, iy, geographic) = match (find(&t.header, LON), find(&t.header, LAT)) {
        (Some(x), Some(y)) => (x, y, true),
        _ => match (find(&t.header, EAST), find(&t.header, NORTH)) {
            (Some(x), Some(y)) => (x, y, false),
            _ => {
                return Err(Error::at(
                    1,
                    format!(
                        "CSV: no coordinate columns; expected longitude and latitude \
                         (lon, lat) or easting and northing (x, y), found: {}",
                        t.header.join(", ")
                    ),
                ));
            }
        },
    };
    let mut records = Vec::new();
    let mut skipped = 0;
    for (_, row) in &t.rows {
        let coord = |i: usize| row.get(i).and_then(|s| s.trim().parse::<f64>().ok());
        let (Some(x), Some(y)) = (coord(ix), coord(iy)) else {
            skipped += 1;
            continue;
        };
        let props = t
            .header
            .iter()
            .enumerate()
            .filter(|&(i, _)| i != ix && i != iy)
            .map(|(i, h)| (h.clone(), Prop::from_text(row.get(i).map_or("", |s| s))))
            .collect();
        records.push(Record { x, y, props });
    }
    let mut warnings = Vec::new();
    if skipped > 0 {
        warnings.push(format!("{skipped} rows without coordinates were left out"));
    }
    Ok(Points {
        records,
        geographic,
        warnings,
    })
}

fn read_geojson(text: &str) -> Result<Points, Error> {
    use json::Value;
    let doc = json::parse(text)?;
    let features: Vec<&Value> = match doc.get("type").and_then(Value::as_str) {
        Some("FeatureCollection") => doc
            .get("features")
            .and_then(Value::as_array)
            .ok_or_else(|| Error::general("GeoJSON: a FeatureCollection needs a features array"))?
            .iter()
            .collect(),
        Some("Feature") => vec![&doc],
        _ => {
            return Err(Error::general(
                "GeoJSON: expected a FeatureCollection or a Feature",
            ));
        }
    };
    let mut records = Vec::new();
    let mut other = 0;
    for (k, f) in features.iter().enumerate() {
        let props: Vec<(String, Prop)> = match f.get("properties") {
            Some(Value::Object(m)) => m
                .iter()
                .map(|(k, v)| {
                    let p = match v {
                        Value::Number(n) => Prop::Number(*n),
                        Value::String(s) => Prop::from_text(s),
                        Value::Bool(b) => Prop::Text(b.to_string()),
                        _ => Prop::Missing,
                    };
                    (k.clone(), p)
                })
                .collect(),
            _ => Vec::new(),
        };
        let geometry = f.get("geometry");
        let kind = geometry.and_then(|g| g.get("type")).and_then(Value::as_str);
        let coords = geometry.and_then(|g| g.get("coordinates"));
        let positions: Vec<&Value> = match (kind, coords) {
            (Some("Point"), Some(c)) => vec![c],
            (Some("MultiPoint"), Some(Value::Array(cs))) => cs.iter().collect(),
            _ => {
                other += 1;
                continue;
            }
        };
        for p in positions {
            let xy = p.as_array().filter(|a| a.len() >= 2);
            let (Some(x), Some(y)) = (
                xy.and_then(|a| a[0].as_f64()),
                xy.and_then(|a| a[1].as_f64()),
            ) else {
                return Err(Error::general(format!(
                    "GeoJSON: feature {} has a malformed position",
                    k + 1
                )));
            };
            records.push(Record {
                x,
                y,
                props: props.clone(),
            });
        }
    }
    let mut warnings = Vec::new();
    if other > 0 {
        warnings.push(format!(
            "{other} features that are not points were left out"
        ));
    }
    // RFC 7946 coordinates are longitude and latitude; older files may carry
    // projected coordinates instead.
    let geographic = records
        .iter()
        .all(|r| r.x.abs() <= 180.0 && r.y.abs() <= 90.0);
    if !geographic {
        warnings.push(
            "coordinates are outside longitude and latitude ranges, so they are used as \
             projected coordinates"
                .into(),
        );
    }
    Ok(Points {
        records,
        geographic,
        warnings,
    })
}

/// Numeric fields in first-seen order: those with at least one number and
/// no text.
fn numeric_fields(records: &[Record]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for r in records {
        for (k, _) in &r.props {
            if !names.contains(k) {
                names.push(k.clone());
            }
        }
    }
    names.retain(|n| {
        let vals = records
            .iter()
            .filter_map(|r| r.props.iter().find(|(k, _)| k == n).map(|(_, p)| p));
        let (mut num, mut text) = (0, 0);
        for v in vals {
            match v {
                Prop::Number(_) => num += 1,
                Prop::Text(_) => text += 1,
                Prop::Missing => {}
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

/// Locations to the nearest thousandth (a metre, in kilometres).
fn coordinate_card(down: f64, across: f64) -> Result<String, Error> {
    Ok(format!(
        "{:10}{}{}",
        "",
        field_with(down, 3)?,
        field_with(across, 3)?
    ))
}

/// Convex hull (Andrew's monotone chain), counter-clockwise.
fn hull(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut p = points.to_vec();
    p.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut h: Vec<(f64, f64)> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let iter: Box<dyn Iterator<Item = &(f64, f64)>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for &q in iter {
            while h.len() >= start + 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
                h.pop();
            }
            h.push(q);
        }
        h.pop();
    }
    h
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

/// Read points from CSV or GeoJSON and write a deck that maps them.
pub fn import(text: &str, opts: &Options) -> Result<Imported, Error> {
    let geojson = text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('{');
    let Points {
        records,
        geographic,
        mut warnings,
    } = if geojson {
        read_geojson(text)?
    } else {
        read_csv(text)?
    };

    let fields = numeric_fields(&records);
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
            .find(|f| !IDS.iter().any(|id| f.eq_ignore_ascii_case(id)))
            .or(fields.first())
            .cloned()
            .ok_or_else(|| Error::general("no numeric field to map"))?,
    };
    let name_field = NAMES.iter().find_map(|n| {
        records
            .iter()
            .flat_map(|r| &r.props)
            .find(|(k, p)| k.eq_ignore_ascii_case(n) && matches!(p, Prop::Text(_)))
            .map(|(k, _)| k.clone())
    });

    // Points with a value for the mapped field.
    let mut missing = 0;
    let mut pts: Vec<(f64, f64, f64, Option<String>)> = Vec::new();
    for r in &records {
        let get = |name: &str| r.props.iter().find(|(k, _)| k == name).map(|(_, p)| p);
        match get(&value) {
            Some(Prop::Number(v)) => {
                let name = name_field.as_deref().and_then(get).and_then(|p| match p {
                    Prop::Text(s) => Some(s.clone()),
                    Prop::Number(n) => Some(n.to_string()),
                    Prop::Missing => None,
                });
                pts.push((r.x, r.y, *v, name));
            }
            _ => missing += 1,
        }
    }
    if missing > 0 {
        warnings.push(format!(
            "{missing} points without a value for {value} were left out"
        ));
    }
    if pts.len() < 3 {
        return Err(Error::general(format!(
            "{} points have a value for {value}; at least three are needed",
            pts.len()
        )));
    }

    // Project to kilometres east and north.
    let (projection, en): (String, Vec<(f64, f64)>) = if geographic {
        let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for &(x, y, ..) in &pts {
            lo = (lo.0.min(x), lo.1.min(y));
            hi = (hi.0.max(x), hi.1.max(y));
        }
        let tm = TransverseMercator {
            lon0: (lo.0 + hi.0) / 2.0,
            lat0: (lo.1 + hi.1) / 2.0,
        };
        let en = pts
            .iter()
            .map(|&(x, y, ..)| {
                let (e, n) = tm.forward(x, y);
                (e / 1000.0, n / 1000.0)
            })
            .collect();
        (
            format!(
                "Longitude and latitude are projected with a transverse Mercator on \
                 WGS84, centred on {:.4}, {:.4}, in kilometres.",
                tm.lon0, tm.lat0
            ),
            en,
        )
    } else {
        let extent = pts
            .iter()
            .map(|p| p.0)
            .fold(f64::MIN, f64::max)
            .max(pts.iter().map(|p| p.1).fold(f64::MIN, f64::max))
            - pts
                .iter()
                .map(|p| p.0)
                .fold(f64::MAX, f64::min)
                .min(pts.iter().map(|p| p.1).fold(f64::MAX, f64::min));
        let k = if extent > 1000.0 { 1000.0 } else { 1.0 };
        (
            if k > 1.0 {
                "Projected coordinates are taken to be metres and given in kilometres.".into()
            } else {
                "Projected coordinates are used as given.".into()
            },
            pts.iter().map(|&(x, y, ..)| (x / k, y / k)).collect(),
        )
    };

    let (mut e0, mut n0, mut e1, mut n1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(e, n) in &en {
        e0 = e0.min(e);
        n0 = n0.min(n);
        e1 = e1.max(e);
        n1 = n1.max(n);
    }
    let span = (e1 - e0).max(n1 - n0);
    if span <= 0.0 {
        return Err(Error::general("the points all lie in one place"));
    }
    let margin = 0.05 * span;
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
    let grid: Vec<(f64, f64)> = en.iter().map(|&(e, n)| (top - n, e - left)).collect();

    let source = opts
        .source
        .as_deref()
        .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s))
        .unwrap_or("the imported data");
    let mut out = String::new();
    comment(
        &mut out,
        &format!(
            "Made by symap import from {source}: {} points, mapping {value}.",
            pts.len()
        ),
    );
    comment(&mut out, &projection);
    comment(
        &mut out,
        &format!(
            "Locations are measured down from {:.3} and across from {:.3}, with a \
             margin of {:.3} around the points.",
            top, left, margin
        ),
    );
    for w in &warnings {
        comment(&mut out, &format!("Note: {w}."));
    }
    if name_field.is_some() {
        let _ = writeln!(out, "#\n# Data points, in order:");
        for (i, p) in pts.iter().enumerate() {
            if let Some(name) = &p.3 {
                let name: String = name.chars().filter(|c| !c.is_control()).take(70).collect();
                let _ = writeln!(out, "# {:>4} {name}", i + 1);
            }
        }
    }

    if opts.outline {
        let h = hull(&grid);
        if h.len() >= 3 {
            let (cd, ca) = h.iter().fold((0.0, 0.0), |s, p| (s.0 + p.0, s.1 + p.1));
            let c = (cd / h.len() as f64, ca / h.len() as f64);
            let _ = writeln!(out, "A-OUTLINE");
            let grown: Vec<(f64, f64)> = h
                .iter()
                .map(|&(d, a)| {
                    let (dd, da) = (d - c.0, a - c.1);
                    let len = (dd * dd + da * da).sqrt().max(1e-9);
                    (d + dd / len * margin * 0.6, a + da / len * margin * 0.6)
                })
                .collect();
            for &(d, a) in grown.iter().chain(grown.first()) {
                let _ = writeln!(out, "{}", coordinate_card(d, a)?);
            }
            let _ = writeln!(out, "99999");
        } else {
            warnings.push("the points are in a line, so no outline was drawn".into());
        }
    }
    let _ = writeln!(out, "B-DATA POINTS");
    for &(d, a) in &grid {
        let _ = writeln!(out, "{}", coordinate_card(d, a)?);
    }
    let _ = writeln!(out, "99999");
    let _ = writeln!(out, "E-VALUES");
    for p in &pts {
        let _ = writeln!(out, "{:10}{}", "", field(p.2)?);
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
    let _ = writeln!(
        out,
        "{}",
        card_text(&format!("{value} at {} points", pts.len()))
    );
    let _ = writeln!(
        out,
        "{}",
        match opts.map {
            MapType::Contour => "CONTOUR MAP - SHEPARD INTERPOLATION",
            MapType::Proximal => "PROXIMAL MAP - NEAREST DATA POINT",
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
    if opts.map == MapType::Proximal {
        let _ = writeln!(out, "   31\n   36\n   37");
    }
    let _ = writeln!(out, "99999\n999999");

    Ok(Imported {
        deck: out,
        fields,
        value,
        points: pts.len(),
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
        assert_eq!(imp.points, 4);
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
            map: MapType::Proximal,
            ..Options::default()
        };
        let imp = import(gj, &opts).unwrap();
        assert_eq!(imp.points, 4);
        assert_eq!(
            imp.warnings,
            ["1 features that are not points were left out"]
        );
        assert!(imp.deck.contains("   31\n   36\n   37\n"));
        let run = crate::run(&imp.deck).unwrap();
        assert_eq!(run.maps[0].kind, crate::MapKind::Proximal);
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
