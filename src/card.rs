//! Reading 80-column card images.
//!
//! SYMAP cards put coordinates and values in right-justified 10-column
//! fields starting at column 11 (11-20, 21-30, ...). Blank-separated free
//! format is accepted as a fallback for hand-typed decks.

/// Characters in 1-based inclusive columns `from..=to`. Short lines read as blank.
pub fn cols(line: &str, from: usize, to: usize) -> String {
    line.chars().skip(from - 1).take(to + 1 - from).collect()
}

/// The character in 1-based column `c` (blank if the card is shorter).
pub fn col(line: &str, c: usize) -> char {
    line.chars().nth(c - 1).unwrap_or(' ')
}

pub fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

fn finite(t: &str) -> Option<f64> {
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Parse a Fortran-style real field: blank is `Some(None)`, junk is `None`.
pub fn real_field(s: &str) -> Option<Option<f64>> {
    let t = s.trim();
    if t.is_empty() {
        return Some(None);
    }
    finite(t).map(Some)
}

/// The `i`-th 10-column field from column 11 (`i = 0` is columns 11-20).
pub fn field(line: &str, i: usize) -> Option<Option<f64>> {
    real_field(&cols(line, 11 + 10 * i, 20 + 10 * i))
}

/// Integer in columns `from..=to`; blank is `Some(None)`.
pub fn int_field(line: &str, from: usize, to: usize) -> Option<Option<i64>> {
    let t = cols(line, from, to);
    let t = t.trim();
    if t.is_empty() {
        Some(None)
    } else {
        t.parse().ok().map(Some)
    }
}

/// A coordinate pair (down, across) from columns 11-20 and 21-30.
pub fn fixed_pair(line: &str) -> Option<(f64, f64)> {
    Some((field(line, 0)??, field(line, 1)??))
}

/// The first `n` blank-separated numbers on a card.
pub fn free_numbers(line: &str, n: usize) -> Option<Vec<f64>> {
    let toks: Vec<&str> = line.split_whitespace().collect();
    if toks.len() < n {
        return None;
    }
    toks[..n].iter().map(|t| finite(t)).collect()
}

/// A coordinate pair: fixed columns first, then free format.
pub fn pair(line: &str) -> Option<(f64, f64)> {
    fixed_pair(line).or_else(|| free_numbers(line, 2).map(|v| (v[0], v[1])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_one_based_and_padded() {
        assert_eq!(cols("abcdef", 2, 4), "bcd");
        assert_eq!(cols("ab", 4, 5), "");
        assert_eq!(col("abc", 3), 'c');
        assert_eq!(col("abc", 9), ' ');
    }

    #[test]
    fn fixed_fields_and_free_fallback() {
        let card = format!("{:10}{:>10}{:>10}", "", "1.9", "-16.6");
        assert_eq!(fixed_pair(&card), Some((1.9, -16.6)));
        assert_eq!(pair("  1.0   1.6"), Some((1.0, 1.6)));
        assert_eq!(pair("x y"), None);
        assert_eq!(field("", 3), Some(None));
        assert_eq!(int_field("   12", 1, 5), Some(Some(12)));
        assert_eq!(free_numbers("nan 1", 2), None);
    }
}
