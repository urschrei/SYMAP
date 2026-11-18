//! Parsing SYMAP input decks.
//!
//! A deck is a sequence of packages, each opened by a header card
//! (`A-OUTLINE`, `B-DATA POINTS`, ...) and closed by a `99999` card. The
//! whole deck ends with `999999`. Package layouts that are documented in
//! surviving sources follow them; the rest are provisional and noted in the
//! crate README.

use crate::card;
use crate::error::{Diagnostic, Error};
use crate::geom::Point;

/// Which kind of map to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    /// Interpolated surface, classed into level bands (isarithmic map).
    Contour,
    /// Each cell takes the value of its nearest data point.
    Proximal,
    /// Each cell takes the value of the zone that contains it (choropleth).
    Conformant,
}

/// A conformant zone: one or more rings sharing a value.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub id: i64,
    pub rings: Vec<Vec<Point>>,
}

/// A numbered F-MAP elective.
#[derive(Debug, Clone, PartialEq)]
pub enum Elective {
    /// Elective 1: printed map size in inches.
    MapSize { height: f64, width: f64 },
    /// Elective 13: map scale (recorded, not yet applied).
    Scale(f64),
    /// Elective 14: margins in inches.
    Margins {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    },
    /// Elective 15: printer pitch, characters per inch.
    CharsPerInch { rows: f64, cols: f64 },
    /// Any elective this implementation does not interpret.
    Other {
        number: u32,
        fields: Vec<Option<f64>>,
    },
}

/// Non-standard `*KEYWORD` cards in an F-MAP package.
///
/// Original SYMAP selected these behaviours through electives whose exact
/// numbering is not yet confirmed, so they use a separate syntax that cannot
/// clash with real elective cards.
#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    Kind(MapKind),
    Levels(usize),
    /// Explicit class boundaries, lowest to highest (levels + 1 values).
    Breaks(Vec<f64>),
    MarkPoints,
    NoSlope,
    NoDirection,
}

/// One requested map: an F-MAP package plus the E-VALUES in force when it was read.
#[derive(Debug, Clone, PartialEq)]
pub struct MapSpec {
    /// Card number of the `F-MAP` header.
    pub line: usize,
    pub titles: [String; 3],
    pub electives: Vec<(usize, Elective)>,
    pub directives: Vec<Directive>,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deck {
    pub outline: Vec<Point>,
    pub zones: Vec<Zone>,
    pub points: Vec<Point>,
    pub maps: Vec<MapSpec>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
enum Header {
    Outline,
    Conformolines,
    DataPoints,
    Values,
    Map,
    Unknown(String),
}

fn header(t: &str) -> Option<Header> {
    let key: String = t
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    Some(match key.as_str() {
        "A-OUTLINE" => Header::Outline,
        "A-CONFORMOLINES" => Header::Conformolines,
        "B-DATAPOINTS" => Header::DataPoints,
        "E-VALUES" => Header::Values,
        "F-MAP" => Header::Map,
        _ => {
            let b = key.as_bytes();
            if b.len() > 2 && b[0].is_ascii_uppercase() && b[1] == b'-' && b[2].is_ascii_uppercase()
            {
                Header::Unknown(t.to_string())
            } else {
                return None;
            }
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Top,
    Outline,
    Zones,
    Points,
    Values,
    Titles(usize),
    Map,
    Skip,
}

/// Accumulates vertices into rings. A vertex equal to its ring's first vertex
/// closes the ring.
#[derive(Default)]
struct RingBuilder {
    current: Vec<Point>,
}

impl RingBuilder {
    /// Returns a finished ring when `p` closes the current one.
    fn push(&mut self, p: Point) -> Option<Vec<Point>> {
        if self.current.len() >= 3 && self.current[0] == p {
            return Some(std::mem::take(&mut self.current));
        }
        self.current.push(p);
        None
    }

    fn finish(&mut self) -> Option<Vec<Point>> {
        let r = std::mem::take(&mut self.current);
        (!r.is_empty()).then_some(r)
    }
}

#[derive(Default)]
struct ZoneBuilder {
    zones: Vec<Zone>,
    id: Option<i64>,
    ring: RingBuilder,
}

impl ZoneBuilder {
    fn add_ring(&mut self, id: i64, ring: Vec<Point>) {
        match self.zones.iter_mut().find(|z| z.id == id) {
            Some(z) => z.rings.push(ring),
            None => self.zones.push(Zone {
                id,
                rings: vec![ring],
            }),
        }
    }

    fn push(&mut self, id: i64, p: Point) {
        if self.id != Some(id) {
            self.flush();
            self.id = Some(id);
        }
        if let Some(r) = self.ring.push(p) {
            self.add_ring(id, r);
        }
    }

    fn flush(&mut self) {
        if let (Some(id), Some(r)) = (self.id, self.ring.finish()) {
            self.add_ring(id, r);
        }
        self.id = None;
    }
}

/// Parse an elective card: number in columns 1-5 (normally 4-5), then
/// 10-column real fields from column 11. Falls back to free format.
fn elective_fields(line: &str) -> Option<(u32, Vec<Option<f64>>)> {
    if let Ok(n) = card::cols(line, 1, 5).trim().parse::<u32>() {
        if card::cols(line, 6, 10).trim().is_empty() {
            let fields: Option<Vec<_>> = (0..7)
                .map(|i| card::real_field(&card::cols(line, 11 + 10 * i, 20 + 10 * i)))
                .collect();
            if let Some(mut f) = fields {
                while f.last() == Some(&None) {
                    f.pop();
                }
                return Some((n, f));
            }
        }
    }
    let mut toks = line.split_whitespace();
    let n = toks.next()?.parse().ok()?;
    let f = toks
        .map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some))
        .collect::<Option<Vec<_>>>()?;
    Some((n, f))
}

fn elective(ln: usize, line: &str) -> Result<Elective, Error> {
    let (n, f) = elective_fields(line).ok_or_else(|| Error::at(ln, "unreadable elective card"))?;
    let get = |i: usize| f.get(i).copied().flatten();
    let need = |i: usize, what: &str| {
        get(i).ok_or_else(|| Error::at(ln, format!("elective {n}: missing {what}")))
    };
    Ok(match n {
        1 => Elective::MapSize {
            height: need(0, "vertical size")?,
            width: need(1, "horizontal size")?,
        },
        13 => Elective::Scale(need(0, "scale")?),
        14 => Elective::Margins {
            top: get(0).unwrap_or(0.0),
            left: get(1).unwrap_or(0.0),
            bottom: get(2).unwrap_or(0.0),
            right: get(3).unwrap_or(0.0),
        },
        15 => Elective::CharsPerInch {
            rows: get(0).unwrap_or(6.0),
            cols: get(1).unwrap_or(10.0),
        },
        number => Elective::Other { number, fields: f },
    })
}

fn directive(ln: usize, t: &str) -> Result<Option<Directive>, Error> {
    let mut toks = t[1..].split_whitespace();
    let kw = toks.next().unwrap_or("").to_ascii_uppercase();
    let rest: Vec<&str> = toks.collect();
    let bad = |m: &str| Error::at(ln, format!("*{kw}: {m}"));
    Ok(Some(match kw.as_str() {
        "TYPE" => match rest.first().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("CONTOUR") => Directive::Kind(MapKind::Contour),
            Some("PROXIMAL") => Directive::Kind(MapKind::Proximal),
            Some("CONFORMANT") => Directive::Kind(MapKind::Conformant),
            _ => return Err(bad("expected CONTOUR, PROXIMAL or CONFORMANT")),
        },
        "LEVELS" => Directive::Levels(
            rest.first()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| bad("expected a whole number"))?,
        ),
        "BREAKS" => Directive::Breaks(
            rest.iter()
                .map(|s| s.parse::<f64>().ok().filter(|v| v.is_finite()))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| bad("expected numbers"))?,
        ),
        "MARKPOINTS" => Directive::MarkPoints,
        "NOSLOPE" => Directive::NoSlope,
        "NODIRECTION" => Directive::NoDirection,
        _ => return Ok(None),
    }))
}

impl Deck {
    pub fn parse(text: &str) -> Result<Deck, Error> {
        let mut deck = Deck::default();
        let mut state = State::Top;
        let mut values: Vec<f64> = Vec::new();
        let mut zones = ZoneBuilder::default();
        let mut outline = RingBuilder::default();
        let mut map: Option<MapSpec> = None;
        let mut last = 0;

        let warn = |deck: &mut Deck, line, m: String| {
            deck.diagnostics.push(Diagnostic { line, message: m })
        };

        // Close whatever package is open.
        let close = |state: State,
                     deck: &mut Deck,
                     zones: &mut ZoneBuilder,
                     outline: &mut RingBuilder,
                     map: &mut Option<MapSpec>,
                     ln: usize|
         -> Result<(), Error> {
            match state {
                State::Outline => {
                    if let Some(r) = outline.finish() {
                        if !deck.outline.is_empty() {
                            return Err(Error::at(ln, "A-OUTLINE holds more than one ring"));
                        }
                        deck.outline = r;
                    }
                    if !deck.outline.is_empty() && deck.outline.len() < 3 {
                        return Err(Error::at(ln, "A-OUTLINE needs at least three vertices"));
                    }
                }
                State::Zones => {
                    zones.flush();
                    deck.zones = std::mem::take(&mut zones.zones);
                }
                State::Titles(_) => return Err(Error::at(ln, "F-MAP needs three title cards")),
                State::Map => deck.maps.extend(map.take()),
                _ => {}
            }
            Ok(())
        };

        for (idx, raw) in text.lines().enumerate() {
            let ln = idx + 1;
            last = ln;
            let t = raw.trim();

            if t == "999999" {
                close(state, &mut deck, &mut zones, &mut outline, &mut map, ln)?;
                state = State::Top;
                break;
            }
            if let State::Titles(k) = state {
                if t == "99999" {
                    return Err(Error::at(ln, "F-MAP needs three title cards"));
                }
                if let Some(m) = map.as_mut() {
                    m.titles[k] = raw.trim_end().chars().take(80).collect();
                }
                state = if k == 2 {
                    State::Map
                } else {
                    State::Titles(k + 1)
                };
                continue;
            }
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if t == "99999" {
                if state == State::Top {
                    warn(&mut deck, ln, "terminator outside a package".into());
                }
                close(state, &mut deck, &mut zones, &mut outline, &mut map, ln)?;
                state = State::Top;
                continue;
            }
            if let Some(h) = header(t) {
                if state != State::Top {
                    warn(
                        &mut deck,
                        ln,
                        "package not closed by 99999 before next header".into(),
                    );
                    close(state, &mut deck, &mut zones, &mut outline, &mut map, ln)?;
                }
                state = match h {
                    Header::Outline => State::Outline,
                    Header::Conformolines => State::Zones,
                    Header::DataPoints => {
                        deck.points.clear();
                        State::Points
                    }
                    Header::Values => {
                        values.clear();
                        State::Values
                    }
                    Header::Map => {
                        map = Some(MapSpec {
                            line: ln,
                            titles: Default::default(),
                            electives: Vec::new(),
                            directives: Vec::new(),
                            values: values.clone(),
                        });
                        State::Titles(0)
                    }
                    Header::Unknown(name) => {
                        warn(
                            &mut deck,
                            ln,
                            format!("package {name} is not supported; skipped"),
                        );
                        State::Skip
                    }
                };
                continue;
            }

            let nums = |n: usize, what: &str| {
                card::numbers(raw, n).ok_or_else(|| Error::at(ln, format!("expected {what}")))
            };
            match state {
                State::Top => {
                    return Err(Error::at(
                        ln,
                        format!("expected a package header, found {t:?}"),
                    ));
                }
                State::Skip => {}
                State::Outline => {
                    let v = nums(2, "two coordinates (down, across)")?;
                    if let Some(r) = outline.push(Point::new(v[0], v[1])) {
                        if !deck.outline.is_empty() {
                            return Err(Error::at(ln, "A-OUTLINE holds more than one ring"));
                        }
                        deck.outline = r;
                    }
                }
                State::Zones => {
                    let v = nums(3, "zone number and two coordinates")?;
                    if v[0].fract() != 0.0 {
                        return Err(Error::at(ln, "zone number must be a whole number"));
                    }
                    zones.push(v[0] as i64, Point::new(v[1], v[2]));
                }
                State::Points => {
                    let v = nums(2, "two coordinates (down, across)")?;
                    deck.points.push(Point::new(v[0], v[1]));
                }
                State::Values => values.push(nums(1, "a data value")?[0]),
                State::Map => {
                    let m = map.as_mut().expect("map open");
                    if t.starts_with('*') {
                        match directive(ln, t)? {
                            Some(d) => m.directives.push(d),
                            None => warn(&mut deck, ln, format!("unknown directive {t:?} ignored")),
                        }
                    } else {
                        m.electives.push((ln, elective(ln, raw)?));
                    }
                }
                State::Titles(_) => unreachable!(),
            }
        }

        if state != State::Top {
            warn(&mut deck, last, "deck ended inside a package".into());
            close(state, &mut deck, &mut zones, &mut outline, &mut map, last)?;
        }
        Ok(deck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_and_free_electives() {
        let fixed = "    1      7.5       12.0";
        assert_eq!(
            elective(1, fixed).unwrap(),
            Elective::MapSize {
                height: 7.5,
                width: 12.0
            }
        );
        assert_eq!(
            elective(1, "15 8 10").unwrap(),
            Elective::CharsPerInch {
                rows: 8.0,
                cols: 10.0
            }
        );
        assert!(elective(1, "  1").is_err());
    }

    #[test]
    fn zones_split_on_id_change_and_closure() {
        let text = "A-CONFORMOLINES\n\
                    1 0 0\n1 0 1\n1 1 1\n1 0 0\n\
                    1 5 5\n1 5 6\n1 6 6\n\
                    2 0 0\n2 0 1\n2 1 1\n99999\n999999\n";
        let d = Deck::parse(text).unwrap();
        assert_eq!(d.zones.len(), 2);
        assert_eq!(d.zones[0].rings.len(), 2);
        assert_eq!(d.zones[0].rings[0].len(), 3);
    }

    #[test]
    fn map_binds_current_values_and_titles() {
        let text = "E-VALUES\n1\n2\n99999\nF-MAP\nT1\n\nT3\n*LEVELS 3\n99999\n\
                    E-VALUES\n5\n99999\nF-MAP\nA\nB\nC\n99999\n999999\n";
        let d = Deck::parse(text).unwrap();
        assert_eq!(d.maps.len(), 2);
        assert_eq!(d.maps[0].values, vec![1.0, 2.0]);
        assert_eq!(
            d.maps[0].titles,
            ["T1".to_string(), String::new(), "T3".to_string()]
        );
        assert_eq!(d.maps[0].directives, vec![Directive::Levels(3)]);
        assert_eq!(d.maps[1].values, vec![5.0]);
    }

    #[test]
    fn errors_carry_line_numbers() {
        let e = Deck::parse("B-DATA POINTS\n1 2\nfoo\n").unwrap_err();
        assert_eq!(e.line, Some(3));
        let e = Deck::parse("hello\n").unwrap_err();
        assert_eq!(e.line, Some(1));
    }

    #[test]
    fn unknown_packages_are_skipped_with_warning() {
        let d = Deck::parse("D-BARRIERS\n1 2 3\n99999\n999999\n").unwrap();
        assert_eq!(d.diagnostics.len(), 1);
    }
}
