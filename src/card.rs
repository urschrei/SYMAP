//! Reading 80-column card images.
//!
//! SYMAP cards put coordinates and values in right-justified 10-column
//! fields starting at column 11 (11-20, 21-30, ...). Blank-separated free
//! format is accepted as a fallback for hand-typed decks.

/// Characters in 1-based inclusive columns `from..=to`. Short lines read as blank.
pub fn cols(line: &str, from: usize, to: usize) -> &str {
    let offset = |chars: usize| {
        line.char_indices()
            .nth(chars)
            .map_or(line.len(), |(i, _)| i)
    };
    let start = offset(from - 1);
    &line[start..offset(to).max(start)]
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

/// A field that is neither blank nor a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// A Fortran-style real field; blank is `Ok(None)`.
pub fn real_field(s: &str) -> Result<Option<f64>, Unreadable> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    finite(t).map(Some).ok_or(Unreadable)
}

/// The `i`-th 10-column field from column 11 (`i = 0` is columns 11-20).
pub fn field(line: &str, i: usize) -> Result<Option<f64>, Unreadable> {
    real_field(cols(line, 11 + 10 * i, 20 + 10 * i))
}

/// Integer in columns `from..=to`; blank is `Ok(None)`.
pub fn int_field(line: &str, from: usize, to: usize) -> Result<Option<i64>, Unreadable> {
    let t = cols(line, from, to).trim();
    if t.is_empty() {
        Ok(None)
    } else {
        t.parse().map(Some).map_err(|_| Unreadable)
    }
}

/// A coordinate pair (down, across) from columns 11-20 and 21-30.
pub fn fixed_pair(line: &str) -> Option<(f64, f64)> {
    Some((field(line, 0).ok()??, field(line, 1).ok()??))
}

/// The first `N` blank-separated numbers on a card.
pub fn free_numbers<const N: usize>(line: &str) -> Option<[f64; N]> {
    let mut toks = line.split_whitespace();
    let mut out = [0.0; N];
    for slot in &mut out {
        *slot = finite(toks.next()?)?;
    }
    Some(out)
}

/// A coordinate pair: fixed columns first, then free format.
pub fn pair(line: &str) -> Option<(f64, f64)> {
    fixed_pair(line).or_else(|| free_numbers(line).map(|[down, across]| (down, across)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_one_based_and_padded() {
        assert_eq!(cols("abcdef", 2, 4), "bcd");
        assert_eq!(cols("ab", 4, 5), "");
        assert_eq!(cols("aé€b", 2, 3), "é€");
        assert_eq!(cols("aé€b", 4, 9), "b");
        assert_eq!(col("abc", 3), 'c');
        assert_eq!(col("abc", 9), ' ');
    }

    #[test]
    fn fixed_fields_and_free_fallback() {
        let card = format!("{:10}{:>10}{:>10}", "", "1.9", "-16.6");
        assert_eq!(fixed_pair(&card), Some((1.9, -16.6)));
        assert_eq!(pair("  1.0   1.6"), Some((1.0, 1.6)));
        assert_eq!(pair("x y"), None);
        assert_eq!(field("", 3), Ok(None));
        assert_eq!(
            field(&format!("{:10}{:>10}", "", "1.x"), 0),
            Err(Unreadable)
        );
        assert_eq!(int_field("   12", 1, 5), Ok(Some(12)));
        assert_eq!(int_field("  1.5", 1, 5), Err(Unreadable));
        assert_eq!(free_numbers::<2>("nan 1"), None);
        assert_eq!(free_numbers("1 2 x"), Some([1.0, 2.0]));
    }
}
