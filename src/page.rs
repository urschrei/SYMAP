//! The printed page: bordered map (in 13-inch panels if wider), titles,
//! text, numeric interpretation and frequency distribution, laid out as in
//! the 1971 manual's examples.

use crate::classify::Level;
use crate::deck::MapKind;
use crate::map::PrintedMap;
use crate::symbols::Strikes;

/// One printed line: plain characters, and up to four strike passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub plain: String,
    pub strikes: Vec<String>,
    /// Starts a new page (carriage control `1`).
    pub eject: bool,
}

impl Line {
    pub fn text(s: impl Into<String>) -> Self {
        let s: String = s.into();
        let s = s.trim_end().to_string();
        Line {
            plain: s.clone(),
            strikes: vec![s],
            eject: false,
        }
    }

    pub fn blank() -> Self {
        Line::text("")
    }

    pub fn from_strikes(cells: &[Strikes]) -> Self {
        let plain: String = cells.iter().map(Strikes::plain).collect();
        let mut strikes: Vec<String> = (0..4)
            .map(|k| {
                cells
                    .iter()
                    .map(|s| s.0[k])
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect();
        while strikes.len() > 1 && strikes.last().unwrap().is_empty() {
            strikes.pop();
        }
        Line {
            plain: plain.trim_end().to_string(),
            strikes,
            eject: false,
        }
    }
}

/// Characters per panel: 13 inches at the map's pitch.
const PANEL_INCHES: f64 = 13.0;

/// Border mark at position `i`: every tenth character carries the tens
/// digit and every fifth a `+` (Example 1-17 counts rows this way too, so the
/// marks are character counts, not inches).
fn border_mark(i: usize, fill: char) -> char {
    if i > 0 && i % 10 == 0 {
        char::from_digit(((i / 10) % 10) as u32, 10).unwrap()
    } else if i > 0 && i % 5 == 0 {
        '+'
    } else {
        fill
    }
}

fn num(v: f64) -> String {
    // SYMAP prints zero as 0.0 (Examples 1-4, 1-12).
    if v == 0.0 {
        return "0.0".into();
    }
    if !(1e-2..1e7).contains(&v.abs()) {
        format!("{v:.3e}")
    } else {
        format!("{v:.2}")
    }
}

const LABEL: usize = 12;
const COL: usize = 11;
const BLOCK: usize = 10;

impl PrintedMap {
    /// The map with its border, as rows of strike stacks.
    pub fn bordered(&self) -> Vec<Vec<Strikes>> {
        let (rows, cols) = (self.frame.rows, self.frame.cols);
        (0..=rows)
            .map(|r| {
                (0..=cols)
                    .map(|c| {
                        let corner = (r == 0 || r == rows) && (c == 0 || c == cols);
                        if corner {
                            Strikes::single('*')
                        } else if r == 0 || r == rows {
                            Strikes::single(border_mark(c, '-'))
                        } else if c == 0 || c == cols {
                            Strikes::single(border_mark(r, 'I'))
                        } else {
                            self.at(r, c)
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Column ranges of the printed panels.
    pub fn panels(&self) -> Vec<std::ops::RangeInclusive<usize>> {
        let per = ((PANEL_INCHES * self.frame.cols_per_inch).round() as usize).max(2);
        let cols = self.frame.cols;
        // Each panel shares its edge column with the next; a map up to 13
        // inches wide is a single panel.
        let mut out = Vec::new();
        let mut start = 0;
        while start < cols {
            let end = (start + per).min(cols);
            out.push(start..=end);
            start = end;
        }
        out
    }

    /// Label for each frequency column: L, levels, H.
    fn columns(&self) -> Vec<Level> {
        let mut v = Vec::new();
        if self.frequency.low > 0 {
            v.push(Level::Low);
        }
        v.extend((0..self.classes.levels()).map(|k| Level::In(k as u8)));
        if self.frequency.high > 0 {
            v.push(Level::High);
        }
        v
    }

    fn row(label: &str, cells: impl IntoIterator<Item = String>) -> Line {
        let mut s = format!(" {label:<w$}", w = LABEL - 1);
        for c in cells {
            s.push_str(&format!("{c:>w$} ", w = COL - 1));
        }
        Line::text(s)
    }

    fn numeric_interpretation(&self) -> Vec<Line> {
        let cols = self.columns();
        let b = self.classes.breaks();
        let (min, max) = (self.classes.min(), self.classes.max());
        let lo = |l: Level| match l {
            Level::Low => "BELOW".to_string(),
            Level::In(k) => num(b[k as usize]),
            Level::High => num(max),
        };
        let hi = |l: Level| match l {
            Level::Low => num(min),
            Level::In(k) => num(b[k as usize + 1]),
            Level::High => "ABOVE".to_string(),
        };
        let pct = |l: Level| match l {
            Level::In(k) => num(100.0 * (b[k as usize + 1] - b[k as usize]) / (max - min)),
            Level::Low | Level::High => String::new(),
        };
        vec![
            Line::blank(),
            Line::text(format!(
                "DATA VALUE EXTREMES ARE   {:>w$}   {:>w$}",
                num(self.data_range.0),
                num(self.data_range.1),
                w = COL - 1
            )),
            Line::blank(),
            Line::text("ABSOLUTE VALUE RANGE APPLYING TO EACH LEVEL"),
            Line::text("      ('MAXIMUM' INCLUDED IN HIGHEST LEVEL ONLY)"),
            Line::blank(),
            Self::row("MINIMUM", cols.iter().map(|&l| lo(l))),
            Self::row("MAXIMUM", cols.iter().map(|&l| hi(l))),
            Line::blank(),
            Line::text("PERCENTAGE OF TOTAL ABSOLUTE VALUE RANGE APPLYING TO EACH LEVEL"),
            Line::blank(),
            Self::row("", cols.iter().map(|&l| pct(l))),
        ]
    }

    fn histogram(&self) -> Vec<Line> {
        let cols = self.columns();
        let s = &self.symbolism;
        let centred = |t: String| format!("{t:^w$} ", w = BLOCK);
        let labelled = |label: &str, f: &dyn Fn(Level) -> String| {
            let mut out = format!(" {label:<w$}", w = LABEL - 1);
            for &l in &cols {
                out.push_str(&centred(f(l)));
            }
            Line::text(out)
        };
        let rule = Line::text(format!(
            "{:w$}{}",
            "",
            "=".repeat(cols.len() * COL),
            w = LABEL
        ));
        let mut out = vec![
            Line::blank(),
            Line::text("FREQUENCY DISTRIBUTION OF DATA POINT VALUES IN EACH LEVEL"),
            labelled("LEVEL", &|l| l.label()),
            rule.clone(),
        ];
        for i in 0..5 {
            let mut cells: Vec<Strikes> = format!(
                " {:<w$}",
                if i == 2 { "SYMBOLS" } else { "" },
                w = LABEL - 1
            )
            .chars()
            .map(Strikes::single)
            .collect();
            for &l in &cols {
                let (general, point) = (s.fill(l), s.point_symbol(l));
                for j in 0..BLOCK {
                    cells.push(if i == 2 && j == BLOCK / 2 {
                        point
                    } else {
                        general
                    });
                }
                cells.push(Strikes::BLANK);
            }
            out.push(Line::from_strikes(&cells));
        }
        out.push(rule);
        let f = &self.frequency;
        let count = |l: Level| match l {
            Level::Low => f.low,
            Level::In(k) => f.levels[k as usize],
            Level::High => f.high,
        };
        out.push(labelled("FREQ.", &|l| count(l).to_string()));
        // Histogram bars (suppressed by elective 9): one row per data point,
        // a small box of the level's symbols under each level (Example 1-5).
        if self.bars {
            let most = cols.iter().map(|&l| count(l)).max().unwrap_or(0);
            for k in 1..=most {
                let mut cells: Vec<Strikes> = format!("{k:>w$} ", w = LABEL - 1)
                    .chars()
                    .map(Strikes::single)
                    .collect();
                for &l in &cols {
                    let (general, point) = (s.fill(l), s.point_symbol(l));
                    for j in 0..COL {
                        let box_cell =
                            (BLOCK / 2 - 3..=BLOCK / 2 + 3).contains(&j) && count(l) >= k;
                        cells.push(if !box_cell {
                            Strikes::BLANK
                        } else if j == BLOCK / 2 - 3 || j == BLOCK / 2 + 3 {
                            Strikes::single('I')
                        } else if j == BLOCK / 2 {
                            point
                        } else {
                            general
                        });
                    }
                }
                out.push(Line::from_strikes(&cells));
            }
        }
        out
    }

    /// The full printed output for this map.
    /// The scale and the data point (or zone) table printed before the map,
    /// as in the manual's Examples 1-4 and 1-21. Elective 17 suppresses it.
    fn prelude(&self) -> Vec<Line> {
        let f = &self.frame;
        let mut out = vec![
            Line::text(format!(
                "MAP SCALE = {:>10.4} INCHES ON OUTPUT MAP/UNITS ON SOURCE MAP",
                f.scale
            )),
            Line::text(format!(
                "MAP SHOULD BE PRINTED AT {:>5.1} ROWS PER INCH AND {:>5.1} COLUMNS PER INCH",
                f.lines_per_inch, f.cols_per_inch
            )),
            Line::blank(),
            Line::text(format!(
                "ROW    = DOWN   COORDINATE * {:>10.4}",
                f.scale * f.lines_per_inch
            )),
            Line::text(format!(
                "COLUMN = ACROSS COORDINATE * {:>10.4}",
                f.scale * f.cols_per_inch
            )),
            Line::blank(),
        ];
        let zones = self.kind == MapKind::Conformant;
        out.push(Line::text(if zones {
            "CONFORMOLINES FOR MAP"
        } else {
            "DATA POINTS FOR MAP"
        }));
        out.push(Line::text(if zones {
            "  ZONE    TYPE       ROW    COLUMN     DATUM       VALUE     LEVEL"
        } else {
            " POINT               ROW    COLUMN     DATUM       VALUE     LEVEL"
        }));
        out.push(Line::blank());
        for t in &self.table {
            let level = t.level.map_or_else(|| "M".to_string(), Level::label);
            out.push(Line::text(format!(
                "{:>5})    {:<4} {:>9} {:>9} {:>9} {:>11} {:>9}",
                t.number,
                t.shape.map(|s| s.letter().to_string()).unwrap_or_default(),
                t.row,
                t.col,
                t.datum.map(|d| d.to_string()).unwrap_or_default(),
                t.value.map(num).unwrap_or_else(|| "INVALID".into()),
                level
            )));
        }
        if let Some(r) = self.search_radius {
            out.push(Line::blank());
            out.push(Line::text(format!("STANDARD SEARCH RADIUS IS {r:>10.5}")));
        }
        out.push(Line::blank());
        out
    }

    pub fn page(&self) -> Vec<Line> {
        let rows = self.bordered();
        let mut out = if self.tabular {
            self.prelude()
        } else {
            Vec::new()
        };
        // The map starts a page of its own, after the tables.
        let first = out.len();
        for (i, cols) in self.panels().into_iter().enumerate() {
            if i > 0 {
                out.push(Line::blank());
            }
            out.extend(rows.iter().map(|r| Line::from_strikes(&r[cols.clone()])));
        }
        if first > 0 && first < out.len() {
            out[first].eject = true;
        }
        // The program's name and the job time follow the map, then the
        // titles double-spaced (Examples 1-5 and 1-16). The time is printed
        // as the manual's small jobs show it.
        out.push(Line::text(" SYMAP"));
        out.push(Line::blank());
        out.push(Line::text(format!("TIME ={:>7.1}", 0.0)));
        out.extend([Line::blank(), Line::blank()]);
        for t in &self.titles {
            out.push(Line::blank());
            out.push(Line::text(t.clone()));
        }
        out.extend(self.text.iter().map(|t| Line::text(t.clone())));
        if self.numeric {
            out.extend(self.numeric_interpretation());
        }
        {
            out.extend(self.histogram());
        }
        out
    }

    /// Elective 21's values as CSV: one line per printed line inside the
    /// border, empty where nothing was computed. `None` without elective 21.
    pub fn values_csv(&self) -> Option<String> {
        let v = self.values.as_ref()?;
        let mut out = String::new();
        for r in 1..self.frame.rows {
            let row: Vec<String> = (1..self.frame.cols)
                .map(|c| {
                    v[self.frame.index(r, c)]
                        .map(|x| format!("{x}"))
                        .unwrap_or_default()
                })
                .collect();
            out.push_str(&row.join(","));
            out.push('\n');
        }
        Some(out)
    }

    /// Plain text, one character per print position.
    pub fn to_text(&self) -> String {
        self.page()
            .into_iter()
            .map(|l| {
                if l.eject {
                    format!("\x0c{}\n", l.plain)
                } else {
                    l.plain + "\n"
                }
            })
            .collect()
    }

    /// Fortran carriage control: column 1 is `1` (new page), ` ` (next line)
    /// or `+` (overprint the line above).
    pub fn to_asa(&self) -> String {
        asa(&self.page())
    }
}

/// The lines of a page with Fortran carriage control (see `to_asa`).
pub(crate) fn asa(page: &[Line]) -> String {
    let mut s = String::new();
    for (i, l) in page.iter().enumerate() {
        for (k, strike) in l.strikes.iter().enumerate() {
            if k > 0 && strike.trim().is_empty() {
                continue;
            }
            s.push(match k {
                0 if i == 0 || l.eject => '1',
                0 => ' ',
                _ => '+',
            });
            s.push_str(strike);
            s.push('\n');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_marks() {
        let top: String = (0..=25).map(|i| border_mark(i, '-')).collect();
        assert_eq!(top, "-----+----1----+----2----+");
        let side: String = (0..=21).map(|i| border_mark(i, 'I')).collect();
        assert_eq!(side, "IIIII+IIII1IIII+IIII2I");
    }

    #[test]
    fn strike_layers() {
        let l = Line::from_strikes(&[
            Strikes::new("."),
            Strikes::new("OXAV"),
            Strikes::BLANK,
            Strikes::new("O-"),
        ]);
        assert_eq!(l.plain, ".█ Θ");
        assert_eq!(l.strikes, vec![".O O", " X -", " A", " V"]);
    }
}
