//! Parsing SYMAP input decks (1971 manual, section 2).
//!
//! A deck is a sequence of packages, each opened by a header card and closed
//! by `99999` in columns 1-5; `999999` ends the deck. A package stays in
//! effect for every later map until a package of the same kind replaces it
//! (an empty package removes it). Each F-MAP package draws one map.

use crate::card;
use crate::elective::{self, Entry};
use crate::error::{Diagnostic, Error};
use crate::geom::Point;
use crate::symbols::Strikes;

/// Which kind of map to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapKind {
    /// Interpolated surface (isarithmic map).
    Contour,
    /// Each character takes the value of its nearest data point.
    Proximal,
    /// Each character takes the value of the zone that contains it.
    Conformant,
}

/// How a conformant zone outline is drawn (column 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneShape {
    Area,
    Line,
    Point,
}

/// One A-CONFORMOLINES outline.
#[derive(Debug, Clone, PartialEq)]
pub struct Conformoline {
    /// 1-based reference into the values.
    pub reference: usize,
    pub shape: ZoneShape,
    pub vertices: Vec<Point>,
}

/// One C-OTOLEGENDS legend.
#[derive(Debug, Clone, PartialEq)]
pub enum Legend {
    /// A single overprinted symbol, shifted by (rows, columns).
    Symbol {
        strikes: Strikes,
        at: Point,
        shift: (f64, f64),
    },
    /// A character string, horizontal or reading down.
    Text {
        text: String,
        vertical: bool,
        at: Point,
        shift: (f64, f64),
    },
    /// A symbol repeated along a polyline.
    Line {
        strikes: Strikes,
        vertices: Vec<Point>,
    },
    /// A symbol filling a polygon.
    Area {
        strikes: Strikes,
        vertices: Vec<Point>,
    },
}

/// One D-BARRIERS barrier: vertices and their strengths.
#[derive(Debug, Clone, PartialEq)]
pub struct Barrier {
    pub vertices: Vec<(Point, Strength)>,
}

/// A barrier vertex's strength. A negative strength on a card (normally -1)
/// is impermeable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strength {
    Impermeable,
    /// Added, squared, to the squared distance across the barrier.
    Permeable(f64),
}

/// The packages in effect when a map is drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Packages {
    pub outlines: Vec<Vec<Point>>,
    pub conformolines: Vec<Conformoline>,
    pub points: Vec<Point>,
    pub legends: Vec<Legend>,
    pub barriers: Vec<Barrier>,
    /// E-VALUES, one per card; `None` is a blank value field.
    pub values: Vec<Option<f64>>,
    /// E1-VALUES INDEX: reference number -> replacement (`None` = missing).
    pub index: Vec<(usize, Option<usize>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapSpec {
    /// Card number of the `F-MAP` header.
    pub line: usize,
    pub titles: [String; 3],
    pub electives: Vec<Entry>,
    pub packages: Packages,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Deck {
    pub maps: Vec<MapSpec>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Outline,
    Conformolines,
    DataPoints,
    Legends,
    Barriers,
    Values,
    Index,
    Map,
}

fn header(card: &str) -> Option<Result<Kind, String>> {
    let name: String = card::cols(card, 1, 22)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    Some(Ok(match name.as_str() {
        "A-OUTLINE" => Kind::Outline,
        "A-CONFORMOLINES" => Kind::Conformolines,
        "B-DATAPOINTS" => Kind::DataPoints,
        "C-OTOLEGENDS" => Kind::Legends,
        "D-BARRIERS" => Kind::Barriers,
        "E-VALUES" => Kind::Values,
        "E1-VALUESINDEX" => Kind::Index,
        "F-MAP" => Kind::Map,
        _ => {
            let b = name.as_bytes();
            let dash = b.iter().position(|&c| c == b'-')?;
            if (1..=2).contains(&dash)
                && b[0].is_ascii_uppercase()
                && b.get(dash + 1).is_some_and(u8::is_ascii_uppercase)
            {
                return Some(Err(card.trim().to_string()));
            }
            return None;
        }
    }))
}

fn is_terminator(card: &str) -> bool {
    card::cols(card, 1, 5) == "99999" && card::is_blank(card::cols(card, 6, 80))
}

fn is_deck_end(card: &str) -> bool {
    card.trim() == "999999"
}

/// Splits a sequence of vertices into rings closed by repeating the first vertex.
fn rings(ln: usize, pts: Vec<(usize, Point)>, what: &str) -> Result<Vec<Vec<Point>>, Error> {
    let mut out = Vec::new();
    let mut cur: Vec<Point> = Vec::new();
    let mut start = ln;
    for (l, p) in pts {
        if cur.is_empty() {
            start = l;
        }
        if cur.len() >= 3 && cur[0] == p {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(p);
        }
    }
    if !cur.is_empty() {
        return Err(Error::at(
            start,
            format!("{what} starting here is not closed by repeating its first vertex"),
        ));
    }
    Ok(out)
}

/// The cards of one package, with the header's measurement conversion.
struct Package<'a> {
    kind: Kind,
    line: usize,
    /// Row and column measurement: units per inch (header columns 31-50).
    per_unit: (f64, f64),
    cards: Vec<(usize, &'a str)>,
}

impl Package<'_> {
    fn point(&self, (d, a): (f64, f64)) -> Point {
        Point::new(d / self.per_unit.0, a / self.per_unit.1)
    }

    fn pair(&self, ln: usize, c: &str) -> Result<Point, Error> {
        card::pair(c).map(|p| self.point(p)).ok_or_else(|| {
            Error::at(
                ln,
                "expected a location: down in columns 11-20, across in columns 21-30",
            )
        })
    }

    fn data_cards(&self) -> impl Iterator<Item = (usize, &str)> {
        self.cards
            .iter()
            .copied()
            .filter(|(_, c)| !card::is_blank(c))
    }
}

fn read_outline(p: &Package) -> Result<Vec<Vec<Point>>, Error> {
    let pts = p
        .data_cards()
        .map(|(l, c)| Ok((l, p.pair(l, c)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    rings(p.line, pts, "outline")
}

fn shape_of(c: char) -> Option<ZoneShape> {
    match c.to_ascii_uppercase() {
        'A' => Some(ZoneShape::Area),
        'L' => Some(ZoneShape::Line),
        'P' => Some(ZoneShape::Point),
        _ => None,
    }
}

fn read_conformolines(p: &Package) -> Result<Vec<Conformoline>, Error> {
    let mut out: Vec<Conformoline> = Vec::new();
    let mut last_ref: usize = 0;
    for (l, c) in p.data_cards() {
        // Fixed format: reference in 1-5, shape in 10.
        let (start, rest) = if let Some(shape) =
            shape_of(card::col(c, 10)).filter(|_| card::is_blank(card::cols(c, 6, 9)))
        {
            let r = card::int_field(c, 1, 5)
                .ok_or_else(|| Error::at(l, "zone reference must be a whole number"))?;
            (Some((r, shape)), c.to_string())
        } else if card::is_blank(card::cols(c, 1, 10)) {
            (None, c.to_string())
        } else {
            // Free format: [reference] A|L|P down across, or down across.
            let toks: Vec<&str> = c.split_whitespace().collect();
            match toks
                .iter()
                .position(|t| t.len() == 1 && shape_of(t.chars().next().unwrap()).is_some())
            {
                Some(k) if k <= 1 => {
                    let r =
                        if k == 1 {
                            Some(toks[0].parse::<i64>().map_err(|_| {
                                Error::at(l, "zone reference must be a whole number")
                            })?)
                        } else {
                            None
                        };
                    let shape = shape_of(toks[k].chars().next().unwrap()).unwrap();
                    (Some((r, shape)), toks[k + 1..].join(" "))
                }
                _ => (None, c.to_string()),
            }
        };
        let v = p.pair(l, &rest)?;
        match start {
            Some((r, shape)) => {
                let reference = match r {
                    Some(r) if r >= 1 => usize::try_from(r)
                        .map_err(|_| Error::at(l, "zone reference is too large"))?,
                    Some(_) => return Err(Error::at(l, "zone reference must be at least 1")),
                    None => last_ref
                        .checked_add(1)
                        .ok_or_else(|| Error::at(l, "zone reference is too large"))?,
                };
                last_ref = reference;
                out.push(Conformoline {
                    reference,
                    shape,
                    vertices: vec![v],
                });
            }
            None => match out.last_mut() {
                Some(z) => z.vertices.push(v),
                None => {
                    return Err(Error::at(
                        l,
                        "first conformoline card needs A, L or P in column 10",
                    ));
                }
            },
        }
    }
    for z in &mut out {
        if z.shape == ZoneShape::Area {
            let n = z.vertices.len();
            if n < 4 || z.vertices[0] != z.vertices[n - 1] {
                return Err(Error::at(
                    p.line,
                    format!(
                        "area zone {} must repeat its first vertex to close",
                        z.reference
                    ),
                ));
            }
            z.vertices.pop();
        }
    }
    Ok(out)
}

/// Continuation cards (columns 1-10 blank) after a line or area legend's first card.
fn more_vertices(p: &Package, i: &mut usize, first: Point) -> Result<Vec<Point>, Error> {
    let mut v = vec![first];
    while let Some(&(l, c)) = p.cards.get(*i) {
        if card::is_blank(c) || !card::is_blank(card::cols(c, 1, 10)) {
            break;
        }
        v.push(p.pair(l, c)?);
        *i += 1;
    }
    Ok(v)
}

fn read_legends(p: &Package) -> Result<Vec<Legend>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    let cards = &p.cards;
    while i < cards.len() {
        let (l, c) = cards[i];
        i += 1;
        if card::is_blank(c) {
            continue;
        }
        let at = p.pair(l, c)?;
        let shift = || -> Result<(f64, f64), Error> {
            let f = |k| card::field(c, k).ok_or_else(|| Error::at(l, "unreadable displacement"));
            Ok((f(2)?.unwrap_or(0.0), f(3)?.unwrap_or(0.0)))
        };
        let strikes = Strikes::new(card::cols(c, 6, 9));
        match card::col(c, 10).to_ascii_uppercase() {
            // A text legend has no symbol in columns 6-9 and its length in
            // columns 2-5 (normally 4-5); column 1 holds '-' for vertical text.
            'P' => match card::int_field(c, 2, 5) {
                None if card::is_blank(card::cols(c, 6, 9)) => {
                    return Err(Error::at(
                        l,
                        "legend length in columns 2-5 must be a whole number",
                    ));
                }
                Some(Some(n)) if card::is_blank(card::cols(c, 6, 9)) => {
                    if !(1..=50).contains(&n) {
                        return Err(Error::at(l, "legend length in columns 2-5 must be 1 to 50"));
                    }
                    let Some(&(_, t)) = cards.get(i) else {
                        return Err(Error::at(
                            l,
                            "text legend needs a second card with the text",
                        ));
                    };
                    i += 1;
                    out.push(Legend::Text {
                        text: card::cols(t, 1, n as usize).trim_end().to_string(),
                        vertical: card::col(c, 1) == '-',
                        at,
                        shift: shift()?,
                    });
                }
                None | Some(None) | Some(Some(_)) => out.push(Legend::Symbol {
                    strikes,
                    at,
                    shift: shift()?,
                }),
            },
            'L' => out.push(Legend::Line {
                strikes,
                vertices: more_vertices(p, &mut i, at)?,
            }),
            'A' => {
                let mut v = more_vertices(p, &mut i, at)?;
                if v.len() < 4 || v[0] != v[v.len() - 1] {
                    return Err(Error::at(
                        l,
                        "area legend must repeat its first vertex to close",
                    ));
                }
                v.pop();
                out.push(Legend::Area {
                    strikes,
                    vertices: v,
                });
            }
            _ => return Err(Error::at(l, "legend card needs P, L or A in column 10")),
        }
    }
    Ok(out)
}

/// A barrier vertex's strength: columns 31-40, or the third number of a
/// free-format card. Blank is 0.
fn barrier_strength(l: usize, c: &str) -> Result<Strength, Error> {
    let bad = || Error::at(l, "unreadable barrier strength");
    let s = if card::fixed_pair(c).is_some() {
        card::field(c, 2).ok_or_else(bad)?.unwrap_or(0.0)
    } else {
        match c.split_whitespace().nth(2) {
            None => 0.0,
            Some(t) => card::real_field(t).flatten().ok_or_else(bad)?,
        }
    };
    Ok(if s < 0.0 {
        Strength::Impermeable
    } else {
        Strength::Permeable(s)
    })
}

fn read_barriers(p: &Package) -> Result<Vec<Barrier>, Error> {
    let mut out = Vec::new();
    let mut cur: Vec<(Point, Strength)> = Vec::new();
    for (l, c) in p.data_cards() {
        let at = p.pair(l, c)?;
        let s = barrier_strength(l, c)?;
        // Repeating the last vertex ends a barrier.
        if cur.last().is_some_and(|&(q, _)| q == at) {
            out.push(Barrier {
                vertices: std::mem::take(&mut cur),
            });
        } else {
            cur.push((at, s));
        }
    }
    if !cur.is_empty() {
        out.push(Barrier { vertices: cur });
    }
    Ok(out)
}

fn read_values(p: &Package) -> Result<Vec<Option<f64>>, Error> {
    p.cards
        .iter()
        .map(|&(l, c)| {
            if card::is_blank(c) {
                return Ok(None);
            }
            match card::field(c, 0) {
                Some(Some(v)) => Ok(Some(v)),
                Some(None) if card::is_blank(card::cols(c, 1, 10)) => Ok(None),
                _ => card::free_numbers(c, 1)
                    .map(|v| Some(v[0]))
                    .ok_or_else(|| Error::at(l, "expected a value in columns 11-20")),
            }
        })
        .collect()
}

fn read_index(p: &Package) -> Result<Vec<(usize, Option<usize>)>, Error> {
    let mut out = Vec::new();
    let (mut cur, mut rep) = (0usize, 0usize);
    // A card with columns 1-5 blank (even an entirely blank one) marks a missing value.
    for &(l, c) in &p.cards {
        let bad = || Error::at(l, "expected reference numbers in columns 1-5 and 6-10");
        let from = card::int_field(c, 1, 5).ok_or_else(bad)?;
        let to = card::int_field(c, 6, 10).ok_or_else(bad)?;
        match from {
            None => {
                cur += 1;
                out.push((cur, None));
            }
            Some(f) if f >= 1 => {
                cur = f as usize;
                rep = match to {
                    Some(t) if t >= 1 => t as usize,
                    Some(_) => return Err(bad()),
                    None => rep + 1,
                };
                out.push((cur, Some(rep)));
            }
            Some(_) => return Err(bad()),
        }
    }
    Ok(out)
}

impl Deck {
    pub fn parse(text: &str) -> Result<Deck, Error> {
        let mut deck = Deck::default();
        let mut state = Packages::default();
        let mut open: Option<Package> = None;
        let mut prev_electives: Vec<Entry> = Vec::new();
        let mut ended = false;
        let warn = |deck: &mut Deck, line, m: String| {
            deck.diagnostics.push(Diagnostic { line, message: m })
        };

        let lines: Vec<&str> = text.lines().collect();
        for (idx, &c) in lines.iter().enumerate() {
            let ln = idx + 1;
            if is_deck_end(c) {
                if open.is_some() {
                    return Err(Error::at(ln, "deck ended inside a package (missing 99999)"));
                }
                ended = true;
                break;
            }
            match open.as_mut() {
                None => {
                    if card::is_blank(c) || c.trim_start().starts_with('#') {
                        continue;
                    }
                    match header(c) {
                        Some(Ok(kind)) => {
                            let per_unit = match (card::field(c, 2), card::field(c, 3)) {
                                (Some(Some(r)), Some(Some(k))) if r > 0.0 && k > 0.0 => (r, k),
                                (Some(None), Some(None)) => (1.0, 1.0),
                                _ => {
                                    warn(
                                        &mut deck,
                                        ln,
                                        "the row and column measurements in columns 31-50 \
                                         need two positive numbers; ignored"
                                            .into(),
                                    );
                                    (1.0, 1.0)
                                }
                            };
                            open = Some(Package {
                                kind,
                                line: ln,
                                per_unit,
                                cards: Vec::new(),
                            });
                        }
                        Some(Err(name)) => {
                            return Err(Error::at(ln, format!("unknown package {name:?}")));
                        }
                        None => {
                            return Err(Error::at(
                                ln,
                                format!("expected a package header, found {:?}", c.trim()),
                            ));
                        }
                    }
                }
                Some(p) => {
                    if !is_terminator(c) {
                        p.cards.push((ln, c));
                        continue;
                    }
                    let p = open.take().unwrap();
                    match p.kind {
                        Kind::Outline => state.outlines = read_outline(&p)?,
                        Kind::Conformolines => state.conformolines = read_conformolines(&p)?,
                        Kind::DataPoints => {
                            state.points = p
                                .data_cards()
                                .map(|(l, c)| p.pair(l, c))
                                .collect::<Result<_, _>>()?
                        }
                        Kind::Legends => state.legends = read_legends(&p)?,
                        Kind::Barriers => state.barriers = read_barriers(&p)?,
                        Kind::Values => state.values = read_values(&p)?,
                        Kind::Index => state.index = read_index(&p)?,
                        Kind::Map => {
                            if p.cards.len() < 3 {
                                return Err(Error::at(p.line, "F-MAP needs three title cards"));
                            }
                            let titles = std::array::from_fn(|k| {
                                card::cols(p.cards[k].1, 1, 72).trim_end().to_string()
                            });
                            let electives = elective::resolve_repeat(
                                &prev_electives,
                                elective::parse(&p.cards[3..])?,
                            );
                            prev_electives = electives.clone();
                            deck.maps.push(MapSpec {
                                line: p.line,
                                titles,
                                electives,
                                packages: state.clone(),
                            });
                        }
                    }
                }
            }
        }
        if let Some(p) = open {
            return Err(Error::at(p.line, "package not closed by 99999"));
        }
        if !ended {
            warn(
                &mut deck,
                lines.len(),
                "deck does not end with 999999".into(),
            );
        }
        Ok(deck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f10(v: &[f64]) -> String {
        v.iter().map(|x| format!("{x:>10}")).collect()
    }

    #[test]
    fn outlines_split_on_closure() {
        let mut d = String::from("A-OUTLINE\n");
        for p in [
            (1.0, 1.6),
            (0.5, 4.1),
            (4.3, 2.6),
            (1.0, 1.6),
            (3.5, 5.8),
            (2.6, 7.1),
            (4.6, 6.8),
            (3.5, 5.8),
        ] {
            d += &format!("{:10}{}\n", "", f10(&[p.0, p.1]));
        }
        d += "99999\n999999\n";
        let deck = Deck::parse(&d).unwrap();
        assert!(deck.maps.is_empty());
        // No map, but parse succeeded; check via a map.
        let d2 = d.replace("999999\n", "F-MAP\nA\nB\nC\n99999\n999999\n");
        let m = &Deck::parse(&d2).unwrap().maps[0];
        assert_eq!(m.packages.outlines.len(), 2);
        assert_eq!(m.packages.outlines[1].len(), 3);
    }

    #[test]
    fn row_and_column_measurement() {
        let d = format!(
            "B-DATA POINTS{:17}{:>10}{:>10}\n{:10}{}\n99999\nF-MAP\n\n\n\n99999\n999999\n",
            "",
            "8.",
            "10.",
            "",
            f10(&[16.0, 25.0])
        );
        let m = &Deck::parse(&d).unwrap().maps[0];
        assert_eq!(m.packages.points, vec![Point::new(2.0, 2.5)]);
        // One measurement alone is ignored, with a warning.
        let d = format!(
            "B-DATA POINTS{:17}{:>10}\n{:10}{}\n99999\nF-MAP\n\n\n\n99999\n999999\n",
            "",
            "8.",
            "",
            f10(&[16.0, 25.0])
        );
        let deck = Deck::parse(&d).unwrap();
        assert_eq!(deck.maps[0].packages.points, vec![Point::new(16.0, 25.0)]);
        assert_eq!(deck.diagnostics.len(), 1);
        assert!(deck.diagnostics[0].message.contains("columns 31-50"));
    }

    #[test]
    fn conformolines_fixed_and_free() {
        let d = format!(
            "A-CONFORMOLINES\n{:>5}    A{}\n{:10}{}\n{:10}{}\n{:10}{}\n         P{}\n2 L 5 5\n6 6\n99999\nF-MAP\n\n\n\n99999\n999999\n",
            3,
            f10(&[0.0, 0.0]),
            "",
            f10(&[0.0, 1.0]),
            "",
            f10(&[1.0, 1.0]),
            "",
            f10(&[0.0, 0.0]),
            f10(&[9.0, 9.0])
        );
        let z = &Deck::parse(&d).unwrap().maps[0].packages.conformolines;
        assert_eq!(z.len(), 3);
        assert_eq!(
            (z[0].reference, z[0].shape, z[0].vertices.len()),
            (3, ZoneShape::Area, 3)
        );
        // Blank reference: previous plus one.
        assert_eq!((z[1].reference, z[1].shape), (4, ZoneShape::Point));
        assert_eq!(
            (z[2].reference, z[2].shape, z[2].vertices.len()),
            (2, ZoneShape::Line, 2)
        );
    }

    #[test]
    fn packages_persist_and_empty_package_deletes() {
        let d = "C-OTOLEGENDS\n    1    P       1.0       1.0\nN\n99999\n\
                 F-MAP\nONE\n\n\n99999\nC-OTOLEGENDS\n99999\nF-MAP\nTWO\n\n\n99999\n999999\n";
        let deck = Deck::parse(d).unwrap();
        assert_eq!(deck.maps[0].packages.legends.len(), 1);
        assert!(deck.maps[1].packages.legends.is_empty());
    }

    #[test]
    fn legends() {
        let d = "C-OTOLEGENDS\n\
                 \x20   5    P        .6       6.8       1.0      -2.0\n\
                 SCALE\n\
                 \x20    R/  L      2.31      4.08\n\
                 \x20             2.35      7.92\n\
                 \x20    A   P       3.8       5.2\n\
                 -   3    P       1.0       1.0\n\
                 ABC\n\
                 99999\nF-MAP\n\n\n\n99999\n999999\n";
        let l = &Deck::parse(d).unwrap().maps[0].packages.legends;
        assert_eq!(l.len(), 4);
        assert_eq!(
            l[0],
            Legend::Text {
                text: "SCALE".into(),
                vertical: false,
                at: Point::new(0.6, 6.8),
                shift: (1.0, -2.0)
            }
        );
        assert!(
            matches!(&l[1], Legend::Line { strikes, vertices } if *strikes == Strikes::new("R/") && vertices.len() == 2)
        );
        assert!(matches!(&l[2], Legend::Symbol { strikes, .. } if *strikes == Strikes::new("A")));
        assert!(matches!(&l[3], Legend::Text { vertical: true, .. }));
    }

    #[test]
    fn values_blank_cards_and_index() {
        let d = "E-VALUES\n          10.0\n\n7\n99999\nE1-VALUES INDEX\n    1    2\n    2\n\n99999\nF-MAP\n\n\n\n99999\n999999\n";
        let p = &Deck::parse(d).unwrap().maps[0].packages;
        assert_eq!(p.values, vec![Some(10.0), None, Some(7.0)]);
        assert_eq!(p.index, vec![(1, Some(2)), (2, Some(3)), (3, None)]);
    }

    #[test]
    fn barrier_strengths_fixed_and_free() {
        let d = "D-BARRIERS\n\
                 \x20                0.0       1.0       -1.\n\
                 5 2 -1\n\
                 6 3\n\
                 99999\nF-MAP\n\n\n\n99999\n999999\n";
        let b = &Deck::parse(d).unwrap().maps[0].packages.barriers;
        let s: Vec<Strength> = b[0].vertices.iter().map(|v| v.1).collect();
        assert_eq!(
            s,
            [
                Strength::Impermeable,
                Strength::Impermeable,
                Strength::Permeable(0.0)
            ]
        );
        for card in ["\x20                0.0       1.0       -1x", "5 2 x"] {
            let d = format!("D-BARRIERS\n{card}\n99999\n999999\n");
            let e = Deck::parse(&d).unwrap_err();
            assert_eq!(e.line, Some(2));
            assert!(e.message.contains("strength"), "{e}");
        }
    }

    #[test]
    fn errors_carry_line_numbers() {
        let e = Deck::parse("B-DATA POINTS\n1 2\nfoo\n99999\n").unwrap_err();
        assert_eq!(e.line, Some(3));
        assert_eq!(Deck::parse("hello\n").unwrap_err().line, Some(1));
        assert!(Deck::parse("Q-NONSENSE\n99999\n").is_err());
        assert!(Deck::parse("F-MAP\nA\n99999\n").is_err());
        // An unreadable text legend length is reported on its own card.
        let d = "C-OTOLEGENDS\n-  x3    P       1.0       1.0\nABC\n99999\n999999\n";
        let e = Deck::parse(d).unwrap_err();
        assert_eq!(e.line, Some(2));
        assert!(e.message.contains("columns 2-5"), "{e}");
    }
}
