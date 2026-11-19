//! The browser front end's view of a run or an import, as serde types.
//!
//! Shape of a run:
//!
//! ```text
//! { "ok": true,
//!   "diagnostics": [{"line": 3, "message": "..."}],
//!   "maps": [{ "kind": "contour", "rows": 40, "cols": 80, "linesPerInch": 8,
//!              "colsPerInch": 10, "titles": [..3], "warnings": [..],
//!              "lines": [{"plain": "...", "strikes": ["...", ...]}],
//!              "valuesCsv": "...", "asa": "..." }] }
//! { "ok": false, "error": {"line": 12, "message": "..."} }
//! ```

use crate::deck::MapKind;
use crate::map::PrintedMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(untagged)]
pub enum Reply<T> {
    Ok(T),
    Err { ok: bool, error: ErrorReply },
}

#[derive(Serialize)]
pub struct ErrorReply {
    pub line: Option<usize>,
    pub message: String,
}

impl<T> From<Result<T, crate::Error>> for Reply<T> {
    fn from(r: Result<T, crate::Error>) -> Self {
        match r {
            Ok(v) => Reply::Ok(v),
            Err(e) => Reply::Err {
                ok: false,
                error: ErrorReply {
                    line: e.line,
                    message: e.message,
                },
            },
        }
    }
}

#[derive(Serialize)]
pub struct RunReply {
    pub ok: bool,
    pub diagnostics: Vec<ErrorReply>,
    pub maps: Vec<MapReply>,
}

#[derive(Serialize)]
pub struct LineReply {
    pub plain: String,
    pub strikes: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub eject: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapReply {
    pub kind: &'static str,
    pub rows: usize,
    pub cols: usize,
    pub lines_per_inch: f64,
    pub cols_per_inch: f64,
    pub titles: [String; 3],
    pub warnings: Vec<String>,
    pub lines: Vec<LineReply>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub values_csv: Option<String>,
    pub asa: String,
}

impl From<&PrintedMap> for MapReply {
    fn from(m: &PrintedMap) -> Self {
        let page = m.page();
        let asa = crate::page::asa(&page);
        MapReply {
            kind: match m.kind {
                MapKind::Contour => "contour",
                MapKind::Proximal => "proximal",
                MapKind::Conformant => "conformant",
            },
            rows: m.frame.rows,
            cols: m.frame.cols,
            lines_per_inch: m.frame.lines_per_inch,
            cols_per_inch: m.frame.cols_per_inch,
            titles: m.titles.clone(),
            warnings: m.warnings.clone(),
            lines: page
                .into_iter()
                .map(|l| LineReply {
                    plain: l.plain,
                    strikes: l.strikes,
                    eject: l.eject,
                })
                .collect(),
            values_csv: m.values_csv(),
            asa,
        }
    }
}

/// Run a deck. Never fails: errors become `{"ok": false, ...}`.
pub fn run(text: &str) -> Reply<RunReply> {
    crate::run(text)
        .map(|r| RunReply {
            ok: true,
            diagnostics: r
                .diagnostics
                .into_iter()
                .map(|d| ErrorReply {
                    line: Some(d.line),
                    message: d.message,
                })
                .collect(),
            maps: r.maps.iter().map(MapReply::from).collect(),
        })
        .into()
}

/// An import request; only `text` is required.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ImportRequest {
    pub text: Option<String>,
    pub name: Option<String>,
    pub value: Option<String>,
    /// "contour", "proximal" or "conformant".
    pub map: Option<String>,
    pub width: Option<f64>,
    pub title: Option<String>,
    pub outline: Option<bool>,
    /// The TopoJSON object.
    pub object: Option<String>,
    /// "legend", "barrier", "both" or "none".
    pub lines: Option<String>,
    pub line_symbol: Option<String>,
    pub lines_text: Option<String>,
    pub lines_name: Option<String>,
}

#[derive(Serialize)]
pub struct ImportReply {
    pub ok: bool,
    pub deck: String,
    pub fields: Vec<String>,
    pub value: String,
    pub map: &'static str,
    pub count: usize,
    /// "points" or "zones".
    pub unit: &'static str,
    pub lines: usize,
    pub warnings: Vec<String>,
}

/// Import points, zones and lines, and describe the deck made.
pub fn import(req: ImportRequest) -> Reply<ImportReply> {
    use crate::import;
    let some = |v: Option<String>| v.filter(|v| !v.is_empty());
    let result = (|| {
        let text = req
            .text
            .ok_or_else(|| crate::Error::general("the request has no text"))?;
        let mut opts = import::Options {
            value: some(req.value),
            title: some(req.title),
            source: req.name,
            object: some(req.object),
            map: some(req.map)
                .map(|m| {
                    import::MapType::parse(&m).ok_or_else(|| {
                        crate::Error::general(format!(
                            "unknown map '{m}': expected contour, proximal or conformant"
                        ))
                    })
                })
                .transpose()?,
            lines_text: some(req.lines_text),
            lines_source: req.lines_name,
            ..import::Options::default()
        };
        if let Some(l) = some(req.lines) {
            opts.lines = import::LineUse::parse(&l).ok_or_else(|| {
                crate::Error::general(format!(
                    "unknown lines '{l}': expected legend, barrier, both or none"
                ))
            })?;
        }
        if let Some(sym) = some(req.line_symbol) {
            opts.line_symbol = sym;
        }
        if let Some(w) = req.width {
            opts.width = w;
        }
        if let Some(b) = req.outline {
            opts.outline = b;
        }
        import::import(&text, &opts)
    })();
    result
        .map(|imp| ImportReply {
            ok: true,
            deck: imp.deck,
            fields: imp.fields,
            value: imp.value,
            map: imp.map.name(),
            count: imp.count,
            unit: imp.unit,
            lines: imp.lines,
            warnings: imp.warnings,
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn to_json<T: Serialize>(v: &T) -> Value {
        serde_json::to_value(v).unwrap()
    }

    fn request(v: Value) -> ImportRequest {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn import_request() {
        let reply = to_json(&import(request(json!({
            "text": "lon,lat,v\n-0.1,51.5,1\n-0.2,51.6,2\n0.0,51.4,3\n",
            "name": "a.csv", "map": "proximal", "width": 6
        }))));
        let deck = reply["deck"].as_str().unwrap();
        assert!(deck.starts_with("# Made by symap import from a.csv"));
        assert!(deck.contains("   31\n   36\n   37"));
        assert_eq!(
            reply,
            json!({"ok": true, "deck": deck, "fields": ["v"], "value": "v",
                   "map": "proximal", "count": 3, "unit": "points", "lines": 0,
                   "warnings": []})
        );
        crate::run(deck).unwrap();
        assert_eq!(
            to_json(&import(request(json!({})))),
            json!({"ok": false, "error": {"line": null, "message": "the request has no text"}})
        );
        let text = "lon,lat,v\n-0.1,51.5,1\n-0.2,51.6,2\n0.0,51.4,3\n";
        let auto = to_json(&import(request(
            json!({"text": text, "map": "", "lines": ""}),
        )));
        assert_eq!(auto["map"], "contour");
        for (k, v) in [("map", "contur"), ("lines", "barriers")] {
            let reply = to_json(&import(request(json!({"text": text, k: v}))));
            assert_eq!(reply["ok"], false);
            let message = reply["error"]["message"].as_str().unwrap();
            assert!(message.contains(&format!("unknown {k} '{v}'")), "{message}");
        }
    }

    #[test]
    fn success_and_error_shapes() {
        let deck = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n99999\nE-VALUES\n1\n2\n3\n4\n99999\n\
                    F-MAP\nT\n\n\n    1       1.0       1.0\n99999\n999999\n";
        let j = to_json(&run(deck));
        assert_eq!(j["ok"], true);
        assert_eq!(j["diagnostics"], json!([]));
        let m = &j["maps"][0];
        assert_eq!(
            (m["kind"].as_str(), m["rows"].as_u64(), m["cols"].as_u64()),
            (Some("contour"), Some(8), Some(10))
        );
        assert!(m["lines"][0]["strikes"].is_array());
        assert!(m["asa"].as_str().unwrap().starts_with("1MAP SCALE"));
        assert_eq!(
            serde_json::to_string(&run("hello\n")).unwrap(),
            r#"{"ok":false,"error":{"line":1,"message":"expected a package header, found \"hello\""}}"#
        );
    }
}
