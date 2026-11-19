//! F-MAP electives (1971 manual, section 3).
//!
//! The elective number goes in columns 1-5 (normally 4-5); values go in
//! 10-column fields from column 11. Electives 6, 7 and 10 take more than
//! one card.

use crate::card;
use crate::error::Error;
use crate::geom::Point;
use crate::symbols::Symbolism;

#[derive(Debug, Clone, PartialEq)]
pub enum Elective {
    /// 1: map height and width in inches, border centre to border centre.
    Size {
        height: Option<f64>,
        width: Option<f64>,
    },
    /// 2: the source-map rectangle to show.
    Extremes {
        min: Point,
        max: Point,
    },
    /// 3: number of levels.
    Levels(usize),
    /// 4, 5: value range limits.
    ValueMin(f64),
    ValueMax(f64),
    /// 6: relative level ranges; empty means equal numbers of data points per level.
    Intervals(Vec<f64>),
    /// 7: non-standard symbolism.
    Symbolism(Box<Symbolism>),
    /// 8: no white contour or boundary lines.
    NoContourLines,
    /// 9: no histogram bars under the frequency distribution.
    NoHistogram,
    /// 10: text printed below the map.
    Text(Vec<String>),
    /// 11: print each data value at its point.
    PrintValues,
    /// 12: repeat the previous map's electives (resolved by the deck parser).
    Repeat,
    /// 13: inches of output map per source-map unit.
    Scale(f64),
    /// 14: border shifts in inches (top, left, bottom, right).
    Margins([f64; 4]),
    /// 15: lines and columns per inch.
    CharsPerInch {
        rows: Option<f64>,
        cols: Option<f64>,
    },
    /// 16: allow maps over 72 inches.
    LargeMaps,
    /// 17: suppress tabular output (accepted; there is none).
    NoTabular,
    /// 18: blank and minus-zero values are invalid.
    InvalidBlank,
    /// 19, 20: invalid data range limits.
    InvalidMin(f64),
    InvalidMax(f64),
    /// 21: keep every character's computed value (the original wrote them to
    /// tape for other programs; here they can be downloaded).
    StoreValues,
    /// 28: dot conformant map. Blank: dot each zone to a density matching its
    /// level; `percent`: dots as a share of the total value range; `per_dot`:
    /// each dot stands for this much.
    DotMap {
        percent: bool,
        per_dot: Option<f64>,
    },
    /// 22: keep contour lines even where they crowd out symbolism.
    ContinuousContours,
    /// 23: no symbol at invalid data points.
    NoInvalidSymbol,
    /// 24: no numeric interpretation below the map.
    NoNumericInterpretation,
    /// 25: no data point symbols on a conformant map.
    NoDataPoints,
    /// 26: overprint alignment (a printer fix; accepted, no effect).
    OverprintAlignment,
    /// 27: contour map even when conformolines are present.
    ForceContour,
    /// 31: fraction of the value range the surface may overshoot a local extreme.
    Extrapolation(Option<f64>),
    /// 32, 33: absolute limits of extrapolation.
    ExtrapolationMin(f64),
    ExtrapolationMax(f64),
    /// 34, 35: search radius, in source-map units.
    InitialRadius(f64),
    MaxRadius(f64),
    /// 36: minimum and maximum data points per interpolation.
    PointCount {
        min: Option<usize>,
        max: Option<usize>,
    },
    /// 37: compute every character independently (standard: one in six).
    Independence,
    /// 38, 39 (Version 6 extras) and unknown numbers.
    Unsupported {
        number: u32,
        fields: Vec<Option<f64>>,
    },
}

/// Read the elective number and fields of one card, fixed or free format.
fn number_and_fields(card: &str) -> Option<(u32, Vec<Option<f64>>)> {
    if let Some(Some(n)) = card::int_field(card, 1, 5) {
        if card::is_blank(&card::cols(card, 6, 10)) {
            if let Some(mut f) = (0..7)
                .map(|i| card::field(card, i))
                .collect::<Option<Vec<_>>>()
            {
                while f.last() == Some(&None) {
                    f.pop();
                }
                return Some((u32::try_from(n).ok()?, f));
            }
        }
    }
    let mut toks = card.split_whitespace();
    let n = toks.next()?.parse().ok()?;
    let f = toks
        .map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()).map(Some))
        .collect::<Option<Vec<_>>>()?;
    Some((n, f))
}

/// One parsed elective: its first card's line, number and value.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub line: usize,
    pub number: u32,
    pub elective: Elective,
}

/// Parse the elective cards of one F-MAP package (after the title cards).
pub fn parse(cards: &[(usize, &str)]) -> Result<Vec<Entry>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < cards.len() {
        let (ln, card) = cards[i];
        i += 1;
        if card::is_blank(card) {
            continue;
        }
        let (n, f) =
            number_and_fields(card).ok_or_else(|| Error::at(ln, "unreadable elective card"))?;
        let get = |k: usize| f.get(k).copied().flatten();
        let need = |k: usize, what: &str| {
            get(k).ok_or_else(|| Error::at(ln, format!("elective {n}: missing {what}")))
        };
        let positive = |k: usize, what: &str| {
            let v = need(k, what)?;
            if v > 0.0 {
                Ok(v)
            } else {
                Err(Error::at(
                    ln,
                    format!("elective {n}: {what} must be positive"),
                ))
            }
        };
        let points = |k: usize| -> Result<Option<usize>, Error> {
            get(k)
                .map(|v| {
                    if v.fract() == 0.0 && (1.0..=10.0).contains(&v) {
                        Ok(v as usize)
                    } else {
                        Err(Error::at(
                            ln,
                            "elective 36: point counts must be whole numbers from 1 to 10",
                        ))
                    }
                })
                .transpose()
        };
        let e = match n {
            1 => {
                let (height, width) = (get(0), get(1));
                if height.is_none() && width.is_none() {
                    return Err(Error::at(ln, "elective 1: give a height, a width, or both"));
                }
                Elective::Size { height, width }
            }
            2 => Elective::Extremes {
                min: Point::new(need(0, "minimum down")?, need(1, "minimum across")?),
                max: Point::new(need(2, "maximum down")?, need(3, "maximum across")?),
            },
            3 => {
                let v = need(0, "number of levels")?;
                if v.fract() != 0.0 || !(1.0..=10.0).contains(&v) {
                    return Err(Error::at(
                        ln,
                        "elective 3: levels must be a whole number from 1 to 10",
                    ));
                }
                Elective::Levels(v as usize)
            }
            4 => Elective::ValueMin(need(0, "minimum")?),
            5 => Elective::ValueMax(need(0, "maximum")?),
            6 => {
                let mut v: Vec<f64> = f.iter().map_while(|x| *x).collect();
                // More than six levels continue on a card with columns 1-5 blank.
                if v.len() == 6 && i < cards.len() {
                    let (_, next) = cards[i];
                    if card::is_blank(&card::cols(next, 1, 10)) && !card::is_blank(next) {
                        let more: Option<Vec<_>> = (0..4).map(|k| card::field(next, k)).collect();
                        let more = more
                            .ok_or_else(|| Error::at(cards[i].0, "unreadable elective 6 card"))?;
                        v.extend(more.into_iter().map_while(|x| x));
                        i += 1;
                    }
                }
                if v.iter().any(|&x| x <= 0.0) {
                    return Err(Error::at(ln, "elective 6: level ranges must be positive"));
                }
                Elective::Intervals(v)
            }
            7 => {
                if i + 4 > cards.len() {
                    return Err(Error::at(
                        ln,
                        "elective 7 needs four symbolism cards after it",
                    ));
                }
                let c: [String; 4] = std::array::from_fn(|k| cards[i + k].1.to_string());
                i += 4;
                Elective::Symbolism(Box::new(Symbolism::from_cards(&c)))
            }
            8 => Elective::NoContourLines,
            9 => Elective::NoHistogram,
            10 => {
                let mut text = Vec::new();
                loop {
                    let Some(&(_, t)) = cards.get(i) else {
                        return Err(Error::at(ln, "elective 10 text must end with a 9999 card"));
                    };
                    i += 1;
                    if t.trim() == "9999" {
                        break;
                    }
                    if text.len() == 30 {
                        return Err(Error::at(ln, "elective 10 allows at most 30 text cards"));
                    }
                    text.push(card::cols(t, 1, 72).trim_end().to_string());
                }
                Elective::Text(text)
            }
            11 => Elective::PrintValues,
            12 => Elective::Repeat,
            13 => {
                let s = need(0, "scale")?;
                if s <= 0.0 {
                    return Err(Error::at(ln, "elective 13: scale must be positive"));
                }
                Elective::Scale(s)
            }
            14 => Elective::Margins(std::array::from_fn(|k| get(k).unwrap_or(0.0))),
            15 => Elective::CharsPerInch {
                rows: get(0),
                cols: get(1),
            },
            16 => Elective::LargeMaps,
            17 => Elective::NoTabular,
            18 => Elective::InvalidBlank,
            19 => Elective::InvalidMin(need(0, "minimum")?),
            20 => Elective::InvalidMax(need(0, "maximum")?),
            21 => Elective::StoreValues,
            28 => Elective::DotMap {
                percent: get(0).is_some_and(|v| v > 0.0),
                per_dot: get(1).filter(|v| *v > 0.0),
            },
            22 => Elective::ContinuousContours,
            23 => Elective::NoInvalidSymbol,
            24 => Elective::NoNumericInterpretation,
            25 => Elective::NoDataPoints,
            26 => Elective::OverprintAlignment,
            27 => Elective::ForceContour,
            31 => Elective::Extrapolation(get(0)),
            32 => Elective::ExtrapolationMin(need(0, "minimum")?),
            33 => Elective::ExtrapolationMax(need(0, "maximum")?),
            34 => Elective::InitialRadius(positive(0, "radius")?),
            35 => Elective::MaxRadius(positive(0, "radius")?),
            36 => Elective::PointCount {
                min: points(0)?,
                max: points(1)?,
            },
            37 => Elective::Independence,
            number => Elective::Unsupported {
                number,
                fields: f.clone(),
            },
        };
        out.push(Entry {
            line: ln,
            number: n,
            elective: e,
        });
    }
    Ok(out)
}

/// Apply elective 12: start from `previous` and replace or add this map's electives.
pub fn resolve_repeat(previous: &[Entry], current: Vec<Entry>) -> Vec<Entry> {
    if !current.iter().any(|e| e.elective == Elective::Repeat) {
        return current;
    }
    let mut out: Vec<Entry> = previous.to_vec();
    for e in current
        .into_iter()
        .filter(|e| e.elective != Elective::Repeat)
    {
        match out.iter_mut().find(|x| x.number == e.number) {
            Some(x) => *x = e,
            None => out.push(e),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cards: &[&str]) -> Result<Vec<Elective>, Error> {
        let c: Vec<(usize, &str)> = cards.iter().enumerate().map(|(i, s)| (i + 1, *s)).collect();
        Ok(parse(&c)?.into_iter().map(|e| e.elective).collect())
    }

    #[test]
    fn fixed_and_free() {
        assert_eq!(
            run(&["    1       5.0       8.0"]).unwrap(),
            vec![Elective::Size {
                height: Some(5.0),
                width: Some(8.0)
            }]
        );
        assert_eq!(
            run(&["1 5 8"]).unwrap(),
            run(&["    1       5.0       8.0"]).unwrap()
        );
        assert!(run(&["    3      11.0"]).is_err());
    }

    #[test]
    fn point_counts_and_radii_are_checked() {
        assert_eq!(
            run(&["   36       3.0"]).unwrap(),
            vec![Elective::PointCount {
                min: Some(3),
                max: None
            }]
        );
        for card in [
            "   36       4.0     1e30",
            "   36       4.7",
            "   36       0.0",
        ] {
            assert!(run(&[card]).unwrap_err().message.contains("from 1 to 10"));
        }
        assert!(
            run(&["   35       0.0"])
                .unwrap_err()
                .message
                .contains("positive")
        );
        assert!(
            run(&["   34      -1.0"])
                .unwrap_err()
                .message
                .contains("positive")
        );
    }

    #[test]
    fn multi_card_electives() {
        // Example 3-6: seven levels across two cards.
        let e = run(&[
            "    6     150.       50.       71.5      228.5     250.      139.",
            "          111.",
        ])
        .unwrap();
        assert_eq!(
            e,
            vec![Elective::Intervals(vec![
                150.0, 50.0, 71.5, 228.5, 250.0, 139.0, 111.0
            ])]
        );
        // Blank elective 6: equal numbers of points per level.
        assert_eq!(run(&["    6"]).unwrap(), vec![Elective::Intervals(vec![])]);
        let e = run(&["   10", "SOURCE: 1970 CENSUS", "9999", "    8"]).unwrap();
        assert_eq!(
            e,
            vec![
                Elective::Text(vec!["SOURCE: 1970 CENSUS".into()]),
                Elective::NoContourLines
            ]
        );
        let e = run(&["    7", "", "", "", "", "   11"]).unwrap();
        assert!(matches!(e[0], Elective::Symbolism(_)));
        assert_eq!(e[1], Elective::PrintValues);
        assert!(run(&["    7", "", ""]).is_err());
    }

    #[test]
    fn repeat_replaces_by_number() {
        // Example 3-10: map two repeats map one's electives, changing 3 and adding 8.
        let one = parse(&[(1, "    3       6."), (2, "    4       0.")]).unwrap();
        let two = parse(&[(3, "    3       4."), (4, "    8"), (5, "   12")]).unwrap();
        let r: Vec<_> = resolve_repeat(&one, two)
            .into_iter()
            .map(|e| e.elective)
            .collect();
        assert_eq!(
            r,
            vec![
                Elective::Levels(4),
                Elective::ValueMin(0.0),
                Elective::NoContourLines
            ]
        );
    }
}
