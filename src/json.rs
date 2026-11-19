//! A JSON rendering of a run, for the browser front end.
//!
//! The crate has no dependencies, so this is a small hand-written encoder.
//! Shape:
//!
//! ```text
//! { "ok": true,
//!   "diagnostics": [{"line": 3, "message": "..."}],
//!   "maps": [{ "kind": "contour", "titles": [..3], "warnings": [..],
//!              "rows": 40, "cols": 80, "linesPerInch": 8, "colsPerInch": 10,
//!              "lines": [{"plain": "...", "strikes": ["...", ...]}],
//!              "asa": "..." }] }
//! { "ok": false, "error": {"line": 12, "message": "..."} }
//! ```

use crate::deck::MapKind;
use crate::map::PrintedMap;
use std::fmt::Write;

fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn list<T>(
    out: &mut String,
    items: impl IntoIterator<Item = T>,
    mut f: impl FnMut(&mut String, T),
) {
    out.push('[');
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        f(out, item);
    }
    out.push(']');
}

fn line_no(out: &mut String, line: Option<usize>) {
    match line {
        Some(n) => {
            let _ = write!(out, "{n}");
        }
        None => out.push_str("null"),
    }
}

fn map(out: &mut String, m: &PrintedMap) {
    let kind = match m.kind {
        MapKind::Contour => "contour",
        MapKind::Proximal => "proximal",
        MapKind::Conformant => "conformant",
    };
    let _ = write!(
        out,
        "{{\"kind\":\"{kind}\",\"rows\":{},\"cols\":{},\"linesPerInch\":{},\"colsPerInch\":{},\"titles\":",
        m.frame.rows, m.frame.cols, m.frame.lines_per_inch, m.frame.cols_per_inch
    );
    list(out, &m.titles, |o, t| string(o, t));
    out.push_str(",\"warnings\":");
    list(out, &m.warnings, |o, w| string(o, w));
    out.push_str(",\"lines\":");
    list(out, m.page(), |o, l| {
        o.push_str("{\"plain\":");
        string(o, &l.plain);
        o.push_str(",\"strikes\":");
        list(o, &l.strikes, |o, s| string(o, s));
        if l.eject {
            o.push_str(",\"eject\":true");
        }
        o.push('}');
    });
    if let Some(csv) = m.values_csv() {
        out.push_str(",\"valuesCsv\":");
        string(out, &csv);
    }
    out.push_str(",\"asa\":");
    string(out, &m.to_asa());
    out.push('}');
}

/// Run a deck and describe the result as JSON. Never fails: errors become
/// `{"ok": false, ...}`.
pub fn run(text: &str) -> String {
    let mut out = String::new();
    match crate::run(text) {
        Ok(r) => {
            out.push_str("{\"ok\":true,\"diagnostics\":");
            list(&mut out, &r.diagnostics, |o, d| {
                o.push_str("{\"line\":");
                line_no(o, Some(d.line));
                o.push_str(",\"message\":");
                string(o, &d.message);
                o.push('}');
            });
            out.push_str(",\"maps\":");
            list(&mut out, &r.maps, map);
            out.push('}');
        }
        Err(e) => error(&mut out, &e),
    }
    out
}

fn error(out: &mut String, e: &crate::Error) {
    out.push_str("{\"ok\":false,\"error\":{\"line\":");
    line_no(out, e.line);
    out.push_str(",\"message\":");
    string(out, &e.message);
    out.push_str("}}");
}

/// Import points or zones and describe the deck made as JSON. The request
/// is a JSON object: `{"text": "...", "name": "file.csv", "value": "no2",
/// "map": "contour" | "proximal" | "conformant", "width": 8, "title": "...",
/// "outline": true, "object": "..."}`; only `text` is required. The reply is
/// `{"ok": true, "deck": "...", "fields": [...], "value": "...",
/// "map": "...", "count": n, "unit": "points" | "zones", "warnings": [...]}`
/// or an error as for `run`.
pub fn import(request: &str) -> String {
    use crate::import::{self, json::Value};
    let mut out = String::new();
    let result = import::json::parse(request).and_then(|req| {
        let text = req
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| crate::Error::general("the request has no text"))?;
        let s = |k| req.get(k).and_then(Value::as_str).map(str::to_string);
        let mut opts = import::Options {
            value: s("value").filter(|v| !v.is_empty()),
            title: s("title").filter(|v| !v.is_empty()),
            source: s("name"),
            object: s("object").filter(|v| !v.is_empty()),
            map: s("map").as_deref().and_then(import::MapType::parse),
            ..import::Options::default()
        };
        if let Some(w) = req.get("width").and_then(Value::as_f64) {
            opts.width = w;
        }
        if let Some(Value::Bool(b)) = req.get("outline") {
            opts.outline = *b;
        }
        import::import(text, &opts)
    });
    match result {
        Ok(imp) => {
            out.push_str("{\"ok\":true,\"deck\":");
            string(&mut out, &imp.deck);
            out.push_str(",\"fields\":");
            list(&mut out, &imp.fields, |o, f| string(o, f));
            out.push_str(",\"value\":");
            string(&mut out, &imp.value);
            let _ = write!(
                out,
                ",\"map\":\"{}\",\"count\":{},\"unit\":\"{}\",\"warnings\":",
                imp.map.name(),
                imp.count,
                imp.unit
            );
            list(&mut out, &imp.warnings, |o, w| string(o, w));
            out.push('}');
        }
        Err(e) => error(&mut out, &e),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_request() {
        let req = r#"{"text": "lon,lat,v\n-0.1,51.5,1\n-0.2,51.6,2\n0.0,51.4,3\n", "name": "a.csv", "map": "proximal", "width": 6}"#;
        let reply = import(req);
        assert!(reply.starts_with("{\"ok\":true,\"deck\":\"# Made by symap import from a.csv"));
        assert!(reply.contains(
            "\"fields\":[\"v\"],\"value\":\"v\",\"map\":\"proximal\",\"count\":3,\"unit\":\"points\",\"warnings\":[]}"
        ));
        assert!(reply.contains("   31\\n   36\\n   37"));
        let parsed = crate::import::json::parse(&reply).unwrap();
        let deck = parsed.get("deck").unwrap().as_str().unwrap();
        crate::run(deck).unwrap();
        assert!(import("{}").contains("\"message\":\"the request has no text\""));
        assert!(import("not json").starts_with("{\"ok\":false"));
    }

    #[test]
    fn escapes() {
        let mut s = String::new();
        string(&mut s, "a\"b\\c\nd\u{1}é█");
        assert_eq!(s, r#""a\"b\\c\nd\u0001é█""#);
    }

    #[test]
    fn success_and_error_shapes() {
        let deck = "B-DATA POINTS\n0 0\n0 10\n10 0\n10 10\n99999\nE-VALUES\n1\n2\n3\n4\n99999\n\
                    F-MAP\nT\n\n\n    1       1.0       1.0\n99999\n999999\n";
        let j = run(deck);
        assert!(j.starts_with("{\"ok\":true,\"diagnostics\":[],\"maps\":[{\"kind\":\"contour\",\"rows\":8,\"cols\":10,"));
        assert!(j.contains("\"strikes\":[") && j.contains("\"asa\":\"1MAP SCALE"));
        assert_eq!(
            run("hello\n"),
            r#"{"ok":false,"error":{"line":1,"message":"expected a package header, found \"hello\""}}"#
        );
    }
}
