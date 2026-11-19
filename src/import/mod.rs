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

mod cards;
mod csv;
mod fields;
mod geometry;
mod lines;
mod options;
mod project;
mod read;
mod topology;

pub use options::{Imported, LineUse, Options};

use crate::Error;
use crate::MapKind;
use fields::{is_id, name_field, numeric_fields};
use geometry::{centroid, hull};
use project::Projection;
use read::{Feature, Geom, Prop, Ring, Source};
use topology::{P, Topology};

/// One mapped item: a point, or a zone's rings (arc references).
struct Item {
    value: f64,
    name: Option<String>,
    point: P,
    outer: Vec<Vec<i64>>,
    holes: Vec<Vec<i64>>,
}

/// A zone ring on the sheet, with its zone's number (from 1).
type ZoneRing = (usize, Vec<P>);

/// A feature with a value for the mapped field, and its name.
struct Valued<'a> {
    feature: &'a Feature,
    value: f64,
    name: Option<String>,
}

/// The printed sheet: the extent mapped, with a margin, and its size.
struct Sheet {
    /// Northing of the top edge and easting of the left edge.
    top: f64,
    left: f64,
    margin: f64,
    /// The extent, down and across, in kilometres (or source units).
    down_extent: f64,
    across_extent: f64,
    /// Printed size in inches.
    width: f64,
    height: f64,
    /// One character across, in kilometres (or source units).
    char_width: f64,
}

impl Sheet {
    /// Down and across from the top-left corner.
    fn grid(&self, (e, n): P) -> P {
        (self.top - n, e - self.left)
    }
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
    let lines = lines::gather(&src, &line_features, opts, &mut warnings)?;
    let zonal = !polys.is_empty();
    if zonal && !points.is_empty() {
        warnings.push(format!(
            "{} point features were left out; the polygons are mapped",
            points.len()
        ));
    }
    let map = opts.map.unwrap_or(if zonal {
        MapKind::Conformant
    } else {
        MapKind::Contour
    });
    if map == MapKind::Conformant && !zonal {
        return Err(Error::general(
            "a conformant map needs polygons; this file has only points",
        ));
    }
    let features = if zonal { polys } else { points };
    let unit = if zonal { "zones" } else { "points" };

    let fields = numeric_fields(&features);
    let value = choose_value(&fields, opts.value.as_deref())?;
    let name_field = name_field(&features);
    let kept = with_values(&features, &value, name_field.as_deref());
    let missing = features.len() - kept.len();
    if missing > 0 {
        warnings.push(format!(
            "{missing} {unit} without a value for {value} were left out"
        ));
    }
    let needed = if map == MapKind::Conformant { 1 } else { 3 };
    if kept.len() < needed {
        return Err(Error::general(format!(
            "{} {unit} have a value for {value}; at least {needed} are needed",
            kept.len()
        )));
    }

    let raw = source_coordinates(&src, &kept);
    let projection = Projection::choose(&raw, src.geographic, lines.geographic, &mut warnings)?;
    let (items, topo) = project_items(&src, &kept, zonal, &projection);
    let sheet = fit_sheet(&items, &topo, zonal, unit, opts.width)?;
    let tolerance = sheet.char_width / 2.0;
    let (items, rings) = if zonal {
        zone_rings(items, topo, &sheet, map, needed, &mut warnings)?
    } else {
        let items = items
            .into_iter()
            .map(|i| Item {
                point: sheet.grid(i.point),
                ..i
            })
            .collect();
        (items, Vec::new())
    };

    let symbol: String = opts.line_symbol.chars().take(4).collect();
    if opts.lines.draws() && symbol.chars().any(|c| c.is_control()) {
        return Err(Error::general(
            "the line symbol must be printable characters",
        ));
    }
    let clipped = lines::clip_to(&lines.paths, &projection, &sheet, tolerance);
    if !lines.paths.is_empty() && clipped.parts.is_empty() {
        warnings.push("the lines all lie outside the map".into());
    }
    // What is written: barriers have no effect on a conformant map.
    let written = match (opts.lines, map, clipped.parts.is_empty()) {
        (_, _, true) => LineUse::Omit,
        (LineUse::Barrier | LineUse::Both, MapKind::Conformant, false) => {
            warnings
                .push("barriers have no effect on a conformant map, so none were written".into());
            if opts.lines == LineUse::Both {
                LineUse::Legend
            } else {
                LineUse::Omit
            }
        }
        (lines, _, false) => lines,
    };
    let point_outline = if opts.outline && !zonal && map != MapKind::Conformant {
        let outline = point_outline(&items, sheet.margin);
        if outline.is_none() {
            warnings.push("the points are in a line, so no outline was drawn".into());
        }
        outline
    } else {
        None
    };

    let source = opts
        .source
        .as_deref()
        .map(|s| s.rsplit(['/', '\\']).next().unwrap_or(s))
        .unwrap_or("the imported data");
    let title = opts.title.clone().unwrap_or_else(|| {
        source
            .rsplit_once('.')
            .map_or(source, |(s, _)| s)
            .replace(['_', '-'], " ")
    });
    let deck = cards::write_deck(&cards::Plan {
        source,
        title: &title,
        unit,
        value: &value,
        map,
        zonal,
        projection: &projection,
        sheet: &sheet,
        tolerance,
        written,
        symbol: &symbol,
        lines_source: opts.lines_source.as_deref(),
        clipped: &clipped,
        warnings: &warnings,
        named: name_field.is_some(),
        items: &items,
        rings: &rings,
        outline: opts.outline,
        point_outline: point_outline.as_deref(),
    })?;
    Ok(Imported {
        deck,
        fields,
        value,
        map,
        count: items.len(),
        unit,
        lines: lines.count,
        warnings,
    })
}

/// The field to map: the one asked for, or the first numeric field that is
/// not an identifier (or else the first).
fn choose_value(fields: &[String], asked: Option<&str>) -> Result<String, Error> {
    match asked {
        Some(v) => {
            if !fields.iter().any(|f| f == v) {
                return Err(Error::general(format!(
                    "'{v}' is not a numeric field; numeric fields: {}",
                    fields.join(", ")
                )));
            }
            Ok(v.to_string())
        }
        None => fields
            .iter()
            .find(|f| !is_id(f))
            .or(fields.first())
            .cloned()
            .ok_or_else(|| Error::general("no numeric field to map")),
    }
}

/// The features with a number for `value`, with the value and a name.
fn with_values<'a>(
    features: &[&'a Feature],
    value: &str,
    name_field: Option<&str>,
) -> Vec<Valued<'a>> {
    features
        .iter()
        .filter_map(|f| {
            let get = |name: &str| f.props.iter().find(|(k, _)| k == name).map(|(_, p)| p);
            let Some(Prop::Number(v)) = get(value) else {
                return None;
            };
            let name = name_field.and_then(get).and_then(|p| match p {
                Prop::Text(s) => Some(s.clone()),
                Prop::Number(n) => Some(n.to_string()),
                Prop::Missing => None,
            });
            Some(Valued {
                feature: f,
                value: *v,
                name,
            })
        })
        .collect()
}

/// Source coordinates in use, to choose the projection. Lines are drawn on
/// the map but do not set its extent.
fn source_coordinates(src: &Source, kept: &[Valued]) -> Vec<P> {
    let mut raw: Vec<P> = Vec::new();
    let mut used_arcs = vec![false; src.arcs.len()];
    for k in kept {
        match &k.feature.geom {
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
            Geom::Line(_) => {}
        }
    }
    for (arc, _) in src.arcs.iter().zip(&used_arcs).filter(|(_, u)| **u) {
        raw.extend(arc);
    }
    raw
}

/// Projected items, and (for zones) the topology of their borders.
fn project_items(
    src: &Source,
    kept: &[Valued],
    zonal: bool,
    projection: &Projection,
) -> (Vec<Item>, Topology) {
    let project = |p: P| projection.project(p);
    let mut topo = Topology::default();
    let mut items: Vec<Item> = Vec::new();
    if zonal {
        let topojson = !src.arcs.is_empty();
        let mut coords: Vec<Vec<P>> = Vec::new();
        let mut shapes: Vec<(usize, usize)> = Vec::new(); // outer, hole counts
        let mut arc_rings: Vec<Vec<i64>> = Vec::new();
        for k in kept {
            let Geom::Polygon { outer, holes } = &k.feature.geom else {
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
        for (k, (no, nh)) in kept.iter().zip(shapes) {
            let outer: Vec<Vec<i64>> = refs.by_ref().take(no).collect();
            let holes: Vec<Vec<i64>> = refs.by_ref().take(nh).collect();
            items.push(Item {
                value: k.value,
                name: k.name.clone(),
                point: (0.0, 0.0),
                outer,
                holes,
            });
        }
    } else {
        for k in kept {
            let Geom::Point(p) = k.feature.geom else {
                unreachable!()
            };
            items.push(Item {
                value: k.value,
                name: k.name.clone(),
                point: project(p),
                outer: Vec::new(),
                holes: Vec::new(),
            });
        }
    }
    (items, topo)
}

/// The sheet: a margin around everything mapped, `width` inches across.
fn fit_sheet(
    items: &[Item],
    topo: &Topology,
    zonal: bool,
    unit: &str,
    width: f64,
) -> Result<Sheet, Error> {
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
    if !(1.0..=30.0).contains(&width) {
        return Err(Error::general("the map width must be from 1 to 30 inches"));
    }
    let down_extent = n1 - n0 + 2.0 * margin;
    let across_extent = e1 - e0 + 2.0 * margin;
    Ok(Sheet {
        top: n1 + margin,
        left: e0 - margin,
        margin,
        down_extent,
        across_extent,
        width,
        height: width * down_extent / across_extent,
        char_width: 0.1 * across_extent / width,
    })
}

/// Simplify shared borders once, then rebuild each zone's rings on the
/// sheet, smaller rings first. Zones with no usable outline are left out.
fn zone_rings(
    items: Vec<Item>,
    mut topo: Topology,
    sheet: &Sheet,
    map: MapKind,
    needed: usize,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Item>, Vec<ZoneRing>), Error> {
    let grid = |p: P| sheet.grid(p);
    let char_area = sheet.char_width * sheet.char_width * 1.25;
    let mut rings: Vec<ZoneRing> = Vec::new();
    topo.simplify(sheet.char_width / 2.0);
    let outer_refs: Vec<Vec<i64>> = items.iter().flat_map(|i| i.outer.clone()).collect();
    let uses = topo.uses(&outer_refs);
    let (mut small, mut filled, mut empty) = (0, 0, 0);
    let mut kept_items = Vec::new();
    for mut item in items {
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
        let Some(point) = centroid(&kept_rings) else {
            empty += 1;
            continue;
        };
        item.point = point;
        filled += item
            .holes
            .iter()
            .filter(|h| h.iter().any(|&a| uses[topology::arc_index(a)] == 0))
            .count();
        kept_items.push(item);
        let z = kept_items.len();
        rings.extend(kept_rings.into_iter().map(|r| (z, r)));
    }
    if empty > 0 {
        warnings.push(format!(
            "{empty} zones with no usable outline were left out"
        ));
    }
    if kept_items.len() < needed {
        return Err(Error::general(format!(
            "{} zones have a usable outline; at least {needed} are needed",
            kept_items.len()
        )));
    }
    if small > 0 {
        warnings.push(format!(
            "{small} parts smaller than half a character were left out"
        ));
    }
    if filled > 0 && map == MapKind::Conformant {
        warnings.push(format!(
            "{filled} holes with no zone inside are filled by the zone around them"
        ));
    }
    // Smaller rings first: the first zone to claim a character keeps it, so
    // enclaves stay distinct from the zones around them.
    let mut by_area: Vec<(f64, ZoneRing)> = rings
        .into_iter()
        .map(|r| (topology::area(&r.1).abs(), r))
        .collect();
    by_area.sort_by(|a, b| a.0.total_cmp(&b.0));
    let rings = by_area.into_iter().map(|(_, r)| r).collect();
    Ok((kept_items, rings))
}

/// The outline around points: their convex hull, enlarged by part of the
/// margin and closed. `None` if the points are in a line.
fn point_outline(items: &[Item], margin: f64) -> Option<Vec<P>> {
    let pts: Vec<P> = items.iter().map(|i| i.point).collect();
    let h = hull(&pts);
    if h.len() < 3 {
        return None;
    }
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
    Some(grown)
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
            (4, "points", MapKind::Contour)
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
            map: Some(MapKind::Proximal),
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
            map: Some(MapKind::Conformant),
            ..Options::default()
        };
        assert!(import(gj, &conformant).is_err());
    }

    #[test]
    fn wide_longitude_spans() {
        let csv = |w: f64| format!("lon,lat,v\n{},0,1\n0,10,2\n{w},0,3\n", -w);
        let imp = import(&csv(20.0), &Options::default()).unwrap();
        assert!(
            imp.warnings
                .iter()
                .any(|w| w.contains("40 degrees of longitude"))
        );
        assert!(
            import(&csv(10.0), &Options::default())
                .unwrap()
                .warnings
                .is_empty()
        );
        let e = import(&csv(170.0), &Options::default()).unwrap_err();
        assert!(e.message.contains("340 degrees"), "{e}");
    }

    #[test]
    fn points_in_a_line_are_noted_in_the_deck() {
        let imp = import("x,y,v\n0,0,1\n1,0,2\n2,0,3\n", &Options::default()).unwrap();
        assert!(!imp.deck.contains("A-OUTLINE"));
        assert!(
            imp.deck
                .contains("# Note: the points are in a line, so no outline was drawn.")
        );
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
            (MapKind::Conformant, 5, "zones")
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
    fn zones_without_an_outline_are_left_out() {
        let empty = r#",{"type":"Feature","properties":{"name":"Empty","v":99},
            "geometry":{"type":"Polygon","coordinates":[]}}]}"#;
        let g = zones_geojson();
        let g = format!("{}{empty}", g.strip_suffix("]}").unwrap());
        for map in [MapKind::Conformant, MapKind::Contour] {
            let opts = Options {
                map: Some(map),
                ..Options::default()
            };
            let imp = import(&g, &opts).unwrap();
            assert_eq!(imp.count, 5);
            assert!(
                imp.warnings
                    .contains(&"1 zones with no usable outline were left out".into())
            );
            assert!(!imp.deck.contains("99."), "{}", imp.deck);
        }
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
        // Each of the four grid zones shares at least two vertices with each
        // of at least three zones (counting itself).
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
            map: Some(MapKind::Contour),
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
        assert_eq!((imp.count, imp.map), (2, MapKind::Conformant));
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
                .contains("1 lines from river.geojson (River) are drawn as line legends (~)")
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
        let gauge = RIVER
            .find(
                r#",
      {"type": "Feature", "properties": {"name": "Gauge"}"#,
            )
            .unwrap();
        let only_lines = format!("{}\n    ]}}", &RIVER[..gauge]);
        let e = import(&only_lines, &Options::default()).unwrap_err();
        assert!(e.message.contains("the file has only lines"), "{e}");
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
}
