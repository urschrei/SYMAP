//! Readers for the importer: CSV, GeoJSON and TopoJSON, all to features
//! with properties and point or polygon geometry.

use super::csv;
use super::topology::P;
use crate::Error;
use geojson::{GeoJson, GeometryValue, Position};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum Prop {
    Number(f64),
    Text(String),
    Missing,
}

impl Prop {
    pub fn from_text(s: &str) -> Prop {
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

    fn from_json(v: &Value) -> Prop {
        match v {
            Value::Number(n) => n.as_f64().map_or(Prop::Missing, Prop::Number),
            Value::String(s) => Prop::from_text(s),
            Value::Bool(b) => Prop::Text(b.to_string()),
            _ => Prop::Missing,
        }
    }
}

/// A ring given by its coordinates, or (TopoJSON) by arc references.
#[derive(Debug, Clone, PartialEq)]
pub enum Ring {
    Coords(Vec<P>),
    Arcs(Vec<i64>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Geom {
    Point(P),
    /// Outer rings and holes; a MultiPolygon has several outer rings.
    Polygon {
        outer: Vec<Ring>,
        holes: Vec<Ring>,
    },
    /// The parts of a LineString or MultiLineString (open paths, held as
    /// rings are).
    Line(Vec<Ring>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    pub props: Vec<(String, Prop)>,
    pub geom: Geom,
}

#[derive(Debug, Default)]
pub struct Source {
    pub features: Vec<Feature>,
    /// TopoJSON arcs, in source coordinates.
    pub arcs: Vec<Vec<P>>,
    /// Known from CSV column names; otherwise judged from the coordinates.
    pub geographic: Option<bool>,
    pub warnings: Vec<String>,
}

const LON: &[&str] = &["lon", "lng", "long", "longitude"];
const LAT: &[&str] = &["lat", "latitude"];
const EAST: &[&str] = &["easting", "east", "x"];
const NORTH: &[&str] = &["northing", "north", "y"];

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|n| header.iter().position(|h| h.eq_ignore_ascii_case(n)))
}

/// Read CSV, GeoJSON or TopoJSON, recognised by content.
pub fn read(text: &str, object: Option<&str>) -> Result<Source, Error> {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if !t.starts_with('{') {
        return read_csv(text);
    }
    let doc: Value =
        serde_json::from_str(t).map_err(|e| Error::at(e.line(), format!("JSON: {e}")))?;
    match doc.get("type").and_then(Value::as_str) {
        Some("Topology") => read_topojson(&doc, object),
        Some("FeatureCollection" | "Feature") => read_geojson(doc),
        _ => Err(Error::general(
            "expected GeoJSON (a FeatureCollection or a Feature) or TopoJSON (a Topology)",
        )),
    }
}

fn read_csv(text: &str) -> Result<Source, Error> {
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
    let mut src = Source {
        geographic: Some(geographic),
        ..Source::default()
    };
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
        src.features.push(Feature {
            props,
            geom: Geom::Point((x, y)),
        });
    }
    if skipped > 0 {
        src.warnings
            .push(format!("{skipped} rows without coordinates were left out"));
    }
    Ok(src)
}

fn props_of(v: Option<&Value>) -> Vec<(String, Prop)> {
    match v {
        Some(Value::Object(m)) => m
            .iter()
            .map(|(k, v)| (k.clone(), Prop::from_json(v)))
            .collect(),
        _ => Vec::new(),
    }
}

fn position(v: &Value, what: &str) -> Result<P, Error> {
    let a = v.as_array().filter(|a| a.len() >= 2);
    match (a.and_then(|a| a[0].as_f64()), a.and_then(|a| a[1].as_f64())) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(Error::general(format!("{what}: malformed position"))),
    }
}

fn positions(v: &Value, what: &str) -> Result<Vec<P>, Error> {
    v.as_array()
        .ok_or_else(|| Error::general(format!("{what}: expected an array of positions")))?
        .iter()
        .map(|p| position(p, what))
        .collect()
}

fn polygon(rings: &[Ring]) -> (Vec<Ring>, Vec<Ring>) {
    let mut it = rings.iter().cloned();
    (it.next().into_iter().collect(), it.collect())
}

fn xy(p: &Position) -> P {
    (p[0], p[1])
}

fn coords(ps: &[Position]) -> Ring {
    Ring::Coords(ps.iter().map(xy).collect())
}

fn read_geojson(doc: Value) -> Result<Source, Error> {
    let features = match serde_json::from_value::<GeoJson>(doc)
        .map_err(|e| Error::general(format!("GeoJSON: {e}")))?
    {
        GeoJson::FeatureCollection(fc) => fc.features,
        GeoJson::Feature(f) => vec![f],
        GeoJson::Geometry(_) => unreachable!("the type was checked"),
    };
    let mut src = Source::default();
    let mut other = 0;
    for f in features {
        let props: Vec<(String, Prop)> = f
            .properties
            .iter()
            .flatten()
            .map(|(k, v)| (k.clone(), Prop::from_json(v)))
            .collect();
        let mut push = |geom| {
            src.features.push(Feature {
                props: props.clone(),
                geom,
            })
        };
        let polygon = |rings: &[Vec<Position>]| -> (Vec<Ring>, Vec<Ring>) {
            let mut it = rings.iter().map(|r| coords(r));
            (it.next().into_iter().collect(), it.collect())
        };
        match f.geometry.map(|g| g.value) {
            Some(GeometryValue::Point { coordinates }) => push(Geom::Point(xy(&coordinates))),
            Some(GeometryValue::MultiPoint { coordinates }) => {
                for p in &coordinates {
                    push(Geom::Point(xy(p)));
                }
            }
            Some(GeometryValue::Polygon { coordinates }) => {
                let (outer, holes) = polygon(&coordinates);
                push(Geom::Polygon { outer, holes });
            }
            Some(GeometryValue::MultiPolygon { coordinates }) => {
                let (mut outer, mut holes) = (Vec::new(), Vec::new());
                for p in &coordinates {
                    let (o, h) = polygon(p);
                    outer.extend(o);
                    holes.extend(h);
                }
                push(Geom::Polygon { outer, holes });
            }
            Some(GeometryValue::LineString { coordinates }) => {
                push(Geom::Line(vec![coords(&coordinates)]));
            }
            Some(GeometryValue::MultiLineString { coordinates }) => {
                push(Geom::Line(coordinates.iter().map(|l| coords(l)).collect()));
            }
            _ => other += 1,
        }
    }
    if other > 0 {
        src.warnings.push(format!(
            "{other} features without point, line or polygon geometry were left out"
        ));
    }
    Ok(src)
}

fn read_topojson(doc: &Value, object: Option<&str>) -> Result<Source, Error> {
    let objects = match doc.get("objects") {
        Some(Value::Object(m)) if !m.is_empty() => m,
        _ => return Err(Error::general("TopoJSON: the topology has no objects")),
    };
    let (name, obj) = match object {
        Some(n) => objects.get_key_value(n).ok_or_else(|| {
            let names: Vec<&str> = objects.iter().map(|(k, _)| k.as_str()).collect();
            Error::general(format!(
                "TopoJSON: no object '{n}'; objects: {}",
                names.join(", ")
            ))
        })?,
        None => objects.iter().next().expect("objects is not empty"),
    };
    // Quantized topologies store positions as integers to be scaled and
    // translated, and arcs as deltas.
    let transform = doc.get("transform").map(|t| {
        let pair = |k| {
            t.get(k)
                .and_then(|v| position(v, "TopoJSON transform").ok())
                .unwrap_or((0.0, 0.0))
        };
        (pair("scale"), pair("translate"))
    });
    let place = |p: P| match transform {
        Some((s, t)) => (p.0 * s.0 + t.0, p.1 * s.1 + t.1),
        None => p,
    };
    let mut src = Source::default();
    for (i, arc) in doc
        .get("arcs")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .enumerate()
    {
        let pts = positions(arc, &format!("TopoJSON arc {i}"))?;
        let pts = match transform {
            Some(_) => {
                let mut acc = (0.0, 0.0);
                pts.iter()
                    .map(|d| {
                        acc = (acc.0 + d.0, acc.1 + d.1);
                        place(acc)
                    })
                    .collect()
            }
            None => pts,
        };
        src.arcs.push(pts);
    }
    let n_arcs = src.arcs.len() as i64;
    let ring = |v: &Value| -> Result<Ring, Error> {
        let refs: Option<Vec<i64>> = v
            .as_array()
            .map(|a| a.iter().map(|x| x.as_f64().map(|f| f as i64)).collect())
            .and_then(|r: Option<Vec<i64>>| r);
        match refs {
            Some(r) if r.iter().all(|&a| (if a < 0 { !a } else { a }) < n_arcs) => {
                Ok(Ring::Arcs(r))
            }
            _ => Err(Error::general(format!(
                "TopoJSON object '{name}': a ring refers to a missing arc"
            ))),
        }
    };
    let rings = |v: &Value| -> Result<Vec<Ring>, Error> {
        v.as_array()
            .ok_or_else(|| Error::general("TopoJSON: expected rings"))?
            .iter()
            .map(ring)
            .collect()
    };
    let geometries: Vec<&Value> = match obj.get("type").and_then(Value::as_str) {
        Some("GeometryCollection") => obj
            .get("geometries")
            .and_then(Value::as_array)
            .map(|g| g.iter().collect())
            .unwrap_or_default(),
        _ => vec![obj],
    };
    let mut other = 0;
    for g in geometries {
        let mut props = props_of(g.get("properties"));
        if let Some(id) = g.get("id") {
            if !props.iter().any(|(k, _)| k == "id") {
                props.insert(0, ("id".into(), Prop::from_json(id)));
            }
        }
        let mut push = |geom| {
            src.features.push(Feature {
                props: props.clone(),
                geom,
            })
        };
        match g.get("type").and_then(Value::as_str) {
            Some("Point") => {
                let c = g.get("coordinates").map(|c| position(c, "TopoJSON point"));
                push(Geom::Point(place(c.ok_or_else(|| {
                    Error::general("TopoJSON: a point has no coordinates")
                })??)));
            }
            Some("MultiPoint") => {
                for p in positions(
                    g.get("coordinates").unwrap_or(&Value::Null),
                    "TopoJSON points",
                )? {
                    push(Geom::Point(place(p)));
                }
            }
            Some("Polygon") => {
                let (outer, holes) = polygon(&rings(g.get("arcs").unwrap_or(&Value::Null))?);
                push(Geom::Polygon { outer, holes });
            }
            Some("MultiPolygon") => {
                let (mut outer, mut holes) = (Vec::new(), Vec::new());
                for p in g
                    .get("arcs")
                    .and_then(Value::as_array)
                    .map_or(&[][..], Vec::as_slice)
                {
                    let (o, h) = polygon(&rings(p)?);
                    outer.extend(o);
                    holes.extend(h);
                }
                push(Geom::Polygon { outer, holes });
            }
            Some("LineString") => {
                push(Geom::Line(vec![ring(
                    g.get("arcs").unwrap_or(&Value::Null),
                )?]));
            }
            Some("MultiLineString") => {
                push(Geom::Line(rings(g.get("arcs").unwrap_or(&Value::Null))?));
            }
            _ => other += 1,
        }
    }
    if other > 0 {
        src.warnings.push(format!(
            "{other} geometries without point, line or polygon geometry were left out"
        ));
    }
    if objects.len() > 1 && object.is_none() {
        src.warnings.push(format!(
            "the topology has {} objects; '{name}' was used",
            objects.len()
        ));
    }
    Ok(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topojson_quantized() {
        // Two unit squares side by side sharing arc 1 (the example in the
        // TopoJSON specification's style), quantized with scale 0.5.
        let t = r#"{"type": "Topology",
          "transform": {"scale": [0.5, 0.5], "translate": [10, 20]},
          "objects": {"zones": {"type": "GeometryCollection", "geometries": [
            {"type": "Polygon", "arcs": [[0, 1]], "properties": {"v": 1}},
            {"type": "Polygon", "arcs": [[2, -2]], "id": "b", "properties": {"v": 2}}
          ]}},
          "arcs": [
            [[2, 0], [-2, 0], [0, 2], [2, 0]],
            [[2, 2], [0, -2]],
            [[2, 2], [2, 0], [0, -2], [-2, 0]]
          ]}"#;
        let s = read(t, None).unwrap();
        assert_eq!(s.arcs[1], [(11.0, 21.0), (11.0, 20.0)]);
        assert_eq!(
            s.arcs[0],
            [(11.0, 20.0), (10.0, 20.0), (10.0, 21.0), (11.0, 21.0)]
        );
        assert_eq!(s.features.len(), 2);
        assert_eq!(
            s.features[1].props[0],
            ("id".into(), Prop::Text("b".into()))
        );
        assert_eq!(
            s.features[1].geom,
            Geom::Polygon {
                outer: vec![Ring::Arcs(vec![2, -2])],
                holes: vec![]
            }
        );
        assert!(
            read(t, Some("nope"))
                .unwrap_err()
                .message
                .contains("objects: zones")
        );
    }

    #[test]
    fn geojson_polygons_with_holes() {
        let g = r#"{"type": "Feature", "properties": {"v": 3},
          "geometry": {"type": "MultiPolygon", "coordinates": [
            [[[0,0],[4,0],[4,4],[0,4],[0,0]], [[1,1],[2,1],[2,2],[1,1]]],
            [[[5,5],[6,5],[6,6],[5,5]]]
          ]}}"#;
        let s = read(g, None).unwrap();
        let Geom::Polygon { outer, holes } = &s.features[0].geom else {
            panic!()
        };
        assert_eq!((outer.len(), holes.len()), (2, 1));
    }
}
