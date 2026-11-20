//! Printed symbolism: overprinted character stacks, and SYMAP's standard sets
//! (1971 manual, Tables 3-1 and 3-2).

use crate::classify::Level;

/// Up to four characters struck in the same print position, in order.
/// Blanks are passes with no strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Strikes(pub [char; 4]);

impl Strikes {
    pub const BLANK: Strikes = Strikes([' '; 4]);

    pub fn new(s: &str) -> Self {
        let mut a = [' '; 4];
        for (slot, ch) in a.iter_mut().zip(s.chars()) {
            *slot = ch;
        }
        Strikes(a)
    }

    pub fn single(c: char) -> Self {
        Strikes([c, ' ', ' ', ' '])
    }

    pub fn is_blank(&self) -> bool {
        self.0.iter().all(|&c| c == ' ')
    }

    /// One character standing in for the overprinted stack in plain text.
    pub fn plain(&self) -> char {
        let mut struck = [' '; 4];
        let mut n = 0;
        for &c in self.0.iter().filter(|&&c| c != ' ') {
            struck[n] = c;
            n += 1;
        }
        match struck[..n] {
            [] => ' ',
            ['O', '-'] => 'Θ',
            ['O', 'X'] => '⊗',
            ['O', 'X', 'A', 'V'] => '█',
            // Otherwise the darkest strike, so that a stack never prints
            // lighter than its heaviest character.
            _ => self.darkest(),
        }
    }

    /// The strike that inks the most (the first wins ties), or a blank.
    pub fn darkest(&self) -> char {
        self.0
            .iter()
            .fold(' ', |a, &c| if ink(c) > ink(a) { c } else { a })
    }

    /// The fraction of the print position the stack inks, from 0 (blank)
    /// to 1: each strike covers its own share of whatever paper the
    /// strikes before it left bare.
    pub fn coverage(&self) -> f64 {
        1.0 - self.0.iter().map(|&c| 1.0 - ink(c)).product::<f64>()
    }
}

/// The share of a print position one strike of `c` inks, as the browser
/// renderer prints it. Ordered so that the ten standard symbols (Table 3-2)
/// darken in turn: on the manual's printouts '=' is a little lighter than
/// '+', and 'O' a little darker than 'X'.
pub fn ink(c: char) -> f64 {
    match c {
        ' ' => 0.0,
        '.' | ',' => 0.06,
        '\'' | '`' | ':' => 0.08,
        '-' | '_' => 0.12,
        '/' | '\\' | '|' | '(' | ')' | '<' | '>' | ';' | '!' => 0.15,
        '=' => 0.20,
        '+' | '*' => 0.23,
        'X' => 0.30,
        'O' | 'Q' => 0.34,
        'M' | 'W' | '#' | '@' | '&' | '$' => 0.36,
        c if c.is_alphanumeric() => 0.28,
        _ => 0.20,
    }
}

/// Symbolism for one map: elective 7's four overprint cards, decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbolism {
    /// General symbolism per level (columns 1-10).
    pub general: [Strikes; 10],
    /// Data point symbol per level (columns 11-20).
    pub point: [Strikes; 10],
    pub low: Strikes,
    pub low_point: Strikes,
    pub high: Strikes,
    pub high_point: Strikes,
    pub background: Strikes,
    pub contour: Strikes,
    pub no_data: Strikes,
    pub superimposed: Strikes,
    pub invalid: Strikes,
}

/// The ten standard level symbols, light to dark (Table 3-2).
pub const STANDARD_TEN: [&str; 10] = [".", "'", "-", "=", "+", "X", "O", "O-", "OX", "OXAV"];

/// Which of the ten standard symbols are used for `n` levels (Table 3-1).
const SUBSETS: [&[usize]; 10] = [
    &[0],
    &[0, 9],
    &[0, 6, 9],
    &[0, 4, 6, 9],
    &[0, 4, 6, 7, 9],
    &[0, 4, 5, 7, 8, 9],
    &[0, 1, 4, 5, 7, 8, 9],
    &[0, 1, 4, 5, 6, 7, 8, 9],
    &[0, 1, 3, 4, 5, 6, 7, 8, 9],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
];

pub const MAX_LEVELS: usize = 10;

impl Symbolism {
    /// The symbol that fills a level.
    pub fn fill(&self, level: Level) -> Strikes {
        match level {
            Level::Low => self.low,
            Level::In(k) => self.general[usize::from(k)],
            Level::High => self.high,
        }
    }

    /// The symbol for a data point at a level.
    pub fn point_symbol(&self, level: Level) -> Strikes {
        match level {
            Level::Low => self.low_point,
            Level::In(k) => self.point[usize::from(k)],
            Level::High => self.high_point,
        }
    }

    /// Standard symbolism for `levels` levels.
    pub fn standard(levels: usize) -> Self {
        assert!((1..=MAX_LEVELS).contains(&levels));
        let mut general = [Strikes::BLANK; 10];
        let mut point = [Strikes::BLANK; 10];
        for (k, &i) in SUBSETS[levels - 1].iter().enumerate() {
            general[k] = Strikes::new(STANDARD_TEN[i]);
            point[k] = Strikes::single(if k == 9 {
                '*'
            } else {
                char::from_digit(k as u32 + 1, 10).unwrap()
            });
        }
        Symbolism {
            general,
            point,
            low: Strikes::new("L"),
            low_point: Strikes::new("L."),
            high: Strikes::new("H"),
            high_point: Strikes::new("HHH/"),
            background: Strikes::BLANK,
            contour: Strikes::BLANK,
            no_data: Strikes::new("N"),
            superimposed: Strikes::new("S"),
            invalid: Strikes::new("M"),
        }
    }

    /// Decode elective 7's second to fifth cards: card `k` gives strike `k`.
    pub fn from_cards(cards: &[String; 4]) -> Self {
        let at = |c: usize| {
            let mut s = [' '; 4];
            for (k, card) in cards.iter().enumerate() {
                s[k] = crate::card::col(card, c);
            }
            Strikes(s)
        };
        Symbolism {
            general: std::array::from_fn(|i| at(i + 1)),
            point: std::array::from_fn(|i| at(i + 11)),
            low: at(21),
            low_point: at(22),
            high: at(23),
            high_point: at(24),
            background: at(25),
            contour: at(26),
            no_data: at(27),
            superimposed: at(28),
            invalid: at(29),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(n: usize) -> String {
        Symbolism::standard(n).general[..n]
            .iter()
            .map(|s| s.plain())
            .collect()
    }

    #[test]
    fn standard_symbols_darken_in_turn() {
        let ink: Vec<f64> = STANDARD_TEN
            .iter()
            .map(|s| Strikes::new(s).coverage())
            .collect();
        assert!(ink.windows(2).all(|w| w[0] < w[1]), "{ink:?}");
        assert_eq!(Strikes::BLANK.coverage(), 0.0);
    }

    #[test]
    fn unnamed_stacks_print_their_darkest_strike() {
        assert_eq!(Strikes::new("-X").plain(), 'X');
        assert_eq!(Strikes::new("X-").plain(), 'X');
        assert_eq!(Strikes::new(".=").plain(), '=');
        assert_eq!(Strikes::new(" ='").plain(), '=');
        assert_eq!(Strikes::new("HHH/").plain(), 'H');
    }

    #[test]
    fn table_3_1() {
        assert_eq!(row(1), ".");
        assert_eq!(row(2), ".█");
        assert_eq!(row(3), ".O█");
        assert_eq!(row(4), ".+O█");
        assert_eq!(row(5), ".+OΘ█");
        assert_eq!(row(6), ".+XΘ⊗█");
        assert_eq!(row(7), ".'+XΘ⊗█");
        assert_eq!(row(8), ".'+XOΘ⊗█");
        assert_eq!(row(9), ".'=+XOΘ⊗█");
        assert_eq!(row(10), ".'-=+XOΘ⊗█");
        let s = Symbolism::standard(10);
        assert_eq!(s.point[9], Strikes::single('*'));
        assert_eq!(s.point[0], Strikes::single('1'));
    }

    #[test]
    fn elective_7_cards() {
        // Example 1-14: background '-', contour lines struck O, X, A, V.
        let pad = |s: &str| format!("{:24}{s}", "");
        let cards = [pad("-O"), pad(" X"), pad(" A"), pad(" V")];
        let s = Symbolism::from_cards(&cards);
        assert_eq!(s.background, Strikes::new("-"));
        assert_eq!(s.contour, Strikes::new("OXAV"));
        assert!(s.general.iter().all(Strikes::is_blank));
    }
}
