//! The printed page: bordered map (in 13-inch panels if wider), titles,
//! text, numeric interpretation and frequency distribution, laid out as in
//! the 1971 manual's examples.

use crate::classify::Level;
use crate::map::PrintedMap;
use crate::symbols::Strikes;

/// One printed line: plain characters, and up to four strike passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub plain: String,
    pub strikes: Vec<String>,
}

impl Line {
    pub fn text(s: impl Into<String>) -> Self {
        let s: String = s.into();
        let s = s.trim_end().to_string();
        Line {
            plain: s.clone(),
            strikes: vec![s],
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
        }
    }
}

/// Characters per panel: 13 inches at the map's pitch.
const PANEL_INCHES: f64 = 13.0;

/// Border mark at position `i`: the inch number on each inch, `+` on each
/// half inch, else `fill`.
fn border_mark(i: usize, per_inch: f64, fill: char) -> char {
    let x = i as f64 / per_inch;
    let near = |v: f64| (v - v.round()).abs() * per_inch < 0.5;
    if i > 0 && near(x) {
        char::from_digit((x.round() as u32) % 10, 10).unwrap()
    } else if i > 0 && near(x * 2.0) {
        '+'
    } else {
        fill
    }
}

fn num(v: f64) -> String {
    if v != 0.0 && !(1e-2..1e7).contains(&v.abs()) {
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
                            Strikes::single(border_mark(c, self.frame.cols_per_inch, '-'))
                        } else if c == 0 || c == cols {
                            Strikes::single(border_mark(r, self.frame.lines_per_inch, 'I'))
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
        if cols < per + 1 {
            return vec![0..=cols];
        }
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
        let b = &self.classes.breaks;
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
            _ => String::new(),
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
            labelled("LEVEL", &|l| match l {
                Level::Low => "L".into(),
                Level::In(k) => (k + 1).to_string(),
                Level::High => "H".into(),
            }),
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
                let (general, point) = match l {
                    Level::Low => (s.low, s.low_point),
                    Level::In(k) => (s.general[k as usize], s.point[k as usize]),
                    Level::High => (s.high, s.high_point),
                };
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
        out.push(labelled("FREQ.", &|l| {
            match l {
                Level::Low => f.low,
                Level::In(k) => f.levels[k as usize],
                Level::High => f.high,
            }
            .to_string()
        }));
        out
    }

    /// The full printed output for this map.
    pub fn page(&self) -> Vec<Line> {
        let rows = self.bordered();
        let mut out = Vec::new();
        for (i, cols) in self.panels().into_iter().enumerate() {
            if i > 0 {
                out.push(Line::blank());
            }
            out.extend(rows.iter().map(|r| Line::from_strikes(&r[cols.clone()])));
        }
        out.extend(self.titles.iter().map(|t| Line::text(t.clone())));
        out.extend(self.text.iter().map(|t| Line::text(t.clone())));
        if self.numeric {
            out.extend(self.numeric_interpretation());
        }
        if self.histogram {
            out.extend(self.histogram());
        }
        out
    }

    /// Plain text, one character per print position.
    pub fn to_text(&self) -> String {
        self.page().into_iter().map(|l| l.plain + "\n").collect()
    }

    /// Fortran carriage control: column 1 is `1` (new page), ` ` (next line)
    /// or `+` (overprint the line above).
    pub fn to_asa(&self) -> String {
        let mut s = String::new();
        for (i, l) in self.page().iter().enumerate() {
            for (k, strike) in l.strikes.iter().enumerate() {
                if k > 0 && strike.trim().is_empty() {
                    continue;
                }
                s.push(match (i, k) {
                    (0, 0) => '1',
                    (_, 0) => ' ',
                    _ => '+',
                });
                s.push_str(strike);
                s.push('\n');
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn border_marks() {
        let top: String = (0..=25).map(|i| border_mark(i, 10.0, '-')).collect();
        assert_eq!(top, "-----+----1----+----2----+");
        let side: String = (0..=17).map(|i| border_mark(i, 8.0, 'I')).collect();
        assert_eq!(side, "IIII+III1III+III2I");
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
