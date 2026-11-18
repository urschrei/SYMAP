//! Line-printer output: symbols, overprinting, and the printed page.

use crate::classify::Classes;
use crate::deck::MapKind;

/// A printed tone: characters struck on top of each other, plus a single
/// character stand-in for plain-text output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol {
    pub strikes: &'static str,
    pub plain: char,
}

const fn sym(strikes: &'static str, plain: char) -> Symbol {
    Symbol { strikes, plain }
}

/// Light-to-dark tone ramp. The overprinted stacks follow the SYMAP style;
/// the exact default sets still need checking against original output.
pub const RAMP: [Symbol; 10] = [
    sym(".", '.'),
    sym("-", '-'),
    sym(":", ':'),
    sym("+", '+'),
    sym("=", '='),
    sym("X", 'X'),
    sym("O", 'O'),
    sym("OX", '8'),
    sym("OXA", '#'),
    sym("OXAV", '@'),
];

pub const MAX_LEVELS: usize = RAMP.len();

/// `n` symbols spread evenly over the ramp, lightest first.
pub fn symbols_for(n: usize) -> Vec<Symbol> {
    assert!((1..=MAX_LEVELS).contains(&n));
    if n == 1 {
        return vec![RAMP[MAX_LEVELS / 2]];
    }
    (0..n)
        .map(|k| RAMP[(k * (MAX_LEVELS - 1) + (n - 1) / 2) / (n - 1)])
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Outside the study area (or outside every zone).
    Blank,
    /// Filled with a level's symbol (0-based).
    Level(u8),
    /// A data point location, printed as its level number (0-based).
    Mark(u8),
}

impl Cell {
    pub fn level(self) -> Option<usize> {
        match self {
            Cell::Blank => None,
            Cell::Level(k) | Cell::Mark(k) => Some(k as usize),
        }
    }
}

fn mark_char(level: u8) -> char {
    char::from_digit((level as u32 + 1) % 10, 10).unwrap()
}

/// One printed line: a plain rendering, and the strike passes for overprinting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub plain: String,
    pub strikes: Vec<String>,
}

impl Line {
    fn text(s: impl Into<String>) -> Self {
        let s = s.into();
        Line {
            plain: s.clone(),
            strikes: vec![s],
        }
    }

    fn blank() -> Self {
        Line::text("")
    }
}

/// Builds a line of cells, each with its own strike stack.
#[derive(Default)]
struct LineBuilder {
    plain: String,
    strikes: Vec<Vec<char>>,
}

impl LineBuilder {
    fn push(&mut self, plain: char, strikes: &str) {
        let col = self.plain.chars().count();
        self.plain.push(plain);
        for (k, ch) in strikes.chars().enumerate() {
            if self.strikes.len() <= k {
                self.strikes.push(Vec::new());
            }
            let layer = &mut self.strikes[k];
            layer.resize(col, ' ');
            layer.push(ch);
        }
        if strikes.is_empty() && self.strikes.is_empty() {
            self.strikes.push(Vec::new());
        }
    }

    fn push_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.push(ch, &ch.to_string());
        }
    }

    fn finish(self) -> Line {
        let mut strikes: Vec<String> = self
            .strikes
            .into_iter()
            .map(|l| l.into_iter().collect::<String>().trim_end().to_string())
            .collect();
        if strikes.is_empty() {
            strikes.push(String::new());
        }
        Line {
            plain: self.plain.trim_end().to_string(),
            strikes,
        }
    }
}

/// Number formatting for the legend: fixed point where readable, else scientific.
fn num(v: f64) -> String {
    let a = v.abs();
    if a != 0.0 && !(1e-2..1e6).contains(&a) {
        format!("{v:.3e}")
    } else {
        format!("{v:.2}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrintedMap {
    pub kind: MapKind,
    pub titles: [String; 3],
    pub rows: usize,
    pub cols: usize,
    /// Row-major cells.
    pub cells: Vec<Cell>,
    pub classes: Classes,
    pub symbols: Vec<Symbol>,
    /// Filled cells per level.
    pub counts: Vec<usize>,
    /// Blank space around the map in (top, left, bottom, right) lines/characters.
    pub margins: (usize, usize, usize, usize),
    /// Value extremes of the input data.
    pub data_range: (f64, f64),
    pub warnings: Vec<String>,
}

const LEGEND_LABEL: usize = 12;
const LEGEND_COL: usize = 11;

impl PrintedMap {
    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.cells[row * self.cols + col]
    }

    fn cell_glyph(&self, c: Cell) -> (char, &'static str) {
        match c {
            Cell::Blank => (' ', ""),
            Cell::Level(k) => {
                let s = self.symbols[k as usize];
                (s.plain, s.strikes)
            }
            Cell::Mark(k) => {
                let ch = mark_char(k);
                // Digits are static strs for the strike stack.
                const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
                (ch, DIGITS[ch.to_digit(10).unwrap() as usize])
            }
        }
    }

    /// The full printed page: titles, bordered map, and legend.
    pub fn page(&self) -> Vec<Line> {
        let (top, left, bottom, _right) = self.margins;
        let mut out: Vec<Line> = self
            .titles
            .iter()
            .map(|t| Line::text(t.trim_end()))
            .collect();
        out.push(Line::blank());
        out.extend((0..top).map(|_| Line::blank()));

        let pad = " ".repeat(left);
        let border = format!("{pad}+{}+", "-".repeat(self.cols));
        out.push(Line::text(border.clone()));
        for r in 0..self.rows {
            let mut b = LineBuilder::default();
            b.push_str(&pad);
            b.push_str("I");
            for c in 0..self.cols {
                let (p, s) = self.cell_glyph(self.cell(r, c));
                b.push(p, s);
            }
            b.push_str("I");
            out.push(b.finish());
        }
        out.push(Line::text(border));
        out.extend((0..bottom).map(|_| Line::blank()));
        out.push(Line::blank());
        out.extend(self.legend());
        out
    }

    fn legend(&self) -> Vec<Line> {
        let n = self.classes.levels();
        let total: usize = self.counts.iter().sum();
        let row = |label: &str, f: &dyn Fn(usize) -> String| {
            let mut s = format!("{label:<LEGEND_LABEL$}");
            for k in 0..n {
                s.push_str(&format!("{:>LEGEND_COL$}", f(k)));
            }
            Line::text(s)
        };
        let mut out = vec![
            Line::text(format!(
                "DATA VALUE EXTREMES ARE {}  AND  {}",
                num(self.data_range.0),
                num(self.data_range.1)
            )),
            Line::blank(),
            row("LEVEL", &|k| (k + 1).to_string()),
            row("MINIMUM", &|k| num(self.classes.breaks[k])),
            row("MAXIMUM", &|k| num(self.classes.breaks[k + 1])),
        ];
        for i in 0..3 {
            let mut b = LineBuilder::default();
            b.push_str(&format!(
                "{:<LEGEND_LABEL$}",
                if i == 1 { "SYMBOLS" } else { "" }
            ));
            for s in &self.symbols {
                b.push_str("  ");
                for _ in 0..LEGEND_COL - 2 {
                    b.push(s.plain, s.strikes);
                }
            }
            out.push(b.finish());
        }
        out.push(row("FREQUENCY %", &|k| {
            if total == 0 {
                "0.0".into()
            } else {
                format!("{:.1}", 100.0 * self.counts[k] as f64 / total as f64)
            }
        }));
        out
    }

    /// Plain text, one character per cell, no overprinting.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for l in self.page() {
            s.push_str(&l.plain);
            s.push('\n');
        }
        s
    }

    /// Fortran ASA carriage-control output: column 1 is `1` (new page),
    /// ` ` (next line) or `+` (overprint the previous line).
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
    fn symbol_sets_span_the_ramp() {
        let s = symbols_for(5);
        assert_eq!(s.len(), 5);
        assert_eq!(s[0], RAMP[0]);
        assert_eq!(s[4], RAMP[9]);
        assert_eq!(symbols_for(10), RAMP.to_vec());
        let mut plains: Vec<char> = symbols_for(7).iter().map(|s| s.plain).collect();
        plains.dedup();
        assert_eq!(plains.len(), 7);
    }

    #[test]
    fn overprint_layers_align() {
        let mut b = LineBuilder::default();
        b.push('.', ".");
        b.push('@', "OXAV");
        b.push(' ', "");
        b.push('8', "OX");
        let l = b.finish();
        assert_eq!(l.plain, ".@ 8");
        assert_eq!(l.strikes, vec![".O O", " X X", " A", " V"]);
    }

    #[test]
    fn marks_are_one_based_digits() {
        assert_eq!(mark_char(0), '1');
        assert_eq!(mark_char(9), '0');
    }
}
